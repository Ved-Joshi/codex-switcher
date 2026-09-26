mod account;
mod desktop;
mod profiles;

use serde::Serialize;
use serde_json::json;
use std::sync::Mutex;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size,
};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_store::StoreExt;

#[derive(Default)]
struct ShellState {
    startup_error: Mutex<Option<String>>,
    last_action: Mutex<Option<String>>,
    profile_add_lock: Mutex<()>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShellStatus {
    version: &'static str,
    core_connected: bool,
    identity: &'static str,
    launch_at_login: bool,
    launch_at_login_available: bool,
    startup_error: Option<String>,
    last_action: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectResult {
    profile: profiles::Profile,
    launched: bool,
    message: String,
}

#[tauri::command]
async fn list_profiles(app: tauri::AppHandle) -> Result<Vec<profiles::Profile>, String> {
    let home = app
        .path()
        .home_dir()
        .map_err(|reason| format!("Could not find the home directory: {reason}"))?;
    let mut connected = Vec::new();
    for mut profile in profiles::list(&home)? {
        if profiles::is_removed(&home, &profile.id) {
            continue;
        }
        if !profiles::is_connected(&home, &profile.id) {
            let status = account::read_profile(&home, &profile.id).await;
            if status.signed_in {
                profiles::mark_connected(&home, &profile.id)?;
            } else if status.error.is_none() {
                profiles::mark_pending(&home, &profile.id)?;
            }
        }
        if profiles::is_connected(&home, &profile.id) {
            profile.custom_name = saved_profile_name(&app, &profile.id)?;
            connected.push(profile);
        }
    }
    Ok(connected)
}

#[tauri::command]
fn list_removed_profiles(app: tauri::AppHandle) -> Result<Vec<profiles::Profile>, String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    let mut removed = Vec::new();
    for mut profile in profiles::list(&home)? {
        if profiles::is_removed(&home, &profile.id) {
            profile.custom_name = saved_profile_name(&app, &profile.id)?;
            removed.push(profile);
        }
    }
    Ok(removed)
}

#[tauri::command]
fn remove_account(app: tauri::AppHandle, profile: String) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    if !profiles::list(&home)?.iter().any(|saved| saved.id == profile) {
        return Err("This account profile does not exist.".into());
    }
    profiles::mark_removed(&home, &profile)
}

#[tauri::command]
fn restore_account(app: tauri::AppHandle, profile: String) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    if !profiles::list(&home)?.iter().any(|saved| saved.id == profile) {
        return Err("This account profile does not exist.".into());
    }
    profiles::restore(&home, &profile)
}

#[tauri::command]
fn open_releases_page() -> Result<(), String> {
    std::process::Command::new("/usr/bin/open")
        .arg("https://github.com/Ved-Joshi/codex-switcher/releases/latest")
        .spawn()
        .map(|_| ())
        .map_err(|reason| format!("Could not open releases: {reason}"))
}

fn saved_profile_name(app: &tauri::AppHandle, id: &str) -> Result<Option<String>, String> {
    let store = app.store("preferences.json")
        .map_err(|_| "Could not read account names.".to_string())?;
    Ok(store.get("profileNames")
        .and_then(|names| names.get(id).and_then(|name| name.as_str()).map(str::to_owned)))
}

#[tauri::command]
fn set_profile_name(app: tauri::AppHandle, profile: String, name: String) -> Result<(), String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    if !profiles::is_connected(&home, &profile) {
        return Err("This account is not connected.".into());
    }
    let name = name.trim();
    if name.chars().count() > 48 || name.chars().any(char::is_control) {
        return Err("Account name must be 48 characters or fewer, without control characters.".into());
    }
    let store = app.store("preferences.json")
        .map_err(|_| "Could not read account names.".to_string())?;
    let previous = store.get("profileNames");
    let mut names = previous.clone()
        .and_then(|value| value.as_object().cloned()).unwrap_or_default();
    if name.is_empty() {
        names.remove(&profile);
    } else {
        names.insert(profile, json!(name));
    }
    store.set("profileNames", json!(names));
    if store.save().is_err() {
        if let Some(previous) = previous {
            store.set("profileNames", previous);
        } else {
            store.delete("profileNames");
        }
        return Err("Could not save account name.".into());
    }
    Ok(())
}

