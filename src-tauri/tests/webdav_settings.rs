use cc_switch_lib::{
    get_webdav_sync_settings, set_webdav_sync_settings, webdav_jianguoyun_preset,
    WebDavSyncSettings, WebDavSyncStatus,
};

#[path = "support.rs"]
mod support;
use support::{ensure_test_home, lock_test_mutex, reset_test_fs};

fn sample_settings() -> WebDavSyncSettings {
    WebDavSyncSettings {
        enabled: true,
        base_url: "https://dav.example.com/remote.php/dav/files/user".to_string(),
        remote_root: " cc-switch-sync ".to_string(),
        profile: " default ".to_string(),
        username: "user@example.com".to_string(),
        password: "app-password".to_string(),
        auto_sync: false,
        status: WebDavSyncStatus::default(),
    }
}

#[test]
fn set_webdav_sync_settings_rejects_invalid_base_url() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "ftp://invalid.example.com".to_string();

    let err = set_webdav_sync_settings(Some(settings))
        .expect_err("invalid non-http(s) base url should be rejected");
    assert!(
        err.to_string().contains("WebDAV") || err.to_string().to_lowercase().contains("base_url"),
        "unexpected error: {err}"
    );
}

#[test]
fn set_webdav_sync_settings_persists_and_normalizes_fields() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    set_webdav_sync_settings(Some(sample_settings())).expect("save webdav settings");

    let saved = get_webdav_sync_settings()
        .expect("settings should be present after writing")
        .clone();
    assert_eq!(
        saved.base_url,
        "https://dav.example.com/remote.php/dav/files/user"
    );
    assert_eq!(saved.remote_root, "cc-switch-sync");
    assert_eq!(saved.profile, "default");
}

#[test]
fn set_webdav_sync_settings_can_clear_config() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    set_webdav_sync_settings(Some(sample_settings())).expect("set webdav settings");
    set_webdav_sync_settings(None).expect("clear webdav settings");
    assert!(
        get_webdav_sync_settings().is_none(),
        "webdav settings should be removed after clearing"
    );
}

#[test]
fn set_webdav_sync_settings_allows_disabled_empty_base_url() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.enabled = false;
    settings.base_url = String::new();

    set_webdav_sync_settings(Some(settings)).expect("disabled webdav should allow empty base_url");
}

#[test]
fn set_webdav_sync_settings_rejects_jianguoyun_base_url_without_dav() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "https://dav.jianguoyun.com".to_string();

    let err = set_webdav_sync_settings(Some(settings))
        .expect_err("jianguoyun root without /dav should be rejected");
    assert!(err.to_string().contains("/dav"), "unexpected error: {err}");
}

#[test]
fn set_webdav_sync_settings_rejects_nutstore_base_url_without_dav() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "https://dav.nutstore.net".to_string();

    let err = set_webdav_sync_settings(Some(settings))
        .expect_err("nutstore root without /dav should be rejected");
    assert!(err.to_string().contains("/dav"), "unexpected error: {err}");
}

#[test]
fn set_webdav_sync_settings_accepts_jianguoyun_base_url_with_dav() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "https://dav.jianguoyun.com/dav/team-space".to_string();

    set_webdav_sync_settings(Some(settings)).expect("jianguoyun /dav url should be accepted");

    let saved = get_webdav_sync_settings().expect("settings should be present after writing");
    assert_eq!(saved.base_url, "https://dav.jianguoyun.com/dav/team-space");
}

#[test]
fn set_webdav_sync_settings_accepts_nutstore_base_url_with_dav() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "https://dav.nutstore.net/dav/team-space".to_string();

    set_webdav_sync_settings(Some(settings)).expect("nutstore /dav url should be accepted");

    let saved = get_webdav_sync_settings().expect("settings should be present after writing");
    assert_eq!(saved.base_url, "https://dav.nutstore.net/dav/team-space");
}

#[test]
fn set_webdav_sync_settings_accepts_generic_provider_without_dav() {
    let _guard = lock_test_mutex();
    reset_test_fs();
    let _home = ensure_test_home();

    let mut settings = sample_settings();
    settings.base_url = "https://webdav.example.com/files/user".to_string();

    set_webdav_sync_settings(Some(settings))
        .expect("generic WebDAV providers should not require /dav");

    let saved = get_webdav_sync_settings().expect("settings should be present after writing");
    assert_eq!(saved.base_url, "https://webdav.example.com/files/user");
}

#[test]
fn jianguoyun_preset_sets_expected_defaults() {
    let preset = webdav_jianguoyun_preset(" demo@nutstore.com ", " app-password ");
    assert!(preset.enabled, "preset should enable webdav sync");
    assert_eq!(preset.base_url, "https://dav.jianguoyun.com/dav");
    assert_eq!(preset.remote_root, "cc-switch-sync");
    assert_eq!(preset.profile, "default");
    assert_eq!(preset.username, "demo@nutstore.com");
    assert_eq!(preset.password, "app-password");
}
