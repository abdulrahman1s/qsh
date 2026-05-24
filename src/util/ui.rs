use crate::config::{SPINNER_FRAMES, SPINNER_SLEEP_MS, TYPEWRITER_CHAR_MS};
use std::io::{IsTerminal, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::watch;

pub fn die(msg: &str) {
    eprintln!("\x1b[1;31mqsh:\x1b[0m {}", msg);
}
pub fn warn(msg: &str) {
    eprintln!("\x1b[1;31mqsh:\x1b[0m {}", msg);
}
pub fn info(msg: &str) {
    eprintln!("\x1b[2m▸ {}\x1b[0m", msg);
}
pub fn net_die(msg: &str) {
    eprintln!("\x1b[1;31mqsh:\x1b[0m network: {}", msg);
}
pub fn api_die(msg: &str) {
    eprintln!("\x1b[1;31mqsh:\x1b[0m api: {}", msg);
}
pub fn parse_die(msg: &str) {
    eprintln!("\x1b[1;31mqsh:\x1b[0m parse: {}", msg);
}

pub fn status_line(provider: &str, model: &str, mode: &str) {
    eprintln!("\x1b[2m{} ▸ {} ▸ {}\x1b[0m", provider, model, mode);
}

pub fn status_line_alts(provider: &str, model: &str, mode: &str, alts: u32) {
    eprintln!(
        "\x1b[2m{} ▸ {} ▸ {} ▸ {} alts\x1b[0m",
        provider, model, mode, alts
    );
}

pub fn status_line_cached(provider: &str, model: &str, mode: &str) {
    eprintln!("\x1b[2m{} ▸ {} ▸ {} ▸ cached\x1b[0m", provider, model, mode);
}

pub fn print_command(cmd: &str) {
    eprintln!("\x1b[1;33m{}\x1b[0m", cmd);
}

pub fn retry_indicator(s: &str) {
    eprintln!("\x1b[2mretrying: {}\x1b[0m", s);
}

/// Wait for either the first byte in `buf` (single mode) or for the
/// pipeline to finish counting sentinels (alts mode). Returns nothing,
/// just renders a spinner to stderr. Honors `cancelled`.
pub async fn spinner_wait(
    buf: Arc<Mutex<String>>,
    join: &mut tokio::task::JoinHandle<crate::providers::stream::StreamResult>,
    alts: u32,
    retry: bool,
    refine: bool,
    cancelled: Arc<AtomicBool>,
) {
    let alts_mode = alts > 1 && !retry && !refine;
    let mut fi = 0usize;
    let frames: Vec<char> = SPINNER_FRAMES.chars().collect();
    let mut interval = tokio::time::interval(Duration::from_millis(SPINNER_SLEEP_MS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    if !std::io::stderr().is_terminal() {
        // Non-terminal stderr: don't draw spinner; just wait for first byte (single) or completion (alts).
        loop {
            if cancelled.load(Ordering::Relaxed) || join.is_finished() {
                break;
            }
            if !alts_mode && !buf.lock().await.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(SPINNER_SLEEP_MS)).await;
        }
        return;
    }

    loop {
        if cancelled.load(Ordering::Relaxed) {
            eprint!("\r\x1b[K");
            let _ = std::io::stderr().flush();
            return;
        }
        if join.is_finished() {
            eprint!("\r\x1b[K");
            let _ = std::io::stderr().flush();
            return;
        }
        if alts_mode {
            let text = buf.lock().await.clone();
            let seen = count_sentinels(&text);
            eprint!(
                "\r\x1b[1;36m{}\x1b[0m generating alternatives… ({}/{})",
                frames[fi % frames.len()],
                seen,
                alts
            );
        } else {
            let has_text = !buf.lock().await.is_empty();
            if has_text {
                eprint!("\r\x1b[K");
                let _ = std::io::stderr().flush();
                return;
            }
            eprint!("\r\x1b[1;36m{}\x1b[0m thinking…", frames[fi % frames.len()]);
        }
        let _ = std::io::stderr().flush();
        fi = fi.wrapping_add(1);
        interval.tick().await;
    }
}

fn count_sentinels(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            let s = l.trim();
            let mut toks = s.split_whitespace();
            matches!(
                (toks.next(), toks.next(), toks.next()),
                (Some("==="), Some("alt"), Some(n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
            ) && toks.next() == Some("===")
        })
        .count()
}

pub async fn typewriter(
    buf: Arc<Mutex<String>>,
    join: &mut tokio::task::JoinHandle<crate::providers::stream::StreamResult>,
    cancelled: Arc<AtomicBool>,
    _cancel_rx: watch::Receiver<bool>,
) {
    if !std::io::stderr().is_terminal() {
        // Non-TTY: just wait for completion, then dump once.
        let _ = join.await;
        let text = buf.lock().await.clone();
        eprintln!("\x1b[1;33m{}\x1b[0m", text);
        return;
    }

    eprint!("\x1b[1;33m");
    let _ = std::io::stderr().flush();
    let mut last = 0usize;
    let mut interval = tokio::time::interval(Duration::from_millis(TYPEWRITER_CHAR_MS));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let snapshot = buf.lock().await.clone();
        if snapshot.len() > last {
            // Emit one char-cluster at a time. We track bytes; ensure we
            // print full UTF-8 sequences by advancing to char boundaries.
            // Easier: pull next char from grapheme-naive char iter.
            let remainder = &snapshot[last..];
            if let Some(ch) = remainder.chars().next() {
                eprint!("{}", ch);
                let _ = std::io::stderr().flush();
                last += ch.len_utf8();
            }
            interval.tick().await;
            continue;
        }
        if join.is_finished() {
            let snapshot = buf.lock().await.clone();
            if snapshot.len() > last {
                let remainder = snapshot[last..].to_string();
                for ch in remainder.chars() {
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    eprint!("{}", ch);
                    let _ = std::io::stderr().flush();
                    interval.tick().await;
                }
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(SPINNER_SLEEP_MS / 4)).await;
    }
    eprintln!("\x1b[0m");
}

pub fn confirm_help() {
    eprintln!("\x1b[2m  y\x1b[0m  run the command");
    eprintln!("\x1b[2m  n\x1b[0m  decline (default — plain Enter also works)");
    eprintln!("\x1b[2m  e\x1b[0m  edit the command before running");
    eprintln!("\x1b[2m  r\x1b[0m  refine: rewrite with a follow-up directive");
}

pub fn confirm_prompt() {
    eprint!(
        "\x1b[1;37mRun?\x1b[0m  [\x1b[32mY\x1b[0m]\x1b[2mes\x1b[0m  [\x1b[1;31mN\x1b[0m]\x1b[2mo\x1b[0m  [\x1b[34mE\x1b[0m]\x1b[2mdit\x1b[0m  [\x1b[33mR\x1b[0m]\x1b[2mefine\x1b[0m  [\x1b[2m?\x1b[0m] "
    );
    let _ = std::io::stderr().flush();
}

pub fn refine_prompt() {
    eprint!("\x1b[2mrefine: \x1b[0m");
    let _ = std::io::stderr().flush();
}

pub fn read_tty_line() -> Option<String> {
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .open(tty_path())
        .ok()?;
    let mut buf = [0u8; 1];
    let mut line = String::new();
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let c = buf[0];
                if c == b'\n' {
                    break;
                }
                line.push(c as char);
            }
            Err(_) => return None,
        }
    }
    Some(line)
}

