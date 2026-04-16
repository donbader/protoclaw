use serde::Serialize;

/// A command handled by anyclaw itself (not dispatched to any agent).
#[derive(Debug, Clone, Serialize)]
pub struct PlatformCommand {
    /// The slash command name, without the leading `/` (e.g. `"new"`).
    pub name: &'static str,
    /// Short description shown in channel command menus.
    pub description: &'static str,
}

/// All built-in platform commands.
pub const PLATFORM_COMMANDS: &[PlatformCommand] = &[
    PlatformCommand {
        name: "new",
        description: "Start a new conversation",
    },
    PlatformCommand {
        name: "cancel",
        description: "Cancel the current operation",
    },
];

/// Returns a typed slice of all platform commands.
pub fn platform_commands() -> &'static [PlatformCommand] {
    PLATFORM_COMMANDS
}

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
/// D-03: serialization boundary — agent content mutation requires Value for array merging.
#[allow(clippy::disallowed_types)]
pub fn platform_commands_json() -> serde_json::Value {
    serde_json::to_value(PLATFORM_COMMANDS).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::new_bare("/new", Some("new"))]
    #[case::new_with_mention("/new@MyBot", Some("new"))]
    #[case::cancel_bare("/cancel", Some("cancel"))]
    #[case::cancel_with_mention("/cancel@MyBot", Some("cancel"))]
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
