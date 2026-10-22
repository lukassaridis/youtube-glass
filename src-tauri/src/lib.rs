use std::sync::Mutex;

use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Position,
    Size, Theme, Webview, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const YOUTUBE_HOME: &str = "https://www.youtube.com";
const PIP_WIDTH: f64 = 480.0;
const PIP_HEIGHT: f64 = 270.0;
const YOUTUBE_BG: tauri::window::Color = tauri::window::Color(15, 15, 15, 255);

const YOUTUBE_SCRIPT: &str = include_str!("../injected/youtube.js");
const PIP_LABEL: &str = "video-pip";
const PIP_CHROME: &str = r#"
(() => {
  const drag = document.createElement("div");
  drag.setAttribute("data-tauri-drag-region", "");
  drag.style.cssText = "position:fixed;top:0;left:0;right:40px;height:28px;z-index:2147483647;";
  const close = document.createElement("button");
  close.textContent = "×";
  close.setAttribute("aria-label", "Close picture in picture");
  close.style.cssText = "position:fixed;top:0;right:0;width:40px;height:28px;border:0;background:rgba(0,0,0,.55);color:#fff;z-index:2147483647;cursor:pointer;font:18px/28px sans-serif;";
  close.addEventListener("click", () => {
    window.__TAURI__?.window?.getCurrentWindow?.().close();
  });
  document.documentElement.appendChild(drag);
  document.documentElement.appendChild(close);
})();
"#;

#[cfg(windows)]
mod taskbar;
mod update;

struct PipRestore {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    maximized: bool,
}

struct AppState {
    last_youtube_url: Mutex<String>,
    pip_hidden: Mutex<bool>,
    pip_restore: Mutex<Option<PipRestore>>,
    signing_in: Mutex<bool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last_youtube_url: Mutex::new(YOUTUBE_HOME.to_string()),
            pip_hidden: Mutex::new(false),
            pip_restore: Mutex::new(None),
            signing_in: Mutex::new(false),
        }
    }
}

fn host_allowed(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    const SUFFIXES: &[&str] = &[
        "youtube.com",
        "youtu.be",
        "youtube-nocookie.com",
        "ytimg.com",
        "ggpht.com",
        "googlevideo.com",
        "google.com",
        "gstatic.com",
        "googleapis.com",
        "googleusercontent.com",
        "gvt1.com",
        "gvt2.com",
    ];
    SUFFIXES
        .iter()
        .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
        || google_country_host(&host)
}

/// google.co.uk, google.de, google.com.au, ... (sign-in sets cookies on the local domain).
fn google_country_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    labels.iter().rposition(|label| *label == "google").is_some_and(|at| {
        match &labels[at + 1..] {
            [cc] => cc.len() == 2,
            [second, cc] => matches!(*second, "co" | "com") && cc.len() == 2,
            _ => false,
        }
    })
}

fn is_sign_in_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "accounts.youtube.com"
        || host
            .strip_prefix("accounts.")
            .is_some_and(|rest| rest == "google.com" || google_country_host(rest))
}

fn is_youtube_page(url: &Url) -> bool {
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    matches!(host.as_str(), "youtube.com" | "www.youtube.com" | "m.youtube.com")
}

/// Google sign-in can hop through hosts we don't otherwise allow (a work or school
/// account's own identity provider, for one). Once a sign-in starts, follow any https
/// page in-app until it lands back on YouTube, so the session ends up in this webview
/// rather than in the user's browser.
fn navigation_allowed(app: &AppHandle, url: &Url) -> bool {
    let state = app.state::<AppState>();
    let Ok(mut signing_in) = state.signing_in.lock() else {
        return url_allowed(url);
    };
    if is_sign_in_url(url) {
        *signing_in = true;
    } else if is_youtube_page(url) {
        *signing_in = false;
    }
    url_allowed(url) || (*signing_in && url.scheme() == "https")
}

fn url_allowed(url: &Url) -> bool {
    match url.scheme() {
        "about" | "blob" | "data" => true,
        "http" | "https" => url.host_str().map(host_allowed).unwrap_or(false),
        _ => false,
    }
}

fn relayout(window: &Window) {
    let Ok(size) = window.inner_size() else {
        return;
    };
    let Some(youtube) = window.get_webview("youtube") else {
        return;
    };
    let _ = youtube.set_position(Position::Physical(PhysicalPosition::new(0, 0)));
    let _ = youtube.set_size(Size::Physical(size));
}

