//! Load `products.yml` templates (Python `repose.template.load_template`).

use std::fs;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template {path} must be a YAML mapping, got {got}")]
    NotAMapping { path: String, got: String },
    #[error("failed to read template {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid YAML in template {path}: {source}")]
    Yaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to convert template {path}: {source}")]
    Convert {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "invalid `<<` merge in template {path}: expected a mapping or a sequence of mappings, got {got}"
    )]
    Merge { path: String, got: String },
}

/// Load products YAML; empty/null → `{}`; non-mapping → [`TemplateError`].
///
/// YAML merge keys (`<<:`) are expanded before conversion, matching the
/// semantics of Python's YAML loader that repose's reference implementation
/// relies on. Merges are resolved recursively in post-order so that nested
/// (`Product → version → <<: *anchor`) *and* chained (`<<: *b` where `*b`
/// itself carries `<<: *a`) merges are fully flattened. `serde_yaml`'s own
/// [`serde_yaml::Value::apply_merge`] leaves a residual `<<` key on chained
/// merges, so it is not sufficient here.
pub(crate) fn load_template(path: &Path) -> Result<Value, TemplateError> {
    let path_s = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|source| TemplateError::Io {
        path: path_s.clone(),
        source,
    })?;
    let mut doc: serde_yaml::Value =
        serde_yaml::from_str(&text).map_err(|source| TemplateError::Yaml {
            path: path_s.clone(),
            source,
        })?;
    expand_merges(&mut doc).map_err(|got| TemplateError::Merge {
        path: path_s.clone(),
        got,
    })?;
    // Convert the merged YAML value to `serde_json::Value`. Integer mapping keys
    // are coerced to their string form here, identical to the previous direct
    // `serde_yaml::from_str::<serde_json::Value>` path.
    let value: Value = serde_json::to_value(&doc).map_err(|source| TemplateError::Convert {
        path: path_s.clone(),
        source,
    })?;
    match value {
        Value::Null => Ok(Value::Object(serde_json::Map::new())),
        Value::Object(_) => Ok(value),
        other => Err(TemplateError::NotAMapping {
            path: path_s,
            got: type_name(&other).to_string(),
        }),
    }
}

