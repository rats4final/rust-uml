//! Splitting text into meaningful lines.

/// One non-empty line with comments removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 1-based line number in the original text.
    pub number: usize,
    pub text: String,
}

/// Front-matter title (if any) and the meaningful lines.
pub(crate) fn split(text: &str) -> (Option<String>, Vec<Line>) {
    let mut title = None;
    let mut out = Vec::new();
    let mut in_front = false;
    let mut seen_content = false;
    for (i, raw) in text.lines().enumerate() {
        let t = raw.trim();
        if t == "---" && (!seen_content || in_front) {
            in_front = !in_front;
            seen_content = true;
            continue;
        }
        if in_front {
            if let Some(v) = t.strip_prefix("title:") {
                title = Some(crate::unquote(v).to_owned());
            }
            continue;
        }
        // `%%` starts a comment; `%%{ … }%%` directives are comments too.
        let t = match t.find("%%") {
            Some(at) => t[..at].trim(),
            None => t,
        };
        if t.is_empty() {
            continue;
        }
        seen_content = true;
        // A trailing `;` ends a statement, as in Mermaid.
        let t = t.strip_suffix(';').unwrap_or(t).trim_end();
        out.push(Line {
            number: i + 1,
            text: t.to_owned(),
        });
    }
    (title, out)
}

/// Lines Mermaid uses only for looks or interaction; skipped silently.
pub(crate) fn is_styling(t: &str) -> bool {
    let first = t.split_whitespace().next().unwrap_or_default();
    matches!(
        first,
        "style" | "classDef" | "cssClass" | "click" | "link" | "callback" | "linkStyle"
    ) || t.starts_with("accTitle")
        || t.starts_with("accDescr")
}
