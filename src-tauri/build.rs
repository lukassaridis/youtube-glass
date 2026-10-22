fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "open_video_pip",
            "hide_main_for_pip",
            "restore_main_from_pip",
        ])),
    )
    .expect("failed to run tauri-build");
}
