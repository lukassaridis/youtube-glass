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
    // Watch / Shorts / embed only — homepage hover autoplay is not a session.
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
    // Call in the same click turn so Chromium keeps the user-gesture token.
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

  function syncButton() {
    const button = ensureButton();
    mount(button);
    const show = videoIsPresent();
    button.style.display = show ? "flex" : "none";
  }

  function invokeCmd(name, args) {
    const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
    if (invoke) {
      invoke(name, args);
    }
  }

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

  let headerHint = false;
  window.addEventListener(
    "mousemove",
    (event) => {
      const want = event.clientY <= 10;
      if (want === headerHint) {
        return;
      }
      headerHint = want;
      if (want) {
        invokeCmd("set_header_visible", { visible: true });
      }
    },
    { passive: true }
  );

  window.addEventListener(
    "keydown",
    (event) => {
      if (event.altKey && !event.ctrlKey && !event.shiftKey && event.code === "KeyP") {
        event.preventDefault();
        event.stopPropagation();
        toggleVideoPip();
      }
    },
    true
  );

  const start = () => {
    syncButton();
    setInterval(syncButton, 500);
    document.addEventListener("yt-navigate-finish", syncButton, true);
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }
})();
