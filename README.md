# YouTube Glass

A thin YouTube desktop app for Windows: real youtube.com in WebView2 with its masthead as the title bar, and Chromium picture-in-picture.

## Download

Grab the latest **Windows installer** from [Releases](https://github.com/lukassaridis/youtube-glass/releases):

- `YouTube Glass_0.1.0_x64-setup.exe` — install for the current user (no admin)
- `youtube-glass.exe` — portable executable

Windows may warn that the app is unsigned. Choose **More info** → **Run anyway**.

## Develop

```bash
npm install
npm run tauri dev
```

```bash
npm run tauri build
```

## Release

The app checks GitHub Releases on launch and offers updates in-app. Updates are signed with the key in `~/.tauri/youtube-glass.key` (keep it safe; losing it means installed copies can no longer update).

1. Bump `version` in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
2. Build, sign and publish:

```bash
npm run release -- --notes "What changed" --publish
```

Without `--publish` the files (`YouTube.Glass_<version>_x64-setup.exe`, `youtube-glass.exe`, `latest.json`) are left in `src-tauri/target/release/bundle/release` to upload by hand. Every release must include `latest.json`.
