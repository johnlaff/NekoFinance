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

The canonical home for the keystore is `~/.local/share/neko-finance/` (durable, outside every
worktree, so a build worktree can be deleted and recreated without touching it):

```bash
mkdir -p ~/.local/share/neko-finance && chmod 700 ~/.local/share/neko-finance
cd ~/.local/share/neko-finance
keytool -genkeypair -v \
  -keystore neko-finance-release.jks \
  -alias neko-finance \
  -keyalg RSA -keysize 2048 -validity 10950 \
  -storepass "<store password>" -keypass "<same password>"
```

Modern `keytool` defaults to a PKCS12 keystore, which does not support a key password different
from the store password — it silently ignores `-keypass` and reuses the store password for the
key. Pass the same value to both flags (or omit `-storepass`/`-keypass` to be prompted) rather than
generating two different passwords that PKCS12 will not actually honor. Identity fields (`-dname`
or the interactive prompts) can be any values — this key is never submitted to a store, it only
needs to stay stable across releases so Android accepts each new APK as an update to the same app.
Validity should be 25+ years: the keystore has to outlive the app.

Then create `keystore.properties` next to the `.jks` (same directory, `chmod 600` both files):

```properties
storeFile=/home/<you>/.local/share/neko-finance/neko-finance-release.jks
storePassword=<the password you set>
keyAlias=neko-finance
keyPassword=<the same password>
```

`storeFile` must be an **absolute path** here, since a release build worktree is disposable and
`keystore.properties` itself lives inside it (copied in, not generated by, the build): copy
`~/.local/share/neko-finance/keystore.properties` to `src-tauri/gen/android/keystore.properties`
in whichever worktree is building the release. `src-tauri/gen/android/app/build.gradle.kts` reads
that copy and wires the `release` signing config only when it exists; without it,
`scripts/build-android.sh` refuses to produce a release build (use `--debug` for an unsigned local
test build instead).

Back up `~/.local/share/neko-finance/` (the `.jks` and `keystore.properties` together) to a
personal password manager or secrets vault outside this machine — losing the keystore means every
future release can no longer update the currently installed app; the only recovery is uninstalling
and reinstalling fresh, which drops local app data (`neko-finance.db` and the rest of the
app-private storage).

## 2. Registering the Google OAuth credential

The Android consent flow needs a Google Cloud credential of **type Android** — Google's OAuth
policy rejects a custom-scheme redirect from any other credential type ("Erro 400:
invalid_request" on the consent screen is that rejection). An Android credential authenticates a
build by the pair (package name, certificate SHA-1) instead of a shared secret, and PKCE alone
secures the code exchange (`oauth::secret_for_exchange` never sends one on this path).

Register one Android credential per signing key that will run a real consent flow (debug and
release keystores produce different SHA-1 fingerprints — most maintainers register both) in the
Google Cloud console: APIs & Services → Credentials → Create Credentials → OAuth client ID →
Android, with:

| Field | Value |
| --- | --- |
| Package name | `app.neko.finance` (the `applicationId`, same value the manifest and `adb install`/`uninstall` commands use) |
| SHA-1 certificate fingerprint | from the signing keystore — see below |

Get the fingerprint from the release keystore provisioned in step 1:

```bash
keytool -list -v -keystore ~/.local/share/neko-finance/neko-finance-release.jks -alias neko-finance | grep SHA1
```

or from the Android SDK's auto-generated debug keystore (used by unsigned `--debug` builds):

```bash
keytool -list -v -keystore ~/.android/debug.keystore -alias androiddebugkey -storepass android | grep SHA1
```

The client id Google issues for an Android credential is **public** (unlike the Desktop
credential's secret) and lives versioned in source, never in `.env` — a variable baked at build
time already produced broken builds when it landed empty or mistyped. It appears in two places
that have to stay byte-for-byte in sync (there is no single source of truth to derive one from the
other — the JSON config and the Rust constant are compared by CI, but a value edited in only one
would drift silently otherwise):

- `src/lib/env.ts` → `GOOGLE_ANDROID_CLIENT_ID`
- `src-tauri/src/oauth/redirect.rs` → `ANDROID_OAUTH_SCHEME` (the same id, reversed — the
  redirect scheme Google requires for a custom-URI installed app,
  `REVERSED_CLIENT_ID:/oauth2redirect`)
- `src-tauri/tauri.conf.json` → `plugins.deep-link.mobile[0].scheme` (same reversed id) and
  `src-tauri/gen/android/app/src/main/AndroidManifest.xml` (the generated `<data
  android:scheme>`/`<data android:path>` pair the deep-link plugin derives from it)

## 3. Building

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

## 4. Installing and updating over ADB

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
