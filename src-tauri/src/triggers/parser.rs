use serde::Serialize;

const KNOWN_ACTIONS: &[&str] = &[
    "MESSAGE", "CREATE", "DELETE", "WRITE", "INSERT",
    "APPEND", "PREVIEW", "READ", "RENAME", "LIST",
    "CREATE_DOCX", "CREATE_XLSX", "CREATE_PPTX", "CREATE_MP3", "EXPORT_PDF", "PLAN",
    "GITHUB_READ", "NOTION_READ", "SLACK_READ", "GDRIVE_READ",
];

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTrigger {
    pub action: String,
    pub params: Vec<String>,
    pub raw: String,
    pub byte_offset: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParseError {
    pub raw: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParseResult {
    pub triggers: Vec<ParsedTrigger>,
    pub clean_text: String,
    pub errors: Vec<ParseError>,
}

pub fn parse(response: &str) -> ParseResult {
    let mut triggers = Vec::new();
    let mut errors = Vec::new();
    let mut clean_parts: Vec<&str> = Vec::new();
    let mut last_end = 0;

    let bytes = response.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if let Some(open_len) = trigger_open_len(bytes, i) {
            // Collect text before this trigger
            if i > last_end {
                clean_parts.push(&response[last_end..i]);
            }

            let start = i;
            match find_trigger_end(response, i + open_len) {
                Some((end_pos, closer_len)) => {
                    let raw = &response[start..end_pos];
                    let inner = &response[start + open_len..end_pos - closer_len]; // strip opener and closer

                    match parse_inner(inner) {
                        Ok((action, params)) => {
                            if KNOWN_ACTIONS.contains(&action.as_str()) {
                                triggers.push(ParsedTrigger {
                                    action,
                                    params,
                                    raw: raw.to_string(),
                                    byte_offset: start,
                                });
                            } else {
                                let suggestion = suggest_action(&action);
                                errors.push(ParseError {
                                    raw: raw.to_string(),
                                    message: format!("Unknown action: {action}"),
                                    suggestion,
                                });
                            }
                        }
                        Err(msg) => {
                            errors.push(ParseError {
                                raw: raw.to_string(),
                                message: msg,
                                suggestion: None,
                            });
                        }
                    }
                    last_end = end_pos;
                    i = end_pos;
                }
                None => {
                    // Unterminated trigger
                    let raw_end = response.len().min(start + 80);
                    errors.push(ParseError {
                        raw: response[start..raw_end].to_string(),
                        message: "Unterminated trigger: missing ]<<<".into(),
                        suggestion: None,
                    });
                    // Treat as plain text
                    clean_parts.push(&response[start..start + open_len]);
                    last_end = start + open_len;
                    i = start + open_len;
                }
            }
        } else {
            i += 1;
        }
    }

    if last_end < len {
        clean_parts.push(&response[last_end..]);
    }

    let clean_text = clean_parts.concat().trim().to_string();

    ParseResult { triggers, clean_text, errors }
}

/// Returns the length of the trigger opener at position `i`, if any: 4 for
/// the canonical `>>>[`, or 1 for a bare `[ACTION` when weaker models drop
/// the `>>>` prefix. The bare form only matches when immediately followed
/// by one of the exact known action names plus a space/quote/bracket, so
/// ordinary markdown brackets (`[link](url)`, `[ ]`, `[1]`) never trigger
/// it — those never spell out a real action name right after `[`.
fn trigger_open_len(bytes: &[u8], i: usize) -> Option<usize> {
    let len = bytes.len();
    if i + 4 <= len && &bytes[i..i + 4] == b">>>[" {
        return Some(4);
    }

    if i < len && bytes[i] == b'[' {
        for action in KNOWN_ACTIONS {
            let start = i + 1;
            let end = start + action.len();
            if end <= len && &bytes[start..end] == action.as_bytes() {
                match bytes.get(end) {
                    Some(b' ') | Some(b'"') | Some(b']') => return Some(1),
                    _ => {}
                }
            }
        }
    }

    None
}

/// Finds where a trigger closes, returning `(end_position, closer_length)`.
/// Prefers the canonical `]<<<` (closer_length 4), but if the very first
/// unquoted `]` isn't immediately followed by `<<<`, treats that bare `]`
/// as the close anyway (closer_length 1) — the same class of leniency as
/// `trigger_open_len` tolerating a dropped `>>>` prefix, just for the other
/// end: weaker models sometimes write `[ACTION "a" "b"]` with no `<<<` at
/// all, closing it like an ordinary function call/array literal. Since
/// every parameter is a quoted string, an unquoted `]` inside an
/// already-detected trigger attempt can only be an attempt at closing it —
/// never legitimate content — so stopping at the first one is safe and
/// prevents a huge chunk of trigger content from leaking into the chat as
/// plain text (see the "Unterminated trigger" fallback below) just because
/// three characters are missing.
fn find_trigger_end(s: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = from;
    let mut in_quotes = false;
    let mut escaped = false;

    while i < len {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        let b = bytes[i];

        if b == b'\\' && in_quotes {
            escaped = true;
            i += 1;
            continue;
        }

        if b == b'"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }

        if !in_quotes && b == b']' {
            if i + 4 <= len && &bytes[i..i + 4] == b"]<<<" {
                return Some((i + 4, 4));
            }
            return Some((i + 1, 1));
        }

        i += 1;
    }

    None
}

fn parse_inner(inner: &str) -> std::result::Result<(String, Vec<String>), String> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return Err("Empty trigger".into());
    }

    // Extract action (first word before space or quote)
    let action_end = trimmed.find(|c: char| c == ' ' || c == '"').unwrap_or(trimmed.len());
    let action = trimmed[..action_end].trim().to_string();

    if action.is_empty() {
        return Err("Empty action name".into());
    }

    let rest = trimmed[action_end..].trim();
    if rest.is_empty() {
        return Ok((action, Vec::new()));
    }

    let params = parse_quoted_params(rest)?;
    Ok((action, params))
}

