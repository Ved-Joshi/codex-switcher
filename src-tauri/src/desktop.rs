use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventSendOptions, NSString};
#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::os::unix::fs::DirBuilderExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::thread;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
const CODEX_BUNDLE_ID: &str = "com.openai.codex";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OpenResult {
    Focused,
    Launched,
}

/// Launch inputs shared by the experiment and the future macOS desktop adapter.
/// Constructing a plan neither launches Codex nor verifies an account.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DesktopLaunchPlan {
    pub(crate) codex_home: PathBuf,
    pub(crate) electron_user_data: PathBuf,
}

impl DesktopLaunchPlan {
    pub(crate) fn for_profile(probe_root: &Path, profile: &str) -> Result<Self, &'static str> {
        if !valid_profile_id(profile) {
            return Err("Invalid account profile.");
        }

        let profile_root = probe_root.join("profiles").join(profile);
        Ok(Self {
            codex_home: profile_root.join("codex"),
            electron_user_data: profile_root.join("electron"),
        })
    }

    pub(crate) fn environment(&self) -> [(OsString, OsString); 1] {
        [(
            OsString::from("CODEX_HOME"),
            self.codex_home.as_os_str().to_owned(),
        )]
    }

    pub(crate) fn arguments(&self) -> [OsString; 1] {
        let mut argument = OsString::from("--user-data-dir=");
        argument.push(&self.electron_user_data);
        [argument]
    }
}

pub(crate) fn valid_profile_id(profile: &str) -> bool {
    if matches!(profile, "A" | "B") {
        return true;
    }
    profile
        .strip_prefix("account-")
        .is_some_and(|number| {
            !number.is_empty()
                && !number.starts_with('0')
                && number.bytes().all(|byte| byte.is_ascii_digit())
                && number.parse::<u64>().is_ok_and(|value| value >= 3)
        })
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare_profile(home: &Path, profile: &str) -> Result<(), String> {
    let root = home.join("Library/Application Support/Codex Switcher/probe");
    let plan = DesktopLaunchPlan::for_profile(&root, profile).map_err(str::to_owned)?;
    ensure_profile_paths(home, &root, &plan)
}

#[cfg(target_os = "macos")]
pub(crate) fn open_or_focus(home: &Path, profile: &str) -> Result<OpenResult, String> {
    let root = home.join("Library/Application Support/Codex Switcher/probe");
    let plan = DesktopLaunchPlan::for_profile(&root, profile).map_err(str::to_owned)?;
    ensure_profile_paths(home, &root, &plan)?;
    let bundle = resolve_codex_bundle(home)?;

    match matching_processes(&bundle, &plan)?[..] {
        [pid] => {
            focus_process(pid, &bundle, &plan)?;
            return Ok(OpenResult::Focused);
        }
        [] => {}
        _ => {
            return Err(format!(
                "More than one Codex process matches profile {profile}."
            ))
        }
    }

    let environment = plan.environment();
    let arguments = plan.arguments();
    let mut codex_setting = OsString::from("CODEX_HOME=");
    codex_setting.push(&environment[0].1);
    let output = Command::new("/usr/bin/open")
        .arg("-n")
        .arg("-a")
        .arg(&bundle)
        .arg("--env")
        .arg(codex_setting)
        .arg("--args")
        .arg(&arguments[0])
        .output()
        .map_err(|error| format!("Could not request a Codex launch: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "macOS could not launch Codex for profile {profile}."
        ));
    }

    for _ in 0..20 {
        match matching_processes(&bundle, &plan)?[..] {
            [_] => return Ok(OpenResult::Launched),
            [] => thread::sleep(Duration::from_millis(250)),
            _ => {
                return Err(format!(
                    "More than one Codex process matches profile {profile}."
                ))
            }
        }
    }
    Err(format!(
        "Codex opened, but no process matched profile {profile}. Account identity is unknown."
    ))
}