fn remember_youtube_url(app: &AppHandle, url: &Url) {
    if !url_allowed(url) {
        return;
    }
    if let Ok(mut last) = app.state::<AppState>().last_youtube_url.lock() {
        *last = url.to_string();
    }
}

fn open_external(app: &AppHandle, url: &Url) {
    let _ = app.opener().open_url(url.as_str(), None::<&str>);
}

fn attach_youtube(window: &Window, app: &AppHandle) -> tauri::Result<Webview> {
    let size = window.inner_size()?;
    let scale = window.scale_factor()?;
    let start_url = app
        .state::<AppState>()
        .last_youtube_url
        .lock()
        .ok()
        .and_then(|url| url.parse().ok())
        .unwrap_or_else(|| YOUTUBE_HOME.parse().expect("home url"));

    let handle = app.clone();
    let opener_handle = app.clone();
    let title_handle = app.clone();
    let load_handle = app.clone();

    window.add_child(
        WebviewBuilder::new("youtube", WebviewUrl::External(start_url))
            .focused(true)
            .enable_clipboard_access()
            .zoom_hotkeys_enabled(true)
            .background_color(YOUTUBE_BG)
            .initialization_script(YOUTUBE_SCRIPT)
            .on_navigation({
                let handle = opener_handle.clone();
                move |url| {
                    if navigation_allowed(&handle, &url) {
                        true
                    } else {
                        open_external(&handle, &url);
                        false
                    }
                }
            })
            .on_new_window(move |url, _features| {
                if navigation_allowed(&handle, &url) {
                    if let Some(youtube) = handle.get_webview("youtube") {
                        let _ = youtube.navigate(url);
                    }
                } else {
                    open_external(&handle, &url);
                }
                NewWindowResponse::Deny
            })
            .on_document_title_changed(move |_webview, title| {
                if let Some(window) = title_handle.get_window("main") {
                    let label = if title.is_empty() || title.eq_ignore_ascii_case("youtube") {
                        "YouTube Glass".to_string()
                    } else {
                        format!("{title} — YouTube Glass")
                    };
                    let _ = window.set_title(&label);
                }
            })
            .on_page_load(move |_webview, payload| {
                if payload.event() != PageLoadEvent::Finished {
                    return;
                }
                remember_youtube_url(&load_handle, payload.url());
            }),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(size.width as f64 / scale, size.height as f64 / scale),
    )?;

    window
        .get_webview("youtube")
        .ok_or(tauri::Error::WebviewNotFound)
}

