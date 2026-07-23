//! Resolve REPA → repo name/url/refresh (Python `Repoq`).

use serde_json::Value;
use thiserror::Error;

use crate::repa::Repa;
use crate::types::{Product, System};

/// One resolved repository (Python `Repos` NamedTuple).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repos {
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) refresh: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepoqError {
    #[error("{0}")]
    Message(String),
}

/// Unsupported installed product when solving defaults for a host.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("Unsupported product: {0}")]
pub struct UnsupportedProductError(String);

/// Repository template resolver.
pub(crate) struct Repoq {
    template: Value,
}

impl Repoq {
    #[must_use]
    pub(crate) const fn new(template: Value) -> Self {
        Self { template }
    }

    /// Resolve repositories for a REPA against host base product.
    pub(crate) fn solve_repa(
        &self,
        orepa: &Repa,
        base: &Product,
    ) -> Result<std::collections::BTreeMap<String, Vec<Repos>>, RepoqError> {
        // Do not mutate caller's Repa — fill arch/version locally.
        let product = orepa.product.clone().unwrap_or_default();
        let arch = orepa.arch.clone().unwrap_or_else(|| base.arch.clone());
        let version = orepa
            .version
            .clone()
            .unwrap_or_else(|| base.version.clone());
        // Python derives `baseversion` only at Repa construction (from the
        // REPA's own version component) and never recomputes it after the
        // version is inherited from the host base product. A version-less
        // REPA therefore has no baseversion fallback.
        let baseversion = orepa.baseversion.clone();

        let products = self
            .template
            .as_object()
            .ok_or_else(|| RepoqError::Message("template is not a mapping".into()))?;

        if !products.contains_key(&product) {
            let keys: Vec<&str> = products.keys().map(|k| k.as_str()).collect();
            let mut msg = format!(
                "Not known product: {}",
                orepa.product.as_deref().unwrap_or("")
            );
            if let Some(suggestion) = close_match(orepa.product.as_deref().unwrap_or(""), &keys) {
                msg.push_str(&format!(" Did you mean {suggestion}?"));
            }
            return Err(RepoqError::Message(msg));
        }

        let product_map = &products[&product];
        let (subtemplate, resolved_version) = if let Some(st) = product_map.get(&version) {
            (st, version.clone())
        } else if let Some(bv) = baseversion.as_ref() {
            if let Some(st) = product_map.get(bv) {
                (st, bv.clone())
            } else {
                return Err(RepoqError::Message(format!(
                    "Unknow version: {version} for product: {product}"
                )));
            }
        } else {
            return Err(RepoqError::Message(format!(
                "Unknow version: {version} for product: {product}"
            )));
        };

        let name_prefix = format!("{product}:{resolved_version}::");
        let shortversion = resolved_version.replace('-', "");

        let result_list = if let Some(repo) = &orepa.repo {
            // Python: `subtemplate.get(repo, {"url": "http://empty.url"})["url"]`
            // — a repo *absent* from the template falls back to the marker URL,
            // but an entry present *without* a `url` key raises (KeyError →
            // ValueError). Never fabricate a URL for a present entry.
            let entry = subtemplate.get(repo);
            let url_raw = match entry {
                None => "http://empty.url",
                Some(r) => r.get("url").and_then(|u| u.as_str()).ok_or_else(|| {
                    RepoqError::Message(format!(
                        "Cannot resolve REPA {name_prefix}{repo}: missing template key or URL placeholder 'url'"
                    ))
                })?,
            };
            let url = substitute(url_raw, &resolved_version, &arch, &shortversion).map_err(
                |e| {
                    RepoqError::Message(format!(
                        "Cannot resolve REPA {name_prefix}{repo}: missing template key or URL placeholder {e}"
                    ))
                },
            )?;
            let refresh = entry
                .and_then(|r| r.get("enabled"))
                .and_then(|e| e.as_bool())
                .unwrap_or(false);
            vec![Repos {
                name: format!("{name_prefix}{repo}"),
                url,
                refresh,
            }]
        } else {
            let defaults = subtemplate
                .get("default_repos")
                .and_then(|d| d.as_array())
                .ok_or_else(|| {
                    RepoqError::Message(format!(
                        "Cannot resolve REPA {name_prefix}: missing template key or URL placeholder 'default_repos'"
                    ))
                })?;
            let mut rlist = Vec::new();
            for x in defaults {
                let repo_name = x.as_str().unwrap_or("");
                // Same missing-`url` semantics as the named-repo branch above.
                let entry = subtemplate.get(repo_name);
                let url_raw = match entry {
                    None => "http://empty.url",
                    Some(r) => r.get("url").and_then(|u| u.as_str()).ok_or_else(|| {
                        RepoqError::Message(format!(
                            "Cannot resolve REPA {name_prefix}: missing template key or URL placeholder 'url'"
                        ))
                    })?,
                };
                let url = substitute(url_raw, &resolved_version, &arch, &shortversion)
                    .map_err(|e| {
                        RepoqError::Message(format!(
                            "Cannot resolve REPA {name_prefix}: missing template key or URL placeholder {e}"
                        ))
                    })?;
                let refresh = entry
                    .and_then(|r| r.get("enabled"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                rlist.push(Repos {
                    name: format!("{name_prefix}{repo_name}"),
                    url,
                    refresh,
                });
            }
            rlist
        };

        let mut result = std::collections::BTreeMap::new();
        result.insert(product, result_list);
        Ok(result)
    }

    /// Resolve default repos for each installed product (exact version keys only).
    pub(crate) fn solve_product(
        &self,
        products: &System,
    ) -> Result<std::collections::BTreeMap<String, Vec<Repos>>, UnsupportedProductError> {
        let mut result = std::collections::BTreeMap::new();
        let installed = flatten_system(products);
        for product in installed {
            let name_prefix = format!("{}:{}::", product.name, product.version);
            let entry = self
                .template
                .get(&product.name)
                .and_then(|p| p.get(&product.version));
            let Some(sub) = entry else {
                return Err(UnsupportedProductError(format!(
                    "{}:{}",
                    product.name, product.version
                )));
            };
            let defaults = sub
                .get("default_repos")
                .and_then(|d| d.as_array())
                .ok_or_else(|| {
                    UnsupportedProductError(format!("{}:{}", product.name, product.version))
                })?;
            let shortver = product.version.replace('-', "");
            let mut rlist = Vec::new();
            for repo in defaults {
                let repo_name = repo.as_str().unwrap_or("");
                // As in `solve_repa`: a repo absent from the template falls
                // back to the marker URL (Python `.get(repo, {"url": ...})`),
                // but a present entry without `url` raises (KeyError →
                // UnsuportedProductMessage in Python).
                let entry = sub.get(repo_name);
                let url_raw = match entry {
                    None => "http://empty.url",
                    Some(r) => r.get("url").and_then(|u| u.as_str()).ok_or_else(|| {
                        UnsupportedProductError(format!("{}:{}", product.name, product.version))
                    })?,
                };
                let url = substitute(url_raw, &product.version, &product.arch, &shortver).map_err(
                    |_| UnsupportedProductError(format!("{}:{}", product.name, product.version)),
                )?;
                let refresh = entry
                    .and_then(|r| r.get("enabled"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                rlist.push(Repos {
                    name: format!("{name_prefix}{repo_name}"),
                    url,
                    refresh,
                });
            }
            result.insert(product.name.clone(), rlist);
        }
        Ok(result)
    }
}

fn flatten_system(system: &System) -> Vec<Product> {
    let mut v = vec![system.base.clone()];
    v.extend(system.addons.iter().cloned());
    v
}

/// Minimal `string.Template.substitute` for `$foo` / `${foo}`.
///
/// Scans string slices in place (all delimiters and key characters are
/// ASCII, so every split index is a char boundary) instead of collecting
/// the template into a `Vec<char>`.
fn substitute(template: &str, version: &str, arch: &str, shortver: &str) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(dollar) = rest.find('$') {
        out.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        if let Some(braced) = after.strip_prefix('{') {
            let end = braced.find('}').ok_or_else(|| "'{'".to_string())?;
            out.push_str(lookup(&braced[..end], version, arch, shortver)?);
            rest = &braced[end + 1..];
        } else {
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            if end == 0 {
                // Bare `$` (including `$$`) stays literal, as before.
                out.push('$');
                rest = after;
            } else {
                out.push_str(lookup(&after[..end], version, arch, shortver)?);
                rest = &after[end..];
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn lookup<'a>(
    key: &str,
    version: &'a str,
    arch: &'a str,
    shortver: &'a str,
) -> Result<&'a str, String> {
    match key {
        "version" => Ok(version),
        "arch" => Ok(arch),
        "shortver" => Ok(shortver),
        other => Err(format!("'{other}'")),
    }
}

/// Very small close-match: longest common prefix among candidates sharing the
/// first character. Char-based — byte slicing would panic on a multi-byte
/// (non-ASCII) first character.
fn close_match<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    let first = word.chars().next()?;
    candidates
        .iter()
        .copied()
        .filter(|c| c.starts_with(first))
        .max_by_key(|c| {
            c.chars()
                .zip(word.chars())
                .take_while(|(a, b)| a == b)
                .count()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::load_template;
    use std::path::PathBuf;

    #[test]
    fn solve_repa_vector() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/repoq/solve_repa.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&raw).unwrap();
        let yaml = doc["template"].as_str().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p.yml");
        std::fs::write(&p, yaml).unwrap();
        let tpl = load_template(&p).unwrap();
        let rq = Repoq::new(tpl);
        let base = Product {
            name: "SLES".into(),
            version: "15-SP3".into(),
            arch: "x86_64".into(),
        };
        for case in doc["cases"].as_array().unwrap() {
            let repa_s = case["repa"].as_str().unwrap();
            let ok = case["ok"].as_bool().unwrap();
            let repa = crate::repa::Repa::parse(repa_s).unwrap();
            match rq.solve_repa(&repa, &base) {
                Ok(map) if ok => {
                    let expected = &case["result"];
                    for (k, repos) in &map {
                        let exp_list = expected[k].as_array().unwrap();
                        assert_eq!(repos.len(), exp_list.len(), "len for {repa_s}");
                        for (got, exp) in repos.iter().zip(exp_list) {
                            assert_eq!(got.name, exp["name"].as_str().unwrap());
                            assert_eq!(got.url, exp["url"].as_str().unwrap());
                            assert_eq!(got.refresh, exp["refresh"].as_bool().unwrap());
                        }
                    }
                }
                Err(_) if !ok => {}
                Ok(v) => panic!("expected error for {repa_s}, got {v:?}"),
                Err(e) => panic!("unexpected error for {repa_s}: {e}"),
            }
        }
    }

    /// A product whose version config inherits via a YAML merge key (`<<:`)
    /// must resolve — not raise `UnsupportedProductError`. Regression for
    /// `PackageHub:15-SP6` (which inherits `default_repos` from an anchor).
    #[test]
    fn solve_product_resolves_merge_inheriting_product() {
        use crate::types::{Product, System};
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("merge.yml");
        std::fs::write(
            &p,
            r#"
PackageHub:
  "15": &ph
    pool:
      url: http://example.invalid/product/$version/$arch/
      enabled: true
    default_repos:
      - pool
  "15-SP6":
    <<: *ph
"#,
        )
        .unwrap();
        let tpl = load_template(&p).unwrap();
        let rq = Repoq::new(tpl);
        let system = System {
            base: Product {
                name: "PackageHub".into(),
                version: "15-SP6".into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        };
        let resolved = rq
            .solve_product(&system)
            .expect("merge-inheriting product must resolve");
        let repos = &resolved["PackageHub"];
        assert_eq!(repos.len(), 1, "one default repo inherited via `<<:`");
        assert_eq!(repos[0].name, "PackageHub:15-SP6::pool");
        assert_eq!(
            repos[0].url,
            "http://example.invalid/product/15-SP6/x86_64/"
        );
        assert!(repos[0].refresh, "enabled: true carries through the merge");
    }

    fn base_15sp3() -> Product {
        Product {
            name: "SLES".into(),
            version: "15-SP3".into(),
            arch: "x86_64".into(),
        }
    }

    /// A non-ASCII product must take the normal "not known product" error
    /// path, not panic on a byte slice inside a multi-byte character.
    #[test]
    fn solve_repa_non_ascii_product_errors_cleanly() {
        let rq = Repoq::new(serde_json::json!({ "SLES": {} }));
        let repa = crate::repa::Repa::parse("\u{3a9}oo:15::pool").unwrap();
        let err = rq.solve_repa(&repa, &base_15sp3()).unwrap_err();
        assert_eq!(
            err,
            RepoqError::Message("Not known product: \u{3a9}oo".into()),
            "clean error, no suggestion, no panic"
        );
    }

    /// A repo entry that exists but has no `url` key must error (Python
    /// KeyError → ValueError), never resolve to the bogus `http://empty.url`.
    /// A repo entirely absent from the template still falls back to it.
    #[test]
    fn solve_repa_entry_without_url_key_errors() {
        let rq = Repoq::new(serde_json::json!({
            "SLES": { "15-SP3": { "nourl": { "enabled": true } } }
        }));
        let repa = crate::repa::Repa::parse("SLES:15-SP3::nourl").unwrap();
        let err = rq.solve_repa(&repa, &base_15sp3()).unwrap_err();
        assert_eq!(
            err,
            RepoqError::Message(
                "Cannot resolve REPA SLES:15-SP3::nourl: missing template key or URL placeholder 'url'"
                    .into()
            )
        );
        // Absent entry keeps the Python `.get(repo, {"url": ...})` fallback.
        let absent = crate::repa::Repa::parse("SLES:15-SP3::absent").unwrap();
        let res = rq.solve_repa(&absent, &base_15sp3()).unwrap();
        assert_eq!(res["SLES"][0].url, "http://empty.url");
    }

    /// Same missing-`url` rule inside the `default_repos` expansion.
    #[test]
    fn solve_repa_default_repo_without_url_key_errors() {
        let rq = Repoq::new(serde_json::json!({
            "SLES": { "15-SP3": {
                "default_repos": ["nourl"],
                "nourl": { "enabled": true }
            } }
        }));
        let repa = crate::repa::Repa::parse("SLES:15-SP3").unwrap();
        let err = rq.solve_repa(&repa, &base_15sp3()).unwrap_err();
        assert_eq!(
            err,
            RepoqError::Message(
                "Cannot resolve REPA SLES:15-SP3::: missing template key or URL placeholder 'url'"
                    .into()
            )
        );
    }

    /// `baseversion` comes only from the REPA's own version component. A
    /// version-less REPA inheriting `15-SP3` from the base product must NOT
    /// re-derive `15` and silently resolve against a `"15"` template key.
    #[test]
    fn solve_repa_inherited_version_has_no_baseversion_fallback() {
        let rq = Repoq::new(serde_json::json!({
            "SLES": { "15": {
                "pool": { "url": "http://example.invalid/$version/" },
                "default_repos": ["pool"]
            } }
        }));
        let repa = crate::repa::Repa::parse("SLES").unwrap();
        let err = rq.solve_repa(&repa, &base_15sp3()).unwrap_err();
        assert_eq!(
            err,
            RepoqError::Message("Unknow version: 15-SP3 for product: SLES".into())
        );
        // An explicit `-SP` version still falls back to its parse-derived
        // baseversion, as in Python.
        let explicit = crate::repa::Repa::parse("SLES:15-SP9").unwrap();
        let res = rq.solve_repa(&explicit, &base_15sp3()).unwrap();
        assert_eq!(res["SLES"][0].name, "SLES:15::pool");
        assert_eq!(res["SLES"][0].url, "http://example.invalid/15/");
    }

    /// `solve_product`: an entry without `url` raises (Python
    /// `UnsuportedProductMessage`); an absent entry keeps the fallback URL.
    #[test]
    fn solve_product_entry_without_url_key_errors() {
        use crate::types::System;
        let system = |version: &str| System {
            base: Product {
                name: "SLES".into(),
                version: version.into(),
                arch: "x86_64".into(),
            },
            addons: vec![],
            transactional: false,
        };
        let rq = Repoq::new(serde_json::json!({
            "SLES": { "15-SP3": {
                "default_repos": ["nourl"],
                "nourl": { "enabled": true }
            } }
        }));
        let err = rq.solve_product(&system("15-SP3")).unwrap_err();
        assert_eq!(err, UnsupportedProductError("SLES:15-SP3".into()));

        let rq = Repoq::new(serde_json::json!({
            "SLES": { "15-SP3": { "default_repos": ["absent"] } }
        }));
        let res = rq.solve_product(&system("15-SP3")).unwrap();
        assert_eq!(res["SLES"][0].url, "http://empty.url");
    }
}
