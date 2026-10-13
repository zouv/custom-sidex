use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde_json::Value;
use sidex_settings::{modify_jsonc, parse_jsonc, Settings};
use tauri::State;

pub struct SettingsStore {
    pub(crate) inner: RwLock<Settings>,
    /// Path of the user settings.json — set during app startup so user-scope
    /// updates can be persisted to disk (otherwise toggles vanish on restart).
    user_path: RwLock<Option<PathBuf>>,
}

impl SettingsStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Settings::new()),
            user_path: RwLock::new(None),
        }
    }

    pub fn load_user(&self, path: &std::path::Path) -> Result<(), String> {
        self.set_user_path(path);
        self.inner
            .write()
            .map_err(|e| e.to_string())?
            .load_user(path)
            .map_err(|e| e.to_string())
    }

    /// Record where the user settings file lives so later updates persist.
    pub fn set_user_path(&self, path: &std::path::Path) {
        if let Ok(mut p) = self.user_path.write() {
            *p = Some(path.to_path_buf());
        }
    }

    // [CUSTOM-BEGIN] CUSTOM-20261013-011 - refresh Rust settings reads from canonical file
    pub fn refresh_user_from_disk(&self) -> Result<(), String> {
        let path = self.user_path.read().map_err(|e| e.to_string())?.clone();
        if let Some(path) = path.filter(|path| path.is_file()) {
            self.load_user(&path)?;
        }
        Ok(())
    }

    // [CUSTOM-END] CUSTOM-20261013-011

    /// Update just one setting in the current user settings file, preserving
    /// unrelated keys and JSONC formatting where the key already exists.
    // [CUSTOM-BEGIN] CUSTOM-20261013-011 - persist single settings without replacing unrelated config
    fn persist_user_setting(&self, key: &str, value: &Value) -> Result<(), String> {
        let path = match self.user_path.read().map_err(|e| e.to_string())?.clone() {
            Some(p) => p,
            None => return Ok(()), // path unknown (tests/headless) — skip silently
        };
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => "{}".to_owned(),
            Err(e) => return Err(e.to_string()),
        };
        let updated = modify_jsonc(&contents, &[key], value).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension(format!(
            "json.tmp.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&tmp, updated).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })?;
        Ok(())
    }
    // [CUSTOM-END] CUSTOM-20261013-011

    pub fn load_workspace(&self, path: &std::path::Path) -> Result<(), String> {
        self.inner
            .write()
            .map_err(|e| e.to_string())?
            .load_workspace(path)
            .map_err(|e| e.to_string())
    }
}

/// Get settings.
///
/// - `section`: if provided, returns only the value for that key
/// - `scope`: one of `"user"`, `"workspace"`, or omitted/`"merged"` for the
///   full merged object across all layers
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_get(
    state: State<'_, Arc<SettingsStore>>,
    section: Option<String>,
    scope: Option<String>,
) -> Result<Value, String> {
    state.refresh_user_from_disk()?;
    let settings = state.inner.read().map_err(|e| e.to_string())?;

    if let Some(key) = section {
        return Ok(settings.get_raw(&key).cloned().unwrap_or(Value::Null));
    }

    match scope.as_deref() {
        Some("user") => Ok(settings.user_layer().clone()),
        Some("workspace") => Ok(settings.workspace_layer().clone()),
        Some("merged") | None => {
            let mut merged = serde_json::Map::new();
            // Collect all keys across layers via the builtin defaults as the
            // canonical key set, then overlay user/workspace via get_raw.
            let defaults = sidex_settings::builtin_defaults();
            if let Some(obj) = defaults.as_object() {
                for key in obj.keys() {
                    if let Some(val) = settings.get_raw(key) {
                        merged.insert(key.clone(), val.clone());
                    }
                }
            }
            Ok(Value::Object(merged))
        }
        Some(other) => Err(format!(
            "invalid scope '{other}': expected \"user\", \"workspace\", or \"merged\""
        )),
    }
}

/// Update a setting.
///
/// `scope` must be `"user"` or `"workspace"`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_update(
    state: State<'_, Arc<SettingsStore>>,
    key: String,
    value: Value,
    scope: String,
) -> Result<(), String> {
    {
        let mut settings = state.inner.write().map_err(|e| e.to_string())?;

        match scope.as_str() {
            "user" => settings.set(&key, value.clone()),
            "workspace" => settings.set_workspace(&key, value.clone()),
            _ => {
                return Err(format!(
                    "invalid scope '{scope}': expected \"user\" or \"workspace\""
                ))
            }
        }
    }

    // [CUSTOM-BEGIN] CUSTOM-20261013-011 - persist only user-scope setting changes
    if scope == "user" {
        if let Err(e) = state.persist_user_setting(&key, &value) {
            log::warn!("failed to persist user setting '{key}': {e}");
        }
    }
    // [CUSTOM-END] CUSTOM-20261013-011
    Ok(())
}

/// Load settings from a JSONC file into the specified layer.
///
/// `scope` must be `"user"` or `"workspace"`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_load(
    state: State<'_, Arc<SettingsStore>>,
    path: String,
    scope: String,
) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    match scope.as_str() {
        "user" => state.load_user(p),
        "workspace" => state.load_workspace(p),
        _ => Err(format!(
            "invalid scope '{scope}': expected \"user\" or \"workspace\""
        )),
    }
}

/// Parse a JSONC string (strips comments & trailing commas) and return the
/// resulting JSON value. Useful for the frontend to preview or validate
/// settings files.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_parse_jsonc(input: String) -> Result<Value, String> {
    parse_jsonc(&input).map_err(|e| e.to_string())
}

/// Edit a value inside a JSONC document by key-path, preserving surrounding
/// comments and formatting. Returns the modified JSONC string.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_modify_jsonc(
    input: String,
    path: Vec<String>,
    value: Value,
) -> Result<String, String> {
    let refs: Vec<&str> = path.iter().map(String::as_str).collect();
    modify_jsonc(&input, &refs, &value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::SettingsStore;

    #[test]
    fn user_update_preserves_unrelated_file_keys_and_jsonc_comments() {
        let root = std::env::temp_dir().join(format!(
            "sidex-settings-store-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let contents = r#"{
  // keep extension config
  "vac.agents": {"Agent": {"command": "agent"}},
  "editor.fontSize": 12
}
"#;
        std::fs::write(&path, contents).unwrap();

        let store = SettingsStore::new();
        store.load_user(&path).unwrap();
        store
            .inner
            .write()
            .unwrap()
            .set("editor.fontSize", serde_json::json!(18));
        store
            .persist_user_setting("editor.fontSize", &serde_json::json!(18))
            .unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("// keep extension config"));
        assert!(updated.contains("\"vac.agents\""));
        assert!(updated.contains("\"editor.fontSize\": 18"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn user_update_creates_settings_file_when_missing() {
        let root = std::env::temp_dir().join(format!(
            "sidex-settings-empty-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let store = SettingsStore::new();
        store.set_user_path(&path);
        store
            .persist_user_setting(
                "vac.customModels",
                &serde_json::json!([{"modelId":"custom"}]),
            )
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["vac.customModels"][0]["modelId"], "custom");
        let _ = std::fs::remove_dir_all(root);
    }
}
