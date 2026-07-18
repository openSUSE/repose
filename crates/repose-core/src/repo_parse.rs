//! Parse `zypper -x lr` XML (Python `parse_repositories`).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::types::Repository;

/// Parse zypper XML into repositories; skip malformed entries.
pub fn parse_repositories(xml: &str) -> Vec<Repository> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut repos = Vec::new();
    let mut buf = Vec::new();

    let mut in_repo = false;
    let mut alias = String::new();
    let mut name = String::new();
    let mut enabled = String::new();
    let mut url = String::new();
    let mut in_url = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if tag == "repo" {
                    in_repo = true;
                    alias.clear();
                    name.clear();
                    enabled.clear();
                    url.clear();
                    for a in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
                        let val = String::from_utf8_lossy(a.value.as_ref()).into_owned();
                        match key.as_str() {
                            "alias" => alias = val.clone(),
                            "name" => name = val.clone(),
                            "enabled" => enabled = val,
                            _ => {}
                        }
                    }
                } else if tag == "url" && in_repo {
                    in_url = true;
                }
            }
            Ok(Event::Text(t)) if in_url => {
                url = t.decode().map(|c| c.into_owned()).unwrap_or_default();
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if tag == "url" {
                    in_url = false;
                } else if tag == "repo" && in_repo {
                    in_repo = false;
                    // Python skips a repo only when a required field is *absent*.
                    // Combined with `trim_text` (above), this non-empty check is a
                    // documented delta on pathological input: it (a) drops
                    // present-but-empty/whitespace-only fields Python keeps (e.g.
                    // `enabled=""`, `<url> </url>`), and (b) strips surrounding
                    // whitespace Python preserves in a padded `<url>`. Real
                    // `zypper -x lr` emits neither.
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

    #[test]
    fn matches_oracle_parse() {
        let raw = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/oracle/zypper_lr/parse.json"),
        )
        .expect("oracle zypper_lr/parse.json");
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
