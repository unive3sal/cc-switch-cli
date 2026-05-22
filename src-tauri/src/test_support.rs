use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, RwLock};

pub(crate) type TestHomeSettingsLock = MutexGuard<'static, ()>;

pub(crate) fn lock_test_home_and_settings() -> TestHomeSettingsLock {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn test_home_store() -> &'static RwLock<Option<PathBuf>> {
    static STORE: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(None))
}

pub(crate) fn set_test_home_override(path: Option<&Path>) {
    let mut guard = test_home_store()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = path.map(Path::to_path_buf);
}

pub(crate) fn test_home_override() -> Option<PathBuf> {
    test_home_store()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) struct TestEnvGuard {
    _lock: TestHomeSettingsLock,
    old_home: Option<OsString>,
    old_userprofile: Option<OsString>,
    old_cc_switch_config_dir: Option<OsString>,
    old_claude_config_dir: Option<OsString>,
    old_codex_home: Option<OsString>,
}

impl TestEnvGuard {
    pub(crate) fn isolated(home: &Path) -> Self {
        let lock = lock_test_home_and_settings();
        let old_home = std::env::var_os("HOME");
        let old_userprofile = std::env::var_os("USERPROFILE");
        let old_cc_switch_config_dir = std::env::var_os("CC_SWITCH_CONFIG_DIR");
        let old_claude_config_dir = std::env::var_os("CLAUDE_CONFIG_DIR");
        let old_codex_home = std::env::var_os("CODEX_HOME");

        std::env::set_var("HOME", home);
        std::env::set_var("USERPROFILE", home);
        std::env::set_var("CC_SWITCH_CONFIG_DIR", home.join(".cc-switch"));
        std::env::set_var("CLAUDE_CONFIG_DIR", home.join(".claude"));
        std::env::set_var("CODEX_HOME", home.join(".codex"));
        set_test_home_override(Some(home));
        crate::settings::reload_test_settings();

        Self {
            _lock: lock,
            old_home,
            old_userprofile,
            old_cc_switch_config_dir,
            old_claude_config_dir,
            old_codex_home,
        }
    }
}

impl Drop for TestEnvGuard {
    fn drop(&mut self) {
        restore_env("HOME", &self.old_home);
        restore_env("USERPROFILE", &self.old_userprofile);
        restore_env("CC_SWITCH_CONFIG_DIR", &self.old_cc_switch_config_dir);
        restore_env("CLAUDE_CONFIG_DIR", &self.old_claude_config_dir);
        restore_env("CODEX_HOME", &self.old_codex_home);
        set_test_home_override(self.old_home.as_deref().map(Path::new));
        crate::settings::reload_test_settings();
    }
}

pub(crate) fn restore_env(key: &str, value: &Option<OsString>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}
