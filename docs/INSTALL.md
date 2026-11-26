# Install CopyWheel

CopyWheel requires macOS 11 or later.

## Install from the DMG

The app in the DMG is ad-hoc signed and unnotarized, so Gatekeeper may ask for approval the first time you open it.

1. Open the DMG and drag **CopyWheel.app** to **Applications**.
2. Eject the DMG and open CopyWheel from **Applications**. If macOS blocks the first launch, right-click CopyWheel and choose **Open** where available. You can also try opening it, then go to **System Settings → Privacy & Security → Open Anyway**. On macOS 11 or 12, look under **System Preferences → Security & Privacy → General**. [Apple’s first-open instructions](https://support.apple.com/en-us/102445)
3. For auto-paste, enable CopyWheel under **System Settings → Privacy & Security → Accessibility**. On macOS 11 or 12, use **System Preferences → Security & Privacy → Privacy → Accessibility**.

## Default shortcuts

- **Cmd+Shift+V** opens the history panel.
- **Ctrl+Shift+F** opens the wheel (change it in Settings).

## Build from source

Install [Rust with rustup](https://rustup.rs/) and the Xcode Command Line Tools (`xcode-select --install`). From the source directory, run `scripts/make-app.sh` to build `CopyWheel.app`.

## Data and uninstall

CopyWheel stores its data in `~/Library/Application Support/CopyWheel`. To uninstall, quit CopyWheel from the menu bar, delete the app from **Applications**, and delete that folder to remove your saved data.
