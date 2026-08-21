//! Fallback tag-writer that piggybacks on Windows' Shell Property System.
//!
//! For formats that Magpie can't yet embed tags into natively (RAW families,
//! video containers, PDF, HEIC, ...) we delegate to `IPropertyStore` — the
//! same mechanism the Explorer *Properties → Details* tab uses. If Windows
//! has a property handler registered for the extension (as it does for
//! virtually every format above), we can read and write `System.Title` and
//! `System.Keywords` (multi-value tags) directly on the source file.
//!
//! The write path opens the store with `GPS_READWRITE`, calls `SetValue` for
//! each property, then `Commit()`s to flush to disk. Foreign properties the
//! user (or Explorer) set — `System.Rating`, `System.Author`, GPS data etc.
//! — are untouched because we only ever SetValue the two keys we own.
//!
//! On non-Windows targets every function is a no-op that reports "not
//! available"; the file compiles unchanged on macOS/Linux.

use super::UserMeta;
use crate::error::{AppError, AppResult};
use std::path::Path;

/// Read `System.Title` and `System.Keywords` via the Shell property system.
/// Returns `None` if the OS has no property handler for this file, or if
/// neither of those properties is present.
pub fn read_user_meta(path: &Path) -> Option<UserMeta> {
    #[cfg(windows)]
    {
        imp::read_user_meta(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// Write `System.Title` and `System.Keywords` via the Shell property system.
///
/// Returns `Err(AppError::MetadataWrite(...))` with a user-visible message
/// when Windows has no property handler for the file, when the handler is
/// read-only, when the file is on a read-only mount, or when Commit fails.
pub fn write_user_meta(path: &Path, meta: &UserMeta) -> AppResult<()> {
    #[cfg(windows)]
    {
        imp::write_user_meta(path, meta)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, meta);
        Err(AppError::MetadataWrite(
            "Windows Shell property system is not available on this platform.".to_string(),
        ))
    }
}

/// Probe: does this file's registered property handler accept writes to
/// `System.Keywords`? Used by the UI to decide whether to enable the tag
/// editor. Result is a per-file check but the callers cache by extension.
pub fn can_write_tags(path: &Path) -> bool {
    #[cfg(windows)]
    {
        imp::can_write_tags(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use super::*;
    use core::mem::ManuallyDrop;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{PROPERTYKEY, S_FALSE, S_OK};
    use windows::Win32::System::Com::StructuredStorage::{
        InitPropVariantFromStringVector, PropVariantClear, PROPVARIANT, PROPVARIANT_0_0,
    };
    use windows::Win32::System::Com::{
        CoInitializeEx, CoTaskMemAlloc, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Variant::VARENUM;
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, IPropertyStoreCapabilities, PSGetPropertyKeyFromName,
        SHGetPropertyStoreFromParsingName, GETPROPERTYSTOREFLAGS, GPS_DEFAULT, GPS_READWRITE,
    };

    const VT_EMPTY: u16 = 0;
    const VT_LPWSTR: u16 = 31;
    const VT_BSTR: u16 = 8;
    const VT_VECTOR: u16 = 0x1000;

    /// RAII helper: matches `CoInitializeEx` with `CoUninitialize` on drop so
    /// each background call is self-contained. `S_FALSE` is expected when the
    /// thread was already CoInitialized (e.g. reused from a pool).
    struct ComGuard {
        initialized: bool,
    }
    impl ComGuard {
        fn new() -> Self {
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let initialized = hr.is_ok() || hr == S_FALSE;
            Self { initialized }
        }
    }
    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.initialized {
                unsafe { CoUninitialize() };
            }
        }
    }

    fn to_wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }
    fn wide_from_str(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn get_key(name: &str) -> AppResult<PROPERTYKEY> {
        let w = wide_from_str(name);
        let mut key = PROPERTYKEY::default();
        unsafe {
            PSGetPropertyKeyFromName(PCWSTR(w.as_ptr()), &mut key).map_err(|e| {
                AppError::Internal(format!("PSGetPropertyKeyFromName({name}) failed: {e}"))
            })?;
        }
        Ok(key)
    }

    fn open_store(
        path: &Path,
        flags: GETPROPERTYSTOREFLAGS,
    ) -> AppResult<IPropertyStore> {
        // `SHGetPropertyStoreFromParsingName` rejects verbatim `\\?\` paths
        // with `E_INVALIDARG (0x80070057)` because the Shell name parser
        // doesn't understand the extended-length escape. Our library-folder
        // path is `fs::canonicalize`d, which on Windows always returns
        // verbatim form, so every file path stored in the DB below that root
        // arrives here with the prefix. Strip it before crossing the FFI.
        let clean = crate::core::formats::common::strip_windows_verbatim_prefix(path);
        let w = to_wide(clean.as_os_str());
        // SAFETY: `w` outlives the FFI call. The 3-arg turbofish anchors the
        // return type (COM interface) so the crate's `T = IPropertyStore` is
        // resolved; the two `_`s let inference pick the PCWSTR / IBindCtx
        // param converters.
        unsafe {
            SHGetPropertyStoreFromParsingName::<_, _, IPropertyStore>(
                PCWSTR(w.as_ptr()),
                None,
                flags,
            )
            .map_err(|e| {
                AppError::MetadataWrite(format!(
                    "Windows Shell has no writable property handler for '{}': {}",
                    clean.display(),
                    e
                ))
            })
        }
    }

    /// Build a VT_LPWSTR PROPVARIANT whose `pwszVal` points at a fresh
    /// `CoTaskMemAlloc`'d wide-string copy of `s`. Ownership of the buffer
    /// transfers to the PROPVARIANT — release it with `PropVariantClear`.
    ///
    /// Some third-party property handlers ignore the "callee copies"
    /// convention and instead call `PropVariantClear` on the parameter you
    /// passed in, which then hits `CoTaskMemFree`. If our string were a
    /// stack `Vec<u16>` buffer that would corrupt the process heap. Using
    /// `CoTaskMemAlloc` matches what those handlers expect.
    unsafe fn make_lpwstr_propvariant(s: &str) -> PROPVARIANT {
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();
        let dst = CoTaskMemAlloc(bytes) as *mut u16;
        assert!(!dst.is_null(), "CoTaskMemAlloc failed");
        std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());

        let mut pv: PROPVARIANT = core::mem::zeroed();
        // Layout: PROPVARIANT { Anonymous: PROPVARIANT_0 (union) { Anonymous:
        // ManuallyDrop<PROPVARIANT_0_0> { vt, ..., Anonymous: union<...> } } }.
        // repr(C) union → same address as first variant, so we can reinterpret
        // the PROPVARIANT bytes as PROPVARIANT_0_0.
        let p: *mut PROPVARIANT_0_0 = &mut pv as *mut PROPVARIANT as *mut PROPVARIANT_0_0;
        (*p).vt = VARENUM(VT_LPWSTR);
        (*p).Anonymous.pwszVal = PWSTR(dst);
        pv
    }

    /// Same layout tricks but with VT_EMPTY — used when the caller wants to
    /// clear (delete) the title.
    unsafe fn make_empty_propvariant() -> PROPVARIANT {
        let mut pv: PROPVARIANT = core::mem::zeroed();
        let p: *mut PROPVARIANT_0_0 = &mut pv as *mut PROPVARIANT as *mut PROPVARIANT_0_0;
        (*p).vt = VARENUM(VT_EMPTY);
        pv
    }

    /// Best-effort free for a VT_LPWSTR PROPVARIANT we built ourselves,
    /// used when `PropVariantClear` isn't safe to call (VT_EMPTY doesn't
    /// need clearing, but freeing the CoTaskMemAlloc'd string still does).
    unsafe fn free_lpwstr_propvariant(pv: &mut PROPVARIANT) {
        let p: *mut PROPVARIANT_0_0 = pv as *mut PROPVARIANT as *mut PROPVARIANT_0_0;
        if (*p).vt.0 == VT_LPWSTR {
            let ptr = (*p).Anonymous.pwszVal.0 as *mut core::ffi::c_void;
            if !ptr.is_null() {
                CoTaskMemFree(Some(ptr));
                (*p).Anonymous.pwszVal = PWSTR(std::ptr::null_mut());
            }
            (*p).vt = VARENUM(VT_EMPTY);
        }
    }

    unsafe fn read_pwsz(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0usize;
        while *p.add(len) != 0 {
            len += 1;
            if len > 1 << 20 {
                break;
            }
        }
        let slice = std::slice::from_raw_parts(p, len);
        String::from_utf16_lossy(slice)
    }

    /// Convert a PROPVARIANT holding a single wide string / BSTR / string
    /// vector into `Vec<String>`. Anything else → empty vec.
    unsafe fn propvariant_to_strings(pv: &PROPVARIANT) -> Vec<String> {
        let inner_ptr = pv as *const PROPVARIANT as *const PROPVARIANT_0_0;
        let vt = (*inner_ptr).vt.0;
        let value = &(*inner_ptr).Anonymous;

        if vt == VT_LPWSTR {
            let s = read_pwsz(value.pwszVal.0);
            return if s.is_empty() { Vec::new() } else { vec![s] };
        }
        if vt == VT_BSTR {
            // BSTR is a wide-string with a 4-byte length prefix; reading it
            // as a null-terminated PWSTR still works because BSTRs are
            // null-terminated by contract.
            let bstr_ref: &ManuallyDrop<windows::core::BSTR> = &value.bstrVal;
            let p = bstr_ref.as_ptr();
            let s = read_pwsz(p);
            return if s.is_empty() { Vec::new() } else { vec![s] };
        }
        if vt == (VT_LPWSTR | VT_VECTOR) {
            let vec = &value.calpwstr;
            let n = vec.cElems as usize;
            if n == 0 || vec.pElems.is_null() {
                return Vec::new();
            }
            let slice = std::slice::from_raw_parts(vec.pElems, n);
            let mut out = Vec::with_capacity(n);
            for w in slice {
                let s = read_pwsz(w.0);
                if !s.trim().is_empty() {
                    out.push(s);
                }
            }
            return out;
        }
        Vec::new()
    }

    /// Safe wrapper around a caller-owned PROPVARIANT from GetValue that
    /// runs PropVariantClear on drop so we don't leak CoTaskMemAlloc'd
    /// strings.
    struct OwnedPropVariant(PROPVARIANT);
    impl Drop for OwnedPropVariant {
        fn drop(&mut self) {
            unsafe {
                let _ = PropVariantClear(&mut self.0);
            }
        }
    }

    pub fn read_user_meta(path: &Path) -> Option<UserMeta> {
        let _com = ComGuard::new();
        let store = open_store(path, GPS_DEFAULT).ok()?;
        let title_key = get_key("System.Title").ok()?;
        let kw_key = get_key("System.Keywords").ok()?;

        let mut out = UserMeta::default();
        unsafe {
            if let Ok(pv) = store.GetValue(&title_key) {
                let owned = OwnedPropVariant(pv);
                let ts = propvariant_to_strings(&owned.0);
                if let Some(t) = ts.into_iter().next() {
                    let trimmed = t.trim().to_string();
                    if !trimmed.is_empty() {
                        out.title = Some(trimmed);
                    }
                }
            }
            if let Ok(pv) = store.GetValue(&kw_key) {
                let owned = OwnedPropVariant(pv);
                out.tags = propvariant_to_strings(&owned.0)
                    .into_iter()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        if out.title.is_none() && out.tags.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    pub fn write_user_meta(path: &Path, meta: &UserMeta) -> AppResult<()> {
        let _com = ComGuard::new();
        let store = open_store(path, GPS_READWRITE)?;
        let title_key = get_key("System.Title")?;
        let kw_key = get_key("System.Keywords")?;

        // ----- Title -----
        // Build a PROPVARIANT whose string is CoTaskMemAlloc'd, then free
        // it ourselves after SetValue. This mirrors what `PropVariantClear`
        // expects and is the shape most third-party handlers assume.
        let mut title_pv: PROPVARIANT = unsafe {
            match meta.title.as_deref() {
                Some(t) if !t.is_empty() => make_lpwstr_propvariant(t),
                _ => make_empty_propvariant(),
            }
        };
        let title_res = unsafe { store.SetValue(&title_key, &title_pv) };
        unsafe { free_lpwstr_propvariant(&mut title_pv) };
        title_res.map_err(|e| {
            AppError::MetadataWrite(format!(
                "SetValue System.Title on '{}' failed: {}",
                path.display(),
                e
            ))
        })?;

        // ----- Keywords (tags) -----
        // `InitPropVariantFromStringVector` internally allocates the
        // CALPWSTR vector on the COM heap and copies the strings into it,
        // so we can safely let the `PROPVARIANT` outlive our local
        // `tag_wides`/`ptrs` and let `PropVariantClear` free it on Drop.
        let tag_wides: Vec<Vec<u16>> = meta
            .tags
            .iter()
            .filter(|s| !s.trim().is_empty())
            .map(|s| wide_from_str(s.trim()))
            .collect();
        let ptrs: Vec<PCWSTR> = tag_wides.iter().map(|v| PCWSTR(v.as_ptr())).collect();
        let kw_pv: PROPVARIANT = unsafe {
            InitPropVariantFromStringVector(Some(&ptrs)).map_err(|e| {
                AppError::MetadataWrite(format!(
                    "InitPropVariantFromStringVector(tags): {e}"
                ))
            })?
        };
        drop(ptrs);
        drop(tag_wides);
        let kw_pv_owned = OwnedPropVariant(kw_pv);

        unsafe {
            store.SetValue(&kw_key, &kw_pv_owned.0).map_err(|e| {
                AppError::MetadataWrite(format!(
                    "SetValue System.Keywords on '{}' failed: {}",
                    path.display(),
                    e
                ))
            })?;
            store.Commit().map_err(|e| {
                AppError::MetadataWrite(format!(
                    "IPropertyStore::Commit on '{}' failed: {}",
                    path.display(),
                    e
                ))
            })?;
        }

        // `kw_pv_owned` drops here → PropVariantClear frees the CALPWSTR
        // vector allocated by InitPropVariantFromStringVector. `store`
        // drops next → Release. `_com` drops last → CoUninitialize.
        drop(kw_pv_owned);
        Ok(())
    }

    pub fn can_write_tags(path: &Path) -> bool {
        let _com = ComGuard::new();
        // Open with GPS_READWRITE first — this filters out extensions with
        // no writable property handler (STG_E_ACCESSDENIED /
        // REGDB_E_CLASSNOTREG).
        let Ok(store) = open_store(path, GPS_READWRITE) else {
            return false;
        };
        // Then ask the store *per-property* whether Keywords accepts writes
        // via `IPropertyStoreCapabilities::IsPropertyWritable`. Some handlers
        // (BMP, DIB, and a few older raster codecs) return a writable store
        // but reject SetValue on `System.Keywords` — this API is the only
        // reliable pre-flight test that catches them.
        let Ok(kw_key) = get_key("System.Keywords") else {
            return false;
        };
        let Ok(caps) = store.cast::<IPropertyStoreCapabilities>() else {
            // No capabilities interface: assume the store's willingness to
            // open R/W is authoritative. Better to try + surface an error
            // than to lock out edits.
            return true;
        };
        let hr = unsafe { caps.IsPropertyWritable(&kw_key) };
        // Contract: S_OK = writable, S_FALSE = not writable, everything
        // else = error.
        hr == S_OK
    }
}
