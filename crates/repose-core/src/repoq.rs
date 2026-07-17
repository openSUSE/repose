//! Resolve REPA → repo name/url/refresh (Python `Repoq`).

use serde_json::Value;
use thiserror::Error;

use crate::repa::Repa;
use crate::types::{Product, System};

/// One resolved repository (Python `Repos` NamedTuple).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repos {
    pub name: String,
    pub url: String,
    pub refresh: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepoqError {
    #[error("{0}")]
    Message(String),
}

/// Unsupported installed product when solving defaults for a host.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("Unsupported product: {0}")]
pub struct UnsupportedProductError(pub String);

/// Repository template resolver.
pub struct Repoq {
    template: Value,
}

impl Repoq {
    #[must_use]
    pub fn new(template: Value) -> Self {
        Self { template }
    }

    /// Resolve repositories for a REPA against host base product.
    pub fn solve_repa(
        &self,
        orepa: &Repa,
        base: &Product,
    ) -> Result<std::collections::BTreeMap<String, Vec<Repos>>, RepoqError> {
        // Do not mutate caller's Repa — fill arch/version locally.
        let product = orepa.product.clone().unwrap_or_default();
        let arch = orepa.arch.clone().unwrap_or_else(|| base.arch.clone());
        let version_opt = orepa.version.clone().or_else(|| Some(base.version.clone()));
        let version = version_opt.clone().unwrap_or_default();
        let baseversion = orepa.baseversion.clone().or_else(|| derive_base(&version));

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
            let url_raw = subtemplate
                .get(repo)
                .and_then(|r| r.get("url"))
                .and_then(|u| u.as_str())
                .unwrap_or("http://empty.url");
            let url = substitute(url_raw, &resolved_version, &arch, &shortversion).map_err(
                |e| {
                    RepoqError::Message(format!(
                        "Cannot resolve REPA {name_prefix}{repo}: missing template key or URL placeholder {e}"
                    ))
                },
            )?;
            let refresh = subtemplate
                .get(repo)
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
                let url_raw = subtemplate
                    .get(repo_name)
                    .and_then(|r| r.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("http://empty.url");
                let url = substitute(url_raw, &resolved_version, &arch, &shortversion)
                    .map_err(|e| {
                        RepoqError::Message(format!(
                            "Cannot resolve REPA {name_prefix}: missing template key or URL placeholder {e}"
                        ))
                    })?;
                let refresh = subtemplate
                    .get(repo_name)
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
    pub fn solve_product(
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
                let url_raw = sub
                    .get(repo_name)
                    .and_then(|r| r.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("http://empty.url");
                let url = substitute(url_raw, &product.version, &product.arch, &shortver).map_err(
                    |_| UnsupportedProductError(format!("{}:{}", product.name, product.version)),
                )?;
                let refresh = sub
                    .get(repo_name)
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

fn derive_base(version: &str) -> Option<String> {
    if version.contains("-SP") {
        version.split('-').next().map(str::to_string)
    } else if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

/// Minimal `string.Template.substitute` for `$foo` / `${foo}`.
fn substitute(template: &str, version: &str, arch: &str, shortver: &str) -> Result<String, String> {
    let mut out = String::new();
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' {
            if i + 1 < chars.len() && chars[i + 1] == '{' {
                let end = chars[i + 2..]
                    .iter()
                    .position(|&c| c == '}')
                    .ok_or_else(|| "'{'".to_string())?;
                let key: String = chars[i + 2..i + 2 + end].iter().collect();
                out.push_str(lookup(&key, version, arch, shortver)?);
                i += 3 + end;
            } else {
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_')
                {
                    end += 1;
                }
                if end == start {
                    out.push('$');
                    i += 1;
                    continue;
                }
                let key: String = chars[start..end].iter().collect();
                out.push_str(lookup(&key, version, arch, shortver)?);
                i = end;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
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

/// Very small close-match: longest common prefix among candidates.
fn close_match<'a>(word: &str, candidates: &[&'a str]) -> Option<&'a str> {
    if word.is_empty() || candidates.is_empty() {
        return None;
    }
    candidates
        .iter()
        .copied()
        .filter(|c| {
            c.starts_with(&word[..word.len().min(1)]) || word.starts_with(&c[..c.len().min(1)])
        })
        .max_by_key(|c| {
            c.chars()
                .zip(word.chars())
                .take_while(|(a, b)| a == b)
                .count()
        })
        .filter(|c| {
            c.chars()
                .zip(word.chars())
                .take_while(|(a, b)| a == b)
                .count()
                >= 1
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::load_template;
    use std::path::PathBuf;

    #[test]
    fn solve_repa_oracle() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/oracle/repoq/solve_repa.json");
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
}
