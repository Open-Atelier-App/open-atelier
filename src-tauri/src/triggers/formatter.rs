fn escape_param(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn wrap_user_message(content: &str) -> String {
    format!(">>>[MESSAGE \"{}\"]<<<", escape_param(content))
}

pub fn format_permission(level_label: &str) -> String {
    format!(">>>[PERMISSION \"{}\"]<<<", escape_param(level_label))
}

pub fn format_result(action: &str, status: &str, detail: &str) -> String {
    format!(
        ">>>[RESULT \"{}\" \"{}\" \"{}\"]<<<",
        escape_param(action),
        escape_param(status),
        escape_param(detail),
    )
}

pub fn format_content(path: &str, content: &str) -> String {
    format!(
        ">>>[CONTENT \"{}\" \"{}\"]<<<",
        escape_param(path),
        escape_param(content),
    )
}

pub fn format_permission_change(new_level: &str, allowed_triggers: &[String]) -> String {
    let triggers_str = allowed_triggers.join(", ");
    format!(
        ">>>[PERMISSION \"{new_level}\"]<<<\n\
         Your permission level has changed. You may now use the following triggers: {triggers_str}\n\
         Triggers not in this list will be rejected."
    )
}

pub fn format_runtime_permission_block(level_label: &str, allowed_triggers: &[String]) -> String {
    let triggers_str = allowed_triggers.join(", ");
    format!(
        "Your current permission level is: {level_label}\n\
         You may use the following triggers: {triggers_str}\n\n\
         Triggers not in your allowed list will be rejected. Do not attempt them."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_message_escapes_quotes() {
        let result = wrap_user_message("He said \"hello\"");
        assert_eq!(result, r#">>>[MESSAGE "He said \"hello\""]<<<"#);
    }

    #[test]
    fn format_result_ok() {
        let result = format_result("CREATE", "OK", "");
        assert_eq!(result, r#">>>[RESULT "CREATE" "OK" ""]<<<"#);
    }

    #[test]
    fn format_result_fail() {
        let result = format_result("DELETE", "FAIL", "File not found: test.txt");
        assert_eq!(
            result,
            r#">>>[RESULT "DELETE" "FAIL" "File not found: test.txt"]<<<"#
        );
    }

    #[test]
    fn format_content_with_newlines() {
        let result = format_content("file.txt", "line1\nline2");
        assert!(result.contains("line1\nline2"));
        assert!(result.starts_with(">>>[CONTENT"));
    }

    #[test]
    fn permission_block_has_triggers() {
        let block = format_runtime_permission_block(
            "Full Access",
            &["MESSAGE".into(), "CREATE".into(), "WRITE".into()],
        );
        assert!(block.contains("Full Access"));
        assert!(block.contains("MESSAGE, CREATE, WRITE"));
    }

    #[test]
    fn permission_change_format() {
        let result = format_permission_change("Read & Preview", &["MESSAGE".into(), "READ".into()]);
        assert!(result.contains(">>>[PERMISSION \"Read & Preview\"]<<<"));
        assert!(result.contains("MESSAGE, READ"));
    }
}
