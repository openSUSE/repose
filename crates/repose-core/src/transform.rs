//! Version transforms for refhost YAML (`transform_version_partialy` spelling preserved).

use serde_json::{Value, json};

/// Normalise a version string into major/minor or pass through unchanged.
///
/// Mirrors Python `repose.types.refhost.transformations.transform_version_partialy`.
#[must_use]
pub(crate) fn transform_version_partialy(version: &str) -> Value {
    let result = (|| {
        if version.contains('-') {
            let (major_str, minor_str) = version.split_once('-')?;
            let major: i64 = major_str.parse().ok()?;
            return Some(json!({"major": major, "minor": minor_str}));
        }
        if version.contains('.') {
            let (major_str, minor_str) = version.split_once('.')?;
            let major: i64 = major_str.parse().ok()?;
            let minor: i64 = minor_str.parse().ok()?;
            return Some(json!({"major": major, "minor": minor}));
        }
        if version == "ALL" {
            return Some(json!({"major": "ALL"}));
        }
        let major: i64 = version.parse().ok()?;
        Some(json!({"major": major}))
    })();

    result.unwrap_or_else(|| Value::String(version.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn matches_vector() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/transform/version.json");
        let raw = std::fs::read_to_string(path).unwrap();
        let cases: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        for case in cases {
            let input = case["input"].as_str().unwrap();
            let expected = &case["output"];
            assert_eq!(
                transform_version_partialy(input),
                *expected,
                "transform({input:?})"
            );
        }
    }
}
