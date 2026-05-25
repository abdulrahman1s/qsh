use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg).join("qsh");
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".cache").join("qsh")
}

pub fn key(
    provider: &str,
    backend: &str,
    model: &str,
    mode: &str,
    system: &str,
    task: &str,
) -> String {
    let task = task
        .trim_start_matches(|c: char| c.is_whitespace())
        .trim_end_matches(|c: char| c.is_whitespace());
    let mut hasher = Sha256::new();
    hasher.update(provider.as_bytes());
    hasher.update(b"\n");
    hasher.update(backend.as_bytes());
    hasher.update(b"\n");
    hasher.update(model.as_bytes());
    hasher.update(b"\n");
    hasher.update(mode.as_bytes());
    hasher.update(b"\n");
    hasher.update(system.as_bytes());
    hasher.update(b"\n");
    hasher.update(task.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn file_for(dir: &Path, key: &str) -> PathBuf {
    dir.join(key)
}

pub fn load(file: &Path) -> Option<String> {
    fs::read_to_string(file).ok()
}

pub fn save(dir: &Path, file: &Path, content: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    save_atomic(file, content.as_bytes())
}

pub fn save_atomic(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content)?;
    tmp.flush()?;
    tmp.persist(target).map_err(|e| e.error)?;
    Ok(())
}

pub fn clear() {
    let _ = fs::remove_dir_all(cache_dir());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_stable_with_whitespace() {
        let a = key("openai", "api", "gpt", "fast", "sys", "  foo  ");
        let b = key("openai", "api", "gpt", "fast", "sys", "foo");
        assert_eq!(a, b);
    }

    #[test]
    fn key_differs_on_provider() {
        let a = key("openai", "api", "x", "fast", "sys", "task");
        let b = key("gemini", "api", "x", "fast", "sys", "task");
        assert_ne!(a, b);
    }

    #[test]
    fn key_differs_on_backend() {
        let a = key("openai", "api", "x", "fast", "sys", "task");
        let b = key("openai", "cli", "x", "fast", "sys", "task");
        assert_ne!(a, b);
    }
}
