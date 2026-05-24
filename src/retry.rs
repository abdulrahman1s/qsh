use crate::cache;
use crate::config::{ATTEMPTS_KEEP, RETRY_WINDOW_MIN, STDERR_CAP};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attempt {
    pub cmd: String,
    pub stderr: String,
}

pub fn attempts_file(cache_dir: &Path) -> PathBuf {
    cache_dir.join(".last_attempts.jsonl")
}

pub fn last_task_file(cache_dir: &Path) -> PathBuf {
    cache_dir.join(".last_task")
}

pub fn recent(cache_dir: &Path) -> bool {
    let f = attempts_file(cache_dir);
    let Ok(meta) = fs::metadata(&f) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = SystemTime::now().duration_since(mtime) else {
        return false;
    };
    elapsed <= Duration::from_secs(RETRY_WINDOW_MIN * 60)
}

pub fn load_attempts(cache_dir: &Path) -> Vec<Attempt> {
    let f = attempts_file(cache_dir);
    let Ok(text) = fs::read_to_string(&f) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

pub fn load_last_task(cache_dir: &Path) -> Option<String> {
    fs::read_to_string(last_task_file(cache_dir)).ok()
}

pub fn format_attempts_for_prompt(attempts: &[Attempt]) -> String {
    attempts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                "attempt {}:\n  command: {}\n  stderr: {}",
                i + 1,
                a.cmd,
                a.stderr
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn record(
    cache_dir: &Path,
    cmd: &str,
    stderr_file: Option<&Path>,
    rc: i32,
    original_task: &str,
) -> std::io::Result<()> {
    fs::create_dir_all(cache_dir)?;
    if rc != 0 {
        let stderr_text = stderr_file
            .and_then(|p| fs::read(p).ok())
            .map(|b| {
                let start = b.len().saturating_sub(STDERR_CAP);
                String::from_utf8_lossy(&b[start..]).to_string()
            })
            .unwrap_or_default();
        let entry = Attempt {
            cmd: cmd.to_string(),
            stderr: stderr_text,
        };
        let mut keep: Vec<Attempt> = load_attempts(cache_dir);
        let want_drop = (keep.len() + 1).saturating_sub(ATTEMPTS_KEEP);
        if want_drop > 0 {
            keep.drain(..want_drop);
        }
        keep.push(entry);
        let mut text = String::new();
        for a in &keep {
            text.push_str(&serde_json::to_string(a).unwrap_or_default());
            text.push('\n');
        }
        cache::save_atomic(&attempts_file(cache_dir), text.as_bytes())?;
        cache::save_atomic(&last_task_file(cache_dir), original_task.as_bytes())?;
    } else {
        let _ = fs::remove_file(attempts_file(cache_dir));
        let _ = fs::remove_file(last_task_file(cache_dir));
        let _ = fs::remove_file(cache_dir.join(".last_cmd"));
        let _ = fs::remove_file(cache_dir.join(".last_stderr"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn record_failure_then_success_clears() {
        let dir = TempDir::new().unwrap();
        record(dir.path(), "cmd1", None, 1, "intent").unwrap();
        assert!(attempts_file(dir.path()).exists());
        assert!(last_task_file(dir.path()).exists());
        record(dir.path(), "cmd2", None, 0, "intent").unwrap();
        assert!(!attempts_file(dir.path()).exists());
        assert!(!last_task_file(dir.path()).exists());
    }

    #[test]
    fn record_trims_to_keep_window() {
        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            record(dir.path(), &format!("cmd{i}"), None, 1, "intent").unwrap();
        }
        let attempts = load_attempts(dir.path());
        assert_eq!(attempts.len(), ATTEMPTS_KEEP);
        assert_eq!(attempts.last().unwrap().cmd, "cmd4");
    }
}
