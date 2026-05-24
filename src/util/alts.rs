use super::clean::clean_command;

pub struct ParsedAlts {
    pub candidates: Vec<String>,
    pub shortfall: i32,
    pub dedupe_loss: i32,
    pub requested: u32,
}

pub fn parse(buf: &str, requested: u32) -> ParsedAlts {
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in buf.lines() {
        if is_sentinel(line) {
            if let Some(b) = current.take() {
                blocks.push(b);
            }
            current = Some(String::new());
            continue;
        }
        if let Some(c) = current.as_mut() {
            if !c.is_empty() {
                c.push('\n');
            }
            c.push_str(line);
        }
    }
    if let Some(b) = current
        && !b.is_empty()
    {
        blocks.push(b);
    }

    let mut cleaned: Vec<String> = blocks
        .into_iter()
        .map(|b| clean_command(&b))
        .filter(|s| !s.is_empty())
        .collect();
    let pre = cleaned.len() as i32;
    let mut seen = std::collections::HashSet::new();
    cleaned.retain(|c| seen.insert(c.clone()));

    let dedupe_loss = pre - cleaned.len() as i32;
    let shortfall = requested as i32 - pre;

    ParsedAlts {
        candidates: cleaned,
        shortfall,
        dedupe_loss,
        requested,
    }
}

fn is_sentinel(line: &str) -> bool {
    let s = line.trim();
    let mut tokens = s.split_whitespace();
    let Some(a) = tokens.next() else {
        return false;
    };
    let Some(b) = tokens.next() else {
        return false;
    };
    let Some(c) = tokens.next() else {
        return false;
    };
    let Some(d) = tokens.next() else {
        return false;
    };
    if tokens.next().is_some() {
        return false;
    }
    a == "===" && b == "alt" && c.chars().all(|x| x.is_ascii_digit()) && !c.is_empty() && d == "==="
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_alts() {
        let buf = "=== alt 1 ===\nls -la\n=== alt 2 ===\nfind . -name '*'\n=== alt 3 ===\nls -1";
        let p = parse(buf, 3);
        assert_eq!(p.candidates.len(), 3);
        assert_eq!(p.candidates[0], "ls -la");
        assert_eq!(p.candidates[1], "find . -name '*'");
        assert_eq!(p.candidates[2], "ls -1");
        assert_eq!(p.shortfall, 0);
        assert_eq!(p.dedupe_loss, 0);
    }

    #[test]
    fn dedupes_preserves_order() {
        let buf = "=== alt 1 ===\nls\n=== alt 2 ===\nls\n=== alt 3 ===\nfind .";
        let p = parse(buf, 3);
        assert_eq!(p.candidates, vec!["ls".to_string(), "find .".to_string()]);
        assert_eq!(p.dedupe_loss, 1);
    }

    #[test]
    fn handles_multiline_for_loop() {
        let buf = "=== alt 1 ===\nfor f in *.tar.gz; do\n  echo \"$f\"\ndone\n=== alt 2 ===\nls";
        let p = parse(buf, 2);
        assert_eq!(p.candidates.len(), 2);
        assert!(p.candidates[0].contains("for f"));
        assert!(p.candidates[0].contains("done"));
    }
}