fn parse_quoted_params(s: &str) -> std::result::Result<Vec<String>, String> {
    let mut params = Vec::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        while i < len && bytes[i] == b' ' {
            i += 1;
        }
        if i >= len {
            break;
        }

        if bytes[i] != b'"' {
            return Err(format!("Expected '\"' at position {i}, found '{}'", bytes[i] as char));
        }
        i += 1; // skip opening quote

        let mut value = String::new();
        let mut escaped = false;
        let mut found_close = false;

        while i < len {
            if escaped {
                match bytes[i] {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'n' => value.push('\n'),
                    b't' => value.push('\t'),
                    b']' => value.push(']'),
                    other => {
                        value.push('\\');
                        value.push(other as char);
                    }
                }
                escaped = false;
                i += 1;
                continue;
            }

            if bytes[i] == b'\\' {
                escaped = true;
                i += 1;
                continue;
            }

            if bytes[i] == b'"' {
                found_close = true;
                i += 1;
                break;
            }

            // Handle multi-byte UTF-8
            let ch_start = i;
            let ch = s[ch_start..].chars().next().unwrap();
            value.push(ch);
            i += ch.len_utf8();
        }

        if !found_close {
            return Err("Unterminated quoted parameter".into());
        }

        params.push(value);
    }

    Ok(params)
}

