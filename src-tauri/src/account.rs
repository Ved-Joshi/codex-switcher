//! Read each profile's account and usage through Codex's own app-server.
//! The JSON line transport is adapted from bartekczyz/ai-profiles (MIT).

use serde::Serialize;
use serde_json::{json, Value};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountStatus {
    pub(crate) profile: String,
    pub(crate) email: Option<String>,
    pub(crate) plan: Option<String>,
    pub(crate) signed_in: bool,
    pub(crate) windows: Vec<UsageWindow>,
    pub(crate) usage_error: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) desktop_identity_verified: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageWindow {
    pub(crate) duration_minutes: u64,
    pub(crate) remaining_percent: u8,
    pub(crate) resets_at: Option<u64>,
}

impl AccountStatus {
    fn empty(profile: &str) -> Self {
        Self {
            profile: profile.to_owned(),
            email: None,
            plan: None,
            signed_in: false,
            windows: Vec::new(),
            usage_error: None,
            error: None,
            desktop_identity_verified: false,
        }
    }
}

pub(crate) fn unavailable(profile: &str, reason: &str) -> AccountStatus {
    let mut status = AccountStatus::empty(profile);
    status.error = Some(reason.to_owned());
    status
}

pub(crate) async fn read_profile(home: &Path, profile: &str) -> AccountStatus {
    let mut status = AccountStatus::empty(profile);
    let root = home.join("Library/Application Support/Codex Switcher/probe");
    let plan = match crate::desktop::DesktopLaunchPlan::for_profile(&root, profile) {
        Ok(plan) => plan,
        Err(reason) => {
            status.error = Some(reason.to_owned());
            return status;
        }
    };
    if let Err(reason) = validate_account_home(&root, &plan.codex_home) {
        status.error = Some(reason);
        return status;
    }
    match read_account(&plan.codex_home).await {
        Ok((email, plan, windows, usage_error)) => {
            status.signed_in = email.is_some();
            status.email = email;
            status.plan = plan;
            status.windows = windows;
            status.usage_error = usage_error;
        }
        Err(reason) => status.error = Some(reason),
    }
    status
}

fn validate_account_home(root: &Path, codex_home: &Path) -> Result<(), String> {
    let profile_root = codex_home.parent().ok_or("Invalid profile path.")?;
    for path in [root, &root.join("profiles"), profile_root, codex_home] {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|_| "Profile has not been opened yet.".to_string())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err("Profile account directory is not private.".into());
        }
    }
    Ok(())
}

async fn read_account(
    codex_home: &Path,
) -> Result<
    (
        Option<String>,
        Option<String>,
        Vec<UsageWindow>,
        Option<String>,
    ),
    String,
> {
    let binary = find_codex_binary().ok_or_else(|| "Codex CLI was not found.".to_string())?;
    let mut rpc = Rpc::start(&binary, codex_home).await?;
    let account = rpc
        .request("account/read", json!({ "refreshToken": false }))
        .await?;
    let details = account.get("account").filter(|value| !value.is_null());
    let Some(details) = details else {
        return Ok((None, None, Vec::new(), None));
    };
    if details.get("type").and_then(Value::as_str) != Some("chatgpt") {
        return Err("This profile is not signed in with a ChatGPT account.".into());
    }
    let email = details
        .get("email")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let plan = details
        .get("planType")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let Some(email) = email else {
        return Err("Codex did not return an account email.".into());
    };
    match rpc.request("account/rateLimits/read", json!({})).await {
        Ok(value) => {
            let windows = parse_windows(&value);
            let usage_error = windows.is_empty().then(|| "Usage unavailable.".into());
            Ok((Some(email), plan, windows, usage_error))
        }
        Err(_) => Ok((
            Some(email),
            plan,
            Vec::new(),
            Some("Usage unavailable.".into()),
        )),
    }
}

fn parse_windows(value: &Value) -> Vec<UsageWindow> {
    let Some(limits) = value.get("rateLimitsByLimitId")
        .and_then(|limits| limits.get("codex"))
        .or_else(|| value.get("rateLimits")) else {
        return Vec::new();
    };
    ["primary", "secondary"]
        .into_iter()
        .filter_map(|key| {
            let window = limits.get(key)?;
            let duration_minutes = window.get("windowDurationMins")?.as_u64()?;
            let used = window.get("usedPercent")?.as_f64()?;
            if !used.is_finite() {
                return None;
            }
            Some(UsageWindow {
                duration_minutes,
                remaining_percent: (100.0 - used).clamp(0.0, 100.0).round() as u8,
                resets_at: window.get("resetsAt").and_then(Value::as_u64),
            })
        })
        .collect()
}

