//! Native reader visibility and supported macOS login registration.
use crate::background::{BackgroundControl, BackgroundStatus};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path, sync::Arc};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, Window,
};
use tauri_plugin_autostart::ManagerExt;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupPreference {
    schema: u32,
    enabled: bool,
}
fn read_preference(path: &Path) -> Result<Option<bool>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if bytes.len() > 4096 {
        return Err("Login preference is too large; it was preserved.".into());
    }
    let preference: StartupPreference = serde_json::from_slice(&bytes)
        .map_err(|_| "Login preference is unreadable; it was preserved.".to_owned())?;
    if preference.schema != 1 {
        return Err("Login preference belongs to an unavailable version; it was preserved.".into());
    }
    Ok(Some(preference.enabled))
}
fn write_preference(path: &Path, enabled: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("Login preference has no parent directory.")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .write_all(
            &serde_json::to_vec(&StartupPreference { schema: 1, enabled })
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn installed_bundle(app: &AppHandle) -> bool {
    if std::env::var_os("KNOWLEDGE_GARDEN_COLLECTION").is_some() {
        return false;
    }
    let Ok(executable) = std::env::current_exe().and_then(fs::canonicalize) else {
        return false;
    };
    let Some(bundle) = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return false;
    };
    if bundle
        .extension()
        .is_none_or(|extension| extension != "app")
    {
        return false;
    }
    bundle.starts_with("/Applications")
        || app
            .path()
            .home_dir()
            .is_ok_and(|home| bundle.starts_with(home.join("Applications")))
}
fn status_text(status: &BackgroundStatus) -> String {
    let label = match status.phase.as_str() {
        "working" => "Working",
        "stopping" => "Stopping",
        "stopped" => "Stopped",
        _ => "Watching · waiting for work or retry",
    };
    let issue = status
        .last_issue
        .as_ref()
        .map(|issue| {
            format!(
                " · Last issue: {}",
                issue.chars().take(90).collect::<String>()
            )
        })
        .unwrap_or_default();
    format!("{label}{issue}")
}
pub(super) fn reopen(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        if let Err(error) = window
            .show()
            .and_then(|()| window.unminimize())
            .and_then(|()| window.set_focus())
        {
            eprintln!("The reader could not be reopened: {error}");
            return;
        }
        if let Some(engine) = app.try_state::<super::Engine>() {
            engine.control.reader_visible(true);
            engine.control.wake();
        }
        eprintln!("Lifecycle: reader opened using the existing collection engine.");
    }
}
pub(super) fn hide(window: &Window) {
    if let Err(error) = window.hide() {
        eprintln!("The reader could not be hidden: {error}");
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = window
        .app_handle()
        .set_activation_policy(tauri::ActivationPolicy::Accessory);
    if let Some(engine) = window.app_handle().try_state::<super::Engine>() {
        engine.control.reader_visible(false);
    }
    eprintln!("Lifecycle: reader hidden; collection maintenance continues.");
}
pub(super) fn install(
    app: &mut App,
    control: Arc<BackgroundControl>,
) -> Result<(), Box<dyn std::error::Error>> {
    let builder = tauri_plugin_autostart::Builder::new()
        .app_name(app.config().identifier.clone())
        .arg("--background");
    #[cfg(target_os = "macos")]
    let builder = builder.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);
    app.handle().plugin(builder.build())?;
    let eligible = installed_bundle(app.handle());
    let preference_path = app.path().app_data_dir()?.join("login-preference.json");
    let mut startup_issue = None;
    if eligible {
        match read_preference(&preference_path) {
            Ok(preference) => {
                let enabled = preference.unwrap_or(true);
                let operation = if enabled {
                    app.autolaunch().enable()
                } else {
                    app.autolaunch().disable()
                };
                if let Err(error) = operation {
                    startup_issue = Some(format!("Login startup unavailable: {error}"));
                } else if preference.is_none() {
                    if let Err(error) = write_preference(&preference_path, enabled) {
                        startup_issue = Some(error);
                    }
                }
            }
            Err(error) => startup_issue = Some(error),
        }
    }
    let status = MenuItem::with_id(
        app,
        "kg-status",
        status_text(&control.status()),
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, "kg-open", "Open reader", true, None::<&str>)?;
    let enabled = eligible && app.autolaunch().is_enabled().unwrap_or(false);
    let startup = CheckMenuItem::with_id(
        app,
        "kg-startup",
        "Launch at login",
        eligible && startup_issue.is_none(),
        enabled,
        None::<&str>,
    )?;
    let startup_detail = MenuItem::with_id(
        app,
        "kg-startup-detail",
        startup_issue.as_deref().unwrap_or(if eligible {
            "Login startup uses this installed application"
        } else {
            "Install in Applications for login startup"
        }),
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "kg-quit", "Quit Knowledge Garden", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[&status, &open, &startup, &startup_detail, &separator, &quit],
    )?;
    let startup_action = startup.clone();
    let startup_detail_action = startup_detail.clone();
    let mut tray = TrayIconBuilder::with_id("knowledge-garden")
        .tooltip("Knowledge Garden · collection maintenance")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "kg-open" => reopen(app),
            "kg-quit" => {
                if let Some(engine) = app.try_state::<super::Engine>() {
                    engine.prepare_exit();
                }
                eprintln!("Lifecycle: explicit Quit requested; pending work remains resumable.");
                app.exit(0);
            }
            "kg-startup" => {
                let enabled = !app.autolaunch().is_enabled().unwrap_or(false);
                let result = (if enabled {
                    app.autolaunch().enable()
                } else {
                    app.autolaunch().disable()
                })
                .map_err(|e| e.to_string())
                .and_then(|()| write_preference(&preference_path, enabled));
                match result {
                    Ok(()) => {
                        let _ = startup_action.set_checked(enabled);
                        let _ = startup_detail_action.set_text(if enabled {
                            "Login startup registered"
                        } else {
                            "Login startup disabled"
                        });
                    }
                    Err(error) => {
                        let _ = startup_action
                            .set_checked(app.autolaunch().is_enabled().unwrap_or(false));
                        let _ = startup_detail_action
                            .set_text(format!("Login startup unavailable: {error}"));
                    }
                }
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    control.observe(Arc::new(move |snapshot| {
        let _ = status.set_text(status_text(&snapshot));
    }));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_opt_out_survives_restart_and_future_preferences_are_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("login-preference.json");
        assert_eq!(read_preference(&path).unwrap(), None);
        write_preference(&path, true).unwrap();
        write_preference(&path, false).unwrap();
        assert_eq!(read_preference(&path).unwrap(), Some(false));
        let future = b"{\"schema\":2,\"enabled\":true,\"owner_note\":\"preserve\"}";
        fs::write(&path, future).unwrap();
        assert!(read_preference(&path).is_err());
        assert_eq!(fs::read(path).unwrap(), future);
    }
}
