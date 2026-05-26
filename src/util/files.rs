use super::xml_escape;
use crate::config::FILE_CAP;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub struct FileBlock {
    pub xml: String,
    pub info: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slice {
    First(usize),
    Last(usize),
    Range(usize, usize),
}

impl Slice {
    fn describe(self) -> String {
        match self {
            Slice::First(n) => format!("first {} line{}", n, if n == 1 { "" } else { "s" }),
            Slice::Last(n) => format!("last {} line{}", n, if n == 1 { "" } else { "s" }),
            Slice::Range(a, b) => format!("lines {}-{}", a, b),
        }
    }
}

pub struct FileRef {
    pub display: String,
    pub path: String,
    pub slice: Option<Slice>,
}

/// Parse a CLI arg as a possibly-sliced `./path` reference.
/// Returns None unless the arg starts with `./` and resolves to an existing file.
/// A plain path that exists wins over a slice interpretation, so `./weird:1`
/// (a file literally named `weird:1`) reads the whole file.
pub fn parse_path_arg(arg: &str) -> Option<FileRef> {
    if !is_dot_slash_arg(arg) {
        return None;
    }
    if Path::new(arg).is_file() {
        let path = absolute_path(arg);
        let path = path.to_string_lossy().into_owned();
        return Some(FileRef {
            display: path.clone(),
            path,
            slice: None,
        });
    }
    let (path, suffix) = arg.rsplit_once(':')?;
    let slice = parse_slice_suffix(suffix)?;
    if !Path::new(path).is_file() {
        return None;
    }
    let abs_path = absolute_path(path);
    let abs_path = abs_path.to_string_lossy().into_owned();
    let abs_display = format!("{}:{}", abs_path, suffix);
    Some(FileRef {
        display: abs_display,
        path: abs_path,
        slice: Some(slice),
    })
}

fn is_dot_slash_arg(arg: &str) -> bool {
    arg.starts_with("./") && arg.len() > 2
}

fn absolute_path(path: &str) -> PathBuf {
    let path = Path::new(path);
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            return path.to_path_buf();
        }
        let rel = path.strip_prefix(".").unwrap_or(path);
        env::current_dir()
            .map(|cwd| cwd.join(rel))
            .unwrap_or_else(|_| path.to_path_buf())
    })
}

fn parse_slice_suffix(s: &str) -> Option<Slice> {
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('-') {
        let n: usize = rest.parse().ok()?;
        if n == 0 {
            return None;
        }
        return Some(Slice::Last(n));
    }
    if let Some((a, b)) = s.split_once('-') {
        let a: usize = a.parse().ok()?;
        let b: usize = b.parse().ok()?;
        if a == 0 || b == 0 || a > b {
            return None;
        }
        return Some(Slice::Range(a, b));
    }
    let n: usize = s.parse().ok()?;
    if n == 0 {
        return None;
    }
    Some(Slice::First(n))
}

pub fn read_files(refs: &[FileRef]) -> FileBlock {
    let mut budget: i64 = FILE_CAP as i64;
    let mut xml = String::new();
    let mut info_lines = Vec::new();

    for fr in refs {
        if budget <= 0 {
            info_lines.push(format!(
                "skipping {} (32K context budget exhausted)",
                fr.display
            ));
            continue;
        }
        let (content, truncated) = match read_content(fr, budget as usize) {
            Some(t) => t,
            None => continue,
        };
        if content.is_empty() {
            if let Some(s) = fr.slice {
                info_lines.push(format!(
                    "reading {} · 0B ({}; no matching lines)",
                    fr.display,
                    s.describe()
                ));
            }
            continue;
        }
        let escaped_path = xml_escape::escape(&fr.display);
        let escaped_content = xml_escape::escape(&content);
        xml.push_str(&format!(
            "<file path=\"{}\">\n{}\n</file>\n",
            escaped_path, escaped_content
        ));

        let take = content.len();
        let size_str = if take >= 1024 {
            format!("{:.1}K", take as f64 / 1024.0)
        } else {
            format!("{}B", take)
        };
        let mut note = String::new();
        if let Some(s) = fr.slice {
            note.push_str(&format!(" ({})", s.describe()));
        }
        if truncated {
            note.push_str(" [truncated, 32K cap]");
        }
        info_lines.push(format!("reading {} · {}{}", fr.display, size_str, note));
        budget -= take as i64;
    }
    FileBlock {
        xml,
        info: info_lines.join("\n"),
    }
}

