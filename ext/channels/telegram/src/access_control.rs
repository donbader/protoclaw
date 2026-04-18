use std::collections::HashMap;

/// A single entry in an allowlist — `"*"` allows everyone, `@username` matches by name,
/// numeric IDs match by stable Telegram user ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowlistEntry {
    Wildcard,
    UserId(i64),
    Username(String),
}

/// Identity of a Telegram sender, used for allowlist matching.
#[derive(Debug, Clone, Default)]
pub struct SenderIdentity {
    pub user_id: Option<i64>,
    pub username: Option<String>,
}

impl SenderIdentity {
    fn matches(&self, entry: &AllowlistEntry) -> bool {
        match entry {
            AllowlistEntry::Wildcard => true,
            AllowlistEntry::UserId(id) => self.user_id == Some(*id),
            AllowlistEntry::Username(name) => self
                .username
                .as_ref()
                .is_some_and(|u| u.eq_ignore_ascii_case(name)),
        }
    }

    fn matches_any(&self, entries: &[AllowlistEntry]) -> bool {
        entries.iter().any(|e| self.matches(e))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupPolicy {
    #[default]
    Open,
    Allowlist,
    Disabled,
}

#[derive(Debug, Clone, Default)]
pub struct GroupConfig {
    pub enabled: bool,
    pub group_policy: Option<GroupPolicy>,
    pub allowed_users: Vec<AllowlistEntry>,
    pub require_mention: bool,
}

#[derive(Debug, Clone)]
pub struct AccessConfig {
    pub group_policy: GroupPolicy,
    /// DM allowlist. Empty = allow all DMs.
    pub allowed_users: Vec<AllowlistEntry>,
    /// Group sender allowlist (used when `group_policy` is `Allowlist`).
    pub group_allowed_users: Vec<AllowlistEntry>,
    pub groups: HashMap<i64, GroupConfig>,
    pub default_require_mention: bool,
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self {
            group_policy: GroupPolicy::Open,
            allowed_users: vec![AllowlistEntry::Wildcard],
            group_allowed_users: vec![AllowlistEntry::Wildcard],
            groups: HashMap::new(),
            default_require_mention: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    Allow,
    Deny(DenyReason),
    SkipNoMention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    GroupDisabled,
    GroupPolicyDisabled,
    GroupSenderNotAllowed,
    DmSenderNotAllowed,
}

pub fn evaluate_access(
    cfg: &AccessConfig,
    chat_id: i64,
    sender: &SenderIdentity,
    is_group: bool,
    bot_mentioned: bool,
) -> AccessDecision {
    if !is_group {
        return evaluate_dm(cfg, sender);
    }
    evaluate_group(cfg, chat_id, sender, bot_mentioned)
}

fn evaluate_dm(cfg: &AccessConfig, sender: &SenderIdentity) -> AccessDecision {
    if sender.matches_any(&cfg.allowed_users) {
        AccessDecision::Allow
    } else {
        AccessDecision::Deny(DenyReason::DmSenderNotAllowed)
    }
}

fn evaluate_group(
    cfg: &AccessConfig,
    chat_id: i64,
    sender: &SenderIdentity,
    bot_mentioned: bool,
) -> AccessDecision {
    let group_cfg = cfg.groups.get(&chat_id);

    if let Some(gc) = group_cfg
        && !gc.enabled
    {
        return AccessDecision::Deny(DenyReason::GroupDisabled);
    }

    let effective_policy = group_cfg
        .and_then(|gc| gc.group_policy)
        .unwrap_or(cfg.group_policy);

    match effective_policy {
        GroupPolicy::Disabled => return AccessDecision::Deny(DenyReason::GroupPolicyDisabled),
        GroupPolicy::Allowlist => {
            let per_group = group_cfg
                .map(|gc| &gc.allowed_users)
                .filter(|u| !u.is_empty());
            let effective = per_group.unwrap_or(&cfg.group_allowed_users);

            if !sender.matches_any(effective) {
                return AccessDecision::Deny(DenyReason::GroupSenderNotAllowed);
            }
        }
        GroupPolicy::Open => {}
    }

    let require_mention = group_cfg
        .map(|gc| gc.require_mention)
        .unwrap_or(cfg.default_require_mention);
    if require_mention && !bot_mentioned {
        return AccessDecision::SkipNoMention;
    }

    AccessDecision::Allow
}

pub fn should_suppress_reply_context(
    cfg: &AccessConfig,
    chat_id: i64,
    reply_sender: &SenderIdentity,
    is_group: bool,
) -> bool {
    if !is_group {
        return false;
    }

    let group_cfg = cfg.groups.get(&chat_id);
    let effective_policy = group_cfg
        .and_then(|gc| gc.group_policy)
        .unwrap_or(cfg.group_policy);

    if effective_policy != GroupPolicy::Allowlist {
        return false;
    }

    let per_group = group_cfg
        .map(|gc| &gc.allowed_users)
        .filter(|u| !u.is_empty());
    let effective = per_group.unwrap_or(&cfg.group_allowed_users);

    !reply_sender.matches_any(effective)
}

fn parse_group_policy(s: &str) -> Option<GroupPolicy> {
    match s {
        "open" => Some(GroupPolicy::Open),
        "allowlist" => Some(GroupPolicy::Allowlist),
        "disabled" => Some(GroupPolicy::Disabled),
        _ => None,
    }
}

// D-03: options is HashMap<String, Value> — channel-specific config with channel-defined schemas
#[allow(clippy::disallowed_types)]
fn parse_allowlist_entry(value: &serde_json::Value) -> Option<AllowlistEntry> {
    if let Some(n) = value.as_i64() {
        return Some(AllowlistEntry::UserId(n));
    }
    if let Some(s) = value.as_str() {
        if s == "*" {
            return Some(AllowlistEntry::Wildcard);
        }
        if let Ok(n) = s.parse::<i64>() {
            return Some(AllowlistEntry::UserId(n));
        }
        let name = s.strip_prefix('@').unwrap_or(s);
        if !name.is_empty() {
            return Some(AllowlistEntry::Username(name.to_lowercase()));
        }
    }
    None
}

#[allow(clippy::disallowed_types)]
fn parse_allowlist(value: &serde_json::Value) -> Vec<AllowlistEntry> {
    value
        .as_array()
        .map(|arr| arr.iter().filter_map(parse_allowlist_entry).collect())
        .unwrap_or_default()
}

impl AccessConfig {
    // D-03: options is HashMap<String, Value> — channel-specific config with channel-defined schemas
    #[allow(clippy::disallowed_types)]
    pub fn from_options(options: &serde_json::Value) -> Self {
        let Some(ac) = options.get("access_control") else {
            return Self::default();
        };

        let group_policy = ac
            .get("group_policy")
            .and_then(|v| v.as_str())
            .and_then(parse_group_policy)
            .unwrap_or(GroupPolicy::Open);

        let allowed_users = ac
            .get("allowed_users")
            .map(parse_allowlist)
            .unwrap_or_default();

        let group_allowed_users = ac
            .get("group_allowed_users")
            .map(parse_allowlist)
            .unwrap_or_default();

        let default_require_mention = ac
            .get("require_mention")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let groups = ac
            .get("groups")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(key, val)| {
                        let chat_id: i64 = key.parse().ok()?;
                        let enabled = val
                            .get("enabled")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true);
                        let gp = val
                            .get("group_policy")
                            .and_then(|v| v.as_str())
                            .and_then(parse_group_policy);
                        let users = val
                            .get("allowed_users")
                            .map(parse_allowlist)
                            .unwrap_or_default();
                        let mention = val
                            .get("require_mention")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        Some((
                            chat_id,
                            GroupConfig {
                                enabled,
                                group_policy: gp,
                                allowed_users: users,
                                require_mention: mention,
                            },
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            group_policy,
            allowed_users,
            group_allowed_users,
            groups,
            default_require_mention,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn sender(user_id: Option<i64>, username: Option<&str>) -> SenderIdentity {
        SenderIdentity {
            user_id,
            username: username.map(String::from),
        }
    }

    fn given_default_config() -> AccessConfig {
        AccessConfig::default()
    }

    fn given_dm_allowlist_config(entries: Vec<AllowlistEntry>) -> AccessConfig {
        AccessConfig {
            allowed_users: entries,
            ..Default::default()
        }
    }

    fn given_group_allowlist_config(entries: Vec<AllowlistEntry>) -> AccessConfig {
        AccessConfig {
            group_policy: GroupPolicy::Allowlist,
            group_allowed_users: entries,
            ..Default::default()
        }
    }

    fn id(n: i64) -> AllowlistEntry {
        AllowlistEntry::UserId(n)
    }

    fn uname(s: &str) -> AllowlistEntry {
        AllowlistEntry::Username(s.to_lowercase())
    }

    // --- DM access ---

    #[rstest]
    fn when_dm_with_wildcard_then_allowed() {
        let cfg = given_default_config();
        let result = evaluate_access(&cfg, 100, &sender(Some(42), None), false, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_dm_with_empty_list_then_denied() {
        let cfg = given_dm_allowlist_config(vec![]);
        let result = evaluate_access(&cfg, 100, &sender(Some(42), None), false, false);
        assert_eq!(result, AccessDecision::Deny(DenyReason::DmSenderNotAllowed));
    }

    #[rstest]
    fn when_dm_with_sender_id_in_allowlist_then_allowed() {
        let cfg = given_dm_allowlist_config(vec![id(42), id(99)]);
        let result = evaluate_access(&cfg, 100, &sender(Some(42), None), false, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_dm_with_username_in_allowlist_then_allowed() {
        let cfg = given_dm_allowlist_config(vec![uname("alice")]);
        let result = evaluate_access(&cfg, 100, &sender(Some(42), Some("alice")), false, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_dm_username_match_is_case_insensitive() {
        let cfg = given_dm_allowlist_config(vec![uname("alice")]);
        let result = evaluate_access(&cfg, 100, &sender(Some(42), Some("Alice")), false, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_dm_with_sender_not_in_allowlist_then_denied() {
        let cfg = given_dm_allowlist_config(vec![id(99)]);
        let result = evaluate_access(&cfg, 100, &sender(Some(42), None), false, false);
        assert_eq!(result, AccessDecision::Deny(DenyReason::DmSenderNotAllowed));
    }

    #[rstest]
    fn when_dm_with_no_sender_identity_and_allowlist_set_then_denied() {
        let cfg = given_dm_allowlist_config(vec![id(99)]);
        let result = evaluate_access(&cfg, 100, &sender(None, None), false, false);
        assert_eq!(result, AccessDecision::Deny(DenyReason::DmSenderNotAllowed));
    }

    // --- Group access: open policy ---

    #[rstest]
    fn when_group_open_policy_then_allowed() {
        let cfg = given_default_config();
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    // --- Group access: disabled policy ---

    #[rstest]
    fn when_group_disabled_policy_then_denied() {
        let cfg = AccessConfig {
            group_policy: GroupPolicy::Disabled,
            ..Default::default()
        };
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(
            result,
            AccessDecision::Deny(DenyReason::GroupPolicyDisabled)
        );
    }

    // --- Group access: allowlist policy ---

    #[rstest]
    fn when_group_allowlist_sender_id_allowed_then_allowed() {
        let cfg = given_group_allowlist_config(vec![id(42), id(99)]);
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_group_allowlist_username_allowed_then_allowed() {
        let cfg = given_group_allowlist_config(vec![uname("bob")]);
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), Some("bob")), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_group_allowlist_sender_not_allowed_then_denied() {
        let cfg = given_group_allowlist_config(vec![id(99)]);
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(
            result,
            AccessDecision::Deny(DenyReason::GroupSenderNotAllowed)
        );
    }

    #[rstest]
    fn when_group_allowlist_empty_then_denied() {
        let cfg = given_group_allowlist_config(vec![]);
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(
            result,
            AccessDecision::Deny(DenyReason::GroupSenderNotAllowed)
        );
    }

    #[rstest]
    fn given_per_group_disabled_when_message_then_denied() {
        let mut cfg = given_default_config();
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: false,
                ..Default::default()
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::Deny(DenyReason::GroupDisabled));
    }

    #[rstest]
    fn given_per_group_allowlist_when_sender_allowed_then_allowed() {
        let mut cfg = AccessConfig {
            group_policy: GroupPolicy::Disabled,
            ..Default::default()
        };
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                group_policy: Some(GroupPolicy::Allowlist),
                allowed_users: vec![uname("alice")],
                require_mention: false,
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), Some("alice")), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn given_per_group_allowlist_when_sender_not_allowed_then_denied() {
        let mut cfg = given_default_config();
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                group_policy: Some(GroupPolicy::Allowlist),
                allowed_users: vec![id(99)],
                require_mention: false,
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(
            result,
            AccessDecision::Deny(DenyReason::GroupSenderNotAllowed)
        );
    }

    #[rstest]
    fn given_per_group_open_overrides_global_allowlist() {
        let mut cfg = given_group_allowlist_config(vec![id(99)]);
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                group_policy: Some(GroupPolicy::Open),
                allowed_users: vec![],
                require_mention: false,
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn given_per_group_with_empty_allowlist_falls_back_to_global() {
        let mut cfg = given_group_allowlist_config(vec![id(42)]);
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                group_policy: None,
                allowed_users: vec![],
                require_mention: false,
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_group_require_mention_and_not_mentioned_then_skip() {
        let cfg = AccessConfig {
            default_require_mention: true,
            ..Default::default()
        };
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::SkipNoMention);
    }

    #[rstest]
    fn when_group_require_mention_and_mentioned_then_allowed() {
        let cfg = AccessConfig {
            default_require_mention: true,
            ..Default::default()
        };
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, true);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn given_per_group_require_mention_overrides_default() {
        let mut cfg = AccessConfig {
            default_require_mention: false,
            ..Default::default()
        };
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                require_mention: true,
                ..Default::default()
            },
        );
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(result, AccessDecision::SkipNoMention);
    }

    #[rstest]
    fn when_dm_require_mention_is_ignored() {
        let cfg = AccessConfig {
            default_require_mention: true,
            ..Default::default()
        };
        let result = evaluate_access(&cfg, 100, &sender(Some(42), None), false, false);
        assert_eq!(result, AccessDecision::Allow);
    }

    #[rstest]
    fn when_mention_check_happens_after_allowlist_check() {
        let cfg = AccessConfig {
            group_policy: GroupPolicy::Allowlist,
            group_allowed_users: vec![id(99)],
            default_require_mention: true,
            ..Default::default()
        };
        let result = evaluate_access(&cfg, -100123, &sender(Some(42), None), true, false);
        assert_eq!(
            result,
            AccessDecision::Deny(DenyReason::GroupSenderNotAllowed)
        );
    }

    #[rstest]
    fn when_reply_sender_in_allowlist_then_no_suppression() {
        let cfg = given_group_allowlist_config(vec![id(42), id(55)]);
        assert!(!should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(Some(55), None),
            true
        ));
    }

    #[rstest]
    fn when_reply_sender_username_in_allowlist_then_no_suppression() {
        let cfg = given_group_allowlist_config(vec![uname("bob")]);
        assert!(!should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(None, Some("bob")),
            true
        ));
    }

    #[rstest]
    fn when_reply_sender_not_in_allowlist_then_suppress() {
        let cfg = given_group_allowlist_config(vec![id(42)]);
        assert!(should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(Some(55), None),
            true
        ));
    }

    #[rstest]
    fn when_reply_in_dm_then_no_suppression() {
        let cfg = given_dm_allowlist_config(vec![id(42)]);
        assert!(!should_suppress_reply_context(
            &cfg,
            100,
            &sender(Some(55), None),
            false
        ));
    }

    #[rstest]
    fn when_reply_sender_empty_and_allowlist_active_then_suppress() {
        let cfg = given_group_allowlist_config(vec![id(42)]);
        assert!(should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(None, None),
            true
        ));
    }

    #[rstest]
    fn when_group_open_policy_then_no_suppression() {
        let cfg = given_default_config();
        assert!(!should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(Some(55), None),
            true
        ));
    }

    #[rstest]
    fn given_per_group_allowlist_reply_sender_not_in_list_then_suppress() {
        let mut cfg = given_default_config();
        cfg.groups.insert(
            -100123,
            GroupConfig {
                enabled: true,
                group_policy: Some(GroupPolicy::Allowlist),
                allowed_users: vec![id(42)],
                require_mention: false,
            },
        );
        assert!(should_suppress_reply_context(
            &cfg,
            -100123,
            &sender(Some(55), None),
            true
        ));
    }

    #[rstest]
    fn when_from_options_empty_then_default_config() {
        let opts = serde_json::json!({});
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.group_policy, GroupPolicy::Open);
        assert_eq!(cfg.allowed_users, vec![AllowlistEntry::Wildcard]);
        assert_eq!(cfg.group_allowed_users, vec![AllowlistEntry::Wildcard]);
        assert!(cfg.groups.is_empty());
        assert!(!cfg.default_require_mention);
    }

    #[rstest]
    fn when_from_options_with_group_policy_disabled() {
        let opts = serde_json::json!({ "access_control": { "group_policy": "disabled" } });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.group_policy, GroupPolicy::Disabled);
    }

    #[rstest]
    fn when_from_options_with_numeric_ids() {
        let opts = serde_json::json!({
            "access_control": {
                "group_policy": "allowlist",
                "allowed_users": [42, 99],
                "group_allowed_users": [42]
            }
        });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.allowed_users, vec![id(42), id(99)]);
        assert_eq!(cfg.group_allowed_users, vec![id(42)]);
    }

    #[rstest]
    fn when_from_options_with_usernames() {
        let opts = serde_json::json!({
            "access_control": {
                "allowed_users": ["@Alice", "@bob", 123]
            }
        });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(
            cfg.allowed_users,
            vec![uname("alice"), uname("bob"), id(123)]
        );
    }

    #[rstest]
    fn when_from_options_with_string_numeric_ids() {
        let opts = serde_json::json!({
            "access_control": { "allowed_users": ["12345"] }
        });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.allowed_users, vec![id(12345)]);
    }

    #[rstest]
    fn when_from_options_with_per_group_config() {
        let opts = serde_json::json!({
            "access_control": {
                "groups": {
                    "-100123": {
                        "enabled": true,
                        "group_policy": "allowlist",
                        "allowed_users": ["@alice", 55],
                        "require_mention": true
                    },
                    "-100456": { "enabled": false }
                }
            }
        });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.groups.len(), 2);

        let g1 = cfg.groups.get(&-100123).unwrap();
        assert!(g1.enabled);
        assert_eq!(g1.group_policy, Some(GroupPolicy::Allowlist));
        assert_eq!(g1.allowed_users, vec![uname("alice"), id(55)]);
        assert!(g1.require_mention);

        let g2 = cfg.groups.get(&-100456).unwrap();
        assert!(!g2.enabled);
    }

    #[rstest]
    fn when_from_options_with_invalid_group_policy_then_defaults_to_open() {
        let opts = serde_json::json!({ "access_control": { "group_policy": "bogus" } });
        let cfg = AccessConfig::from_options(&opts);
        assert_eq!(cfg.group_policy, GroupPolicy::Open);
    }

    #[rstest]
    fn when_parse_allowlist_entry_with_at_prefix_strips_it() {
        let entry = parse_allowlist_entry(&serde_json::json!("@Corey")).unwrap();
        assert_eq!(entry, AllowlistEntry::Username("corey".into()));
    }

    #[rstest]
    fn when_parse_allowlist_entry_without_at_prefix_still_works() {
        let entry = parse_allowlist_entry(&serde_json::json!("alice")).unwrap();
        assert_eq!(entry, AllowlistEntry::Username("alice".into()));
    }
}
