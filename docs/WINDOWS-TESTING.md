# Windows test checklist

Windows 10 or 11, x64. No admin rights needed.

1. Download `CopyWheel-Setup.exe` from https://github.com/guneskursad/copywheel/releases/latest
   - If SmartScreen says "Windows protected your PC": click **More info → Run anyway** (the installer is not signed).
2. Install with the default options. Keep **Start CopyWheel at login** checked. Let it launch at the end.
   - Expected: a CopyWheel icon appears in the system tray (maybe under the `^` overflow arrow). No console window.
3. Copy a few things: some text in Notepad, a link from the browser, a screenshot (`Win+Shift+S`).
4. Press **Ctrl+Shift+V**.
   - Expected: the panel opens near the mouse with the items from step 3 at the top.
   - Type part of an item to search. Arrow keys move. **Enter** pastes it into the app you were in (try Notepad).
5. Press **Ctrl+Alt+F**.
   - Expected: the wheel opens. Click a segment, then click an item in the list on the right; it pastes into Notepad.
   - Hold **Ctrl**: number badges appear.
6. Multi-monitor / scaling (if available): open the panel on the second screen and on a 125 % / 150 % scaled screen.
   - Expected: it opens where the mouse is, not on the wrong screen; text is sharp.
7. Password manager (if you use one: Bitwarden, 1Password, KeePass): copy a password.
   - Expected: the password does **not** appear in the CopyWheel history.
8. Settings (gear icon): check **Start CopyWheel at login** is on. Switch the theme Light/Dark.
9. Restart Windows (or sign out and in).
   - Expected: CopyWheel is in the tray after login, without any window popping up. Ctrl+Shift+V works.
10. Start CopyWheel again from the Start menu while it is already running.
    - Expected: no second copy (only one tray icon).
11. Uninstall: Settings → Apps → CopyWheel → Uninstall.
    - Expected: it asks whether to delete your CopyWheel data. After uninstall, nothing starts at the next login.

Please also note: Windows version (`winver`), screen scaling, and anything that looked wrong (black corners, blurry text, wrong position, slow open).
