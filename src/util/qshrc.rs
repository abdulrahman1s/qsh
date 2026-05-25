use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct Askrc {
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
    pub provider: Option<String>,
    pub backend: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub prompt: String,
}

pub fn find() -> Option<PathBuf> {
    let mut dir: PathBuf = env::current_dir().ok()?;

    loop {
        let candidate = dir.join(".qshrc");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    let root = Path::new("/.qshrc");
    if root.is_file() {
        return Some(root.to_path_buf());
    }
    None
}

pub fn load(path: &Path) -> Askrc {
    let mut rc = Askrc {
        path: Some(path.to_path_buf()),
        ..Default::default()
    };
    let Ok(content) = fs::read_to_string(path) else {
        return rc;
    };
    let mut in_prompt = false;
    let mut prompt = String::new();
    for line in content.lines() {
        if in_prompt {
            prompt.push_str(line);
            prompt.push('\n');
            continue;
        }
        if line == "---" {
            in_prompt = true;
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key: String = line[..eq].chars().filter(|c| !c.is_whitespace()).collect();
        let val = line[eq + 1..].trim().to_string();
        match key.as_str() {
            "provider" if rc.provider.is_none() => rc.provider = Some(val),
            "backend" if rc.backend.is_none() => rc.backend = Some(val),
            "mode" if rc.mode.is_none() => rc.mode = Some(val),
            "model" if rc.model.is_none() => rc.model = Some(val),
            _ => {}
        }
    }
    // Trim trailing newline accumulated by the loop.
    while prompt.ends_with('\n') {
        prompt.pop();
    }
    rc.prompt = prompt;
    rc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn basic_kv_and_prompt() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "provider=openai").unwrap();
        writeln!(f, "backend = cli").unwrap();
        writeln!(f, "mode = smart").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "extra rules").unwrap();
        writeln!(f, "more rules").unwrap();
        let rc = load(f.path());
        assert_eq!(rc.provider.as_deref(), Some("openai"));
        assert_eq!(rc.backend.as_deref(), Some("cli"));
        assert_eq!(rc.mode.as_deref(), Some("smart"));
        assert!(rc.model.is_none());
        assert_eq!(rc.prompt, "extra rules\nmore rules");
    }

    #[test]
    fn empty_lines_and_comments_skipped() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f).unwrap();
        writeln!(f, "  # nope").unwrap();
        writeln!(f, "model=foo").unwrap();
        let rc = load(f.path());
        assert_eq!(rc.model.as_deref(), Some("foo"));
    }
}
