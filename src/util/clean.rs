pub fn clean_command(s: &str) -> String {
    let mut text = s.to_string();

    // Strip leading code fence on first line (```bash, ```, ~~~, etc.).
    if let Some(nl) = text.find('\n') {
        let (first, rest) = text.split_at(nl);
        let f = first.trim_start();
        if is_fence_open(f) {
            text = rest.trim_start_matches('\n').to_string();
        }
    } else {
        let f = text.trim_start();
        if is_fence_open(f) {
            text.clear();
        }
    }

    // Strip trailing fence on last line.
    if let Some(nl) = text.rfind('\n') {
        let (head, tail) = text.split_at(nl);
        let t = tail.trim();
        if is_fence_close(t) {
            text = head.to_string();
        }
    } else {
        let t = text.trim();
        if is_fence_close(t) {
            text.clear();
        }
    }

    let mut trimmed = text.trim().to_string();
    if let Some(stripped) = trimmed
        .strip_prefix("$ ")
        .or_else(|| trimmed.strip_prefix("% "))
    {
        trimmed = stripped.to_string();
    }
    trimmed
}

fn is_fence_open(s: &str) -> bool {
    let s = s.trim();
    if s.starts_with("```") {
        let rest = s.trim_start_matches('`');
        rest.chars().all(|c| c.is_ascii_alphanumeric())
    } else if s.starts_with("~~~") {
        let rest = s.trim_start_matches('~');
        rest.chars().all(|c| c.is_ascii_alphanumeric())
    } else {
        false
    }
}

fn is_fence_close(s: &str) -> bool {
    let s = s.trim();
    s == "```" || s == "~~~"
}

pub fn strip_why_comment(s: &str) -> String {
    if let Some(idx) = s.find(" #") {
        return s[..idx].trim_end().to_string();
    }
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_bash_fence() {
        let s = "```bash\nfind . -name '*.rs'\n```";
        assert_eq!(clean_command(s), "find . -name '*.rs'");
    }

    #[test]
    fn strip_dollar_prompt() {
        assert_eq!(clean_command("$ ls -la"), "ls -la");
    }

    #[test]
    fn strip_percent_prompt() {
        assert_eq!(clean_command("% pwd"), "pwd");
    }

    #[test]
    fn no_fence_passthrough() {
        assert_eq!(clean_command("  echo hi  "), "echo hi");
    }

    #[test]
    fn strip_why_comment_keeps_command() {
        assert_eq!(strip_why_comment("ls -la # why: long form"), "ls -la");
        assert_eq!(strip_why_comment("ls -la"), "ls -la");
    }
}
