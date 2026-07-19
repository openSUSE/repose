//! Parse `zypper -x lr` XML (Python `parse_repositories`).

use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

use crate::product_parse::resolve_general_ref;
use crate::types::Repository;

/// Parse zypper XML into repositories; skip malformed entries.
pub fn parse_repositories(xml: &str) -> Vec<Repository> {
    let mut reader = Reader::from_str(xml);
    let mut repos = Vec::new();
    let mut buf = Vec::new();

    let mut in_repo = false;
    let mut alias = String::new();
    let mut name = String::new();
    let mut enabled = String::new();
    let mut url = String::new();
    // Collecting text of the first `<url>` child (Python `find("./url").text`).
    let mut in_url = false;
    // Python `find` returns the first match: later `<url>` siblings are ignored.
    let mut url_seen = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                // Python `.text` covers only the run up to the first child
                // element, so any element opening inside `<url>` ends it.
                if in_url {
                    in_url = false;
                }
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if tag == "repo" {
                    in_repo = true;
                    alias.clear();
                    name.clear();
                    enabled.clear();
                    url.clear();
                    url_seen = false;
                    for a in e.attributes().flatten() {
                        // Resolve entity/char references in attribute values
                        // (`name="A &amp; B"` → `A & B`), as Python's
                        // ElementTree does (including standard attribute-value
                        // whitespace normalization). An undefined entity makes
                        // Python raise ParseError; keep the documented lenient
                        // delta and skip just that attribute.
                        let Ok(val) = a.normalized_value(XmlVersion::Implicit1_0) else {
                            continue;
                        };
                        match a.key.as_ref() {
                            b"alias" => alias = val.into_owned(),
                            b"name" => name = val.into_owned(),
                            b"enabled" => enabled = val.into_owned(),
                            _ => {}
                        }
                    }
                } else if tag == "url" && in_repo && !url_seen {
                    url_seen = true;
                    in_url = true;
                    url.clear();
                }
            }
            // Text arrives fragmented (entity references and CDATA split it
            // into separate events): accumulate, never assign.
            Ok(Event::Text(t)) if in_url => {
                if let Ok(text) = t.decode() {
                    url.push_str(&text);
                }
            }
            Ok(Event::CData(t)) if in_url => {
                if let Ok(text) = t.decode() {
                    url.push_str(&text);
                }
            }
            Ok(Event::GeneralRef(e)) if in_url => {
                match resolve_general_ref(&e) {
                    Some(s) => url.push_str(&s),
                    // Undefined entity: Python raises ParseError for the whole
                    // document; mirror the malformed-XML delta and stop here.
                    None => break,
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if tag == "url" {
                    in_url = false;
                    // Python keeps `.text` verbatim; trimming padding is a
                    // documented delta (see below). Idempotent on re-entry.
                    url = url.trim().to_string();
                } else if tag == "repo" && in_repo {
                    in_repo = false;
                    // Python skips a repo only when a required field is *absent*.
                    // Combined with the whitespace trim (above), this non-empty
                    // check is a documented delta on pathological input: it (a)
                    // drops present-but-empty/whitespace-only fields Python
                    // keeps (e.g. `enabled=""`, `<url> </url>`), and (b) strips
                    // surrounding whitespace Python preserves in a padded
                    // `<url>`. Real `zypper -x lr` emits neither.
                    if !alias.is_empty()
                        && !name.is_empty()
                        && !enabled.is_empty()
                        && !url.is_empty()
                    {
                        repos.push(Repository {
                            alias: alias.clone(),
                            name: name.clone(),
                            url: url.clone(),
                            state: enabled == "1",
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    repos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_repo_list() {
        let xml = r#"<?xml version="1.0"?>
<stream>
  <repo-list>
    <repo alias="A" name="n" enabled="1">
      <url>http://example.com/</url>
    </repo>
    <repo alias="B" name="n2" enabled="0">
      <url></url>
    </repo>
  </repo-list>
</stream>"#;
        let repos = parse_repositories(xml);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].alias, "A");
        assert!(repos[0].state);
    }

    /// XML entities in attribute values and url text must be resolved and the
    /// fragmented text accumulated, matching Python's ElementTree.
    #[test]
    fn entities_in_attributes_and_url_are_unescaped() {
        let xml = r#"<stream><repo-list><repo alias="r1" name="A &amp; B" enabled="1"><url>http://example.invalid/?a=1&amp;b=2</url></repo></repo-list></stream>"#;
        let repos = parse_repositories(xml);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "A & B");
        assert_eq!(repos[0].url, "http://example.invalid/?a=1&b=2");
    }

    /// Numeric character references and CDATA sections are part of the text.
    #[test]
    fn char_refs_and_cdata_in_url() {
        let xml = r#"<stream><repo-list><repo alias="a" name="n" enabled="1"><url>http&#58;//example.invalid<![CDATA[/cdata/]]>end</url></repo></repo-list></stream>"#;
        let repos = parse_repositories(xml);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].url, "http://example.invalid/cdata/end");
    }

    /// Python `repo.find("./url")` takes the first `<url>` child; a second one
    /// must be ignored, not overwrite or concatenate.
    #[test]
    fn first_url_child_wins() {
        let xml = r#"<stream><repo-list><repo alias="a" name="n" enabled="1"><url>http://example.invalid/first/</url><url>http://example.invalid/second/</url></repo></repo-list></stream>"#;
        let repos = parse_repositories(xml);
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].url, "http://example.invalid/first/");
    }

    #[test]
    fn matches_vector_parse() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/vectors/zypper_lr/parse.json"),
        )
        .expect("vector zypper_lr/parse.json");
        for case in serde_json::from_str::<Vec<serde_json::Value>>(&raw).unwrap() {
            let name = case["name"].as_str().unwrap();
            let xml = case["xml"].as_str().unwrap();
            // Malformed XML: Python raises ParseError; Rust tolerates → empty (delta).
            if case.get("raises").and_then(serde_json::Value::as_bool) == Some(true) {
                assert!(
                    parse_repositories(xml).is_empty(),
                    "case {name}: rust tolerates malformed XML"
                );
                continue;
            }
            let mut got: Vec<(String, String, String, bool)> = parse_repositories(xml)
                .into_iter()
                .map(|r| (r.alias, r.name, r.url, r.state))
                .collect();
            got.sort();
            let mut want: Vec<(String, String, String, bool)> = case["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    (
                        e["alias"].as_str().unwrap().to_string(),
                        e["name"].as_str().unwrap().to_string(),
                        e["url"].as_str().unwrap().to_string(),
                        e["state"].as_bool().unwrap(),
                    )
                })
                .collect();
            want.sort();
            assert_eq!(got, want, "case {name}");
        }
    }
}
