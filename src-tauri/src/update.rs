//! In-app updates from GitHub Releases. The YouTube page only gets these narrow
//! commands, never the updater plugin's own permissions.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::async_runtime::Mutex;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct UpdateState {
    /// `None` until a check succeeds; then the result, so page reloads don't re-check.
    found: Mutex<Option<Option<Update>>>,
    dismissed: AtomicBool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum UpdateProgress {
    Progress { downloaded: u64, total: Option<u64> },
    Installing,
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
) -> Result<Option<UpdateInfo>, String> {
    if state.dismissed.load(Ordering::Relaxed) {
        return Ok(None);
    }
    let mut found = state.found.lock().await;
    if found.is_none() {
        let update = app
            .updater()
            .map_err(|err| err.to_string())?
            .check()
            .await
            .map_err(|err| err.to_string())?;
        *found = Some(update);
    }
    Ok(found.as_ref().and_then(Option::as_ref).map(|update| UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
    }))
}

#[tauri::command]
pub async fn dismiss_update(state: State<'_, UpdateState>) -> Result<(), String> {
    state.dismissed.store(true, Ordering::Relaxed);
    Ok(())
}

/// Downloads and verifies the signed installer, then runs it. On Windows the updater
/// exits the app and the NSIS installer relaunches it once done.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, UpdateState>,
    on_progress: Channel<UpdateProgress>,
) -> Result<(), String> {
    let update = state
        .found
        .lock()
        .await
        .clone()
        .flatten()
        .ok_or("no update available")?;
    let finished = on_progress.clone();
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(UpdateProgress::Progress { downloaded, total });
            },
            || {
                let _ = finished.send(UpdateProgress::Installing);
            },
        )
        .await
        .map_err(|err| err.to_string())?;
    app.restart();
}
