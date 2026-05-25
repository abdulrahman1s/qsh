use crate::config::Mode;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILENAME: &str = "qsh.toml";

#[derive(Debug, Default, Clone)]
pub struct ProjectConfig {
    #[allow(dead_code)]
    pub path: Option<PathBuf>,
    pub provider: Option<String>,
    pub backend: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub model_fast: Option<String>,
    pub model_smart: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    provider: Option<String>,
    backend: Option<String>,
    mode: Option<String>,
    model: Option<String>,
    models: Option<RawModels>,
    prompt: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawModels {
    fast: Option<String>,
    smart: Option<String>,
}

pub fn find() -> Option<PathBuf> {
    let mut dir: PathBuf = env::current_dir().ok()?;

    loop {
        let candidate = dir.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    let root = PathBuf::from("/").join(CONFIG_FILENAME);
    if root.is_file() {
        return Some(root);
    }
    None
}

pub fn load(path: &Path) -> ProjectConfig {
    let mut cfg = ProjectConfig {
        path: Some(path.to_path_buf()),
        ..Default::default()
    };
    let Ok(content) = fs::read_to_string(path) else {
        return cfg;
    };
    let raw: RawConfig = match toml::from_str(&content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("qsh: {} parse error: {}", path.display(), e);
            return cfg;
        }
    };
    cfg.provider = raw.provider.filter(|s| !s.is_empty());
    cfg.backend = raw.backend.filter(|s| !s.is_empty());
    cfg.mode = raw.mode.filter(|s| !s.is_empty());
    cfg.model = raw.model.filter(|s| !s.is_empty());
    if let Some(m) = raw.models {
        cfg.model_fast = m.fast.filter(|s| !s.is_empty());
        cfg.model_smart = m.smart.filter(|s| !s.is_empty());
    }
    cfg.prompt = raw
        .prompt
        .map(|s| s.trim_end().to_string())
        .unwrap_or_default();
    cfg
}

impl ProjectConfig {
    pub fn model_for(&self, mode: Mode) -> Option<&str> {
        let per_mode = match mode {
            Mode::Fast => self.model_fast.as_deref(),
            Mode::Smart => self.model_smart.as_deref(),
        };
        per_mode.or(self.model.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(s: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(s.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_full_toml() {
        // Note: `prompt` (and every other top-level key) must precede any
        // `[table]` header, otherwise TOML treats it as nested under that
        // table. We document this in the schema, and `[models]` is the only
        // sub-table users typically need.
        let f = write_toml(
            r#"
provider = "openai"
backend  = "cli"
mode     = "smart"
model    = "gpt-default"
prompt = """
extra rules
more rules
"""

[models]
fast  = "gpt-mini"
smart = "gpt-pro"
"#,
        );
        let cfg = load(f.path());
        assert_eq!(cfg.provider.as_deref(), Some("openai"));
        assert_eq!(cfg.backend.as_deref(), Some("cli"));
        assert_eq!(cfg.mode.as_deref(), Some("smart"));
        assert_eq!(cfg.model.as_deref(), Some("gpt-default"));
        assert_eq!(cfg.model_fast.as_deref(), Some("gpt-mini"));
        assert_eq!(cfg.model_smart.as_deref(), Some("gpt-pro"));
        assert_eq!(cfg.prompt, "extra rules\nmore rules");
        assert_eq!(cfg.model_for(Mode::Fast), Some("gpt-mini"));
        assert_eq!(cfg.model_for(Mode::Smart), Some("gpt-pro"));
    }

    #[test]
    fn model_for_falls_back_to_default() {
        let f = write_toml(
            r#"
model = "only-this"
"#,
        );
        let cfg = load(f.path());
        assert_eq!(cfg.model_for(Mode::Fast), Some("only-this"));
        assert_eq!(cfg.model_for(Mode::Smart), Some("only-this"));
    }

    #[test]
    fn empty_strings_become_none() {
        let f = write_toml(
            r#"
provider = ""
model = ""
[models]
fast = ""
"#,
        );
        let cfg = load(f.path());
        assert!(cfg.provider.is_none());
        assert!(cfg.model.is_none());
        assert!(cfg.model_fast.is_none());
    }

    #[test]
    fn missing_file_returns_default() {
        let cfg = load(Path::new("/nonexistent/qsh.toml"));
        assert!(cfg.provider.is_none());
        assert!(cfg.prompt.is_empty());
    }

    #[test]
    fn comments_and_blank_lines_ok() {
        let f = write_toml(
            r#"
# this is a comment
mode = "fast"

# another
"#,
        );
        let cfg = load(f.path());
        assert_eq!(cfg.mode.as_deref(), Some("fast"));
    }
}
