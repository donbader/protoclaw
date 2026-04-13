use anyclaw_sdk_types::PermissionOption;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::state::SharedState;

pub fn build_permission_keyboard(
    request_id: &str,
    options: &[PermissionOption],
) -> InlineKeyboardMarkup {
    let buttons: Vec<InlineKeyboardButton> = options
        .iter()
        .map(|opt| {
            let mut callback_data = format!("{}:{}", request_id, opt.option_id);
            if callback_data.len() > 64 {
                let max_req_len = 64 - 1 - opt.option_id.len();
                let truncated_req: String = request_id.chars().take(max_req_len).collect();
                callback_data = format!("{}:{}", truncated_req, opt.option_id);
            }
            InlineKeyboardButton::callback(&opt.label, callback_data)
        })
        .collect();
    InlineKeyboardMarkup::new(vec![buttons])
}

pub fn parse_callback_data(data: &str) -> Option<(&str, &str)> {
    data.rfind(':').map(|i| (&data[..i], &data[i + 1..]))
}

pub async fn process_callback(request_id: &str, option_id: &str, state: &SharedState) {
    tracing::info!(%request_id, %option_id, "permission broker resolving");
    state
        .permission_broker
        .lock()
        .await
        .resolve(request_id, option_id);
    state.permission_messages.lock().await.remove(request_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_keyboard_with_two_options_returns_one_row_two_buttons() {
        let options = vec![
            PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            },
            PermissionOption {
                option_id: "deny".into(),
                label: "Deny".into(),
            },
        ];
        let kb = build_permission_keyboard("req-1", &options);
        assert_eq!(kb.inline_keyboard.len(), 1);
        assert_eq!(kb.inline_keyboard[0].len(), 2);
    }

    #[test]
    fn build_keyboard_button_labels_match_options() {
        let options = vec![
            PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            },
            PermissionOption {
                option_id: "deny".into(),
                label: "Deny".into(),
            },
        ];
        let kb = build_permission_keyboard("req-1", &options);
        assert_eq!(kb.inline_keyboard[0][0].text, "Allow");
        assert_eq!(kb.inline_keyboard[0][1].text, "Deny");
    }

    #[test]
    fn build_keyboard_callback_data_format() {
        let options = vec![PermissionOption {
            option_id: "allow".into(),
            label: "Allow".into(),
        }];
        let kb = build_permission_keyboard("req-1", &options);
        let btn = &kb.inline_keyboard[0][0];
        match &btn.kind {
            teloxide::types::InlineKeyboardButtonKind::CallbackData(data) => {
                assert_eq!(data, "req-1:allow");
            }
            _ => panic!("expected CallbackData"),
        }
    }

    #[test]
    fn build_keyboard_truncates_long_callback_data() {
        let long_id = "a".repeat(100);
        let options = vec![PermissionOption {
            option_id: "ok".into(),
            label: "OK".into(),
        }];
        let kb = build_permission_keyboard(&long_id, &options);
        let btn = &kb.inline_keyboard[0][0];
        match &btn.kind {
            teloxide::types::InlineKeyboardButtonKind::CallbackData(data) => {
                assert!(data.len() <= 64);
                assert!(data.ends_with(":ok"));
            }
            _ => panic!("expected CallbackData"),
        }
    }

    #[test]
    fn parse_callback_data_simple() {
        assert_eq!(parse_callback_data("req-1:allow"), Some(("req-1", "allow")));
    }

    #[test]
    fn parse_callback_data_no_colon_returns_none() {
        assert_eq!(parse_callback_data("invalid"), None);
    }

    #[test]
    fn parse_callback_data_splits_on_last_colon() {
        assert_eq!(
            parse_callback_data("req:with:colons:allow"),
            Some(("req:with:colons", "allow"))
        );
    }

    #[tokio::test]
    async fn process_callback_resolves_oneshot() {
        let state = SharedState::new();
        let rx = state.permission_broker.lock().await.register("req-1");

        process_callback("req-1", "allow", &state).await;

        let resp = rx.await.unwrap();
        assert_eq!(resp.request_id, "req-1");
        assert_eq!(resp.option_id, "allow");
    }

    #[tokio::test]
    async fn process_callback_unknown_request_does_nothing() {
        let state = SharedState::new();
        process_callback("unknown", "allow", &state).await;
    }

    #[tokio::test]
    async fn process_callback_removes_resolver_after_resolving() {
        let state = SharedState::new();
        let _rx = state.permission_broker.lock().await.register("req-1");

        process_callback("req-1", "allow", &state).await;

        assert!(
            !state
                .permission_broker
                .lock()
                .await
                .resolve("req-1", "allow")
        );
    }

    #[tokio::test]
    async fn process_callback_removes_permission_message() {
        let state = SharedState::new();
        let _rx = state.permission_broker.lock().await.register("req-1");
        state
            .permission_messages
            .lock()
            .await
            .insert("req-1".into(), (12345, 99));

        process_callback("req-1", "allow", &state).await;

        assert!(
            state
                .permission_messages
                .lock()
                .await
                .get("req-1")
                .is_none()
        );
    }
}
