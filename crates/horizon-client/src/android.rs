//! Android's way in: NativeActivity loads `libhorizon_android.so` (crates/horizon-android) and calls
//! `android_main` on its own thread, where `main` then runs as it does on every other platform.
use std::sync::OnceLock;
use winit::platform::android::activity::AndroidApp;

/// The activity, for `game::event_loop`.
pub(crate) static APP: OnceLock<AndroidApp> = OnceLock::new();

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    // The game's home (logs, crash reports, settings) is `$HOME/.local/share/horizon`, and an app has no HOME:
    // the private data folder takes its place. Set before any other thread of ours exists.
    if let Some(home) = app.internal_data_path() {
        unsafe { std::env::set_var("HOME", home) };
    }
    // Saves, resources and imported archives go where `adb push` and a file manager reach, as Documents
    // does on iOS: Android/data/dev.horizon.game/files.
    if let Some(files) = app
        .external_data_path()
        .or_else(|| app.internal_data_path())
    {
        let _ = std::fs::create_dir_all(&files);
        let _ = std::env::set_current_dir(&files);
    }
    let _ = APP.set(app);
    crate::main();
}
