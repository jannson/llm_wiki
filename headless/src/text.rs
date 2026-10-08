//! Shared multilingual search utilities extracted from the desktop search module.
use std::collections::BTreeSet;
const SNIPPET_CONTEXT: usize = 80;

pub fn tokenize_query(query: &str) -> Vec<String> {
    let raw = query
        .to_lowercase()
        .split(is_query_separator)
        .filter(|token| token.chars().count() > 1)
        .filter(|token| !is_stop_word(token))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut out = Vec::new();
    for token in raw {
        let chars = token.chars().collect::<Vec<_>>();
        let has_cjk = chars.iter().any(|c| ('\u{3400}'..='\u{9fff}').contains(c));
        if has_cjk && chars.len() > 2 {
            for pair in chars.windows(2) {
                out.push(pair.iter().collect());
            }
            for ch in &chars {
                let s = ch.to_string();
                if !is_stop_word(&s) {
                    out.push(s);
                }
            }
            out.push(token);
        } else {
            out.push(token);
        }
    }
    out.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn is_query_separator(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(
            c,
            '，' | '。'
                | '！'
                | '？'
                | '、'
                | '；'
                | '：'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '（'
                | '）'
                | '·'
                | '～'
                | '…'
        )
}

fn is_stop_word(token: &str) -> bool {
    matches!(
        token,
        "的" | "是"
            | "了"
            | "什么"
            | "在"
            | "有"
            | "和"
            | "与"
            | "对"
            | "从"
            | "the"
            | "is"
            | "a"
            | "an"
            | "what"
            | "how"
            | "are"
            | "was"
            | "were"
            | "do"
            | "does"
            | "did"
            | "be"
            | "been"
            | "being"
            | "have"
            | "has"
            | "had"
            | "it"
            | "its"
            | "in"
            | "on"
            | "at"
            | "to"
            | "for"
            | "of"
            | "with"
            | "by"
            | "this"
            | "that"
            | "these"
            | "those"
    )
}

pub fn extract_title(content: &str, file_name: &str) -> String {
    let has_frontmatter = content.starts_with("---");
    let mut in_frontmatter = has_frontmatter;
    let mut frontmatter_closed = false;
    for line in content.lines().skip(if has_frontmatter { 1 } else { 0 }) {
        let trimmed = line.trim();
        if in_frontmatter && trimmed == "---" {
            in_frontmatter = false;
            frontmatter_closed = true;
            continue;
        }
        if in_frontmatter && trimmed.starts_with("title:") {
            return trimmed
                .trim_start_matches("title:")
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
        if has_frontmatter && !frontmatter_closed {
            continue;
        }
        if let Some(title) = trimmed.strip_prefix("# ") {
            return title.trim().to_string();
        }
    }
    file_name.trim_end_matches(".md").replace('-', " ")
}

pub fn build_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    let q = query.to_lowercase();
    let idx = lower.find(&q).unwrap_or(0);
    let char_positions: Vec<usize> = content.char_indices().map(|(idx, _)| idx).collect();
    if char_positions.is_empty() {
        return String::new();
    }
    let match_char = char_positions
        .iter()
        .position(|byte| *byte >= idx)
        .unwrap_or(char_positions.len().saturating_sub(1));
    let query_chars = query.chars().count().max(1);
    let start_char = match_char.saturating_sub(SNIPPET_CONTEXT);
    let end_char = (match_char + query_chars + SNIPPET_CONTEXT).min(char_positions.len());
    let start = char_positions[start_char];
    let end = if end_char < char_positions.len() {
        char_positions[end_char]
    } else {
        content.len()
    };
    let mut snippet = content[start..end].replace('\n', " ");
    if start > 0 {
        snippet = format!("...{snippet}");
    }
    if end < content.len() {
        snippet.push_str("...");
    }
    snippet
}