fn tty_path() -> String {
    if let Ok(t) = std::env::var("TTY")
        && !t.is_empty()
    {
        return t;
    }
    "/dev/tty".to_string()
}

pub enum EditResult {
    Accepted(String),
    Cancelled,
}

pub fn vared_edit(initial: &str) -> EditResult {
    use rustyline::Editor;
    use rustyline::config::{Behavior, Config};
    use rustyline::error::ReadlineError;
    use rustyline::history::DefaultHistory;

    let config = Config::builder().behavior(Behavior::PreferTerm).build();
    let mut rl: Editor<(), DefaultHistory> = match Editor::with_config(config) {
        Ok(rl) => rl,
        Err(e) => {
            warn(&format!("edit unavailable: {}", e));
            return EditResult::Cancelled;
        }
    };
    match rl.readline_with_initial("> ", (initial, "")) {
        Ok(s) => {
            let s = s.trim().to_string();
            if s.is_empty() {
                EditResult::Cancelled
            } else {
                EditResult::Accepted(s)
            }
        }
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => EditResult::Cancelled,
        Err(e) => {
            warn(&format!("edit failed: {}", e));
            EditResult::Cancelled
        }
    }
}

pub fn alts_picker(candidates: &[String]) -> Option<String> {
    if which::which("fzf").is_ok() {
        return fzf_pick(candidates);
    }
    numbered_pick(candidates)
}

fn fzf_pick(candidates: &[String]) -> Option<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("fzf")
        .args([
            "--read0",
            "--prompt=alt > ",
            "--height=60%",
            "--reverse",
            "--ansi",
            "--header=enter = pick · esc = abort",
            "--color=header:dim",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        for c in candidates {
            stdin.write_all(c.as_bytes()).ok()?;
            stdin.write_all(&[0u8]).ok()?;
        }
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
    while s.ends_with('\n') {
        s.pop();
    }
    if s.is_empty() { None } else { Some(s) }
}

fn numbered_pick(candidates: &[String]) -> Option<String> {
    eprintln!("\x1b[2m── alternatives ──\x1b[0m");
    for (i, c) in candidates.iter().enumerate() {
        eprintln!("\x1b[1;36m{})\x1b[0m \x1b[1;33m{}\x1b[0m", i + 1, c);
    }
    eprint!("pick [1-{}]: ", candidates.len());
    let _ = std::io::stderr().flush();
    let pick = read_tty_line()?;
    let n: usize = pick.trim().parse().ok()?;
    if n >= 1 && n <= candidates.len() {
        Some(candidates[n - 1].clone())
    } else {
        None
    }
}
