use std::path::{Path, PathBuf};

use figment::{
    Error, Metadata, Profile, Provider,
    providers::Format,
    value::{Dict, Map},
};

pub struct SubstYaml(PathBuf);

impl SubstYaml {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl Provider for SubstYaml {
    fn metadata(&self) -> Metadata {
        Metadata::named("YAML file with env interpolation")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        if !Path::new(&self.0).exists() {
            return figment::providers::Yaml::file(&self.0).data();
        }

        let raw = std::fs::read_to_string(&self.0).map_err(|e| Error::from(e.to_string()))?;

        let substituted = subst::substitute(&raw, &subst::Env)
            .map_err(|e| Error::from(format!("env var substitution failed: {e}")))?;

        let mut value: serde_yaml::Value =
            serde_yaml::from_str(&substituted).map_err(|e| Error::from(e.to_string()))?;

        coerce_substituted_strings(&mut value);

        let serialized = serde_yaml::to_string(&value).map_err(|e| Error::from(e.to_string()))?;
        figment::providers::Yaml::string(&serialized).data()
    }
}

fn coerce_substituted_strings(value: &mut serde_yaml::Value) {
    match value {
        serde_yaml::Value::String(s) => {
            if s == "true" {
                *value = serde_yaml::Value::Bool(true);
            } else if s == "false" {
                *value = serde_yaml::Value::Bool(false);
            } else if let Ok(n) = s.parse::<i64>() {
                *value = serde_yaml::Value::Number(n.into());
            } else if s.contains('.')
                && let Ok(f) = s.parse::<f64>()
            {
                *value = serde_yaml::Value::Number(f.into());
            }
        }
        serde_yaml::Value::Sequence(arr) => {
            for v in arr {
                coerce_substituted_strings(v);
            }
        }
        serde_yaml::Value::Mapping(map) => {
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
    fn when_yaml_contains_env_var_reference_then_substituted_from_env() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("TEST_BINARY", "my-agent");
            jail.create_file("test.yaml", "agent:\n  binary: \"${TEST_BINARY}\"\n")?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstYaml::file("test.yaml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "my-agent");
            Ok(())
        });
    }

    #[test]
    fn when_env_var_unset_then_uses_default_value_from_yaml() {
        figment::Jail::expect_with(|jail| {
            jail.create_file(
                "test.yaml",
                "agent:\n  binary: \"${NONEXISTENT_VAR:fallback-agent}\"\n",
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstYaml::file("test.yaml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "fallback-agent");
            Ok(())
        });
    }

    #[test]
    fn when_env_var_is_true_string_then_coerced_to_boolean() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("TEST_CHANNEL_ENABLED", "true");
            jail.create_file(
                "test.yaml",
                "channels:\n  - name: telegram\n    binary: telegram-channel\n    enabled: \"${TEST_CHANNEL_ENABLED:false}\"\n",
            )?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstYaml::file("test.yaml"))
                .extract()?;
            assert_eq!(value["channels"][0]["enabled"], true);
            Ok(())
        });
    }

    #[test]
    fn when_yaml_has_no_env_var_reference_then_value_unchanged() {
        figment::Jail::expect_with(|jail| {
            jail.create_file("test.yaml", "agent:\n  binary: plain-value\n")?;
            let value: serde_json::Value = Figment::new()
                .merge(SubstYaml::file("test.yaml"))
                .extract()?;
            assert_eq!(value["agent"]["binary"], "plain-value");
            Ok(())
        });
    }

    #[test]
    fn when_yaml_file_does_not_exist_then_produces_empty_data() {
        figment::Jail::expect_with(|_jail| {
            let result: Result<serde_json::Value, _> = Figment::new()
                .merge(SubstYaml::file("nonexistent.yaml"))
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
            jail.create_file(
                "test.yaml",
                "agent:\n  binary: \"${PROTOCLAW_MISSING_XYZ}\"\n",
            )?;
            let result: Result<serde_json::Value, _> =
                Figment::new().merge(SubstYaml::file("test.yaml")).extract();
            assert!(
                result.is_err(),
                "missing env var without default should fail"
            );
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("PROTOCLAW_MISSING_XYZ"),
                "error should name the missing var: {msg}"
            );
            Ok(())
        });
    }
}
