# Building the Android APK

The Android target reuses the same Tauri project as desktop and Windows — same React UI, same
Rust core, cross-compiled to `aarch64-linux-android` via `cargo-ndk` and packaged by the
Android Gradle Plugin. The generated project (`src-tauri/gen/android/`) is versioned, per
ADR-0014 clause 2: platform capabilities enter through adapter traits selected by
`cfg(target_os)`, so the same commands and domain code run on every target without a fork.

Distribution is sideload-only (ADR-0014, spec 044 Out of Scope: no Play Store). Release is a
signed APK installed and updated by hand over ADB inside the maintainer's private mesh — there is
no CI Android build and no automatic updater on this platform (the OS scheduler and the Tauri
updater plugin are both absent on Android; see `src-tauri/src/os_scheduler.rs` and
`src/features/updater/`).

## Toolchain (pinned by the spec-042 gate)

| Component                       | Version                                                                                                                                                                                                        |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Android SDK                      | `/opt/android-sdk` (platform 36, build-tools, platform-tools)                                                                                                                                                 |
| NDK                               | 28.2.13676358                                                                                                                                                                                                 |
| cargo-ndk                         | 4.1.2                                                                                                                                                                                                          |
| Rust targets                      | `aarch64-linux-android` (the only ABI exercised on the reference device); `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android` installed for `--all-abis` builds                          |
| rustc / cargo                     | 1.96.0                                                                                                                                                                                                         |
| AGP (Android Gradle Plugin)       | 8.11.0                                                                                                                                                                                                         |
| Gradle                            | 8.14.3                                                                                                                                                                                                         |
| Kotlin                            | 1.9.25                                                                                                                                                                                                         |
| compileSdk / targetSdk / minSdk   | 36 / 36 / 24                                                                                                                                                                                                   |
| JDK for the Gradle daemon         | Temurin 21 — **required**. Gradle 8.14.3 does not run under JDK 25 ("Unsupported class file major version 69"). `JAVA_HOME` must point at a JDK 21 for the duration of the build only — never pinned in `gradle.properties`, the JDK 21 path is per-machine. |
| Linker flags                      | Only what `tauri-cli` already sets per target by default (`-Clink-arg=-landroid -Clink-arg=-llog -Clink-arg=-lOpenSLES`); no extra flags needed.                                                             |

No 128-bit floating-point symbol issue (the known problem with old NDKs and `sqlx-sqlite`) shows
up with this NDK/cargo-ndk combination.

### One-time environment setup

The SDK/NDK live in `/opt/android-sdk`, group `android` (add your user to it, `chmod g+s`
recursively so builds don't need `sudo`). Export in `/etc/profile.d/android.sh`:

```bash
export ANDROID_HOME=/opt/android-sdk
export ANDROID_SDK_ROOT=/opt/android-sdk
export NDK_HOME=/opt/android-sdk/ndk/28.2.13676358
export ANDROID_NDK_HOME=/opt/android-sdk/ndk/28.2.13676358
```

Debian's `/etc/zsh/zprofile` does not source `/etc/profile.d/` on its own (unlike bash's
`/etc/profile`) — add an explicit loop there, or these variables never reach a login zsh session
(the default shell on this machine).

`cargo-ndk` and the Rust targets: `cargo install --locked cargo-ndk && rustup target add
aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android`.

## 1. Provisioning the release keystore (one-time, per machine or per maintainer)

The signed release build needs the maintainer's own keystore — generated once, kept **outside the
repository**, never committed. `src-tauri/gen/android/.gitignore` already excludes `*.jks`,
`*.keystore`, and `keystore.properties`.

```bash
cd src-tauri/gen/android
keytool -genkeypair -v \
  -keystore neko-finance-release.jks \
  -alias neko-finance \
  -keyalg RSA -keysize 2048 -validity 10000
```

`keytool` prompts for a store password, a key password, and identity fields (any values — this
key is never submitted to a store, it only needs to stay stable across releases so Android accepts
each new APK as an update to the same app). Then create `keystore.properties` next to it (same
directory, also gitignored):

```properties
storeFile=neko-finance-release.jks
storePassword=<the store password you set>
keyAlias=neko-finance
keyPassword=<the key password you set>
```

`src-tauri/gen/android/app/build.gradle.kts` reads this file and wires the `release` signing
config only when it exists; without it, `scripts/build-android.sh` refuses to produce a release
build (use `--debug` for an unsigned local test build instead). Back up the `.jks` file and both
passwords somewhere durable outside the repo — losing the keystore means every future release can
no longer update the currently installed app; the only recovery is uninstalling and reinstalling
fresh, which drops local app data (`neko-finance.db` and the rest of the app-private storage).

## 2. Building

```bash
npm run build:android            # release APK, aarch64 only (the reference device's ABI)
npm run build:android -- --all-abis   # release APK, every ABI
npm run build:android -- --debug      # debug-signed APK, no keystore.properties needed
```

`scripts/build-android.sh` switches `JAVA_HOME` to a JDK 21 discovered via `mise` for the duration
of the build (falls back to whatever `JAVA_HOME` already points at, letting Gradle's own version
check fail with its real error if no JDK 21 is available), then runs `npx tauri android build`.
The built APK path is printed at the end; it also lives under
`src-tauri/gen/android/app/build/outputs/apk/`.

## 3. Installing and updating over ADB

There is no Play Store and no in-app updater on this platform — every install and update is a
manual ADB command, run inside the maintainer's private mesh (Tailscale) once the device is paired
(see the `adb-remoto` skill for connecting/reconnecting).

```bash
adb devices                       # confirm the device is attached
adb install -r path/to/app.apk    # -r: replace, keeps app data if the signature matches
```

**Signature changes are not updates.** Android refuses to install an APK signed with a different
key over an existing install of the same `applicationId` (`app.neko.finance`) —
`INSTALL_FAILED_UPDATE_INCOMPATIBLE`. This matters most the first time a signed release replaces a
debug build (Tauri's debug builds use the Android SDK's auto-generated debug keystore, never the
release one): `adb uninstall app.neko.finance` is required first, which deletes all local app data
including `neko-finance.db`. Back up anything worth keeping before that uninstall —
`adb pull /data/data/app.neko.finance/neko-finance.db <local backup path>` requires either a
debuggable build or `run-as app.neko.finance` on a device with developer options enabled; pull the
`-wal`/`-shm` sidecar files alongside it (and ideally `am force-stop` the app first) for a
consistent snapshot, since SQLite may not have checkpointed the WAL into the main file yet.

Every signed release after that first swap is a normal `adb install -r` — same key, same
`applicationId`, data preserved.
