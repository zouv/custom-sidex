//! Centralized application directory resolution for SideX.
//!
//! On Linux, follows the XDG Base Directory Specification:
//!   - Config/user data: `$XDG_CONFIG_HOME/SideX` (default `~/.config/SideX`)
//!
//! On macOS:
//!   - `~/Library/Application Support/SideX`
//!
//! On Windows:
//!   - `%APPDATA%/SideX`
//!
//! This matches the convention used by VSCode (`~/.config/Code`),
//! Cursor (`~/.config/Cursor`), and VSCodium (`~/.config/VSCodium`).

use std::path::PathBuf;

const APP_NAME: &str = "SideX";

/// Returns the primary application data directory for SideX.
///
/// This is where settings.json, databases, and other user data live.
/// The path follows platform conventions and respects XDG on Linux.
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join(APP_NAME)
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("Application Support")
            .join(APP_NAME)
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }
}

/// Legacy application data directory path (used before XDG migration).
///
/// On Linux this was `~/.local/share/com.siden.sidex`.
/// On macOS this was `~/Library/Application Support/com.siden.sidex`.
/// On Windows this was `%LOCALAPPDATA%/com.siden.sidex`.
fn legacy_app_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local")
                    .join("share")
            });
        Some(base.join("com.siden.sidex"))
    }

    #[cfg(target_os = "macos")]
    {
        Some(
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join("com.siden.sidex"),
        )
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir().map(|d| d.join("com.siden.sidex"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Resolves the app data directory, migrating from legacy locations
/// if they exist and the new location does not.
///
/// Legacy locations checked (in order):
/// 1. Tauri's `app_data_dir()` path: `~/.local/share/com.siden.sidex/` (Linux)
/// 2. Old hard-coded path: `~/.sidex/` (all platforms)
///
/// Migration is atomic: we rename the entire directory. If the rename fails
/// (e.g., cross-device), we copy recursively instead.
pub fn resolve_and_migrate() -> PathBuf {
    let new_dir = app_data_dir();

    if new_dir.exists() {
        migrate_dot_sidex_into(&new_dir);
        return new_dir;
    }

    if let Some(legacy) = legacy_app_data_dir() {
        if legacy.exists() {
            if let Some(parent) = new_dir.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::rename(&legacy, &new_dir) {
                Ok(()) => {
                    log::info!(
                        "Migrated app data from {} to {}",
                        legacy.display(),
                        new_dir.display()
                    );
                    migrate_dot_sidex_into(&new_dir);
                    return new_dir;
                }
                Err(e) => {
                    log::warn!(
                        "Could not migrate app data from {} to {}: {e}. \
                         Attempting copy instead.",
                        legacy.display(),
                        new_dir.display()
                    );
                    if copy_dir_recursive(&legacy, &new_dir).is_ok() {
                        log::info!(
                            "Copied app data from {} to {}",
                            legacy.display(),
                            new_dir.display()
                        );
                        migrate_dot_sidex_into(&new_dir);
                        return new_dir;
                    }
                    log::warn!("Copy also failed; using new directory path anyway.");
                }
            }
        }
    }

    let _ = std::fs::create_dir_all(&new_dir);
    migrate_dot_sidex_into(&new_dir);
    new_dir
}

/// Migrates contents from `~/.sidex/` into the new config directory if
/// the old directory exists and contains files not yet present in the
/// new location.
fn migrate_dot_sidex_into(new_dir: &std::path::Path) {
    let dot_sidex = match dirs::home_dir() {
        Some(h) => h.join(".sidex"),
        None => return,
    };

    if !dot_sidex.exists() || dot_sidex == *new_dir {
        return;
    }

    let dominated_files = ["hooks.json", "mcp.json", "state.db"];
    for file in &dominated_files {
        let src = dot_sidex.join(file);
        let dst = new_dir.join(file);
        if src.exists() && !dst.exists() {
            let _ = std::fs::create_dir_all(new_dir);
            if let Err(e) = std::fs::copy(&src, &dst) {
                log::warn!("Failed to migrate {}: {e}", src.display());
            } else {
                log::info!("Migrated {} to {}", src.display(), dst.display());
            }
        }
    }

    let dominated_dirs = ["extensions", "data"];
    for dir_name in &dominated_dirs {
        let src = dot_sidex.join(dir_name);
        let dst = new_dir.join(dir_name);
        if src.is_dir() && !dst.exists() {
            let _ = std::fs::create_dir_all(new_dir);
            if let Err(e) = copy_dir_recursive(&src, &dst) {
                log::warn!("Failed to migrate {}: {e}", src.display());
            } else {
                log::info!("Migrated {} to {}", src.display(), dst.display());
            }
        }
    }
}

// [CUSTOM-BEGIN] CUSTOM-20261013-010 - migrate legacy user data and settings without overwrites
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    copy_dir_recursive_skipping(src, dst, None)
}

fn copy_dir_recursive_skipping(
    src: &std::path::Path,
    dst: &std::path::Path,
    skip_file: Option<&std::path::Path>,
) -> std::io::Result<()> {
    if src == dst {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let source_path = entry.path();
        let dest_path = dst.join(entry.file_name());
        // [CUSTOM-BEGIN] CUSTOM-20261013-010 - do not overwrite canonical files during data migration
        if dest_path == src
            || dest_path.starts_with(src)
            || skip_file == Some(source_path.as_path())
        {
            continue;
        }
        // [CUSTOM-END] CUSTOM-20261013-010
        if file_type.is_dir() {
            if dest_path.exists() && !dest_path.is_dir() {
                continue;
            }
            copy_dir_recursive_skipping(&source_path, &dest_path, skip_file)?;
        } else if !dest_path.exists() {
            std::fs::copy(source_path, dest_path)?;
        }
    }
    Ok(())
}

/// Copy user-data files into the canonical app-data root without replacing
/// anything that already exists there. Keep the source tree as a recovery copy.
// [CUSTOM-BEGIN] CUSTOM-20261013-010 - migrate legacy user data without overwrites
pub(crate) fn migrate_user_data_into_skipping(
    src: &std::path::Path,
    dst: &std::path::Path,
    skip_file: Option<&std::path::Path>,
) {
    if !src.is_dir() || src == dst {
        return;
    }

    if let Err(e) = copy_dir_recursive_skipping(src, dst, skip_file) {
        log::warn!(
            "Could not fully migrate user data from {} to {}: {e}",
            src.display(),
            dst.display()
        );
    }
}
// [CUSTOM-END] CUSTOM-20261013-010

/// Migrate a settings file if the canonical target does not exist. Invalid
/// JSONC is left untouched so it can be recovered manually.
// [CUSTOM-BEGIN] CUSTOM-20261013-010 - migrate legacy settings safely without overwrites
pub(crate) fn migrate_settings_file(src: &std::path::Path, dst: &std::path::Path) {
    if dst.exists() || !src.is_file() {
        return;
    }

    let contents = match std::fs::read_to_string(src) {
        Ok(contents) => contents,
        Err(e) => {
            log::warn!("Could not read legacy settings {}: {e}", src.display());
            return;
        }
    };
    if let Err(e) = sidex_settings::parse_jsonc(&contents) {
        log::warn!(
            "Not migrating invalid legacy settings {}: {e}",
            src.display()
        );
        return;
    }
    let Some(parent) = dst.parent() else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(parent) {
        log::warn!(
            "Could not create settings directory {}: {e}",
            parent.display()
        );
        return;
    }
    let tmp = dst.with_extension(format!(
        "json.migrate.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    if let Err(e) = std::fs::write(&tmp, contents) {
        log::warn!("Could not stage legacy settings {}: {e}", src.display());
        return;
    }
    if let Err(e) = std::fs::hard_link(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            log::warn!(
                "Could not migrate settings {} to {}: {e}",
                src.display(),
                dst.display()
            );
        }
        return;
    }
    let _ = std::fs::remove_file(&tmp);
    log::info!(
        "Migrated settings from {} to {}",
        src.display(),
        dst.display()
    );
}
// [CUSTOM-END] CUSTOM-20261013-010
#[cfg(test)]
mod tests {
    use super::{migrate_settings_file, migrate_user_data_into_skipping};
    use std::path::Path;

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let unique = format!(
            "sidex-user-data-migration-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }

    #[test]
    fn copies_missing_user_data_without_replacing_existing_files() {
        let source = temp_dir("source");
        let target = temp_dir("target");
        std::fs::create_dir_all(source.join("User")).unwrap();
        std::fs::create_dir_all(target.join("User")).unwrap();
        std::fs::write(
            source.join("User/settings.json"),
            r#"{"vac.customModels":[]}"#,
        )
        .unwrap();
        std::fs::write(source.join("User/keybindings.json"), "{}").unwrap();
        std::fs::write(target.join("User/settings.json"), "target settings").unwrap();

        migrate_user_data_into_skipping(&source, &target, None);

        assert_eq!(
            std::fs::read_to_string(target.join("User/settings.json")).unwrap(),
            "target settings"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("User/keybindings.json")).unwrap(),
            "{}"
        );
        assert_eq!(
            std::fs::read_to_string(source.join("User/settings.json")).unwrap(),
            r#"{"vac.customModels":[]}"#
        );
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(target);
    }

    #[test]
    fn migrates_valid_jsonc_without_rewriting_and_keeps_source() {
        let root = temp_dir("settings");
        let source = root.join("legacy/settings.json");
        let target = root.join("new/User/settings.json");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        let contents = r#"{
  // custom models
  "vac.customModels": [],
}
"#;
        std::fs::write(&source, contents).unwrap();

        migrate_settings_file(&source, &target);

        assert_eq!(std::fs::read_to_string(&target).unwrap(), contents);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), contents);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn does_not_overwrite_target_or_migrate_invalid_jsonc() {
        let root = temp_dir("settings-conflict");
        let source = root.join("legacy/settings.json");
        let target = root.join("new/User/settings.json");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&source, "not json").unwrap();
        std::fs::write(&target, "target settings").unwrap();

        migrate_settings_file(&source, &target);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target settings");

        std::fs::remove_file(&target).unwrap();
        migrate_settings_file(&source, &target);
        assert!(!target.exists());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "not json");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_missing_source() {
        let target = temp_dir("missing-target");
        migrate_user_data_into_skipping(Path::new("missing-user-data-source"), &target, None);
        assert!(!target.exists());
    }
}
