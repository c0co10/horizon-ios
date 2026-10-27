# Horizon on Android

The game itself, as a NativeActivity: no Java, no Gradle.

- [`crates/horizon-android`](../../crates/horizon-android) builds `horizon-client`'s bin code (`main.rs`,
  `game.rs`, `game/`) as `libhorizon_android.so`. Its `[lib] path` is the bin's own `main.rs`, because rustc
  emits no library from a bin target. It is its own workspace (the root one excludes it), so host builds never
  compile the game twice; its `Cargo.lock` started as a copy of the root one, keep the two in step.
- `crates/horizon-client/src/android.rs` is `android_main`: `HOME` becomes the app's private data folder (logs,
  settings: `files/.local/share/horizon`), the working directory its external files folder
  (`Android/data/dev.horizon.game/files`: saves, `resources/`, imported archives), then `main` runs as anywhere else.
- `AndroidManifest.xml`: `dev.horizon.game`, Android 8 (API 26) or later, landscape, Vulkan.

`.github/workflows/android.yml` builds, packs (aapt2), aligns, signs and uploads `Horizon.apk`. By hand, with an NDK:

```sh
bin=$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC_aarch64_linux_android=$bin/aarch64-linux-android26-clang AR_aarch64_linux_android=$bin/llvm-ar
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$CC_aarch64_linux_android
cargo build --release --manifest-path crates/horizon-android/Cargo.toml --target aarch64-linux-android
```

## Game files

The .apk carries no Mojang assets. As on iOS, a game with none waits on the import screen and asks for the ZIP
from `horizon-client --pack-resources` over Wi-Fi (see [../ios/README.md](../ios/README.md)), or take the cable:
`adb push Horizon-26.3.zip /sdcard/Android/data/dev.horizon.game/files/` and relaunch.

## Not done yet

- Leaving the app (home button, screen off) takes the window away; the game does not rebuild its surface, so
  coming back may show nothing or crash. Relaunch.
- No soft keyboard for text fields, no system clipboard (the game's own copy buffer only), no controllers
  (gilrs has no Android backend), no mods.
- Touch controls are the iOS ones (`game/touch.rs`), on from the start.
