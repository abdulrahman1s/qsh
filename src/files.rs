use crate::config::FILE_CAP;
use crate::xml_escape;
use std::fs;
use std::io::Read;
use std::path::Path;

pub struct FileBlock {
    pub xml: String,
    pub info: String,
}

pub fn read_files(paths: &[String]) -> FileBlock {
    let mut budget: i64 = FILE_CAP as i64;
    let mut xml = String::new();
    let mut info_lines = Vec::new();

    for p in paths {
        if budget <= 0 {
            info_lines.push(format!("skipping {} (32K context budget exhausted)", p));
            continue;
        }
        let path = Path::new(p);
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut buf = Vec::with_capacity(budget.min(8192) as usize);
        let mut handle = file.take(budget as u64);
        if handle.read_to_end(&mut buf).is_err() {
            continue;
        }
        let content = String::from_utf8_lossy(&buf).to_string();
        if content.is_empty() {
            continue;
        }
        let escaped_path = xml_escape::escape(p);
        let escaped_content = xml_escape::escape(&content);
        xml.push_str(&format!(
            "<file path=\"{}\">\n{}\n</file>\n",
            escaped_path, escaped_content
        ));

        let actual = fs::metadata(path).ok().map(|m| m.len());
        let take = content.len();
        let size_str = if take >= 1024 {
            format!("{:.1}K", take as f64 / 1024.0)
        } else {
            format!("{}B", take)
        };
        let mut note = String::new();
        if let Some(a) = actual
            && a > budget as u64
        {
            note = " (truncated, 32K cap)".to_string();
        }
        info_lines.push(format!("reading {} · {}{}", p, size_str, note));
        budget -= take as i64;
    }
    FileBlock {
        xml,
        info: info_lines.join("\n"),
    }
}
