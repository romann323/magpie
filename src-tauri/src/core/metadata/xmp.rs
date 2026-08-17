//! XMP handling.
//!
//! We support:
//! - Reading XMP either from a sidecar `.xmp` file or from a JPEG APP1
//!   `http://ns.adobe.com/xap/1.0/` segment.
//! - Writing XMP as both:
//!   * A **sidecar file** (`Image.xmp` next to `Image.jpg`) — Lightroom-standard
//!     and safe for every format.
//!   * **Embedded XMP inside JPEG/PNG source files** — so tags/titles/ratings
//!     also survive in Windows Explorer, Photos, and any other viewer that
//!     doesn't read sidecars. For formats we can't embed into (HEIC, RAW, …)
//!     we still get the sidecar.
//!
//! Field mapping:
//! - `dc:title`       ← Title
//! - `xmp:Rating`     ← Rating (0..=5)
//! - `dc:description` ← Comment
//! - `dc:subject`     ← Tags
//!
//! Interoperability: this mapping is the industry standard used by Adobe
//! Lightroom, Adobe Bridge, digiKam, and Windows File Explorer's "Tags".

use crate::error::{PicOrgError, PicOrgResult};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct UserMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub rating: Option<i64>,
    pub subjects: Option<Vec<String>>,
}

const XMP_JPEG_MARKER: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const XMP_JPEG_EXT_MARKER: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
/// Standard XMP APP1 payload (marker + packet) must fit in a JPEG segment
/// whose length field is a 16-bit big-endian unsigned int. That gives us
/// 65533 bytes of payload max. Adobe defines an ExtendedXMP mechanism for
/// larger packets, but our packets are always < 4 KB in practice.
const JPEG_MAX_SEGMENT_PAYLOAD: usize = 65533;

/// Extract the raw XMP packet bytes embedded in an image, if any.
/// Currently supports JPEG only.
pub fn extract_embedded_xmp(path: &Path) -> PicOrgResult<Option<Vec<u8>>> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif" => extract_xmp_from_jpeg(path),
        _ => Ok(None),
    }
}

fn extract_xmp_from_jpeg(path: &Path) -> PicOrgResult<Option<Vec<u8>>> {
    let mut f = File::open(path)?;
    let mut buf = Vec::with_capacity(64 * 1024);
    // XMP lives in an APP1 marker that must appear before Start-of-Scan (0xFFDA).
    // We read up to 2 MiB of the file header (Windows Explorer sometimes writes
    // several kilobytes of thumbnail data plus XMP; small margin costs us nothing).
    let cap = 2 * 1024 * 1024;
    let n = (&mut f).take(cap as u64).read_to_end(&mut buf)?;
    let data = &buf[..n];

    // JPEG must start with SOI.
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Ok(None);
    }
    let mut i = 2usize;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            break;
        }
        // Skip fill bytes
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            break;
        }
        let marker = data[i];
        i += 1;
        // Standalone markers with no length
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if i + 2 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            break;
        }
        let payload = &data[i + 2..i + seg_len];
        i += seg_len;
        if marker == 0xE1 && payload.starts_with(XMP_JPEG_MARKER) {
            let start = XMP_JPEG_MARKER.len();
            return Ok(Some(payload[start..].to_vec()));
        }
        if marker == 0xDA {
            // Start of Scan — pixel data begins; no more headers.
            break;
        }
    }
    Ok(None)
}

/// Write (or replace) the embedded XMP packet in the source image at `path`.
///
/// Returns `Ok(true)` if the file was rewritten in place with the new XMP,
/// `Ok(false)` if the format doesn't support embedded XMP writing (caller
/// should still write a sidecar), or `Err(_)` on failure.
///
/// Atomic on Windows: writes a temp file then renames over the original.
pub fn embed_xmp_in_source(path: &Path, xmp_bytes: &[u8]) -> PicOrgResult<bool> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" | "jpe" | "jfif" | "jif" => embed_xmp_in_jpeg(path, xmp_bytes).map(|_| true),
        _ => Ok(false),
    }
}

