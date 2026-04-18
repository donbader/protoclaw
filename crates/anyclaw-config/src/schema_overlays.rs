/// Post-process the schemars-generated schema to add extension-specific option definitions.
///
/// Extension binaries define their own option schemas (e.g., Telegram's `access_control`),
/// but schemars only sees `HashMap<String, Value>` for the `options` field. This function
/// injects typed definitions so IDE autocomplete works for known extension options.
#[allow(clippy::disallowed_types)]
pub fn apply_extension_overlays(schema: &mut serde_json::Value) {
    let Some(defs) = schema
        .as_object_mut()
        .and_then(|s| s.get_mut("$defs"))
        .and_then(|d| d.as_object_mut())
    else {
        return;
    };

    // Insert TelegramAccessControlGroupConfig
    defs.insert(
        "TelegramAccessControlGroupConfig".to_string(),
        serde_json::json!({
            "description": "Per-group access control override, keyed by Telegram chat ID.",
            "properties": {
                "enabled": {
                    "default": true,
                    "description": "Whether this group can interact with the bot.",
                    "type": "boolean"
                },
                "group_policy": {
                    "description": "Policy override for this group. Omit to inherit the top-level group_policy.",
                    "enum": ["open", "allowlist", "disabled"],
                    "type": "string"
                },
                "allowed_users": {
                    "default": [],
                    "description": "Per-group allowlist. Empty array falls back to the top-level allowed_users.",
                    "items": { "oneOf": [{ "type": "string" }, { "type": "integer" }] },
                    "type": "array"
                },
                "require_mention": {
                    "default": false,
                    "description": "Whether the bot must be @mentioned in this group.",
                    "type": "boolean"
                }
            },
            "type": "object"
        }),
    );

    // Insert TelegramAccessControlConfig
    defs.insert(
        "TelegramAccessControlConfig".to_string(),
        serde_json::json!({
            "description": "Telegram channel access control. Controls which users and groups can interact with the bot.",
            "properties": {
                "group_policy": {
                    "default": "open",
                    "description": "Default group policy. 'open' allows anyone, 'allowlist' restricts to allowed_users, 'disabled' blocks all groups.",
                    "enum": ["open", "allowlist", "disabled"],
                    "type": "string"
                },
                "allowed_users": {
                    "default": ["*"],
                    "description": "User allowlist for DMs and groups (when group_policy is 'allowlist'). Use '*' for everyone, '@username' or a numeric Telegram user ID. Omitting defaults to ['*']; an explicit empty array blocks all users.",
                    "items": { "oneOf": [{ "type": "string" }, { "type": "integer" }] },
                    "type": "array"
                },
                "require_mention": {
                    "default": false,
                    "description": "Whether the bot must be @mentioned in groups. Overridable per-group.",
                    "type": "boolean"
                },
                "groups": {
                    "default": {},
                    "description": "Per-group overrides keyed by chat ID string (e.g. '-100123456789').",
                    "additionalProperties": { "$ref": "#/$defs/TelegramAccessControlGroupConfig" },
                    "type": "object"
                }
            },
            "type": "object"
        }),
    );

    // Navigate to $defs/ChannelConfig/properties/options and inject access_control property
    if let Some(channel_options) = schema
        .as_object_mut()
        .and_then(|s| s.get_mut("$defs"))
        .and_then(|d| d.as_object_mut())
        .and_then(|d| d.get_mut("ChannelConfig"))
        .and_then(|c| c.as_object_mut())
        .and_then(|c| c.get_mut("properties"))
        .and_then(|p| p.as_object_mut())
        .and_then(|p| p.get_mut("options"))
        .and_then(|o| o.as_object_mut())
    {
        channel_options.insert(
            "properties".to_string(),
            serde_json::json!({
                "access_control": {
                    "$ref": "#/$defs/TelegramAccessControlConfig",
                    "description": "Access control configuration (Telegram channel)."
                }
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::generate_schema;
    use rstest::rstest;

    #[rstest]
    fn when_overlay_applied_then_telegram_access_control_def_exists() {
        let schema = generate_schema();
        let defs = schema["$defs"]
            .as_object()
            .expect("$defs must be an object");
        assert!(
            defs.contains_key("TelegramAccessControlConfig"),
            "$defs missing TelegramAccessControlConfig"
        );
        assert!(
            defs.contains_key("TelegramAccessControlGroupConfig"),
            "$defs missing TelegramAccessControlGroupConfig"
        );
    }

    #[rstest]
    fn when_overlay_applied_then_channel_options_references_access_control() {
        let schema = generate_schema();
        let access_control = &schema["$defs"]["ChannelConfig"]["properties"]["options"]["properties"]
            ["access_control"];
        assert!(
            access_control.is_object(),
            "ChannelConfig.options.properties.access_control must be an object"
        );
        assert!(
            access_control["$ref"].is_string(),
            "access_control must have a $ref"
        );
    }
}