#[tauri::command]
async fn get_account_status(app: tauri::AppHandle, profile: String) -> account::AccountStatus {
    match app.path().home_dir() {
        Ok(home) => account::read_profile(&home, &profile).await,
        Err(_) => account::unavailable(&profile, "Could not find the home directory."),
    }
}

#[tauri::command]
fn open_account(app: tauri::AppHandle, profile: String) -> Result<String, String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    if !profiles::is_connected(&home, &profile) {
        return Err("This account profile is not connected.".into());
    }
    let result = desktop::open_or_focus(&home, &profile)?;
    let message = match result {
        desktop::OpenResult::Focused => "Focused",
        desktop::OpenResult::Launched => "Opened",
    };
    if let Some(state) = app.try_state::<ShellState>() {
        if let Ok(mut last_action) = state.last_action.lock() {
            *last_action = Some(format!("{profile} {message}. Desktop identity unverified."));
        }
    }
    Ok(message.into())
}

#[tauri::command]
fn connect_account(app: tauri::AppHandle, state: tauri::State<'_, ShellState>) -> Result<ConnectResult, String> {
    let home = app.path().home_dir().map_err(|_| "Could not find the home directory.")?;
    let _guard = state.profile_add_lock.lock().map_err(|_| "Account setup is busy.")?;
    let profile = profiles::add(&home)?;
    // Keep the new profile even if Codex cannot launch, so sign in can be retried.
    match desktop::open_or_focus(&home, &profile.id) {
        Ok(_) => Ok(ConnectResult {
            message: "Codex opened for account setup. Finish sign-in there, then reopen this panel.".into(),
            profile,
            launched: true,
        }),
        Err(reason) => {
            let message = format!("Account setup is saved, but Codex could not open: {reason}");
            if let Ok(mut last_action) = state.last_action.lock() {
                *last_action = Some(message.clone());
            }
            Ok(ConnectResult { profile, launched: false, message })
        }
    }
}

#[tauri::command]
fn hide_panel(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let _ = window.emit("panel-closed", ());
    }
}

#[tauri::command]
fn quit_switcher(app: tauri::AppHandle) {
    app.exit(0);
}

fn status_for(app: &tauri::AppHandle, state: &ShellState) -> ShellStatus {
    let mut error = state
        .startup_error
        .lock()
        .ok()
        .and_then(|value| value.clone());
    if error.is_none() {
        if let Err(reason) = app.store("preferences.json") {
            error = Some(format!("Could not read preferences: {reason}"));
        }
    }

    ShellStatus {
        version: env!("CARGO_PKG_VERSION"),
        core_connected: true,
        identity: "Unknown",
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        launch_at_login_available: !cfg!(debug_assertions),
        startup_error: error,
        last_action: state
            .last_action
            .lock()
            .ok()
            .and_then(|value| value.clone()),
    }
}

#[tauri::command]
fn get_shell_status(app: tauri::AppHandle, state: tauri::State<'_, ShellState>) -> ShellStatus {
    status_for(&app, &state)
}

