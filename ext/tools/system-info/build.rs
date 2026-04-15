use std::process::Command;

fn main() {
    // CI/Docker builds can set ANYCLAW_VERSION explicitly
    println!("cargo::rerun-if-env-changed=ANYCLAW_VERSION");

    let version = std::env::var("ANYCLAW_VERSION")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(git_describe)
        .unwrap_or_else(|| "unknown".into());

    println!("cargo::rustc-env=ANYCLAW_VERSION={version}");
}

fn git_describe() -> Option<String> {
    // Rerun when HEAD changes (branch switch, new commit, tag)
    if let Ok(git_dir) = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
    {
        let dir = String::from_utf8_lossy(&git_dir.stdout).trim().to_string();
        println!("cargo::rerun-if-changed={dir}/HEAD");
        // Also track the ref HEAD points to (e.g. refs/heads/main)
        if let Ok(head) = std::fs::read_to_string(format!("{dir}/HEAD")) {
            if let Some(ref_path) = head.strip_prefix("ref: ") {
                println!("cargo::rerun-if-changed={dir}/{}", ref_path.trim());
            }
        }
    }

    Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty", "--match", "v*"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}
