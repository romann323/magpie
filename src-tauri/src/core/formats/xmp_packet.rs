//! XMP packet construction and parsing.
//!
//! This module is intentionally format-agnostic: it produces / consumes an
//! `<x:xmpmeta>...</x:xmpmeta>` byte block and knows nothing about how that
//! block gets embedded (JPEG APP1, PNG iTXt, WebP RIFF chunk, GIF89a
//! Application Extension, TIFF tag 700, ...). Each format handler is
//! responsible for extracting the packet from a file, calling
//! [`parse_xmp`], mutating the returned struct, then handing the output of
//! [`build_xmp_packet`] back to the format-specific embedder.
//!
//! Field mapping (industry standard, understood by Adobe Bridge / Lightroom,
//! digiKam, and Windows File Explorer's "Tags"):
//! - `dc:title`       ↔ Title
//! - `dc:subject`     ↔ Tags
//! - `dc:description` ↔ preserved from disk, no Magpie UI
//! - `xmp:Rating`     ↔ preserved from disk, no Magpie UI

use crate::brand;
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

/// Build an XMP packet from user metadata. Values that are `None`/empty are
/// omitted (rather than emitted as empty elements) so downstream readers
/// don't see junk fields.
///
/// `xmp:MetadataDate` is stamped with the current UTC time so tools that
/// track edit history know a new edit landed.
pub fn build_xmp_packet(m: &XmpUserMeta) -> String {
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
    xml.push_str(&format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"{}\">\n",
        brand::PRODUCT_NAME
    ));
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

/// Merge the caller-supplied edits into an existing XMP packet. Missing
/// UserMeta fields leave the existing packet field untouched — this is how
/// Magpie avoids clobbering rating/description written by other tools.
pub fn merge_user_edits(existing: XmpUserMeta, edits: &super::UserMeta) -> XmpUserMeta {
    XmpUserMeta {
        title: edits.title.clone().or(existing.title),
        description: existing.description,
        rating: existing.rating,
        subjects: if edits.tags.is_empty() {
            existing.subjects
        } else {
            Some(edits.tags.clone())
        },
    }
}

/// Turn XmpUserMeta into the trait-level UserMeta the UI sees. Rating and
/// description are silently dropped.
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
    fn roundtrip_preserves_all_fields() {
        let m = XmpUserMeta {
            title: Some("Sunset".into()),
            description: Some("From the balcony".into()),
            rating: Some(4),
            subjects: Some(vec!["outdoor".into(), "sunset".into()]),
        };
        let xml = build_xmp_packet(&m);
        let parsed = parse_xmp(xml.as_bytes()).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Sunset"));
        assert_eq!(parsed.description.as_deref(), Some("From the balcony"));
        assert_eq!(parsed.rating, Some(4));
        assert_eq!(
            parsed.subjects.as_deref(),
            Some(&["outdoor".to_string(), "sunset".to_string()][..])
        );
    }

    /// Magpie no longer surfaces rating/description, but files written by
    /// Lightroom or digiKam still contain them. The read-modify-write cycle
    /// must preserve those values byte-identical.
    #[test]
    fn merge_preserves_foreign_rating_and_description() {
        let existing = XmpUserMeta {
            title: Some("Old title".into()),
            description: Some("Lightroom caption".into()),
            rating: Some(5),
            subjects: Some(vec!["old".into()]),
        };
        let edits = super::super::UserMeta {
            title: Some("New title".into()),
            tags: vec!["new".into()],
        };
        let merged = merge_user_edits(existing, &edits);
        assert_eq!(merged.title.as_deref(), Some("New title"));
        assert_eq!(merged.description.as_deref(), Some("Lightroom caption"));
        assert_eq!(merged.rating, Some(5));
        assert_eq!(
            merged.subjects.as_deref(),
            Some(&["new".to_string()][..])
        );
    }

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
