use std::path::{Path, PathBuf};

use figment::{
    Error, Metadata, Profile, Provider,
    providers::Format,
    value::{Dict, Map},
};

/// Figment provider that loads a YAML file and resolves `!env VAR_NAME` /
/// `!env "VAR_NAME:default"` YAML tags using environment variables before
/// parsing. Missing variables without a default cause a hard error.
pub struct EnvYaml(PathBuf);

impl EnvYaml {
    /// Create a provider that reads from the given YAML file path.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl Provider for EnvYaml {
    fn metadata(&self) -> Metadata {
        Metadata::named("YAML file with !env tag resolution")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        if !Path::new(&self.0).exists() {
            return figment::providers::Yaml::file(&self.0).data();
        }

        let raw = std::fs::read_to_string(&self.0).map_err(|e| Error::from(e.to_string()))?;

        let mut value: serde_yaml::Value =
            serde_yaml::from_str(&raw).map_err(|e| Error::from(e.to_string()))?;

        resolve_env_tags(&mut value).map_err(Error::from)?;

        let serialized = serde_yaml::to_string(&value).map_err(|e| Error::from(e.to_string()))?;
        figment::providers::Yaml::string(&serialized).data()
    }
}

pub(crate) fn resolve_env_tags(value: &mut serde_yaml::Value) -> Result<(), String> {
    match value {
        serde_yaml::Value::Tagged(tagged) if tagged.tag == "env" => {
            let spec = match &tagged.value {
                serde_yaml::Value::String(s) => s.clone(),
                other => {
                    return Err(format!("!env tag value must be a string, got: {other:?}"));
                }
            };
            let resolved = resolve_env_spec(&spec)?;
            *value = coerce_value(resolved);
        }
        serde_yaml::Value::Mapping(map) => {
            for (_, v) in map.iter_mut() {
                resolve_env_tags(v)?;
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for v in seq.iter_mut() {
                resolve_env_tags(v)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn resolve_env_spec(spec: &str) -> Result<String, String> {
    if let Some((var_name, default)) = spec.split_once(':') {
        Ok(std::env::var(var_name).unwrap_or_else(|_| default.to_string()))
    } else {
        std::env::var(spec).map_err(|_| {
            format!("env var substitution failed: variable '{spec}' is not set and has no default")
        })
    }
}

fn coerce_value(s: String) -> serde_yaml::Value {
    match s.as_str() {
        "true" => serde_yaml::Value::Bool(true),
        "false" => serde_yaml::Value::Bool(false),
        "" => serde_yaml::Value::String(s),
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                serde_yaml::Value::Number(n.into())
            } else if let Ok(f) = s.parse::<f64>() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::String(s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::Figment;

    #[test]
    fn when_yaml_contains_env_tag_then_substituted_from_env() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("TEST_BINARY", "my-agent");
            jail.create_file("test.yaml", "agent:\n  binary: !env TEST_BINARY\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["agent"]["binary"], "my-agent");
            Ok(())
        });
    }

    #[test]
    fn when_env_var_unset_then_uses_default_value_from_yaml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "test.yaml",
                "agent:\n  binary: !env \"NONEXISTENT_VAR:fallback-agent\"\n",
            )?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["agent"]["binary"], "fallback-agent");
            Ok(())
        });
    }

    #[test]
    fn when_yaml_has_no_env_tag_then_value_unchanged() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("test.yaml", "agent:\n  binary: plain-value\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["agent"]["binary"], "plain-value");
            Ok(())
        });
    }

    #[test]
    fn when_yaml_file_does_not_exist_then_produces_empty_data() {
        figment::Jail::expect_with(|_jail| {
            let result: Result<serde_json::Value, _> = Figment::new()
                .merge(EnvYaml::file("nonexistent.yaml"))
                .extract();
            assert!(
                result.is_ok(),
                "missing file should produce empty data, not error"
            );
            Ok(())
        });
    }

    #[test]
    fn when_env_var_missing_and_no_default_then_returns_error() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("test.yaml", "agent:\n  binary: !env ANYCLAW_MISSING_XYZ\n")?;
            let result: Result<serde_json::Value, _> =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract();
            assert!(
                result.is_err(),
                "missing env var without default should fail"
            );
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("ANYCLAW_MISSING_XYZ"),
                "error should name the missing var: {msg}"
            );
            Ok(())
        });
    }

    #[test]
    fn when_env_var_set_with_default_spec_then_env_value_takes_precedence() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("MY_TOKEN", "real-token");
            jail.create_file(
                "test.yaml",
                "channel:\n  token: !env \"MY_TOKEN:fallback\"\n",
            )?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["token"], "real-token");
            Ok(())
        });
    }

    #[test]
    fn when_empty_default_then_resolves_to_empty_string() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("test.yaml", "channel:\n  token: !env \"UNSET_VAR:\"\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["token"], "");
            Ok(())
        });
    }

    #[test]
    fn when_env_resolves_to_false_then_coerced_to_bool() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("MY_ENABLED", "false");
            jail.create_file("test.yaml", "channel:\n  enabled: !env MY_ENABLED\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["enabled"], false);
            Ok(())
        });
    }

    #[test]
    fn when_env_resolves_to_true_then_coerced_to_bool() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("MY_FLAG", "true");
            jail.create_file("test.yaml", "channel:\n  enabled: !env MY_FLAG\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["enabled"], true);
            Ok(())
        });
    }

    #[test]
    fn when_env_resolves_to_number_then_coerced_to_number() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("MY_PORT", "8080");
            jail.create_file("test.yaml", "channel:\n  port: !env MY_PORT\n")?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["port"], 8080);
            Ok(())
        });
    }

    #[test]
    fn when_default_is_false_then_coerced_to_bool() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "test.yaml",
                "channel:\n  enabled: !env \"UNSET_ENABLED:false\"\n",
            )?;
            let value: serde_json::Value =
                Figment::new().merge(EnvYaml::file("test.yaml")).extract()?;
            assert_eq!(value["channel"]["enabled"], false);
            Ok(())
        });
    }
}