fn embed_xmp_in_jpeg(path: &Path, xmp_bytes: &[u8]) -> PicOrgResult<()> {
    let orig = std::fs::read(path)
        .map_err(|e| PicOrgError::MetadataWrite(format!("read {}: {e}", path.display())))?;

    if orig.len() < 4 || orig[0] != 0xFF || orig[1] != 0xD8 {
        return Err(PicOrgError::MetadataWrite(format!(
            "{}: not a JPEG (missing SOI)",
            path.display()
        )));
    }

    // Build the new APP1 XMP segment.
    let payload_len = XMP_JPEG_MARKER.len() + xmp_bytes.len();
    if payload_len > JPEG_MAX_SEGMENT_PAYLOAD {
        return Err(PicOrgError::MetadataWrite(format!(
            "XMP packet too large for a single JPEG APP1 segment ({} bytes)",
            payload_len
        )));
    }
    let seg_len_field = (2 + payload_len) as u16;
    let mut new_xmp_segment = Vec::with_capacity(4 + payload_len);
    new_xmp_segment.extend_from_slice(&[0xFF, 0xE1]);
    new_xmp_segment.extend_from_slice(&seg_len_field.to_be_bytes());
    new_xmp_segment.extend_from_slice(XMP_JPEG_MARKER);
    new_xmp_segment.extend_from_slice(xmp_bytes);

    // Rewrite: copy SOI, insert new XMP APP1 right after (Adobe convention
    // says XMP should be the first APP1), then copy the remaining segments
    // while dropping any pre-existing standard XMP or ExtendedXMP APP1s.
    let mut out = Vec::with_capacity(orig.len() + new_xmp_segment.len());
    out.extend_from_slice(&orig[0..2]); // SOI
    out.extend_from_slice(&new_xmp_segment);

    let mut i = 2usize;
    while i < orig.len() {
        if orig[i] != 0xFF {
            // Malformed / trailer bytes — copy the rest as-is.
            out.extend_from_slice(&orig[i..]);
            break;
        }
        // Skip any 0xFF fill bytes.
        let seg_start = i;
        while i < orig.len() && orig[i] == 0xFF {
            i += 1;
        }
        if i >= orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let marker = orig[i];
        i += 1;

        // Standalone markers with no length field.
        if matches!(marker, 0xD8 | 0xD9 | 0x01) || (0xD0..=0xD7).contains(&marker) {
            out.extend_from_slice(&orig[seg_start..i]);
            continue;
        }
        // Start of Scan — everything from here on is compressed image data plus
        // the EOI marker; copy verbatim to the end.
        if marker == 0xDA {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        // Length-prefixed segment.
        if i + 2 > orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let seg_len = u16::from_be_bytes([orig[i], orig[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > orig.len() {
            out.extend_from_slice(&orig[seg_start..]);
            break;
        }
        let payload_start = i + 2;
        let seg_end = i + seg_len;

        // Detect an old standard-XMP or ExtendedXMP APP1 and skip it.
        let is_std_xmp = marker == 0xE1
            && seg_end.saturating_sub(payload_start) >= XMP_JPEG_MARKER.len()
            && orig[payload_start..payload_start + XMP_JPEG_MARKER.len()] == *XMP_JPEG_MARKER;
        let is_ext_xmp = marker == 0xE1
            && seg_end.saturating_sub(payload_start) >= XMP_JPEG_EXT_MARKER.len()
            && orig[payload_start..payload_start + XMP_JPEG_EXT_MARKER.len()]
                == *XMP_JPEG_EXT_MARKER;

        if !is_std_xmp && !is_ext_xmp {
            out.extend_from_slice(&orig[seg_start..seg_end]);
        }
        i = seg_end;
    }

    atomic_write_bytes(path, &out)?;
    Ok(())
}

/// Atomic file replace: write to `<path>.picorg-tmp` in the same directory,
/// then rename over `path`. Renames within the same volume are atomic on both
/// Windows (via MoveFileEx replace-existing) and POSIX.
fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> PicOrgResult<()> {
    use std::io::Write;
    let tmp = {
        let file_name = path
            .file_name()
            .ok_or_else(|| PicOrgError::MetadataWrite("no file name".into()))?
            .to_owned();
        let mut tmp_name = file_name;
        tmp_name.push(".picorg-tmp");
        path.with_file_name(tmp_name)
    };
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| {
            PicOrgError::MetadataWrite(format!("create {}: {e}", tmp.display()))
        })?;
        f.write_all(bytes)
            .map_err(|e| PicOrgError::MetadataWrite(format!("write {}: {e}", tmp.display())))?;
        f.sync_all().ok();
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        // Try to remove the temp file to avoid leaving debris.
        let _ = std::fs::remove_file(&tmp);
        PicOrgError::MetadataWrite(format!(
            "rename {} → {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Parse the small subset of XMP we care about.
pub fn parse_user_metadata(xmp_bytes: &[u8]) -> PicOrgResult<UserMetadata> {
    let mut r = Reader::from_reader(BufReader::new(xmp_bytes));
    r.config_mut().trim_text(true);

    #[derive(PartialEq, Debug)]
    enum State {
        Idle,
        Title,
        Description,
        Rating,
        SubjectBag,
        SubjectItem,
        MsKeywordBag,
        MsKeywordItem,
    }
    let mut state = State::Idle;
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut out = UserMetadata::default();
    let mut buf = Vec::new();
    let mut subjects: Vec<String> = Vec::new();
    let mut ms_keywords: Vec<String> = Vec::new();

    loop {
        match r.read_event_into(&mut buf) {
            Err(e) => return Err(PicOrgError::MetadataRead(format!("xmp xml: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_ascii_lowercase();
                stack.push(name.clone());

                // Attributes on rdf:Description often contain simple values
                // (xmp:Rating, dc:title, dc:description). Also handle Microsoft
                // subject variants written as attributes.
                if name.ends_with(b"rdf:description") || name == b"description" {
                    for attr in e.attributes().with_checks(false).flatten() {
                        let key = attr.key.as_ref().to_ascii_lowercase();
                        if key == b"xmp:rating" {
                            if let Ok(v) = std::str::from_utf8(attr.value.as_ref()) {
                                if let Ok(n) = v.trim().parse::<i64>() {
                                    out.rating = Some(n.clamp(0, 5));
                                }
                            }
                        } else if key == b"dc:title" {
                            if let Ok(v) = std::str::from_utf8(attr.value.as_ref()) {
                                out.title = Some(v.trim().to_string());
                            }
                        } else if key == b"dc:description" {
                            if let Ok(v) = std::str::from_utf8(attr.value.as_ref()) {
                                out.description = Some(v.trim().to_string());
                            }
                        } else if key.ends_with(b":lastkeywordxmp")
                            || key.ends_with(b":keywords")
                            || key.ends_with(b":subject")
                        {
                            if let Ok(v) = std::str::from_utf8(attr.value.as_ref()) {
                                for k in v.split(|c: char| c == ';' || c == ',') {
                                    let t = k.trim();
                                    if !t.is_empty() {
                                        subjects.push(t.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                if name.ends_with(b":title") || name == b"title" {
                    state = State::Title;
                } else if name.ends_with(b":description") || name == b"description" {
                    // Ignore rdf:Description wrapper; only dc:description holds our text.
                    if name.ends_with(b"dc:description") || name == b"description" {
                        state = State::Description;
                    }
                } else if name.ends_with(b":rating") || name == b"rating" {
                    state = State::Rating;
                } else if name.ends_with(b":subject") || name == b"subject" {
                    state = State::SubjectBag;
                } else if name.ends_with(b":lastkeywordxmp")
                    || name.ends_with(b":keywords")
                {
                    // MicrosoftPhoto:LastKeywordXMP, and some other keyword bags
                    // that mirror dc:subject.
                    state = State::MsKeywordBag;
                } else if state == State::SubjectBag && (name.ends_with(b":li") || name == b"li") {
                    state = State::SubjectItem;
                } else if state == State::MsKeywordBag && (name.ends_with(b":li") || name == b"li") {
                    state = State::MsKeywordItem;
                }
            }
            Ok(Event::End(_e)) => {
                let popped = stack.pop();
                if let Some(name) = popped {
                    if state == State::SubjectItem && (name.ends_with(b":li") || name == b"li") {
                        state = State::SubjectBag;
                    } else if state == State::MsKeywordItem
                        && (name.ends_with(b":li") || name == b"li")
                    {
                        state = State::MsKeywordBag;
                    } else if state == State::SubjectBag
                        && (name.ends_with(b":subject") || name == b"subject")
                    {
                        state = State::Idle;
                    } else if state == State::MsKeywordBag
                        && (name.ends_with(b":lastkeywordxmp") || name.ends_with(b":keywords"))
                    {
                        state = State::Idle;
                    } else if matches!(state, State::Title | State::Description | State::Rating) {
                        state = State::Idle;
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let raw = t.unescape().unwrap_or_default().to_string();
                if raw.trim().is_empty() {
                    buf.clear();
                    continue;
                }
                match state {
                    State::Title => {
                        if in_list_item(&stack) || out.title.is_none() {
                            out.title = Some(raw.trim().to_string());
                        }
                    }
                    State::Description => {
                        if in_list_item(&stack) || out.description.is_none() {
                            out.description = Some(raw.trim().to_string());
                        }
                    }
                    State::Rating => {
                        if let Ok(n) = raw.trim().parse::<i64>() {
                            out.rating = Some(n.clamp(0, 5));
                        }
                    }
                    State::SubjectItem => {
                        subjects.push(raw.trim().to_string());
                    }
                    State::MsKeywordItem => {
                        ms_keywords.push(raw.trim().to_string());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }

    // dc:subject wins; fall back to Microsoft keywords only when dc:subject is empty.
    if subjects.is_empty() && !ms_keywords.is_empty() {
        subjects = ms_keywords;
    }

    if !subjects.is_empty() {
        // De-dupe while preserving order.
        let mut seen = std::collections::HashSet::<String>::new();
        subjects.retain(|s| {
            let k = s.to_lowercase();
            if seen.contains(&k) {
                false
            } else {
                seen.insert(k);
                true
            }
        });
        out.subjects = Some(subjects);
    }

    Ok(out)
}

fn in_list_item(stack: &[Vec<u8>]) -> bool {
    stack.iter().any(|n| n.ends_with(b":li") || n == b"li")
}

/// Build an XMP packet for the given user metadata.
/// The output starts with the standard XMP packet wrapper so it can be used
/// as a sidecar file directly.
pub fn build_xmp_packet(m: &UserMetadata) -> String {
    let title = m.title.as_deref().unwrap_or("");
    let description = m.description.as_deref().unwrap_or("");
    let has_title = !title.is_empty();
    let has_desc = !description.is_empty();
    let has_rating = m.rating.is_some();
    let empty: Vec<String> = Vec::new();
    let subjects = m.subjects.as_ref().unwrap_or(&empty);

    let now = chrono::Utc::now().to_rfc3339();

    let mut xml = String::new();
    xml.push_str("<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n");
    xml.push_str("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"PicOrg\">\n");
    xml.push_str("  <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n");
    xml.push_str("    <rdf:Description rdf:about=\"\"\n");
    xml.push_str("        xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n");
    xml.push_str("        xmlns:dc=\"http://purl.org/dc/elements/1.1/\"");
    if has_rating {
        xml.push_str(&format!("\n        xmp:Rating=\"{}\"", m.rating.unwrap_or(0)));
    }
    xml.push_str(&format!("\n        xmp:MetadataDate=\"{}\">\n", now));

    if has_title {
        xml.push_str("      <dc:title>\n");
        xml.push_str("        <rdf:Alt>\n");
        xml.push_str(&format!(
            "          <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n",
            xml_escape(title)
        ));
        xml.push_str("        </rdf:Alt>\n");
        xml.push_str("      </dc:title>\n");
    }

    if has_desc {
        xml.push_str("      <dc:description>\n");
        xml.push_str("        <rdf:Alt>\n");
        xml.push_str(&format!(
            "          <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n",
            xml_escape(description)
        ));
        xml.push_str("        </rdf:Alt>\n");
        xml.push_str("      </dc:description>\n");
    }

    if !subjects.is_empty() {
        xml.push_str("      <dc:subject>\n");
        xml.push_str("        <rdf:Bag>\n");
        for s in subjects {
            xml.push_str(&format!(
                "          <rdf:li>{}</rdf:li>\n",
                xml_escape(s)
            ));
        }
        xml.push_str("        </rdf:Bag>\n");
        xml.push_str("      </dc:subject>\n");
    }

    xml.push_str("    </rdf:Description>\n");
    xml.push_str("  </rdf:RDF>\n");
    xml.push_str("</x:xmpmeta>\n");
    xml.push_str("<?xpacket end=\"w\"?>\n");
    xml
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let m = UserMetadata {
            title: Some("Sunset".into()),
            description: Some("From the balcony".into()),
            rating: Some(4),
            subjects: Some(vec!["outdoor".into(), "sunset".into()]),
        };
        let xml = build_xmp_packet(&m);
        let parsed = parse_user_metadata(xml.as_bytes()).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Sunset"));
        assert_eq!(parsed.description.as_deref(), Some("From the balcony"));
        assert_eq!(parsed.rating, Some(4));
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["outdoor".to_string(), "sunset".to_string()][..])
        );
    }

    /// Windows File Explorer writes tags into both `dc:subject` and
    /// `MicrosoftPhoto:LastKeywordXMP`. Make sure we can read either.
    #[test]
    fn windows_explorer_tags() {
        let xml = r#"<?xml version="1.0"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:dc="http://purl.org/dc/elements/1.1/"
        xmlns:MicrosoftPhoto="http://ns.microsoft.com/photo/1.0/">
      <dc:subject>
        <rdf:Bag>
          <rdf:li>vacation</rdf:li>
          <rdf:li>family</rdf:li>
        </rdf:Bag>
      </dc:subject>
      <MicrosoftPhoto:LastKeywordXMP>
        <rdf:Bag>
          <rdf:li>vacation</rdf:li>
          <rdf:li>family</rdf:li>
          <rdf:li>2024</rdf:li>
        </rdf:Bag>
      </MicrosoftPhoto:LastKeywordXMP>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>"#;
        let parsed = parse_user_metadata(xml.as_bytes()).unwrap();
        // dc:subject wins when present.
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["vacation".to_string(), "family".to_string()][..])
        );
    }

    #[test]
    fn microsoft_only_keywords() {
        let xml = r#"<?xml version="1.0"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
  <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
    <rdf:Description rdf:about=""
        xmlns:MicrosoftPhoto="http://ns.microsoft.com/photo/1.0/">
      <MicrosoftPhoto:LastKeywordXMP>
        <rdf:Bag>
          <rdf:li>alpha</rdf:li>
          <rdf:li>beta</rdf:li>
        </rdf:Bag>
      </MicrosoftPhoto:LastKeywordXMP>
    </rdf:Description>
  </rdf:RDF>
</x:xmpmeta>"#;
        let parsed = parse_user_metadata(xml.as_bytes()).unwrap();
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["alpha".to_string(), "beta".to_string()][..])
        );
    }
}