#[cfg(target_os = "macos")]
pub(crate) fn open_or_focus_default(home: &Path) -> Result<OpenResult, String> {
    let bundle = resolve_codex_bundle(home)?;
    match matching_default_processes(&bundle)?[..] {
        [pid] => {
            focus_default_process(pid, &bundle)?;
            return Ok(OpenResult::Focused);
        }
        [] => {}
        _ => return Err("More than one existing Codex process was found.".into()),
    }
    let output = Command::new("/usr/bin/open")
        .arg("-n")
        .arg("-a")
        .arg(&bundle)
        .env_remove("CODEX_HOME")
        .output()
        .map_err(|error| format!("Could not request a Codex launch: {error}"))?;
    if !output.status.success() {
        return Err("macOS could not launch the existing Codex profile.".into());
    }
    for _ in 0..20 {
        match matching_default_processes(&bundle)?[..] {
            [_] => return Ok(OpenResult::Launched),
            [] => thread::sleep(Duration::from_millis(250)),
            _ => return Err("More than one existing Codex process was found.".into()),
        }
    }
    Err("Codex opened, but the existing desktop process could not be identified.".into())
}

#[cfg(target_os = "macos")]
fn ensure_profile_paths(home: &Path, root: &Path, plan: &DesktopLaunchPlan) -> Result<(), String> {
    if !root.starts_with(home) {
        return Err("The profile root is outside the home directory.".into());
    }
    let switcher_root = root
        .parent()
        .ok_or_else(|| "The profile root has no parent.".to_string())?;
    let mut ancestor = switcher_root
        .parent()
        .ok_or_else(|| "The switcher root has no parent.".to_string())?
        .to_path_buf();
    while ancestor != home {
        let metadata = fs::symlink_metadata(&ancestor)
            .map_err(|_| format!("Missing profile directory: {}", ancestor.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!("Invalid profile directory: {}", ancestor.display()));
        }
        ancestor = ancestor
            .parent()
            .ok_or_else(|| "The profile root is outside the home directory.".to_string())?
            .to_path_buf();
    }
    for path in [
        switcher_root.to_path_buf(),
        root.to_path_buf(),
        root.join("profiles"),
        plan.codex_home.parent().unwrap().to_path_buf(),
        plan.codex_home.clone(),
        plan.electron_user_data.clone(),
    ] {
        if !path.exists() {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .map_err(|error| {
                    format!(
                        "Could not create profile directory {}: {error}",
                        path.display()
                    )
                })?;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| format!("Missing profile directory: {}.", path.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || (path != switcher_root && metadata.mode() & 0o077 != 0)
        {
            return Err(format!(
                "Profile directory is not owner only: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn resolve_codex_bundle(home: &Path) -> Result<PathBuf, String> {
    for (bundle, expected_executable) in [
        (PathBuf::from("/Applications/ChatGPT.app"), "ChatGPT"),
        (home.join("Applications/ChatGPT.app"), "ChatGPT"),
        (PathBuf::from("/Applications/Codex.app"), "Codex"),
        (home.join("Applications/Codex.app"), "Codex"),
    ] {
        if !bundle.is_dir() || bundle.is_symlink() {
            continue;
        }
        let info = bundle.join("Contents/Info.plist");
        let identifier = plist_value(&info, "CFBundleIdentifier");
        let executable = plist_value(&info, "CFBundleExecutable");
        if identifier.as_deref() == Ok(CODEX_BUNDLE_ID)
            && executable.as_deref() == Ok(expected_executable)
        {
            return bundle
                .canonicalize()
                .map_err(|error| format!("Could not resolve Codex app path: {error}"));
        }
    }
    Err("Codex desktop was not found in Applications.".into())
}

#[cfg(target_os = "macos")]
fn plist_value(info: &Path, key: &str) -> Result<String, String> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-extract")
        .arg(key)
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(info)
        .output()
        .map_err(|error| format!("Could not read Codex app metadata: {error}"))?;
    if !output.status.success() {
        return Err("Could not validate the Codex app bundle.".into());
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| "Codex app metadata is not valid text.".into())
}

#[cfg(target_os = "macos")]
fn matching_processes(bundle: &Path, plan: &DesktopLaunchPlan) -> Result<Vec<i32>, String> {
    let identifier = NSString::from_str(CODEX_BUNDLE_ID);
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&identifier);
    let marker = plan.arguments()[0].to_string_lossy().into_owned();
    let mut matches = Vec::new();
    for app in apps.iter() {
        let Some(url) = app.bundleURL() else { continue };
        let Some(path) = url.path() else { continue };
        let path = PathBuf::from(path.to_string());
        if path.canonicalize().ok().as_deref() != Some(bundle) {
            continue;
        }
        let pid = app.processIdentifier();
        let output = Command::new("/bin/ps")
            .arg("-ww")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("command=")
            .output()
            .map_err(|error| format!("Could not inspect Codex processes: {error}"))?;
        if !output.status.success() {
            continue;
        }
        let command = String::from_utf8_lossy(&output.stdout);
        if contains_exact_argument(&command, &marker) {
            matches.push(pid);
        }
    }
    matches.sort_unstable();
    Ok(matches)
}

#[cfg(target_os = "macos")]
fn matching_default_processes(bundle: &Path) -> Result<Vec<i32>, String> {
    let identifier = NSString::from_str(CODEX_BUNDLE_ID);
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&identifier);
    let mut matches = Vec::new();
    for app in apps.iter() {
        let Some(url) = app.bundleURL() else { continue };
        let Some(path) = url.path() else { continue };
        if PathBuf::from(path.to_string()).canonicalize().ok().as_deref() != Some(bundle) {
            continue;
        }
        let pid = app.processIdentifier();
        let output = Command::new("/bin/ps")
            .arg("-ww")
            .arg("-p")
            .arg(pid.to_string())
            .arg("-o")
            .arg("command=")
            .output()
            .map_err(|error| format!("Could not inspect Codex processes: {error}"))?;
        if !output.status.success() {
            continue;
        }
        let command = String::from_utf8_lossy(&output.stdout);
        if !has_user_data_argument(&command) {
            matches.push(pid);
        }
    }
    matches.sort_unstable();
    Ok(matches)
}

fn has_user_data_argument(command: &str) -> bool {
    command.split_whitespace().any(|part| part == "--user-data-dir" || part.starts_with("--user-data-dir="))
}

#[cfg(target_os = "macos")]
fn focus_process(pid: i32, bundle: &Path, plan: &DesktopLaunchPlan) -> Result<(), String> {
    if matching_processes(bundle, plan)? != [pid] {
        return Err("The matching Codex process changed before focus.".into());
    }
    activate_process(pid)
}

#[cfg(target_os = "macos")]
fn focus_default_process(pid: i32, bundle: &Path) -> Result<(), String> {
    if matching_default_processes(bundle)? != [pid] {
        return Err("The existing Codex process changed before focus.".into());
    }
    activate_process(pid)
}

#[cfg(target_os = "macos")]
fn activate_process(pid: i32) -> Result<(), String> {
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .ok_or_else(|| "The matching Codex process exited before focus.".to_string())?;
    // Adapted from ai-profiles' focus_pid: a windowless process needs the
    // standard reopen event, addressed to this exact PID, before activation.
    send_reopen_event(pid);
    if app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows) {
        Ok(())
    } else {
        Err("macOS could not focus the matching Codex process.".into())
    }
}

#[cfg(target_os = "macos")]
fn send_reopen_event(pid: i32) {
    const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
    const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");
    let target = NSAppleEventDescriptor::descriptorWithProcessIdentifier(pid);
    let event = NSAppleEventDescriptor::appleEventWithEventClass_eventID_targetDescriptor_returnID_transactionID(
        CORE_EVENT_CLASS,
        REOPEN_APPLICATION,
        Some(&target),
        -1,
        0,
    );
    let _ = event.sendEventWithOptions_timeout_error(NSAppleEventSendOptions::NoReply, 5.0);
}

fn contains_exact_argument(command: &str, argument: &str) -> bool {
    command.match_indices(argument).any(|(offset, _)| {
        let before = command[..offset].chars().last();
        let after = command[offset + argument.len()..].chars().next();
        before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::{has_user_data_argument, DesktopLaunchPlan};

    #[test]
    fn distinguishes_existing_desktop_from_isolated_profiles() {
        assert!(!has_user_data_argument("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT"));
        assert!(has_user_data_argument("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT --user-data-dir=/tmp/isolated profile"));
        assert!(has_user_data_argument("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT --user-data-dir /tmp/isolated"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires the existing Codex desktop process to be running"]
    fn focuses_existing_default_profile_without_launching_another() {
        let home = std::env::var_os("HOME").unwrap();
        let home = std::path::Path::new(&home);
        let bundle = super::resolve_codex_bundle(home).unwrap();
        let before = super::matching_default_processes(&bundle).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(super::open_or_focus_default(home).unwrap(), super::OpenResult::Focused);
        assert_eq!(super::matching_default_processes(&bundle).unwrap(), before);
    }
    use std::ffi::OsString;
    use std::path::Path;

    #[test]
    fn profiles_use_distinct_codex_and_electron_state() {
        let root = Path::new("/tmp/codex-switcher/probe");
        let a = DesktopLaunchPlan::for_profile(root, "A").unwrap();
        let b = DesktopLaunchPlan::for_profile(root, "B").unwrap();

        assert_eq!(a.codex_home, root.join("profiles/A/codex"));
        assert_eq!(a.electron_user_data, root.join("profiles/A/electron"));
        assert_ne!(a.codex_home, b.codex_home);
        assert_ne!(a.electron_user_data, b.electron_user_data);
        assert_eq!(a.environment()[0].0, OsString::from("CODEX_HOME"));
        assert_eq!(a.environment()[0].1, a.codex_home.as_os_str());
        assert_eq!(
            a.arguments()[0],
            OsString::from("--user-data-dir=/tmp/codex-switcher/probe/profiles/A/electron")
        );
    }

    #[test]
    fn arbitrary_profile_names_cannot_select_other_paths() {
        let root = Path::new("/tmp/codex-switcher/probe");
        for profile in ["", "C", "../A", "A/../B", "a", "account-0", "account-02", "account-3/evil", "account-18446744073709551616"] {
            assert!(DesktopLaunchPlan::for_profile(root, profile).is_err());
        }
        assert!(DesktopLaunchPlan::for_profile(root, "account-3").is_ok());
        assert!(DesktopLaunchPlan::for_profile(root, "account-1000").is_ok());
    }

    #[test]
    fn matches_only_a_complete_user_data_argument() {
        use super::contains_exact_argument;
        let marker =
            "--user-data-dir=/Library/Application Support/Codex Switcher/probe/profiles/A/electron";
        assert!(contains_exact_argument(
            &format!("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT {marker} --no-sandbox"),
            marker
        ));
        assert!(!contains_exact_argument(
            &format!("x {marker}/other"),
            marker
        ));
        assert!(!contains_exact_argument(
            &format!("x --fake={marker}"),
            marker
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn creates_private_profile_directories_without_probe_prepare() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let home = std::env::temp_dir().join(format!(
            "codex-switcher-directory-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let support = home.join("Library/Application Support");
        fs::create_dir_all(&support).unwrap();
        let root = support.join("Codex Switcher/probe");
        let plan = DesktopLaunchPlan::for_profile(&root, "A").unwrap();
        super::ensure_profile_paths(&home, &root, &plan).unwrap();
        for path in [&plan.codex_home, &plan.electron_user_data] {
            assert!(path.is_dir());
            assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
        }
        fs::set_permissions(root.parent().unwrap(), fs::Permissions::from_mode(0o755)).unwrap();
        super::ensure_profile_paths(&home, &root, &plan).unwrap();
        fs::remove_dir_all(&home).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires existing profile A and B Codex processes in the desktop session"]
    fn focuses_existing_profiles_without_launching_again() {
        let home = std::env::var_os("HOME").expect("HOME must be set");
        for profile in ["A", "B"] {
            let result = super::open_or_focus(Path::new(&home), profile).unwrap();
            assert_eq!(result, super::OpenResult::Focused);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires profile B to be stopped in the desktop session"]
    fn launches_stopped_profile_b() {
        let home = std::env::var_os("HOME").expect("HOME must be set");
        let result = super::open_or_focus(Path::new(&home), "B").unwrap();
        assert_eq!(result, super::OpenResult::Launched);
    }
}
