pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ampersand_first() {
        // & must come first or < → &lt; → &amp;lt;
        assert_eq!(escape("a&b<c"), "a&amp;b&lt;c");
    }

    #[test]
    fn quote_and_gt() {
        assert_eq!(escape("\"hi\" > out"), "&quot;hi&quot; &gt; out");
    }
}
