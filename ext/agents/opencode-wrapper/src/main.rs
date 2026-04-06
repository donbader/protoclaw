use std::collections::HashMap;
use std::process::ExitCode;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

fn build_child_env(explicit_env: &HashMap<String, String>) -> HashMap<String, String> {
    explicit_env.clone()
}

fn parse_args(args: &[String]) -> (String, Vec<String>, Option<String>) {
    let mut binary = "opencode".to_string();
    let mut opencode_config: Option<String> = None;
    let mut extra_args: Vec<String> = vec!["acp".to_string()];
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--opencode-binary" => {
                if i + 1 < args.len() {
                    binary = args[i + 1].clone();
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--opencode-config" => {
                if i + 1 < args.len() {
                    opencode_config = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--" => {
                extra_args = args[i + 1..].to_vec();
                break;
            }
            _ => {
                i += 1;
            }
        }
    }

    (binary, extra_args, opencode_config)
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let (opencode_binary, opencode_args, opencode_config) = parse_args(&args);

    let mut child_env: HashMap<String, String> = std::env::vars().collect();

    if let Some(ref config_path) = opencode_config {
        child_env.insert("OPENCODE_CONFIG".to_string(), config_path.clone());
    }

    let child_env = build_child_env(&child_env);

    let mut child = match Command::new(&opencode_binary)
        .args(&opencode_args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env_clear()
        .envs(&child_env)
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tracing::error!(binary = %opencode_binary, error = %e, "failed to spawn opencode");
            return ExitCode::from(127);
        }
    };

    let child_stdin = child.stdin.take().expect("stdin was piped");
    let child_stdout = child.stdout.take().expect("stdout was piped");
    let child_stderr = child.stderr.take().expect("stderr was piped");

    let stdin_proxy = tokio::spawn(async move {
        let mut wrapper_stdin = BufReader::new(tokio::io::stdin()).lines();
        let mut child_stdin = child_stdin;
        while let Ok(Some(line)) = wrapper_stdin.next_line().await {
            let mut buf = line.into_bytes();
            buf.push(b'\n');
            if child_stdin.write_all(&buf).await.is_err() {
                break;
            }
            if child_stdin.flush().await.is_err() {
                break;
            }
        }
    });

    let stdout_proxy = tokio::spawn(async move {
        let mut child_lines = BufReader::new(child_stdout).lines();
        let mut wrapper_stdout = tokio::io::stdout();
        while let Ok(Some(line)) = child_lines.next_line().await {
            let mut buf = line.into_bytes();
            buf.push(b'\n');
            if wrapper_stdout.write_all(&buf).await.is_err() {
                break;
            }
            if wrapper_stdout.flush().await.is_err() {
                break;
            }
        }
    });

    let stderr_proxy = tokio::spawn(async move {
        let mut child_err_lines = BufReader::new(child_stderr).lines();
        let mut wrapper_stderr = tokio::io::stderr();
        while let Ok(Some(line)) = child_err_lines.next_line().await {
            tracing::debug!(target: "opencode_stderr", "{}", line);
            let mut buf = line.into_bytes();
            buf.push(b'\n');
            let _ = wrapper_stderr.write_all(&buf).await;
            let _ = wrapper_stderr.flush().await;
        }
    });

    let status = child.wait().await;

    stdin_proxy.abort();
    stdout_proxy.abort();
    stderr_proxy.abort();

    match status {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            tracing::error!(error = %e, "failed to wait for opencode process");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_no_args_then_defaults_to_opencode_acp() {
        let args = vec!["opencode-wrapper".to_string()];
        let (binary, child_args, config) = parse_args(&args);
        assert_eq!(binary, "opencode");
        assert_eq!(child_args, vec!["acp"]);
        assert!(config.is_none());
    }

    #[rstest]
    fn when_opencode_binary_flag_then_uses_custom_binary() {
        let args = vec![
            "opencode-wrapper".to_string(),
            "--opencode-binary".to_string(),
            "/usr/bin/opencode-custom".to_string(),
        ];
        let (binary, _, _) = parse_args(&args);
        assert_eq!(binary, "/usr/bin/opencode-custom");
    }

    #[rstest]
    fn when_opencode_config_flag_then_returns_config_path() {
        let args = vec![
            "opencode-wrapper".to_string(),
            "--opencode-config".to_string(),
            "/etc/opencode/config.json".to_string(),
        ];
        let (_, _, config) = parse_args(&args);
        assert_eq!(config, Some("/etc/opencode/config.json".to_string()));
    }

    #[rstest]
    fn when_dashdash_then_remaining_args_passed_to_child() {
        let args = vec![
            "opencode-wrapper".to_string(),
            "--".to_string(),
            "acp".to_string(),
            "--verbose".to_string(),
        ];
        let (_, child_args, _) = parse_args(&args);
        assert_eq!(child_args, vec!["acp", "--verbose"]);
    }

    #[rstest]
    fn when_all_flags_combined_then_all_parsed_correctly() {
        let args = vec![
            "opencode-wrapper".to_string(),
            "--opencode-binary".to_string(),
            "my-opencode".to_string(),
            "--opencode-config".to_string(),
            "/path/to/config.json".to_string(),
            "--".to_string(),
            "acp".to_string(),
            "--debug".to_string(),
        ];
        let (binary, child_args, config) = parse_args(&args);
        assert_eq!(binary, "my-opencode");
        assert_eq!(child_args, vec!["acp", "--debug"]);
        assert_eq!(config, Some("/path/to/config.json".to_string()));
    }

    #[rstest]
    fn when_build_child_env_with_explicit_vars_then_only_those_returned() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "sk-test".to_string());
        env.insert("MODEL".to_string(), "claude".to_string());
        let result = build_child_env(&env);
        assert_eq!(result.len(), 2);
        assert_eq!(result["API_KEY"], "sk-test");
        assert_eq!(result["MODEL"], "claude");
    }

    #[rstest]
    fn when_build_child_env_with_empty_map_then_returns_empty() {
        let env = HashMap::new();
        let result = build_child_env(&env);
        assert!(result.is_empty());
    }
}
