//! XMP packet parsing (read-only after the DB redesign).
//!
//! This module is intentionally format-agnostic: it consumes an
//! `<x:xmpmeta>...</x:xmpmeta>` byte block extracted by a format
//! handler (JPEG APP1, PNG iTXt, WebP RIFF chunk, GIF89a Application
//! Extension, TIFF tag 700, sidecar `.xmp`) and returns whichever
//! title + tags it can find.
//!
//! Field mapping (industry standard, understood by Adobe Bridge /
//! Lightroom, digiKam, and Windows File Explorer's "Tags"):
//! - `dc:title`       ↔ Title
//! - `dc:subject`     ↔ Tags
//! - `MicrosoftPhoto:LastKeywordXMP` ↔ fallback Tags source used by
//!   older versions of Windows Explorer.
//!
//! The old writer (`build_xmp_packet` + `merge_user_edits`) was
//! deleted with the DB redesign — Magpie never round-trips changes
//! back into the source file anymore.

use crate::error::{AppError, AppResult};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::BufReader;

/// Full XMP user metadata we parse and write. Magpie's UI only exposes
/// `title` and `subjects` (tags) — the other two fields are preserved
/// on the read-modify-write cycle so we don't clobber values other tools
/// (Lightroom, digiKam) wrote.
#[derive(Debug, Default, Clone)]
pub struct XmpUserMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub rating: Option<i64>,
    pub subjects: Option<Vec<String>>,
}

/// Parse an XMP packet, extracting the fields Magpie tracks.
pub fn parse_xmp(xmp_bytes: &[u8]) -> AppResult<XmpUserMeta> {
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
    let mut out = XmpUserMeta::default();
    let mut buf = Vec::new();
    let mut subjects: Vec<String> = Vec::new();
    let mut ms_keywords: Vec<String> = Vec::new();

    loop {
        match r.read_event_into(&mut buf) {
            Err(e) => return Err(AppError::MetadataRead(format!("xmp xml: {e}"))),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name().as_ref().to_ascii_lowercase();
                stack.push(name.clone());

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
                    if name.ends_with(b"dc:description") || name == b"description" {
                        state = State::Description;
                    }
                } else if name.ends_with(b":rating") || name == b"rating" {
                    state = State::Rating;
                } else if name.ends_with(b":subject") || name == b"subject" {
                    state = State::SubjectBag;
                } else if name.ends_with(b":lastkeywordxmp") || name.ends_with(b":keywords") {
                    state = State::MsKeywordBag;
                } else if state == State::SubjectBag && (name.ends_with(b":li") || name == b"li") {
                    state = State::SubjectItem;
                } else if state == State::MsKeywordBag && (name.ends_with(b":li") || name == b"li")
                {
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

    if subjects.is_empty() && !ms_keywords.is_empty() {
        subjects = ms_keywords;
    }

    if !subjects.is_empty() {
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

/// Turn XmpUserMeta into the trait-level UserMeta the UI sees. Rating
/// and description are silently dropped.
pub fn to_user_meta(x: &XmpUserMeta) -> super::UserMeta {
    super::UserMeta {
        title: x.title.clone(),
        tags: x.subjects.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_explorer_tags_read() {
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
        let parsed = parse_xmp(xml.as_bytes()).unwrap();
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["vacation".to_string(), "family".to_string()][..])
        );
    }

    #[test]
    fn microsoft_only_keywords_read() {
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
        let parsed = parse_xmp(xml.as_bytes()).unwrap();
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["alpha".to_string(), "beta".to_string()][..])
        );
    }
}
