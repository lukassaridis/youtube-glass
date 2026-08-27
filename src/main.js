const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const appWindow = getCurrentWindow();
let hideTimer = 0;

function showHeader() {
  window.clearTimeout(hideTimer);
  document.body.classList.add("header-visible");
  invoke("set_header_visible", { visible: true });
}

function hideHeader() {
  window.clearTimeout(hideTimer);
  hideTimer = window.setTimeout(() => {
    document.body.classList.remove("header-visible");
    invoke("set_header_visible", { visible: false });
  }, 160);
}

window.addEventListener("DOMContentLoaded", () => {
  document.getElementById("reload").addEventListener("click", () => invoke("reload_youtube"));
  document.getElementById("min").addEventListener("click", () => appWindow.minimize());
  document.getElementById("max").addEventListener("click", () => appWindow.toggleMaximize());
  document.getElementById("close").addEventListener("click", () => appWindow.close());

  document.documentElement.addEventListener("mouseenter", showHeader);
  document.documentElement.addEventListener("mouseleave", hideHeader);

  document.querySelector(".spacer").addEventListener("dblclick", () => appWindow.toggleMaximize());
  document.addEventListener("contextmenu", (event) => event.preventDefault());
});
