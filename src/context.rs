use std::path::Path;
use std::process::Command;

fn git_branch() -> Option<String> {
    let inside = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()?;
    if !inside.status.success() {
        return None;
    }
    let out = Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    let out = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn exists_any<P: AsRef<Path>>(paths: &[P]) -> bool {
    paths.iter().any(|p| p.as_ref().exists())
}

pub fn cwd_context() -> String {
    let mut hints: Vec<String> = Vec::new();

    if let Some(branch) = git_branch() {
        hints.push(format!("git {}", branch));
    }

    let mut langs = Vec::new();
    if Path::new("Cargo.toml").exists() {
        langs.push("rust");
    }
    if Path::new("package.json").exists() {
        langs.push("node");
    }
    if exists_any(&["pyproject.toml", "setup.py", "requirements.txt"]) {
        langs.push("python");
    }
    if Path::new("go.mod").exists() {
        langs.push("go");
    }
    if Path::new("Gemfile").exists() {
        langs.push("ruby");
    }
    if exists_any(&["pom.xml", "build.gradle", "build.gradle.kts"]) {
        langs.push("jvm");
    }
    if Path::new("composer.json").exists() {
        langs.push("php");
    }
    if Path::new("mix.exs").exists() {
        langs.push("elixir");
    }
    if exists_any(&["deno.json", "deno.jsonc"]) {
        langs.push("deno");
    }
    if !langs.is_empty() {
        hints.push(format!("lang {}", langs.join(",")));
    }

    let mut tools = Vec::new();
    if exists_any(&["flake.nix", "shell.nix", "default.nix"]) {
        tools.push("nix");
    }
    if exists_any(&["Makefile", "makefile"]) {
        tools.push("make");
    }
    if exists_any(&["justfile", "Justfile"]) {
        tools.push("just");
    }
    if exists_any(&["docker-compose.yml", "docker-compose.yaml", "compose.yaml"]) {
        tools.push("compose");
    }
    if Path::new("Dockerfile").exists() {
        tools.push("docker");
    }
    if Path::new(".github/workflows").is_dir() {
        tools.push("gh-actions");
    }
    if !tools.is_empty() {
        hints.push(format!("tools {}", tools.join(",")));
    }

    hints.join(" | ")
}