/// Recursively expand YAML merge keys (`<<:`) in place.
///
/// Post-order: children (including each `<<` source, which is itself a value of
/// the mapping) are resolved first, then the current mapping's own `<<` is
/// merged. Existing keys win over merged keys (`entry().or_insert`), matching
/// YAML merge semantics; because a nearer merge source is inserted before a
/// farther one is reached, chained precedence (own > nearer > farther) is
/// preserved.
///
/// Errors with the offending YAML type name when a `<<` value is not a mapping
/// or a sequence of mappings — ruamel/PyYAML raise `ConstructorError` there,
/// so silently dropping the value would hide a broken template.
fn expand_merges(node: &mut serde_yaml::Value) -> Result<(), String> {
    match node {
        serde_yaml::Value::Mapping(map) => {
            for (_k, v) in map.iter_mut() {
                expand_merges(v)?;
            }
            if let Some(merged) = map.remove("<<") {
                merge_into(map, merged)?;
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for v in seq.iter_mut() {
                expand_merges(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Fold a resolved `<<` source into `map` without overriding existing keys.
///
/// A single mapping merges directly; a sequence merges each mapping in order
/// (earlier entries take precedence, per the YAML merge spec). The source has
/// already had its own merges expanded by [`expand_merges`]. Any other value —
/// a scalar, or a sequence containing a non-mapping — is a template error.
fn merge_into(map: &mut serde_yaml::Mapping, merged: serde_yaml::Value) -> Result<(), String> {
    match merged {
        serde_yaml::Value::Mapping(m) => {
            for (k, v) in m {
                map.entry(k).or_insert(v);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                match item {
                    serde_yaml::Value::Mapping(m) => {
                        for (k, v) in m {
                            map.entry(k).or_insert(v);
                        }
                    }
                    other => return Err(yaml_type_name(&other).to_string()),
                }
            }
        }
        other => return Err(yaml_type_name(&other).to_string()),
    }
    Ok(())
}

const fn yaml_type_name(v: &serde_yaml::Value) -> &'static str {
    match v {
        serde_yaml::Value::Null => "null",
        serde_yaml::Value::Bool(_) => "bool",
        serde_yaml::Value::Number(_) => "number",
        serde_yaml::Value::String(_) => "str",
        serde_yaml::Value::Sequence(_) => "list",
        serde_yaml::Value::Mapping(_) => "dict",
        serde_yaml::Value::Tagged(_) => "tagged",
    }
}

const fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn loads_sample_products_yml() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/vectors/template/sample.yml");
        let t = load_template(&path).unwrap();
        assert!(t.get("SLES").is_some());
        assert!(t["SLES"].get("15-SP3").is_some());
    }

    #[test]
    fn empty_file_is_empty_object() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("empty.yml");
        fs::write(&p, "# comment only\n").unwrap();
        let t = load_template(&p).unwrap();
        assert_eq!(t, Value::Object(serde_json::Map::new()));
    }

    #[test]
    fn sequence_top_level_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("seq.yml");
        fs::write(&p, "- a\n- b\n").unwrap();
        let err = load_template(&p).unwrap_err();
        assert!(matches!(err, TemplateError::NotAMapping { .. }));
    }

    /// Nested + chained YAML merge keys (`<<:`) must be fully flattened, the way
    /// Python's YAML loader does. Regression for merge-inheriting products
    /// (e.g. `PackageHub:15-SP6`) being wrongly treated as unsupported.
    #[test]
    fn expands_nested_and_chained_merge_keys() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("merge.yml");
        // Product -> version -> `<<: *anchor`, plus a chain SP2 -> SP1 -> base.
        fs::write(
            &p,
            r#"
PackageHub:
  "15": &ph
    pool:
      url: http://example.invalid/product/$version/
    default_repos:
      - pool
  "15-SP1": &ph1
    <<: *ph
    update:
      url: http://example.invalid/update/$version/
      enabled: true
    default_repos:
      - pool
      - update
  "15-SP2":
    <<: *ph1
"#,
        )
        .unwrap();
        let t = load_template(&p).unwrap();

        // Direct nested merge: SP1 inherits `pool` from *ph, keeps its own
        // (overriding) `default_repos`, and carries no literal `<<` key.
        let sp1 = &t["PackageHub"]["15-SP1"];
        assert!(sp1.get("pool").is_some(), "SP1 inherits pool from anchor");
        assert!(sp1.get("<<").is_none(), "no literal '<<' key survives");
        assert_eq!(
            sp1["default_repos"],
            serde_json::json!(["pool", "update"]),
            "own default_repos overrides the merged one"
        );

        // Chained merge: SP2 <- SP1 <- base must be fully flattened.
        let sp2 = &t["PackageHub"]["15-SP2"];
        assert!(
            sp2.get("<<").is_none(),
            "no literal '<<' key survives chain"
        );
        assert!(
            sp2.get("pool").is_some(),
            "SP2 inherits pool through the chain"
        );
        assert!(sp2.get("update").is_some(), "SP2 inherits update from SP1");
        assert_eq!(
            sp2["default_repos"],
            serde_json::json!(["pool", "update"]),
            "SP2 inherits SP1's default_repos through the chain"
        );

        // No `<<` key anywhere in the whole document.
        assert_eq!(count_merge_keys(&t), 0);
    }

    /// A `<<:` whose value is a scalar must be a load error (ruamel raises
    /// `ConstructorError`), not a silently dropped key.
    #[test]
    fn scalar_merge_value_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("scalar-merge.yml");
        fs::write(&p, "a:\n  <<: 5\n  own: 1\n").unwrap();
        let err = load_template(&p).unwrap_err();
        assert!(
            matches!(&err, TemplateError::Merge { got, .. } if got == "number"),
            "got: {err}"
        );
    }

    /// A `<<:` sequence containing a non-mapping item must also error.
    #[test]
    fn sequence_with_non_mapping_merge_item_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("seq-merge.yml");
        fs::write(
            &p,
            "x: &a\n  ka: A\nc:\n  <<:\n    - *a\n    - scalar\n  own: 1\n",
        )
        .unwrap();
        let err = load_template(&p).unwrap_err();
        assert!(
            matches!(&err, TemplateError::Merge { got, .. } if got == "str"),
            "got: {err}"
        );
    }

    /// A `<<:` sequence of mappings stays supported, with earlier entries
    /// taking precedence over later ones and own keys over both.
    #[test]
    fn sequence_of_mappings_merge_earlier_wins() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("seq-ok.yml");
        fs::write(
            &p,
            "a: &a\n  x: 1\n  ka: A\nb: &b\n  x: 2\n  kb: B\nc:\n  <<: [*a, *b]\n  own: 1\n",
        )
        .unwrap();
        let t = load_template(&p).unwrap();
        assert_eq!(
            t["c"],
            serde_json::json!({"x": 1, "ka": "A", "kb": "B", "own": 1}),
            "earlier mapping wins for x; own key kept"
        );
    }

    fn count_merge_keys(v: &Value) -> usize {
        match v {
            Value::Object(o) => o
                .iter()
                .map(|(k, val)| usize::from(k == "<<") + count_merge_keys(val))
                .sum(),
            Value::Array(a) => a.iter().map(count_merge_keys).sum(),
            _ => 0,
        }
    }
}
