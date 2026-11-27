# Install CopyWheel

Requirements: macOS 11+ or Windows 10/11 x64.

## macOS

### Install from the DMG

The app in the DMG is ad-hoc signed and unnotarized, so Gatekeeper may ask for approval the first time you open it.

1. Download [`CopyWheel.dmg`](https://github.com/guneskursad/copywheel/releases/latest/download/CopyWheel.dmg), open it, and drag **CopyWheel.app** to **Applications**.
2. Eject the DMG and open CopyWheel from **Applications**. If macOS blocks the first launch, right-click CopyWheel and choose **Open** where available. You can also try opening it, then go to **System Settings → Privacy & Security → Open Anyway**. On macOS 11 or 12, look under **System Preferences → Security & Privacy → General**. [Apple’s first-open instructions](https://support.apple.com/en-us/102445)
3. For auto-paste, enable CopyWheel under **System Settings → Privacy & Security → Accessibility**. On macOS 11 or 12, use **System Preferences → Security & Privacy → Privacy → Accessibility**.

### Start at login

Start at login is enabled by default. CopyWheel manages a user LaunchAgent at `~/Library/LaunchAgents/local.copywheel.plist` when running from an `.app` bundle. You can turn it on or off in **Settings > General** (**Start CopyWheel at login**).

### Data and uninstall

CopyWheel stores its data in `~/Library/Application Support/CopyWheel`. To uninstall, disable **Start CopyWheel at login** in Settings (or remove `~/Library/LaunchAgents/local.copywheel.plist`), quit CopyWheel from the menu bar, delete the app from **Applications**, and delete that data folder to remove your saved history and favorites.

## Windows

### Install with the setup executable

1. Download and run [`CopyWheel-Setup.exe`](https://github.com/guneskursad/copywheel/releases/latest/download/CopyWheel-Setup.exe).
2. The installer is unsigned, so Windows Defender SmartScreen may display a warning. Click **More info**, then click **Run anyway**.
3. Follow the installer prompts. CopyWheel installs per user into `%LOCALAPPDATA%\Programs\CopyWheel` without requiring administrator privileges.

### Installer options and start at login

- **Start CopyWheel at login** is checked by default. It registers a per-user startup entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` and can be toggled at any time in **Settings > General**.
- **Create a desktop shortcut** is unchecked by default.

### Data and uninstall

CopyWheel stores its data in `%APPDATA%\CopyWheel`. To uninstall, open **Settings > Apps**, select **CopyWheel**, and choose **Uninstall**. The uninstaller removes the startup entry automatically and asks before deleting your user data in `%APPDATA%\CopyWheel`.

## Default shortcuts

| Action | macOS | Windows |
| --- | --- | --- |
| Open panel (History, Favorites, Settings) | `Cmd+Shift+V` | `Ctrl+Shift+V` |
| Open wheel | `Ctrl+Shift+F` | `Ctrl+Alt+F` |
| Open favorites list | `Ctrl+Alt+F` | `Ctrl+Alt+Shift+F` |

All global shortcuts can be changed in **Settings**.

## Verify the download

Each GitHub Release includes a `SHA256SUMS` file with SHA-256 checksums for the macOS DMGs and Windows installers.

**macOS:**

```sh
shasum -a 256 CopyWheel.dmg
```

**Windows (PowerShell):**

```powershell
Get-FileHash CopyWheel-Setup.exe -Algorithm SHA256
```

Compare the printed hash against the corresponding line in `SHA256SUMS` from the release page.

## Build from source

**macOS:** Install [Rust with rustup](https://rustup.rs/) and the Xcode Command Line Tools (`xcode-select --install`). From the repository root, run `scripts/make-app.sh` to build `CopyWheel.app`, or `scripts/make-dmg.sh` to package the DMG and `SHA256SUMS` in `dist/`.

**Windows:** Install [Rust with rustup](https://rustup.rs/). From the repository root, run `cargo build --release --locked` to produce `target\release\copywheel.exe`. To build the installer (`dist\CopyWheel-Setup.exe`), install Inno Setup 6 and run `iscc installer\windows\copywheel.iss`.

## Maintainers: release order

1. Build the macOS DMG locally (`scripts/make-dmg.sh`) and create the GitHub Release with the macOS DMGs and `SHA256SUMS` first.
2. Push the `v*` tag. The tag push triggers the Windows release workflow (`.github/workflows/release.yml`), which builds `CopyWheel-Setup.exe` and the versioned Windows installer, uploads them to the release, and appends their checksums to `SHA256SUMS` while preserving the existing macOS entries.
