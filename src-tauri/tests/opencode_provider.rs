use serde_json::json;
use std::str::FromStr;

use cc_switch_lib::{AppError, AppType, MultiAppConfig, Provider, ProviderService};

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, lock_test_mutex, reset_test_fs, state_from_config};

#[test]
fn opencode_add_syncs_all_providers_to_live_config() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let app_type = AppType::from_str("opencode").expect("opencode app type should parse");
    let state = state_from_config(MultiAppConfig::default());

    let first = Provider::with_id(
        "openai".to_string(),
        "OpenAI Compatible".to_string(),
        json!({
            "npm": "@ai-sdk/openai-compatible",
            "options": {
                "baseURL": "https://api.example.com/v1",
                "apiKey": "sk-first"
            },
            "models": {
                "gpt-4o": { "name": "GPT-4o" }
            }
        }),
        None,
    );

    let second = Provider::with_id(
        "anthropic".to_string(),
        "Anthropic".to_string(),
        json!({
            "npm": "@ai-sdk/anthropic",
            "options": {
                "apiKey": "sk-second"
            },
            "models": {
                "claude-3-7-sonnet": { "name": "Claude 3.7 Sonnet" }
            }
        }),
        None,
    );

    ProviderService::add(&state, app_type.clone(), first).expect("first add should succeed");
    ProviderService::add(&state, app_type, second).expect("second add should succeed");

    let opencode_path = home.join(".config").join("opencode").join("opencode.json");
    let live: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&opencode_path).expect("read opencode live config"),
    )
    .expect("parse opencode live config");

    let providers = live
        .get("provider")
        .and_then(|value| value.as_object())
        .expect("opencode config should contain provider map");

    assert!(providers.contains_key("openai"));
    assert!(providers.contains_key("anthropic"));
}

#[test]
fn openclaw_add_syncs_all_providers_to_live_config() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let app_type = AppType::from_str("openclaw").expect("openclaw app type should parse");
    let state = state_from_config(MultiAppConfig::default());

    let first = Provider::with_id(
        "openai".to_string(),
        "OpenAI Compatible".to_string(),
        json!({
            "api": "openai-responses",
            "apiKey": "sk-first",
            "baseUrl": "https://api.example.com/v1",
            "models": [
                { "id": "gpt-4.1", "name": "GPT-4.1", "contextWindow": 128000 }
            ]
        }),
        None,
    );

    let second = Provider::with_id(
        "anthropic".to_string(),
        "Anthropic".to_string(),
        json!({
            "apiKey": "sk-second",
            "baseUrl": "https://anthropic.example/v1",
            "models": [
                { "id": "claude-sonnet-4", "name": "Claude Sonnet 4", "contextWindow": 200000 }
            ]
        }),
        None,
    );

    ProviderService::add(&state, app_type.clone(), first).expect("first add should succeed");
    ProviderService::add(&state, app_type, second).expect("second add should succeed");

    let openclaw_path = home.join(".openclaw").join("openclaw.json");
    let live: serde_json::Value = json5::from_str(
        &std::fs::read_to_string(&openclaw_path).expect("read openclaw live config"),
    )
    .expect("parse openclaw live config");

    assert_eq!(live["models"]["mode"], "merge");
    let providers = live["models"]["providers"]
        .as_object()
        .expect("openclaw config should contain models.providers map");

    assert!(providers.contains_key("openai"));
    assert!(providers.contains_key("anthropic"));
}

#[test]
fn openclaw_add_rejects_non_provider_like_object_before_syncing_live_config() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let home = ensure_test_home();

    let app_type = AppType::from_str("openclaw").expect("openclaw app type should parse");
    let state = state_from_config(MultiAppConfig::default());

    let invalid = Provider::with_id(
        "broken".to_string(),
        "Broken".to_string(),
        json!({
            "foo": "bar"
        }),
        None,
    );

    let err = ProviderService::add(&state, app_type, invalid)
        .expect_err("invalid OpenClaw provider should be rejected before live sync");

    match err {
        AppError::Localized { key, .. } => {
            assert_eq!(key, "provider.openclaw.models.missing");
        }
        other => panic!("expected localized missing-openclaw-models error, got {other:?}"),
    }

    let guard = state.config.read().expect("read config after rejected add");
    let manager = guard
        .get_manager(&AppType::OpenClaw)
        .expect("openclaw manager after rejected add");
    assert!(
        !manager.providers.contains_key("broken"),
        "rejected OpenClaw add should not persist the invalid provider"
    );

    let openclaw_path = home.join(".openclaw").join("openclaw.json");
    assert!(
        !openclaw_path.exists(),
        "rejected OpenClaw add should not create or update live openclaw.json"
    );
}
