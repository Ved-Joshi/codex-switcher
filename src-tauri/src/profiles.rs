use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) custom_name: Option<String>,
}

fn profiles_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Codex Switcher/probe/profiles")
}

pub(crate) fn list(home: &Path) -> Result<Vec<Profile>, String> {
    let dir = profiles_dir(home);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(&dir).map_err(|_| "Could not inspect account profiles.")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Account profile directory is invalid.".into());
    }
    let mut profiles = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|_| "Could not list account profiles.")? {
        let entry = entry.map_err(|_| "Could not read an account profile.")?;
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if !crate::desktop::valid_profile_id(&id) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|_| "Could not inspect an account profile.")?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            profiles.push(Profile { label: label(&id), id, custom_name: None });
        }
    }
    profiles.sort_by_key(|profile| sort_key(&profile.id));
    Ok(profiles)
}

fn sort_key(id: &str) -> u64 {
    match id {
        "A" => 1,
        "B" => 2,
        _ => id.strip_prefix("account-").and_then(|value| value.parse().ok()).unwrap_or(u64::MAX),
    }
}

fn label(id: &str) -> String {
    match id {
        "A" => "Account A".into(),
        "B" => "Account B".into(),
        _ => format!("Account {}", id.trim_start_matches("account-")),
    }
}

fn marker(home: &Path, id: &str, name: &str) -> PathBuf {
    profiles_dir(home).join(id).join(name)
}

pub(crate) fn is_connected(home: &Path, id: &str) -> bool {
    !is_removed(home, id) && fs::symlink_metadata(marker(home, id, ".connected"))
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

pub(crate) fn is_removed(home: &Path, id: &str) -> bool {
    fs::symlink_metadata(marker(home, id, ".removed"))
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

fn is_pending(home: &Path, id: &str) -> bool {
    !is_removed(home, id) && !is_connected(home, id) && fs::symlink_metadata(marker(home, id, ".pending"))
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

pub(crate) fn quarantine_pending_duplicate(home: &Path, id: &str) -> Result<(), String> {
    create_marker(home, id, ".removed")
}

fn create_marker(home: &Path, id: &str, name: &str) -> Result<(), String> {
    let path = marker(home, id, name);
    match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink()) {
                Ok(())
            } else {
                Err("Account state marker is invalid.".into())
            }
        }
        Err(_) => Err("Could not save account setup state.".into()),
    }
}

pub(crate) fn mark_pending(home: &Path, id: &str) -> Result<(), String> {
    create_marker(home, id, ".pending")
}

pub(crate) fn mark_connected(home: &Path, id: &str) -> Result<(), String> {
    create_marker(home, id, ".connected")?;
    let pending = marker(home, id, ".pending");
    if pending.exists() {
        fs::remove_file(pending).map_err(|_| "Could not finish account setup.".to_string())?;
    }
    Ok(())
}

pub(crate) fn mark_removed(home: &Path, id: &str) -> Result<(), String> {
    if !is_connected(home, id) {
        return Err("This account is not connected.".into());
    }
    create_marker(home, id, ".removed")
}

pub(crate) fn restore(home: &Path, id: &str) -> Result<(), String> {
    if !is_removed(home, id) {
        return Err("This account is not removed.".into());
    }
    fs::remove_file(marker(home, id, ".removed"))
        .map_err(|_| "Could not restore this account.".into())
}

#[cfg(target_os = "macos")]
pub(crate) fn add(home: &Path) -> Result<Profile, String> {
    let dir = profiles_dir(home);
    if let Some(profile) = list(home)?.into_iter().find(|profile| is_pending(home, &profile.id)) {
        return Ok(profile);
    }
    for id in ["A", "B"] {
        if fs::symlink_metadata(dir.join(id)).is_err() {
            crate::desktop::prepare_profile(home, id)?;
            mark_pending(home, id)?;
            return Ok(Profile { label: label(id), id: id.into(), custom_name: None });
        }
    }
    for number in 3..=u64::MAX {
        let id = format!("account-{number}");
        if fs::symlink_metadata(dir.join(&id)).is_err() {
            crate::desktop::prepare_profile(home, &id)?;
            mark_pending(home, &id)?;
            return Ok(Profile { label: label(&id), id, custom_name: None });
        }
    }
    Err("Could not allocate another account profile.".into())
}

#[cfg(test)]
mod tests {
    use super::{is_connected, is_pending, is_removed, label, mark_connected, mark_pending, mark_removed, profiles_dir, quarantine_pending_duplicate, restore, sort_key};
    #[test]
    fn legacy_accounts_precede_numbered_accounts() {
        assert!(sort_key("A") < sort_key("B"));
        assert!(sort_key("B") < sort_key("account-3"));
        assert_eq!(label("account-24"), "Account 24");
    }

    #[test]
    fn unfinished_account_stays_pending_until_connected() {
        let home = std::env::temp_dir().join(format!("codex-switcher-profiles-{}", std::process::id()));
        let account = profiles_dir(&home).join("account-3");
        std::fs::create_dir_all(&account).unwrap();
        mark_pending(&home, "account-3").unwrap();
        assert!(is_pending(&home, "account-3"));
        assert!(!is_connected(&home, "account-3"));
        mark_connected(&home, "account-3").unwrap();
        assert!(!is_pending(&home, "account-3"));
        assert!(is_connected(&home, "account-3"));
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn removed_account_is_hidden_and_can_be_restored_without_deleting_data() {
        let home = std::env::temp_dir().join(format!("codex-switcher-removal-{}", std::process::id()));
        let account = profiles_dir(&home).join("account-3");
        std::fs::create_dir_all(&account).unwrap();
        let retained_data = account.join("history.txt");
        std::fs::write(&retained_data, "kept").unwrap();
        mark_connected(&home, "account-3").unwrap();
        mark_removed(&home, "account-3").unwrap();
        assert!(is_removed(&home, "account-3"));
        assert!(!is_connected(&home, "account-3"));
        restore(&home, "account-3").unwrap();
        assert!(is_connected(&home, "account-3"));
        assert_eq!(std::fs::read_to_string(retained_data).unwrap(), "kept");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn duplicate_pending_profile_is_not_reused_for_another_connection() {
        let home = std::env::temp_dir().join(format!("codex-switcher-duplicate-{}", std::process::id()));
        let account = profiles_dir(&home).join("account-3");
        std::fs::create_dir_all(&account).unwrap();
        mark_pending(&home, "account-3").unwrap();
        quarantine_pending_duplicate(&home, "account-3").unwrap();
        assert!(is_removed(&home, "account-3"));
        assert!(!is_pending(&home, "account-3"));
        restore(&home, "account-3").unwrap();
        assert!(is_pending(&home, "account-3"));
        std::fs::remove_dir_all(home).unwrap();
    }
}
