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
}

/// Load products YAML; empty/null → `{}`; non-mapping → [`TemplateError`].
pub fn load_template(path: &Path) -> Result<Value, TemplateError> {
    let path_s = path.display().to_string();
    let text = fs::read_to_string(path).map_err(|source| TemplateError::Io {
        path: path_s.clone(),
        source,
    })?;
    let value: Value = serde_yaml::from_str(&text).map_err(|source| TemplateError::Yaml {
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

fn type_name(v: &Value) -> &'static str {
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
            .join("../../tests/oracle/template/sample.yml");
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
}