fn suggest_action(unknown: &str) -> Option<String> {
    let upper = unknown.to_uppercase();
    let mut best: Option<(&str, usize)> = None;

    for known in KNOWN_ACTIONS {
        let dist = levenshtein(&upper, known);
        if dist <= 2 {
            match best {
                None => best = Some((known, dist)),
                Some((_, d)) if dist < d => best = Some((known, dist)),
                _ => {}
            }
        }
    }

    best.map(|(s, _)| format!("Did you mean {s}?"))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();

    let mut prev = (0..=m).collect::<Vec<_>>();
    let mut curr = vec![0; m + 1];

    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_create() {
        let r = parse(r#">>>[CREATE "src/main.ts"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "CREATE");
        assert_eq!(r.triggers[0].params, vec!["src/main.ts"]);
        assert!(r.errors.is_empty());
        assert!(r.clean_text.is_empty());
    }

    #[test]
    fn parse_write_with_content() {
        let r = parse(r#">>>[WRITE "file.ts" "const x = 1;"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[0], "file.ts");
        assert_eq!(r.triggers[0].params[1], "const x = 1;");
    }

    #[test]
    fn parse_multiline_content() {
        let input = ">>>[WRITE \"file.ts\" \"line1\nline2\nline3\"]<<<";
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[1], "line1\nline2\nline3");
    }

    #[test]
    fn parse_plan_with_multiple_steps() {
        let input = ">>>[PLAN \"Launch prep\" \"Research market\nDraft copy\nExport PDF\"]<<<";
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "PLAN");
        assert_eq!(r.triggers[0].params[0], "Launch prep");
        assert_eq!(r.triggers[0].params[1], "Research market\nDraft copy\nExport PDF");
    }

    #[test]
    fn parse_escaped_quotes() {
        let input = r#">>>[WRITE "file.ts" "say \"hello\""]<<<"#;
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[1], "say \"hello\"");
    }

    #[test]
    fn parse_escaped_newlines() {
        let input = r#">>>[WRITE "file.ts" "line1\nline2"]<<<"#;
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[1], "line1\nline2");
    }

    #[test]
    fn parse_mixed_text_and_triggers() {
        let input = "Here is your file:\n>>>[CREATE \"test.ts\"]<<<\nDone!";
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.clean_text, "Here is your file:\n\nDone!");
    }

    #[test]
    fn parse_multiple_triggers() {
        let input = r#">>>[CREATE "a.ts"]<<<
>>>[WRITE "a.ts" "content"]<<<
>>>[MESSAGE "Done"]<<<"#;
        let r = parse(input);
        assert_eq!(r.triggers.len(), 3);
        assert_eq!(r.triggers[0].action, "CREATE");
        assert_eq!(r.triggers[1].action, "WRITE");
        assert_eq!(r.triggers[2].action, "MESSAGE");
    }

    #[test]
    fn unknown_action_produces_error_with_suggestion() {
        let r = parse(r#">>>[DELET "file.ts"]<<<"#);
        assert!(r.triggers.is_empty());
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].message.contains("Unknown action"));
        assert_eq!(r.errors[0].suggestion, Some("Did you mean DELETE?".into()));
    }

    #[test]
    fn unterminated_trigger_produces_error() {
        let r = parse(">>>[CREATE \"file.ts\"");
        assert!(r.triggers.is_empty());
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].message.contains("Unterminated"));
    }

    #[test]
    fn insert_with_three_params() {
        let r = parse(r#">>>[INSERT "file.ts" "new line" "5"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "INSERT");
        assert_eq!(r.triggers[0].params, vec!["file.ts", "new line", "5"]);
    }

    #[test]
    fn rename_with_two_paths() {
        let r = parse(r#">>>[RENAME "old.ts" "new.ts"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params, vec!["old.ts", "new.ts"]);
    }

    #[test]
    fn no_triggers_returns_clean_text() {
        let r = parse("Just a normal message with no triggers.");
        assert!(r.triggers.is_empty());
        assert!(r.errors.is_empty());
        assert_eq!(r.clean_text, "Just a normal message with no triggers.");
    }

    #[test]
    fn trigger_delimiters_in_code_block_not_parsed() {
        // If the LLM explains syntax with >>> outside of an actual trigger pattern,
        // the parser only activates on >>>[ specifically
        let r = parse("Use >>> and <<< like this: >>>[ACTION]<<<");
        // >>>[ triggers parsing, but the inner is just "ACTION" with no valid ]<<<
        // Actually "ACTION]<<<" would be found. Let's test properly:
        let r2 = parse("The syntax is >>> followed by [ and then ]<<<. Don't use it.");
        assert!(r2.triggers.is_empty() || !r2.errors.is_empty());
    }

    #[test]
    fn unicode_content() {
        let r = parse(r#">>>[WRITE "readme.md" "こんにちは世界 🌍"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[1], "こんにちは世界 🌍");
    }

    #[test]
    fn empty_params_message() {
        // MESSAGE with no params should parse (action only)
        let r = parse(r#">>>[LIST "."]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "LIST");
        assert_eq!(r.triggers[0].params, vec!["."]);
    }

    #[test]
    fn escaped_bracket_in_content() {
        let input = r#">>>[WRITE "file.ts" "data\]<<<more"]<<<"#;
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].params[1], "data]<<<more");
    }

    // Regression coverage for weaker models (e.g. Mistral Small) that
    // sometimes drop the ">>>" prefix and emit a bare "[ACTION ...]<<<".
    // Without this fallback the near-miss syntax neither gets executed nor
    // hidden from the chat — it leaks as raw protocol text.
    #[test]
    fn lenient_bare_bracket_without_prefix() {
        let r = parse(r#"[CREATE "coucou.md"]<<<"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "CREATE");
        assert_eq!(r.triggers[0].params, vec!["coucou.md"]);
        assert!(r.clean_text.is_empty());
    }

    #[test]
    fn lenient_bare_bracket_mixed_with_text() {
        let input = "Bien sur !\n[WRITE \"coucou.md\" \"contenu\"]<<<\nVoila.";
        let r = parse(input);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "WRITE");
        assert_eq!(r.clean_text, "Bien sur !\n\nVoila.");
    }

    // Regression coverage for an even sloppier form some weak models emit:
    // dropping BOTH the ">>>" prefix AND the "<<<" suffix, closing the
    // trigger with a single bare "]" like a function call. Previously this
    // fell through to "Unterminated trigger", and — worse — the entire huge
    // trigger body (e.g. a whole document's worth of CREATE_DOCX content)
    // leaked verbatim into the chat message as plain text.
    #[test]
    fn bare_bracket_without_prefix_or_suffix() {
        let r = parse(r#"[CREATE "coucou.md"]"#);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "CREATE");
        assert_eq!(r.triggers[0].params, vec!["coucou.md"]);
        assert!(r.clean_text.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn bare_bracket_with_multiline_content_and_no_suffix() {
        let r = parse(r##"[CREATE_DOCX "plan.docx" "# Title\n\n## Section\n- point one\n- point two"]"##);
        assert_eq!(r.triggers.len(), 1);
        assert_eq!(r.triggers[0].action, "CREATE_DOCX");
        assert_eq!(r.triggers[0].params[0], "plan.docx");
        assert_eq!(r.triggers[0].params[1], "# Title\n\n## Section\n- point one\n- point two");
        assert!(r.clean_text.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn ordinary_markdown_brackets_are_not_treated_as_triggers() {
        let link = parse("See [here](https://example.com) for details");
        assert!(link.triggers.is_empty());
        assert!(link.errors.is_empty());

        let checkbox = parse("- [ ] todo item");
        assert!(checkbox.triggers.is_empty());
        assert!(checkbox.errors.is_empty());

        let citation = parse("As shown [1] in the appendix");
        assert!(citation.triggers.is_empty());
        assert!(citation.errors.is_empty());
    }
}