fn read_content(fr: &FileRef, budget: usize) -> Option<(String, bool)> {
    let file = fs::File::open(&fr.path).ok()?;
    Some(match fr.slice {
        None => read_whole(file, &fr.path, budget),
        Some(Slice::First(n)) => read_first_lines(file, n, budget),
        Some(Slice::Last(n)) => read_last_lines(file, n, budget),
        Some(Slice::Range(a, b)) => read_range_lines(file, a, b, budget),
    })
}

fn read_whole(file: fs::File, path: &str, budget: usize) -> (String, bool) {
    let mut buf = Vec::with_capacity(budget.min(8192));
    let mut handle = file.take(budget as u64);
    if handle.read_to_end(&mut buf).is_err() {
        return (String::new(), false);
    }
    let content = String::from_utf8_lossy(&buf).into_owned();
    let truncated = fs::metadata(path)
        .ok()
        .map(|m| m.len() > budget as u64)
        .unwrap_or(false);
    (content, truncated)
}

fn read_first_lines(file: fs::File, n: usize, budget: usize) -> (String, bool) {
    let reader = BufReader::new(file);
    let mut out = String::new();
    let mut truncated = false;
    for line in reader.lines().map_while(Result::ok).take(n) {
        if out.len() + line.len() + 1 > budget {
            truncated = true;
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    (out, truncated)
}

fn read_last_lines(file: fs::File, n: usize, budget: usize) -> (String, bool) {
    let reader = BufReader::new(file);
    let mut window: VecDeque<String> = VecDeque::with_capacity(n);
    for line in reader.lines().map_while(Result::ok) {
        if window.len() == n {
            window.pop_front();
        }
        window.push_back(line);
    }
    let mut total: usize = window.iter().map(|s| s.len() + 1).sum();
    let mut truncated = false;
    while total > budget && window.len() > 1 {
        if let Some(front) = window.pop_front() {
            total -= front.len() + 1;
            truncated = true;
        }
    }
    let mut out = String::with_capacity(total);
    for line in &window {
        out.push_str(line);
        out.push('\n');
    }
    (out, truncated)
}

fn read_range_lines(file: fs::File, a: usize, b: usize, budget: usize) -> (String, bool) {
    let reader = BufReader::new(file);
    let mut out = String::new();
    let mut truncated = false;
    for (idx, line) in reader.lines().map_while(Result::ok).enumerate() {
        let line_no = idx + 1;
        if line_no < a {
            continue;
        }
        if line_no > b {
            break;
        }
        if out.len() + line.len() + 1 > budget {
            truncated = true;
            break;
        }
        out.push_str(&line);
        out.push('\n');
    }
    (out, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_file(name: &str, body: &str) -> String {
        let dir = std::env::temp_dir().join(format!("qsh-files-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(name);
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn rel_tmp_file(name: &str, body: &str) -> (String, String) {
        let rel_dir = format!("target/qsh-files-test-{}", std::process::id());
        let dir = Path::new(&rel_dir);
        let _ = fs::create_dir_all(dir);
        let rel = format!("{}/{}", rel_dir, name);
        let mut f = fs::File::create(&rel).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        let arg = format!("./{}", rel);
        let abs = fs::canonicalize(&rel)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        (arg, abs)
    }

    #[test]
    fn parse_slice_first() {
        assert_eq!(parse_slice_suffix("10"), Some(Slice::First(10)));
        assert_eq!(parse_slice_suffix("1"), Some(Slice::First(1)));
    }

    #[test]
    fn parse_slice_last() {
        assert_eq!(parse_slice_suffix("-10"), Some(Slice::Last(10)));
        assert_eq!(parse_slice_suffix("-1"), Some(Slice::Last(1)));
    }

    #[test]
    fn parse_slice_range() {
        assert_eq!(parse_slice_suffix("5-20"), Some(Slice::Range(5, 20)));
        assert_eq!(parse_slice_suffix("1-1"), Some(Slice::Range(1, 1)));
    }

    #[test]
    fn parse_slice_rejects_bad() {
        assert_eq!(parse_slice_suffix(""), None);
        assert_eq!(parse_slice_suffix("0"), None);
        assert_eq!(parse_slice_suffix("-0"), None);
        assert_eq!(parse_slice_suffix("0-5"), None);
        assert_eq!(parse_slice_suffix("5-0"), None);
        assert_eq!(parse_slice_suffix("20-5"), None); // a > b
        assert_eq!(parse_slice_suffix("abc"), None);
        assert_eq!(parse_slice_suffix("1-2-3"), None);
        assert_eq!(parse_slice_suffix("--5"), None);
    }

    #[test]
    fn parse_path_arg_rejects_non_dot_slash() {
        assert!(parse_path_arg("foo.txt").is_none());
        assert!(parse_path_arg("/abs/path").is_none());
        assert!(parse_path_arg("./").is_none());
    }

    #[test]
    fn parse_path_arg_uses_absolute_display_path() {
        let (arg, abs) = rel_tmp_file("abs.txt", "hello\n");
        let fr = parse_path_arg(&arg).unwrap();
        assert_eq!(fr.display, abs);
        assert_eq!(fr.path, abs);
        assert_eq!(fr.slice, None);
    }

    #[test]
    fn parse_path_arg_uses_absolute_display_path_for_slice() {
        let (arg, abs) = rel_tmp_file("slice.txt", "a\nb\n");
        let fr = parse_path_arg(&format!("{}:1", arg)).unwrap();
        assert_eq!(fr.display, format!("{}:1", abs));
        assert_eq!(fr.path, abs);
        assert_eq!(fr.slice, Some(Slice::First(1)));
    }

    #[test]
    fn read_first_n_lines() {
        let path = tmp_file("first.txt", "a\nb\nc\nd\ne\n");
        let f = fs::File::open(&path).unwrap();
        let (out, trunc) = read_first_lines(f, 3, 1024);
        assert_eq!(out, "a\nb\nc\n");
        assert!(!trunc);
    }

    #[test]
    fn read_last_n_lines() {
        let path = tmp_file("last.txt", "a\nb\nc\nd\ne\n");
        let f = fs::File::open(&path).unwrap();
        let (out, trunc) = read_last_lines(f, 2, 1024);
        assert_eq!(out, "d\ne\n");
        assert!(!trunc);
    }

    #[test]
    fn read_range_inclusive() {
        let path = tmp_file("range.txt", "a\nb\nc\nd\ne\n");
        let f = fs::File::open(&path).unwrap();
        let (out, trunc) = read_range_lines(f, 2, 4, 1024);
        assert_eq!(out, "b\nc\nd\n");
        assert!(!trunc);
    }

    #[test]
    fn read_first_more_than_available() {
        let path = tmp_file("short.txt", "a\nb\n");
        let f = fs::File::open(&path).unwrap();
        let (out, trunc) = read_first_lines(f, 10, 1024);
        assert_eq!(out, "a\nb\n");
        assert!(!trunc);
    }

    #[test]
    fn read_last_trims_to_budget() {
        let path = tmp_file("trim.txt", "aaaa\nbbbb\ncccc\n");
        let f = fs::File::open(&path).unwrap();
        // budget only fits ~one 5-byte line ("cccc\n")
        let (out, trunc) = read_last_lines(f, 3, 5);
        assert_eq!(out, "cccc\n");
        assert!(trunc);
    }
}