#[tauri::command]
fn set_launch_at_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, ShellState>,
    enabled: bool,
) -> Result<ShellStatus, String> {
    // A development binary has no stable location for a login item.
    if cfg!(debug_assertions) {
        return Err("Launch at login can be changed in a built app.".into());
    }

    let store = app
        .store("preferences.json")
        .map_err(|reason| format!("Could not read preferences: {reason}"))?;
    let previous = app
        .autolaunch()
        .is_enabled()
        .map_err(|reason| format!("Could not check launch at login: {reason}"))?;

    let change = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    change.map_err(|reason| format!("Could not change launch at login: {reason}"))?;

    store.set("launchAtLogin", json!(enabled));
    if let Err(reason) = store.save() {
        let _ = if previous {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        store.set("launchAtLogin", json!(previous));
        return Err(format!("Could not save preferences: {reason}"));
    }

    Ok(status_for(&app, &state))
}

fn initialize_preferences(app: &tauri::App) -> Option<String> {
    let store = match app.store("preferences.json") {
        Ok(store) => store,
        Err(reason) => return Some(format!("Could not read preferences: {reason}")),
    };

    let first_launch = store.get("schemaVersion").is_none();
    if first_launch {
        store.set("schemaVersion", json!(1));
        store.set("launchAtLogin", json!(true));
        if let Err(reason) = store.save() {
            return Some(format!("Could not save preferences: {reason}"));
        }
    }

    if !cfg!(debug_assertions)
        && store.get("launchAtLogin").and_then(|value| value.as_bool()) == Some(true)
    {
        if let Err(reason) = app.autolaunch().enable() {
            return Some(format!("Could not enable launch at login: {reason}"));
        }
    }

    None
}

fn show_panel(app: &tauri::AppHandle, tray_rect: Option<tauri::Rect>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Some(rect) = tray_rect {
            if let Ok(size) = window.outer_size() {
                let scale = window.scale_factor().unwrap_or(1.0);
                let tray_position = rect.position.to_physical::<f64>(scale);
                let tray_size = rect.size.to_physical::<f64>(scale);
                let width = size.width as f64;
                let height = size.height as f64;
                let mut x = tray_position.x + tray_size.width / 2.0 - width / 2.0;
                let mut y = tray_position.y + tray_size.height + 7.0;
                if let Ok(monitors) = window.available_monitors() {
                    if let Some(monitor) = monitors.iter().find(|monitor| {
                        let left = monitor.position().x as f64;
                        let top = monitor.position().y as f64;
                        tray_position.x >= left
                            && tray_position.x < left + monitor.size().width as f64
                            && tray_position.y >= top
                            && tray_position.y < top + monitor.size().height as f64
                    }) {
                        let left = monitor.position().x as f64;
                        let top = monitor.position().y as f64;
                        x = x.clamp(left + 8.0, left + monitor.size().width as f64 - width - 8.0);
                        y = y.clamp(top + 8.0, top + monitor.size().height as f64 - height - 8.0);
                    }
                }
                let _ = window.set_position(Position::Physical(PhysicalPosition::new(x.round() as i32, y.round() as i32)));
            }
        }
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to("main", "panel-opened", ());
    }
}

#[tauri::command]
fn resize_panel(app: tauri::AppHandle, height: u32) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("Popup window unavailable.")?;
    window
        .set_size(Size::Logical(LogicalSize::new(430.0, height.clamp(180, 560) as f64)))
        .map_err(|reason| format!("Could not resize popup: {reason}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(ShellState::default())
        .invoke_handler(tauri::generate_handler![
            get_shell_status,
            set_launch_at_login,
            get_account_status,
            list_profiles,
            list_removed_profiles,
            remove_account,
            restore_account,
            open_releases_page,
            set_profile_name,
            open_account,
            connect_account,
            hide_panel,
            resize_panel,
            quit_switcher
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let startup_error = initialize_preferences(app);
            if let Some(state) = app.try_state::<ShellState>() {
                if let Ok(mut error) = state.startup_error.lock() {
                    *error = startup_error;
                }
            }
            TrayIconBuilder::new()
                .icon(
                    app.default_window_icon()
                        .expect("the app bundle should contain an icon")
                        .clone(),
                )
                .tooltip("Codex Switcher")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left | MouseButton::Right,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                                let _ = window.emit("panel-closed", ());
                            } else {
                                show_panel(app, Some(rect));
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                tauri::WindowEvent::Focused(false) => {
                    let _ = window.hide();
                    let _ = window.emit("panel-closed", ());
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("Could not run Codex Switcher");
}