fn find_codex_binary() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".local/bin/codex"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join("codex")));
    }
    candidates.into_iter().find(|candidate| {
        std::fs::metadata(candidate)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    })
}

struct Rpc {
    _child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    last_id: i64,
}

impl Rpc {
    async fn start(binary: &Path, codex_home: &Path) -> Result<Self, String> {
        let mut child = Command::new(binary)
            .arg("app-server")
            .env("CODEX_HOME", codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| "Could not start Codex app-server.".to_string())?;
        let stdin = child
            .stdin
            .take()
            .ok_or("Codex app-server input is unavailable.")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Codex app-server output is unavailable.")?;
        let mut rpc = Self {
            _child: child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            last_id: 0,
        };
        rpc.request(
            "initialize",
            json!({ "clientInfo": { "name": "codex_switcher", "title": "Codex Switcher", "version": env!("CARGO_PKG_VERSION") } }),
        )
        .await?;
        rpc.send(&json!({ "method": "initialized" })).await?;
        Ok(rpc)
    }

    async fn send(&mut self, message: &Value) -> Result<(), String> {
        let mut line = message.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|_| "Could not write to Codex app-server.".to_string())?;
        self.stdin
            .flush()
            .await
            .map_err(|_| "Could not flush Codex app-server request.".to_string())
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.last_id += 1;
        let id = self.last_id;
        self.send(&json!({ "id": id, "method": method, "params": params }))
            .await?;
        let response = async {
            while let Some(line) = self
                .lines
                .next_line()
                .await
                .map_err(|_| "Could not read Codex app-server response.")?
            {
                if let Some(result) = result_for_id(&line, id) {
                    return result;
                }
            }
            Err("Codex app-server closed before responding.".to_string())
        };
        tokio::time::timeout(REQUEST_TIMEOUT, response)
            .await
            .map_err(|_| "Codex app-server timed out.".to_string())?
    }
}

fn result_for_id(line: &str, id: i64) -> Option<Result<Value, String>> {
    let mut message: Value = serde_json::from_str(line).ok()?;
    if message.get("id").and_then(Value::as_i64) != Some(id) || message.get("method").is_some() {
        return None;
    }
    if message.get("error").is_some() {
        return Some(Err("Codex app-server rejected the request.".into()));
    }
    message.get_mut("result").map(|result| Ok(result.take()))
}

#[cfg(test)]
mod tests {
    use super::{parse_windows, result_for_id};
    use serde_json::json;

    #[test]
    fn parses_only_matching_rpc_reply() {
        assert!(result_for_id(r#"{"method":"account/updated"}"#, 2).is_none());
        assert!(result_for_id(r#"{"id":1,"result":{}}"#, 2).is_none());
        assert_eq!(
            result_for_id(r#"{"id":2,"result":{"ok":true}}"#, 2)
                .unwrap()
                .unwrap(),
            json!({"ok": true})
        );
    }

    #[test]
    fn reports_remaining_usage_in_each_window() {
        let windows = parse_windows(
            &json!({"rateLimits": {"primary": {"usedPercent": 25.4, "windowDurationMins": 300, "resetsAt": 1234}, "secondary": {"usedPercent": 90, "windowDurationMins": 10080}}}),
        );
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].remaining_percent, 75);
        assert_eq!(windows[1].remaining_percent, 10);
        assert_eq!(windows[0].resets_at, Some(1234));
    }

    #[test]
    fn uses_codex_limit_when_other_limits_are_present() {
        let windows = parse_windows(&json!({
            "rateLimits": {"primary": {"usedPercent": 80, "windowDurationMins": 300}},
            "rateLimitsByLimitId": {"codex": {"primary": {"usedPercent": 10, "windowDurationMins": 300}}}
        }));
        assert_eq!(windows[0].remaining_percent, 90);
    }

    #[test]
    #[ignore = "reads the two signed-in profile accounts and usage on this Mac"]
    fn live_profiles_have_distinct_accounts() {
        let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (a, b) = runtime.block_on(async {
            (
                super::read_profile(&home, "A").await,
                super::read_profile(&home, "B").await,
            )
        });
        assert!(a.signed_in, "A account could not be read: {:?}", a.error);
        assert!(b.signed_in, "B account could not be read: {:?}", b.error);
        assert!(
            a.email != b.email,
            "A and B returned the same account email"
        );
        assert!(!a.windows.is_empty(), "A usage was unavailable");
        assert!(!b.windows.is_empty(), "B usage was unavailable");
    }
}
