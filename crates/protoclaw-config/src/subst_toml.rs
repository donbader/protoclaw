use std::path::{Path, PathBuf};

use figment::{
    providers::Format,
    value::{Dict, Map},
    Error, Metadata, Profile, Provider,
};

pub struct SubstToml(PathBuf);

impl SubstToml {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl Provider for SubstToml {
    fn metadata(&self) -> Metadata {
        Metadata::named("TOML file with env interpolation")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        if !Path::new(&self.0).exists() {
            return figment::providers::Toml::file(&self.0).data();
        }

        let raw = std::fs::read_to_string(&self.0).map_err(|e| Error::from(e.to_string()))?;
        let mut value: toml::Value =
            toml::from_str(&raw).map_err(|e| Error::from(e.to_string()))?;

        if let Err(e) = subst::toml::substitute_string_values(&mut value, &subst::Env) {
            tracing::warn!(error = %e, "env var interpolation failed, using raw values");
        }

        coerce_substituted_strings(&mut value);

        let serialized = toml::to_string(&value).map_err(|e| Error::from(e.to_string()))?;
        figment::providers::Toml::string(&serialized).data()
    }
}

fn coerce_substituted_strings(value: &mut toml::Value) {
    match value {
        toml::Value::String(s) => {
            if s == "true" {
                *value = toml::Value::Boolean(true);
            } else if s == "false" {
                *value = toml::Value::Boolean(false);
            } else if let Ok(n) = s.parse::<i64>() {
                *value = toml::Value::Integer(n);
            } else if let Ok(f) = s.parse::<f64>() {
                if s.contains('.') {
                    *value = toml::Value::Float(f);
                }
            }
        }
        toml::Value::Array(arr) => {
            for v in arr {
                coerce_substituted_strings(v);
            }
        }
        toml::Value::Table(map) => {
            for (_, v) in map.iter_mut() {
                coerce_substituted_strings(v);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::Figment;

    #[test]
    fn substitutes_env_var_in_string_value() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("TEST_BINARY", "my-agent");
            jail.create_file(
                "test.toml",
                r#"
                [agent]
                binary = "${TEST_BINARY}"
            "#,
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstToml::file("test.toml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "my-agent");
            Ok(())
        });
    }

    #[test]
    fn substitutes_with_default_value() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "test.toml",
                r#"
                [agent]
                binary = "${NONEXISTENT_VAR:fallback-agent}"
            "#,
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstToml::file("test.toml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "fallback-agent");
            Ok(())
        });
    }

    #[test]
    fn substitutes_in_array_elements() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("TEST_CHANNEL_ENABLED", "true");
            jail.create_file(
                "test.toml",
                r#"
                [[channels]]
                name = "telegram"
                binary = "telegram-channel"
                enabled = "${TEST_CHANNEL_ENABLED:false}"
            "#,
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstToml::file("test.toml"))
                .extract()?;
            assert_eq!(value["channels"][0]["enabled"], true);
            Ok(())
        });
    }

    #[test]
    fn passes_through_non_interpolated_values() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "test.toml",
                r#"
                [agent]
                binary = "plain-value"
            "#,
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstToml::file("test.toml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "plain-value");
            Ok(())
        });
    }

    #[test]
    fn missing_file_falls_through_to_figment() {
        figment::Jail::expect_with(|_jail| {
            let result: Result<serde_json::Value, _> = Figment::new()
                .merge(SubstToml::file("nonexistent.toml"))
                .extract();
            assert!(
                result.is_ok(),
                "missing file should produce empty data, not error"
            );
            Ok(())
        });
    }
}
