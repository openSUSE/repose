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
}
