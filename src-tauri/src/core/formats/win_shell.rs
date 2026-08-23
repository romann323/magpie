//! Read-only fallback that piggybacks on Windows' Shell Property System.
//!
//! For formats Magpie doesn't have a native XMP reader for (RAW
//! families, MP4/MOV/HEIC, PDF, ...) we ask `IPropertyStore` — the same
//! mechanism Explorer's *Properties → Details* tab uses — for whatever
//! `System.Title` and `System.Keywords` values the file already
//! carries. This is how libraries pre-tagged from Windows Explorer
//! import cleanly on first scan.
//!
//! After the DB redesign this module is read-only: Magpie never writes
//! back into the source file's property store. All persistence goes
//! into the per-folder library DB.
//!
//! On non-Windows targets [`read_user_meta`] is a no-op returning
//! `None`; the file compiles unchanged on macOS/Linux.

use super::UserMeta;
use std::path::Path;

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

#[cfg(windows)]
mod imp {
    use super::*;
    use core::mem::ManuallyDrop;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{PROPERTYKEY, S_FALSE};
    use windows::Win32::System::Com::StructuredStorage::{
        PropVariantClear, PROPVARIANT, PROPVARIANT_0_0,
    };
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::{
        IPropertyStore, PSGetPropertyKeyFromName, SHGetPropertyStoreFromParsingName,
        GETPROPERTYSTOREFLAGS, GPS_DEFAULT,
    };

    const VT_LPWSTR: u16 = 31;
    const VT_BSTR: u16 = 8;
    const VT_VECTOR: u16 = 0x1000;

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

    fn get_key(name: &str) -> Option<PROPERTYKEY> {
        let w = wide_from_str(name);
        let mut key = PROPERTYKEY::default();
        unsafe {
            PSGetPropertyKeyFromName(PCWSTR(w.as_ptr()), &mut key).ok()?;
        }
        Some(key)
    }

    fn open_store(path: &Path, flags: GETPROPERTYSTOREFLAGS) -> Option<IPropertyStore> {
        let clean = crate::core::formats::common::strip_windows_verbatim_prefix(path);
        let w = to_wide(clean.as_os_str());
        unsafe {
            SHGetPropertyStoreFromParsingName::<_, _, IPropertyStore>(
                PCWSTR(w.as_ptr()),
                None,
                flags,
            )
            .ok()
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

    unsafe fn propvariant_to_strings(pv: &PROPVARIANT) -> Vec<String> {
        let inner_ptr = pv as *const PROPVARIANT as *const PROPVARIANT_0_0;
        let vt = (*inner_ptr).vt.0;
        let value = &(*inner_ptr).Anonymous;

        if vt == VT_LPWSTR {
            let s = read_pwsz(value.pwszVal.0);
            return if s.is_empty() { Vec::new() } else { vec![s] };
        }
        if vt == VT_BSTR {
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
        let store = open_store(path, GPS_DEFAULT)?;
        let title_key = get_key("System.Title")?;
        let kw_key = get_key("System.Keywords")?;

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
}
