use serde::Serialize;
use std::env;
use tauri::Manager;

#[derive(Debug, Serialize)]
pub struct OsInfo {
    pub platform: String,
    pub arch: String,
    pub hostname: String,
    pub homedir: String,
    pub tmpdir: String,
}

#[tauri::command]
pub fn get_os_info() -> OsInfo {
    OsInfo {
        platform: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        hostname: hostname::get().map_or_else(
            |_| "unknown".to_string(),
            |h| h.to_string_lossy().to_string(),
        ),
        homedir: dirs::home_dir().map_or_else(
            || "unknown".to_string(),
            |p| p.to_string_lossy().to_string(),
        ),
        tmpdir: env::temp_dir().to_string_lossy().to_string(),
    }
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn get_env(key: String) -> Option<String> {
    env::var(&key).ok()
}

const SENSITIVE_ENV_PATTERNS: &[&str] = &[
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "PRIVATE_KEY",
    "API_KEY",
    "APIKEY",
    "AUTH",
    "AWS_",
    "AZURE_",
    "GCP_",
    "GITHUB_TOKEN",
    "NPM_TOKEN",
    "DOCKER_PASSWORD",
    "SSH_",
    "GPG_",
];

#[tauri::command]
pub fn get_all_env() -> std::collections::HashMap<String, String> {
    env::vars()
        .filter(|(key, _)| {
            let upper = key.to_uppercase();
            !SENSITIVE_ENV_PATTERNS.iter().any(|p| upper.contains(p))
        })
        .collect()
}

#[tauri::command]
pub fn get_shell() -> String {
    env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            super::terminal::resolve_windows_shell()
        } else {
            "/bin/sh".to_string()
        }
    })
}

#[allow(clippy::needless_pass_by_value)]
// [CUSTOM-BEGIN] CUSTOM-20261013-008 - use SideX root for Workbench user data and migrate legacy files
#[tauri::command]
pub fn get_user_data_dir() -> Result<String, String> {
    let user_dir = crate::app_dirs::app_data_dir();
    std::fs::create_dir_all(&user_dir)
        .map_err(|e| format!("failed to create SideX app data dir: {e}"))?;
    Ok(user_dir.to_string_lossy().to_string())
}

pub(crate) fn migrate_legacy_user_data(app: &tauri::AppHandle, app_data: &std::path::Path) {
    let legacy_user_root = if cfg!(target_os = "linux") {
        Some(app_data.join("UserData").join("User"))
    } else {
        app.path()
            .app_data_dir()
            .ok()
            .map(|dir| dir.join("UserData").join("User"))
    };
    let legacy_rust_settings = app_data.join("UserData").join("User").join("settings.json");
    let canonical_settings = app_data.join("User").join("settings.json");

    if let Some(legacy_user_root) = legacy_user_root {
        let legacy_workbench_settings = legacy_user_root.join("settings.json");
        crate::app_dirs::migrate_user_data_into_skipping(
            &legacy_user_root,
            &app_data.join("User"),
            Some(&legacy_workbench_settings),
        );
        crate::app_dirs::migrate_settings_file(&legacy_workbench_settings, &canonical_settings);
    }

    crate::app_dirs::migrate_settings_file(&legacy_rust_settings, &canonical_settings);
    if canonical_settings.is_file() && legacy_rust_settings.is_file() {
        if let (Ok(current), Ok(legacy)) = (
            std::fs::read(&canonical_settings),
            std::fs::read(&legacy_rust_settings),
        ) {
            if current != legacy {
                log::warn!(
                    "Legacy settings files differ; using the canonical settings at {} and keeping {} unchanged",
                    canonical_settings.display(),
                    legacy_rust_settings.display()
                );
            }
        }
    }
}
// [CUSTOM-END] CUSTOM-20261013-008