fn valid_video_id(video_id: &str) -> bool {
    let len = video_id.len();
    (6..=16).contains(&len)
        && video_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn hide_main_for_pip_inner(app: &AppHandle) -> Result<(), String> {
    let window = app.get_window("main").ok_or("main window missing")?;
    let state = app.state::<AppState>();
    {
        let mut hidden = state.pip_hidden.lock().map_err(|err| err.to_string())?;
        if *hidden {
            return Ok(());
        }
        *hidden = true;
    }
    let maximized = window.is_maximized().unwrap_or(false);
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let size = window.outer_size().map_err(|err| err.to_string())?;
    *state.pip_restore.lock().map_err(|err| err.to_string())? = Some(PipRestore {
        position,
        size,
        maximized,
    });
    window.minimize().map_err(|err| err.to_string())
}

fn restore_main_from_pip_inner(app: &AppHandle) -> Result<(), String> {
    let window = app.get_window("main").ok_or("main window missing")?;
    let state = app.state::<AppState>();
    {
        let mut hidden = state.pip_hidden.lock().map_err(|err| err.to_string())?;
        if !*hidden {
            return Ok(());
        }
        *hidden = false;
    }
    let snapshot = state
        .pip_restore
        .lock()
        .map_err(|err| err.to_string())?
        .take();
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    if let Some(snapshot) = snapshot {
        if snapshot.maximized {
            let _ = window.maximize();
        } else {
            let _ = window.unmaximize();
            let _ = window.set_position(Position::Physical(snapshot.position));
            let _ = window.set_size(Size::Physical(snapshot.size));
        }
    }
    Ok(())
}

#[tauri::command]
async fn hide_main_for_pip(app: AppHandle) -> Result<(), String> {
    hide_main_for_pip_inner(&app)?;
    #[cfg(windows)]
    taskbar::adopt_native_pip(&app);
    Ok(())
}

#[tauri::command]
async fn restore_main_from_pip(app: AppHandle) -> Result<(), String> {
    restore_main_from_pip_inner(&app)
}

fn place_pip_window(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let pip_w = (PIP_WIDTH * scale).round() as i32;
    let pip_h = (PIP_HEIGHT * scale).round() as i32;
    let margin = (16.0 * scale).round() as i32;
    let x = work.position.x + work.size.width as i32 - pip_w - margin;
    let y = work.position.y + work.size.height as i32 - pip_h - margin;
    let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
}

#[tauri::command]
async fn open_video_pip(app: AppHandle, video_id: String, start: f64) -> Result<(), String> {
    if !valid_video_id(&video_id) {
        return Err("invalid video id".into());
    }
    let start = start.max(0.0).floor() as u64;
    let url = Url::parse(&format!(
        "https://www.youtube.com/embed/{video_id}?autoplay=1&start={start}&rel=0"
    ))
    .map_err(|err| err.to_string())?;

    if let Some(existing) = app.get_webview_window(PIP_LABEL) {
        if let Some(webview) = existing.get_webview(PIP_LABEL) {
            webview.navigate(url).map_err(|err| err.to_string())?;
        }
        let _ = existing.set_always_on_top(true);
        let _ = existing.set_icon(tauri::include_image!("icons/pip.png"));
        let _ = existing.unminimize();
        let _ = existing.set_focus();
        hide_main_for_pip_inner(&app)?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(&app, PIP_LABEL, WebviewUrl::External(url))
        .title("YouTube Glass PiP")
        .inner_size(PIP_WIDTH, PIP_HEIGHT)
        .min_inner_size(320.0, 180.0)
        .decorations(false)
        .shadow(true)
        .resizable(true)
        .always_on_top(true)
        .visible(true)
        .theme(Some(Theme::Dark))
        .background_color(YOUTUBE_BG)
        .initialization_script(PIP_CHROME)
        .icon(tauri::include_image!("icons/pip.png"))
        .map_err(|err| err.to_string())?
        .build()
        .map_err(|err| err.to_string())?;
    place_pip_window(&window);
    let _ = window.set_always_on_top(true);
    let _ = window.set_icon(tauri::include_image!("icons/pip.png"));
    let restore_handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            let _ = restore_main_from_pip_inner(&restore_handle);
        }
    });
    hide_main_for_pip_inner(&app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .manage(update::UpdateState::default())
        .invoke_handler(tauri::generate_handler![
            open_video_pip,
            hide_main_for_pip,
            restore_main_from_pip,
            update::check_for_update,
            update::dismiss_update,
            update::install_update
        ])
        .setup(|app| {
            #[cfg(windows)]
            taskbar::set_app_id(&app.config().identifier);

            let window = WindowBuilder::new(app, "main")
                .title("YouTube Glass")
                .inner_size(1280.0, 800.0)
                .min_inner_size(480.0, 300.0)
                .decorations(false)
                .shadow(true)
                .resizable(true)
                .visible(false)
                .theme(Some(Theme::Dark))
                .background_color(YOUTUBE_BG)
                .build()?;

            attach_youtube(&window, app.handle())?;
            relayout(&window);

            let layout_window = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    relayout(&layout_window);
                }
                _ => {}
            });

            window.show()?;
            if let Some(youtube) = window.get_webview("youtube") {
                let _ = youtube.set_focus();
            }
            relayout(&window);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running YouTube Glass");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn google_country_domains_are_allowed() {
        assert!(host_allowed("accounts.google.co.uk"));
        assert!(host_allowed("www.google.de"));
        assert!(host_allowed("www.google.com.au"));
        assert!(!host_allowed("google.evil.example"));
        assert!(!host_allowed("notgoogle.co.uk"));
        assert!(!host_allowed("google.attacker.com.example"));
        assert!(!host_allowed("google.foo.io"));
    }

    #[test]
    fn sign_in_urls() {
        assert!(is_sign_in_url(&url("https://accounts.google.com/ServiceLogin")));
        assert!(is_sign_in_url(&url("https://accounts.google.co.uk/accounts/SetSID")));
        assert!(is_sign_in_url(&url("https://accounts.youtube.com/accounts/SetSID")));
        assert!(!is_sign_in_url(&url("https://www.youtube.com/")));
        assert!(!is_sign_in_url(&url("https://accounts.example.com/")));
        assert!(is_youtube_page(&url("https://www.youtube.com/watch?v=x")));
        assert!(!is_youtube_page(&url("https://accounts.youtube.com/")));
    }
}
