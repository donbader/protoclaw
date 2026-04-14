/// A command handled by anyclaw itself (not dispatched to any agent).
pub struct PlatformCommand {
    /// The slash command name, without the leading `/` (e.g. `"new"`).
    pub name: &'static str,
    /// Short description shown in channel command menus.
    pub description: &'static str,
}

/// All built-in platform commands.
pub const PLATFORM_COMMANDS: &[PlatformCommand] = &[PlatformCommand {
    name: "new",
    description: "Start a new conversation",
}];

/// Returns the matching `PlatformCommand` if `text` is exactly a platform command
/// (i.e. `"/new"` or `"/new@botname"` — the Telegram bot-mention suffix is stripped).
pub fn match_platform_command(text: &str) -> Option<&'static PlatformCommand> {
    // Strip optional `@mention` suffix (Telegram sends `/cmd@BotName`)
    let command = text.split_once('@').map(|(cmd, _)| cmd).unwrap_or(text);

    // Must start with `/`
    let name = command.strip_prefix('/')?;

    PLATFORM_COMMANDS.iter().find(|c| c.name == name)
}

/// Returns a JSON array of available-command objects for all platform commands,
/// suitable for merging into an `available_commands_update` payload.
pub fn platform_commands_json() -> serde_json::Value {
    let cmds: Vec<serde_json::Value> = PLATFORM_COMMANDS
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "description": c.description,
            })
        })
        .collect();
    serde_json::Value::Array(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::new_bare("/new", Some("new"))]
    #[case::new_with_mention("/new@MyBot", Some("new"))]
    #[case::unknown_command("/unknown", None)]
    #[case::plain_text("hello world", None)]
    #[case::empty("", None)]
    fn when_text_matched_then_returns_expected_command(
        #[case] text: &str,
        #[case] expected_name: Option<&str>,
    ) {
        let result = match_platform_command(text);
        assert_eq!(result.map(|c| c.name), expected_name);
    }

    #[test]
    fn when_platform_commands_json_called_then_returns_array_with_name_field() {
        let val = platform_commands_json();
        let arr = val.as_array().expect("should be array");
        assert!(!arr.is_empty());
        for item in arr {
            assert!(item["name"].as_str().is_some(), "name must be present");
            assert!(
                item["description"].as_str().is_some(),
                "description must be present"
            );
        }
    }

    #[test]
    fn when_new_command_matched_then_description_is_set() {
        let cmd = match_platform_command("/new").expect("should match /new");
        assert_eq!(cmd.name, "new");
        assert!(!cmd.description.is_empty());
    }
}
