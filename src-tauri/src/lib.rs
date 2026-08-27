use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Position,
    Size, State, Theme, Webview, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

const YOUTUBE_HOME: &str = "https://www.youtube.com";
const CHROME_HEIGHT: f64 = 36.0;
const HOVER_STRIP: f64 = 6.0;
const PIP_WIDTH: f64 = 480.0;
const PIP_HEIGHT: f64 = 270.0;
const YOUTUBE_BG: tauri::window::Color = tauri::window::Color(15, 15, 15, 255);
const TRANSPARENT: tauri::window::Color = tauri::window::Color(0, 0, 0, 0);

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

struct PipRestore {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    maximized: bool,
}

struct AppState {
    last_youtube_url: Mutex<String>,
    recover_times: Mutex<Vec<Instant>>,
    header_visible: Mutex<bool>,
    pip_hidden: Mutex<bool>,
    pip_restore: Mutex<Option<PipRestore>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last_youtube_url: Mutex::new(YOUTUBE_HOME.to_string()),
            recover_times: Mutex::new(Vec::new()),
            header_visible: Mutex::new(false),
            pip_hidden: Mutex::new(false),
            pip_restore: Mutex::new(None),
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
}

fn url_allowed(url: &Url) -> bool {
    match url.scheme() {
        "about" | "blob" | "data" => true,
        "http" | "https" => url.host_str().map(host_allowed).unwrap_or(false),
        _ => false,
    }
}

fn header_height(visible: bool) -> f64 {
    if visible {
        CHROME_HEIGHT
    } else {
        HOVER_STRIP
    }
}

fn layout_webviews(
    chrome: &Webview,
    youtube: &Webview,
    size: PhysicalSize<u32>,
    scale: f64,
    header_visible: bool,
) -> tauri::Result<()> {
    youtube.set_position(Position::Physical(PhysicalPosition::new(0, 0)))?;
    youtube.set_size(Size::Physical(PhysicalSize::new(size.width, size.height)))?;
    let chrome_h = (header_height(header_visible) * scale).round().max(1.0) as u32;
    chrome.set_position(Position::Physical(PhysicalPosition::new(0, 0)))?;
    chrome.set_size(Size::Physical(PhysicalSize::new(size.width, chrome_h)))?;
    Ok(())
}

fn header_is_visible(window: &Window) -> bool {
    window
        .try_state::<AppState>()
        .and_then(|state| state.header_visible.lock().ok().map(|flag| *flag))
        .unwrap_or(false)
}

fn relayout(window: &Window) {
    let Ok(size) = window.inner_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let Some(chrome) = window.get_webview("chrome") else {
        return;
    };
    let Some(youtube) = window.get_webview("youtube") else {
        return;
    };
    let _ = layout_webviews(&chrome, &youtube, size, scale, header_is_visible(window));
}

fn remember_youtube_url(app: &AppHandle, url: &Url) {
    if !url_allowed(url) {
        return;
    }
    if let Ok(mut last) = app.state::<AppState>().last_youtube_url.lock() {
        *last = url.to_string();
    }
}

fn recover_allowed(state: &AppState) -> bool {
    let Ok(mut times) = state.recover_times.lock() else {
        return false;
    };
    let now = Instant::now();
    times.retain(|time| now.duration_since(*time) < Duration::from_secs(10));
    if times.len() >= 3 {
        return false;
    }
    times.push(now);
    true
}

fn open_external(app: &AppHandle, url: &Url) {
    let _ = app.opener().open_url(url.as_str(), None::<&str>);
}

fn attach_chrome(window: &Window, size: PhysicalSize<u32>, scale: f64) -> tauri::Result<Webview> {
    window.add_child(
        WebviewBuilder::new("chrome", WebviewUrl::App("index.html".into()))
            .focused(false)
            .transparent(true)
            .background_color(TRANSPARENT),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(size.width as f64 / scale, HOVER_STRIP),
    )?;
    window.get_webview("chrome").ok_or(tauri::Error::WebviewNotFound)
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
                    if url_allowed(&url) {
                        true
                    } else {
                        open_external(&handle, &url);
                        false
                    }
                }
            })
            .on_new_window(move |url, _features| {
                if url_allowed(&url) {
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

fn navigate_youtube(app: &AppHandle, url: Url) -> Result<(), String> {
    if let Some(youtube) = app.get_webview("youtube") {
        youtube.navigate(url).map_err(|err| err.to_string())
    } else {
        let window = app.get_window("main").ok_or("main window missing")?;
        attach_youtube(&window, app).map_err(|err| err.to_string())?;
        relayout(&window);
        Ok(())
    }
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
    hide_main_for_pip_inner(&app)
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

#[tauri::command]
async fn reload_youtube(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if !recover_allowed(&state) {
        return Err("reload rate limited".into());
    }
    let url = state
        .last_youtube_url
        .lock()
        .map_err(|err| err.to_string())?
        .parse()
        .unwrap_or_else(|_| YOUTUBE_HOME.parse().expect("home url"));
    navigate_youtube(&app, url)
}

#[tauri::command]
async fn set_header_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut flag = state.header_visible.lock().map_err(|err| err.to_string())?;
        if *flag == visible {
            return Ok(());
        }
        *flag = visible;
    }
    if let Some(window) = app.get_window("main") {
        relayout(&window);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_video_pip,
            hide_main_for_pip,
            restore_main_from_pip,
            reload_youtube,
            set_header_visible
        ])
        .setup(|app| {
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

            let size = window.inner_size()?;
            let scale = window.scale_factor()?;
            attach_youtube(&window, app.handle())?;
            attach_chrome(&window, size, scale)?;
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
