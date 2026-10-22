(() => {
  const host = location.hostname || "";
  if (!(host === "youtu.be" || host.endsWith("youtube.com"))) {
    return;
  }
  if (window !== window.top) {
    return;
  }
  if (window.__ytgPipInit) {
    return;
  }
  window.__ytgPipInit = true;

  const BUTTON_ID = "ytg-pip-fab";
  const CONTROLS_ID = "ytg-window-controls";
  const CONTROLS_STYLE_ID = "ytg-window-controls-style";
  const UPDATE_ID = "ytg-update";
  // Empty parts of YouTube's masthead act as the window's title bar.
  const DRAG_ZONE = ["ytd-masthead", "#masthead-container", "#" + CONTROLS_ID].join(",");
  const NO_DRAG = [
    "a",
    "button",
    "input",
    "textarea",
    "select",
    "[contenteditable]",
    "[role='button']",
    "[role='link']",
    "[role='combobox']",
    "yt-icon-button",
    "yt-button-shape",
    "ytd-topbar-logo-renderer",
    "yt-searchbox",
    "ytd-searchbox",
    "#search-form",
    "#voice-search-button",
    "ytd-topbar-menu-button-renderer",
    "ytd-notification-topbar-button-renderer",
    "#avatar-btn",
    "tp-yt-iron-dropdown",
  ].join(",");

  const nativeRequestPip = HTMLVideoElement.prototype.requestPictureInPicture;

  try {
    Object.defineProperty(HTMLVideoElement.prototype, "disablePictureInPicture", {
      get() {
        return false;
      },
      set() {},
      configurable: true,
    });
  } catch {
    // ignore
  }

  function isPreviewVideo(video) {
    if (!video || !video.closest) {
      return false;
    }
    return Boolean(
      video.closest(
        [
          "ytd-video-preview",
          "#video-preview",
          "ytd-thumbnail",
          "ytd-inline-playback-player",
          "ytd-rich-grid-media ytd-player",
          "[id='preview']",
        ].join(",")
      )
    );
  }

  function usable(video) {
    if (!video || isPreviewVideo(video)) {
      return false;
    }
    const rect = video.getBoundingClientRect();
    return rect.width > 80 && rect.height > 80;
  }

  function getMainVideo() {
    const ranked = [...document.querySelectorAll("video")]
      .filter(usable)
      .sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight);
    const playing = ranked.filter((video) => !video.paused && !video.ended);
    if (playing[0] || ranked[0]) {
      return playing[0] || ranked[0];
    }
    const main = document.querySelector(
      "#movie_player video.html5-main-video, #movie_player video.video-stream, ytd-reel-video-renderer video"
    );
    return usable(main) ? main : null;
  }

  function videoIdFromLocation() {
    try {
      const url = new URL(location.href);
      const watch = url.searchParams.get("v");
      if (watch) {
        return watch;
      }
      const shorts = url.pathname.match(/\/shorts\/([\w-]{6,})/);
      if (shorts) {
        return shorts[1];
      }
      const embed = url.pathname.match(/\/embed\/([\w-]{6,})/);
      if (embed) {
        return embed[1];
      }
    } catch {
      // ignore
    }
    return null;
  }

  function videoIsPresent() {
    return Boolean(videoIdFromLocation());
  }

  function toast(message) {
    let el = document.getElementById("ytg-toast");
    if (!el) {
      el = document.createElement("div");
      el.id = "ytg-toast";
      el.style.cssText =
        "position:fixed;bottom:72px;left:50%;transform:translateX(-50%);background:#111;color:#fff;padding:10px 14px;border-radius:8px;z-index:2147483647;font:13px/1.3 Segoe UI,Roboto,sans-serif;pointer-events:none;border:1px solid #444";
      mount(el);
    }
    el.textContent = message;
    el.style.display = "block";
    clearTimeout(el._hide);
    el._hide = setTimeout(() => {
      el.style.display = "none";
    }, 1800);
  }

  function unlock(video) {
    try {
      video.disablePictureInPicture = false;
      video.removeAttribute("disablepictureinpicture");
    } catch {
      // ignore
    }
  }

  function toggleVideoPip() {
    if (document.pictureInPictureElement) {
      document.exitPictureInPicture().catch(() => {});
      return;
    }
    const video = getMainVideo();
    if (!video) {
      toast("Play a video first");
      return;
    }
    unlock(video);
    const request = nativeRequestPip || video.requestPictureInPicture;
    if (typeof request !== "function") {
      toast("Picture in picture is unavailable");
      return;
    }
    Promise.resolve(request.call(video)).catch((err) => {
      console.error("ytg pip", err);
      toast("Picture in picture is unavailable");
    });
  }

  window.__ytgTogglePip = toggleVideoPip;

  function makeIcon() {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("width", "20");
    svg.setAttribute("height", "20");
    svg.setAttribute("aria-hidden", "true");
    svg.style.cssText = "display:block;filter:drop-shadow(0 1px 1px rgba(0,0,0,.28))";
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("fill", "rgba(255,255,255,.94)");
    path.setAttribute(
      "d",
      "M3 5.5A2.5 2.5 0 0 1 5.5 3h13A2.5 2.5 0 0 1 21 5.5v8a.75.75 0 0 1-1.5 0v-8A1 1 0 0 0 18.5 4.5h-13A1 1 0 0 0 4.5 5.5v9A1 1 0 0 0 5.5 15.5H11a.75.75 0 0 1 0 1.5H5.5A2.5 2.5 0 0 1 3 14.5v-9zm10 7.25A1.75 1.75 0 0 1 14.75 11h5.5A1.75 1.75 0 0 1 22 12.75v5.5A1.75 1.75 0 0 1 20.25 20h-5.5A1.75 1.75 0 0 1 13 18.25v-5.5z"
    );
    svg.appendChild(path);
    return svg;
  }

  function mount(node) {
    const root = document.documentElement || document.body;
    if (root && node.parentNode !== root) {
      root.appendChild(node);
    }
  }

  function ensureButton() {
    let button = document.getElementById(BUTTON_ID);
    if (button) {
      return button;
    }
    button = document.createElement("button");
    button.id = BUTTON_ID;
    button.type = "button";
    button.title = "Picture in picture (Alt+P)";
    button.setAttribute("aria-label", "Picture in picture");
    button.style.cssText = [
      "all:unset",
      "position:fixed",
      "right:16px",
      "bottom:16px",
      "z-index:2147483647",
      "box-sizing:border-box",
      "display:none",
      "align-items:center",
      "justify-content:center",
      "width:48px",
      "height:48px",
      "padding:0",
      "border-radius:50%",
      "border:1px solid rgba(255,255,255,.38)",
      "background:linear-gradient(180deg, rgba(255,90,90,.42) 0%, rgba(220,0,0,.28) 46%, rgba(140,0,0,.34) 100%)",
      "color:#fff",
      "backdrop-filter:blur(22px) saturate(180%)",
      "-webkit-backdrop-filter:blur(22px) saturate(180%)",
      "box-shadow:inset 0 1px 0 rgba(255,255,255,.45), inset 0 -8px 16px rgba(120,0,0,.22), 0 8px 24px rgba(180,0,0,.28)",
      "cursor:pointer",
      "pointer-events:auto",
      "transition:transform .15s ease, filter .15s ease, background .15s ease",
    ].join(";");
    button.appendChild(makeIcon());
    button.addEventListener("mouseenter", () => {
      button.style.transform = "scale(1.06)";
      button.style.filter = "brightness(1.08)";
    });
    button.addEventListener("mouseleave", () => {
      button.style.transform = "scale(1)";
      button.style.filter = "none";
    });
    button.addEventListener(
      "click",
      (event) => {
        event.preventDefault();
        event.stopPropagation();
        toggleVideoPip();
      },
      true
    );
    mount(button);
    return button;
  }

  function appWindow() {
    const api = window.__TAURI__ && window.__TAURI__.window;
    return api && api.getCurrentWindow ? api.getCurrentWindow() : null;
  }

  function controlIcon(d) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 16 16");
    svg.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", d);
    svg.appendChild(path);
    return svg;
  }

  function ensureControls() {
    if (!document.getElementById(CONTROLS_STYLE_ID)) {
      const style = document.createElement("style");
      style.id = CONTROLS_STYLE_ID;
      style.textContent = `
        #${CONTROLS_ID}{position:fixed;top:0;right:0;height:56px;display:none;align-items:center;gap:2px;padding:0 8px 0 4px;z-index:2030;}
        #${CONTROLS_ID} button{all:unset;box-sizing:border-box;width:36px;height:36px;border-radius:50%;display:grid;place-items:center;cursor:pointer;color:var(--yt-spec-text-primary,#f1f1f1);}
        #${CONTROLS_ID} button:hover{background:var(--yt-spec-badge-chip-background,rgba(255,255,255,.1));}
        #${CONTROLS_ID} button.ytg-close:hover{background:#e81123;color:#fff;}
        #${CONTROLS_ID} svg{width:16px;height:16px;fill:currentColor;}
        html.ytg-controls-visible ytd-masthead #end{margin-right:124px;}
      `;
      mount(style);
    }
    let bar = document.getElementById(CONTROLS_ID);
    if (bar) {
      return bar;
    }
    bar = document.createElement("div");
    bar.id = CONTROLS_ID;
    const buttons = [
      ["Minimize", "", "M3 7.4h10v1.2H3V7.4Z", (win) => win.minimize()],
      ["Maximize", "", "M3.5 3.5h9v9h-9v-9Zm1.2 1.2v6.6h6.6V4.7H4.7Z", (win) => win.toggleMaximize()],
      [
        "Close",
        "ytg-close",
        "m4.2 3.4 3.8 3.8 3.8-3.8 1 1-3.8 3.8 3.8 3.8-1 1-3.8-3.8-3.8 3.8-1-1 3.8-3.8-3.8-3.8 1-1Z",
        (win) => win.close(),
      ],
    ];
    for (const [label, className, d, action] of buttons) {
      const button = document.createElement("button");
      button.type = "button";
      button.title = label;
      button.setAttribute("aria-label", label);
      if (className) {
        button.className = className;
      }
      button.appendChild(controlIcon(d));
      button.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        const win = appWindow();
        if (win) {
          action(win);
        }
      });
      bar.appendChild(button);
    }
    mount(bar);
    return bar;
  }

  function updateStyle() {
    return `
      #${UPDATE_ID}{position:fixed;left:16px;bottom:16px;z-index:2147483647;box-sizing:border-box;width:320px;padding:14px 16px;border-radius:12px;
        background:rgba(33,33,33,.92);color:#f1f1f1;border:1px solid rgba(255,255,255,.12);box-shadow:0 8px 28px rgba(0,0,0,.45);
        backdrop-filter:blur(22px) saturate(160%);-webkit-backdrop-filter:blur(22px) saturate(160%);font:13px/1.4 Roboto,"Segoe UI",sans-serif;}
      #${UPDATE_ID} .ytg-title{font-size:15px;font-weight:500;margin:0 0 4px;}
      #${UPDATE_ID} .ytg-text{color:#aaa;margin:0;}
      #${UPDATE_ID} .ytg-actions{display:flex;justify-content:flex-end;gap:8px;margin-top:12px;}
      #${UPDATE_ID} button{all:unset;box-sizing:border-box;height:36px;padding:0 16px;border-radius:18px;cursor:pointer;font-weight:500;}
      #${UPDATE_ID} .ytg-secondary{color:#f1f1f1;}
      #${UPDATE_ID} .ytg-secondary:hover{background:rgba(255,255,255,.1);}
      #${UPDATE_ID} .ytg-primary{background:#f1f1f1;color:#0f0f0f;}
      #${UPDATE_ID} .ytg-primary:hover{background:#d9d9d9;}
      #${UPDATE_ID} .ytg-track{height:4px;margin-top:12px;border-radius:2px;background:rgba(255,255,255,.2);overflow:hidden;}
      #${UPDATE_ID} .ytg-bar{height:100%;width:0;background:#f00;transition:width .15s linear;}
      #${UPDATE_ID} .ytg-bar.ytg-busy{width:30%;animation:ytg-busy 1.1s ease-in-out infinite alternate;}
      @keyframes ytg-busy{from{margin-left:0}to{margin-left:70%}}
    `;
  }

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) {
      node.className = className;
    }
    if (text) {
      node.textContent = text;
    }
    return node;
  }

  function showUpdate(info) {
    const core = window.__TAURI__ && window.__TAURI__.core;
    if (!core || document.getElementById(UPDATE_ID)) {
      return;
    }
    const style = el("style");
    style.textContent = updateStyle();
    mount(style);

    const card = el("div");
    card.id = UPDATE_ID;
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-label", "Update available");
    const title = el("p", "ytg-title");
    const text = el("p", "ytg-text");
    const actions = el("div", "ytg-actions");
    card.append(title, text, actions);
    mount(card);

    const close = () => {
      card.remove();
      style.remove();
    };
    const button = (label, kind, onClick) => {
      const node = el("button", kind, label);
      node.type = "button";
      node.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        onClick();
      });
      return node;
    };

    const offer = () => {
      title.textContent = "Update available";
      text.textContent = `YouTube Glass ${info.version} is ready to install. You're on ${info.currentVersion}.`;
      actions.replaceChildren(
        button("Later", "ytg-secondary", () => {
          core.invoke("dismiss_update").catch(() => {});
          close();
        }),
        button("Update", "ytg-primary", install)
      );
    };

    const install = () => {
      title.textContent = `Updating to ${info.version}`;
      text.textContent = "Downloading…";
      const track = el("div", "ytg-track");
      const bar = el("div", "ytg-bar");
      track.appendChild(bar);
      actions.replaceChildren();
      card.replaceChildren(title, text, track);

      const channel = new core.Channel();
      channel.onmessage = (message) => {
        if (message.event === "progress") {
          if (message.total) {
            const percent = Math.min(100, Math.round((message.downloaded / message.total) * 100));
            bar.classList.remove("ytg-busy");
            bar.style.width = `${percent}%`;
            text.textContent = `Downloading… ${percent}%`;
          } else {
            bar.classList.add("ytg-busy");
            text.textContent = `Downloading… ${(message.downloaded / 1048576).toFixed(1)} MB`;
          }
        } else if (message.event === "installing") {
          bar.classList.remove("ytg-busy");
          bar.style.width = "100%";
          text.textContent = "Installing… YouTube Glass will restart.";
        }
      };
      core.invoke("install_update", { onProgress: channel }).catch((err) => {
        console.error("ytg update", err);
        title.textContent = "Update failed";
        text.textContent = "Check your connection and try again.";
        card.replaceChildren(title, text, actions);
        actions.replaceChildren(button("Close", "ytg-secondary", close), button("Retry", "ytg-primary", install));
      });
    };

    offer();
  }

  function checkForUpdate() {
    const core = window.__TAURI__ && window.__TAURI__.core;
    if (!core) {
      return;
    }
    core
      .invoke("check_for_update")
      .then((info) => {
        if (info) {
          showUpdate(info);
        }
      })
      .catch(() => {});
  }

  function syncChrome() {
    const fullscreen = Boolean(document.fullscreenElement);
    const update = document.getElementById(UPDATE_ID);
    if (update) {
      update.style.display = fullscreen ? "none" : "block";
    }
    const button = ensureButton();
    mount(button);
    button.style.display = videoIsPresent() && !fullscreen ? "flex" : "none";

    const controls = ensureControls();
    mount(controls);
    const showControls = !fullscreen && Boolean(document.querySelector("ytd-masthead"));
    controls.style.display = showControls ? "flex" : "none";
    document.documentElement.classList.toggle("ytg-controls-visible", showControls);
  }

  function invokeCmd(name, args) {
    const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
    if (invoke) {
      invoke(name, args);
    }
  }

  function inDragZone(event) {
    // composedPath so a click inside any shadow root is judged by its real target.
    const path = event.composedPath().filter((node) => node instanceof Element);
    if (path.some((el) => el.matches(NO_DRAG))) {
      return false;
    }
    return path.some((el) => el.matches(DRAG_ZONE));
  }

  // Start the native move on the first real movement so plain clicks on the masthead
  // (e.g. to dismiss search suggestions) still reach the page.
  let dragStart = null;
  window.addEventListener(
    "mousedown",
    (event) => {
      dragStart = null;
      if (event.button !== 0 || !inDragZone(event)) {
        return;
      }
      if (event.detail === 2) {
        event.preventDefault();
        const win = appWindow();
        if (win) {
          win.toggleMaximize();
        }
        return;
      }
      dragStart = { x: event.screenX, y: event.screenY };
    },
    true
  );
  window.addEventListener(
    "mousemove",
    (event) => {
      if (!dragStart) {
        return;
      }
      if (!(event.buttons & 1)) {
        dragStart = null;
        return;
      }
      if (Math.abs(event.screenX - dragStart.x) + Math.abs(event.screenY - dragStart.y) < 4) {
        return;
      }
      dragStart = null;
      const win = appWindow();
      if (win) {
        win.startDragging();
      }
    },
    { passive: true }
  );
  window.addEventListener(
    "mouseup",
    () => {
      dragStart = null;
    },
    true
  );

  window.addEventListener(
    "enterpictureinpicture",
    () => {
      window.setTimeout(() => invokeCmd("hide_main_for_pip"), 80);
    },
    true
  );
  window.addEventListener(
    "leavepictureinpicture",
    () => {
      invokeCmd("restore_main_from_pip");
    },
    true
  );

  window.addEventListener(
    "keydown",
    (event) => {
      if (event.code === "F5" && !event.altKey) {
        event.preventDefault();
        event.stopPropagation();
        location.reload();
        return;
      }
      if (event.altKey && !event.ctrlKey && !event.shiftKey && event.code === "KeyP") {
        event.preventDefault();
        event.stopPropagation();
        toggleVideoPip();
      }
    },
    true
  );

  const start = () => {
    syncChrome();
    setInterval(syncChrome, 500);
    document.addEventListener("yt-navigate-finish", syncChrome, true);
    document.addEventListener("fullscreenchange", syncChrome, true);
    checkForUpdate();
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }
})();
