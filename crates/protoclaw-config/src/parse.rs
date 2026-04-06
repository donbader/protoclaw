use crate::ConfigError;

/// Parses K8s-style memory string to bytes.
/// Accepts: "256k", "512m", "1g", "2g" (case-insensitive).
/// Returns: i64 (bytes) for bollard's HostConfig.memory
pub fn parse_memory_limit(s: &str) -> Result<i64, ConfigError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ConfigError::InvalidMemoryLimit {
            value: s.to_string(),
            reason: "empty string".to_string(),
        });
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let multiplier: i64 = match unit.to_ascii_lowercase().as_str() {
        "k" => 1024,
        "m" => 1024 * 1024,
        "g" => 1024 * 1024 * 1024,
        _ => {
            return Err(ConfigError::InvalidMemoryLimit {
                value: s.to_string(),
                reason: format!("unknown unit suffix '{unit}', expected k, m, or g"),
            });
        }
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| ConfigError::InvalidMemoryLimit {
            value: s.to_string(),
            reason: format!("'{num_str}' is not a valid number"),
        })?;

    if num <= 0.0 {
        return Err(ConfigError::InvalidMemoryLimit {
            value: s.to_string(),
            reason: "value must be positive".to_string(),
        });
    }

    Ok((num * multiplier as f64) as i64)
}

/// Parses K8s-style CPU string to Docker nanocpus.
/// Accepts: "0.5" (cores), "500m" (millicores), "2" (cores).
/// Returns: i64 (nanocpus) for bollard's HostConfig.nano_cpus
pub fn parse_cpu_limit(s: &str) -> Result<i64, ConfigError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ConfigError::InvalidCpuLimit {
            value: s.to_string(),
            reason: "empty string".to_string(),
        });
    }

    const NANOCPUS_PER_CORE: i64 = 1_000_000_000;

    if let Some(millis_str) = s.strip_suffix('m').or_else(|| s.strip_suffix('M')) {
        let millis: f64 = millis_str
            .parse()
            .map_err(|_| ConfigError::InvalidCpuLimit {
                value: s.to_string(),
                reason: format!("'{millis_str}' is not a valid number"),
            })?;
        if millis <= 0.0 {
            return Err(ConfigError::InvalidCpuLimit {
                value: s.to_string(),
                reason: "value must be positive".to_string(),
            });
        }
        Ok((millis * 1_000_000.0) as i64)
    } else {
        let cores: f64 = s.parse().map_err(|_| ConfigError::InvalidCpuLimit {
            value: s.to_string(),
            reason: format!(
                "'{s}' is not a valid number or millicore value (e.g. '1.5' or '500m')"
            ),
        })?;
        if cores <= 0.0 {
            return Err(ConfigError::InvalidCpuLimit {
                value: s.to_string(),
                reason: "value must be positive".to_string(),
            });
        }
        Ok((cores * NANOCPUS_PER_CORE as f64) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_memory_limit_has_k_suffix_then_converts_to_bytes() {
        assert_eq!(parse_memory_limit("256k").unwrap(), 262_144);
        assert_eq!(parse_memory_limit("256K").unwrap(), 262_144);
    }

    #[test]
    fn when_memory_limit_has_m_suffix_then_converts_to_bytes() {
        assert_eq!(parse_memory_limit("512m").unwrap(), 536_870_912);
        assert_eq!(parse_memory_limit("512M").unwrap(), 536_870_912);
    }

    #[test]
    fn when_memory_limit_has_g_suffix_then_converts_to_bytes() {
        assert_eq!(parse_memory_limit("1g").unwrap(), 1_073_741_824);
        assert_eq!(parse_memory_limit("2G").unwrap(), 2_147_483_648);
    }

    #[test]
    fn when_memory_limit_has_no_unit_suffix_then_returns_error() {
        assert!(parse_memory_limit("1024").is_err());
    }

    #[test]
    fn when_memory_limit_is_empty_string_then_returns_error() {
        assert!(parse_memory_limit("").is_err());
    }

    #[test]
    fn when_memory_limit_is_only_unit_suffix_then_returns_error() {
        assert!(parse_memory_limit("m").is_err());
        assert!(parse_memory_limit("g").is_err());
    }

    #[test]
    fn when_memory_limit_has_unknown_unit_suffix_then_returns_error() {
        assert!(parse_memory_limit("512x").is_err());
    }

    #[test]
    fn when_cpu_limit_is_decimal_cores_then_converts_to_nanocpus() {
        assert_eq!(parse_cpu_limit("0.5").unwrap(), 500_000_000);
        assert_eq!(parse_cpu_limit("1.5").unwrap(), 1_500_000_000);
    }

    #[test]
    fn when_cpu_limit_is_whole_cores_then_converts_to_nanocpus() {
        assert_eq!(parse_cpu_limit("1").unwrap(), 1_000_000_000);
        assert_eq!(parse_cpu_limit("2").unwrap(), 2_000_000_000);
    }

    #[test]
    fn when_cpu_limit_has_m_suffix_then_converts_millicores_to_nanocpus() {
        assert_eq!(parse_cpu_limit("500m").unwrap(), 500_000_000);
        assert_eq!(parse_cpu_limit("1500m").unwrap(), 1_500_000_000);
        assert_eq!(parse_cpu_limit("250m").unwrap(), 250_000_000);
    }

    #[test]
    fn when_cpu_limit_is_empty_string_then_returns_error() {
        assert!(parse_cpu_limit("").is_err());
    }

    #[test]
    fn when_cpu_limit_is_only_m_suffix_then_returns_error() {
        assert!(parse_cpu_limit("m").is_err());
    }

    #[test]
    fn when_cpu_limit_has_unknown_unit_suffix_then_returns_error() {
        assert!(parse_cpu_limit("500x").is_err());
    }
}
