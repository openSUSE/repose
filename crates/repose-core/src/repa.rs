//! REPA parse (`product:version:arch:repo`) — Python `repose.types.repa.Repa`.

use thiserror::Error;

/// Parsed repository/product pattern from CLI REPA tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repa {
    pub(crate) product: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) arch: Option<String>,
    pub(crate) repo: Option<String>,
    pub(crate) baseversion: Option<String>,
    smallver: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepaError {
    #[error("REPA can't have more than 4 components")]
    TooManyComponents,
}

impl Repa {
    /// Parse a colon-separated REPA string (≤4 components).
    ///
    /// Empty segments become `None`. More than four components → error.
    pub fn parse(raw: &str) -> Result<Self, RepaError> {
        let parts: Vec<&str> = raw.split(':').collect();
        if parts.len() > 4 {
            return Err(RepaError::TooManyComponents);
        }
        let mut padded: Vec<Option<String>> = parts
            .into_iter()
            .map(|p| {
                if p.is_empty() {
                    None
                } else {
                    Some(p.to_string())
                }
            })
            .collect();
        while padded.len() < 4 {
            padded.push(None);
        }
        let product = padded[0].clone();
        let version = padded[1].clone();
        let arch = padded[2].clone();
        let repo = padded[3].clone();
        Ok(Self::from_parts(product, version, arch, repo))
    }

    #[must_use]
    pub(crate) fn from_parts(
        product: Option<String>,
        version: Option<String>,
        arch: Option<String>,
        repo: Option<String>,
    ) -> Self {
        let (baseversion, smallver) = derive_version_parts(version.as_deref());
        Self {
            product,
            version,
            arch,
            repo,
            baseversion,
            smallver,
        }
    }
}

fn derive_version_parts(version: Option<&str>) -> (Option<String>, Option<String>) {
    match version {
        Some(v) if v.contains("-SP") => {
            let base = v.split('-').next().unwrap_or(v).to_string();
            let small = format!("-{}", v.split('-').next_back().unwrap_or(""));
            (Some(base), Some(small))
        }
        Some(v) => (Some(v.to_string()), None),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn vector_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors/repa/parse.json")
    }

    #[test]
    fn matches_vector_parse_json() {
        let raw = std::fs::read_to_string(vector_path()).expect("vector repa/parse.json");
        let cases: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        for case in cases {
            let input = case["input"].as_str().unwrap();
            let ok = case["ok"].as_bool().unwrap();
            match Repa::parse(input) {
                Ok(r) if ok => {
                    assert_eq!(
                        r.product.as_deref(),
                        case["product"].as_str(),
                        "product for {input:?}"
                    );
                    assert_eq!(
                        r.version.as_deref(),
                        case["version"].as_str(),
                        "version for {input:?}"
                    );
                    assert_eq!(
                        r.arch.as_deref(),
                        case["arch"].as_str(),
                        "arch for {input:?}"
                    );
                    assert_eq!(
                        r.repo.as_deref(),
                        case["repo"].as_str(),
                        "repo for {input:?}"
                    );
                    assert_eq!(
                        r.baseversion.as_deref(),
                        case["baseversion"].as_str(),
                        "baseversion for {input:?}"
                    );
                    assert_eq!(
                        r.smallver.as_deref(),
                        case["smallver"].as_str(),
                        "smallver for {input:?}"
                    );
                }
                Err(_) if !ok => {}
                Ok(_) => panic!("expected error for {input:?}"),
                Err(e) => panic!("unexpected error for {input:?}: {e}"),
            }
        }
    }
}
