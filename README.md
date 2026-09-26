# Codex Switcher

A local macOS menu bar tool for opening several Codex desktop accounts from one macOS login. Click the menu bar icon to see a compact account panel, choose an account, or connect another one. No second macOS user is needed.

## Current behavior

- Each account has a separate Codex home and Electron data directory. The switcher keeps existing A and B profiles in place and creates a new private directory for each additional account. Local project folders can be opened from any profile, while Codex sign-in and history stay in that profile's directories.
- **Open** focuses a matching Codex process or launches that profile. **Sign in** opens a new profile for Codex's own sign-in flow. The switcher does not copy, read, or store credentials.
- The panel uses each signed-in email as its default name. The edit control beside a name saves a custom name; clearing it restores the email. Cards show the plan and remaining 5 hour and weekly usage with reset times when Codex reports them. Usage refreshes whenever the panel opens and every 30 seconds while it stays open. The refresh button fetches again. Reads are limited to three profiles at a time so a long account list does not start many Codex app-server processes at once.
- **Connect another account** opens Codex for sign-in. An unfinished setup stays out of the account list and is reused if Connect is pressed again. It appears once Codex reports a signed-in email. There is no fixed two-account limit. The account list scrolls within the menu bar panel.
- Each account card has a Remove action with an inline confirmation. Remove hides the account without deleting its Codex data. Settings shows removed accounts with their Codex email when available and offers Restore. Settings also includes Open at login, version details, an update check, and Quit. In development builds, Open at login is disabled because the development binary has no stable location.
- The menu bar app checks GitHub Releases for updates when the panel opens, at most once every six hours. Settings can check immediately and open the latest release page when a newer version exists. This source build does not download or install updates inside the app.

This is an experimental workaround for Codex desktop. The account and usage readouts come from Codex app-server in each profile. A matching process and a signed-in profile do **not** prove which identity a visible Codex window uses or which account a new task will debit. Before relying on usage routing, check the email shown in each Codex window and verify a small task against that account's usage. The [desktop test report](docs/reviews/desktop-probe-2026-09-23.md) records what has been verified so far.

The profile launch approach and app-server transport were adapted from open source work described in [research](docs/research/open-source-switchers.md) and [third-party notices](THIRD_PARTY_NOTICES.md).

The project is MIT licensed. Adapted source retains the upstream notices listed in [third-party notices](THIRD_PARTY_NOTICES.md).

## Run from source

Requires macOS 14 or newer, Node.js, npm, Rust, the Codex CLI, and Xcode Command Line Tools. The Rust toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml). The Codex CLI is needed for account and usage readouts; the desktop launch action does not need it.

```sh
npm ci
DEVELOPER_DIR=/Library/Developer/CommandLineTools npm run tauri dev
```

Click the menu bar icon to open the account panel directly. Click again or press Escape to close it. Settings in the panel has Quit. There is no Dock icon.

To check or package the app locally:

```sh
npm run build
DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo test --manifest-path src-tauri/Cargo.toml
DEVELOPER_DIR=/Library/Developer/CommandLineTools npm run tauri build
```

The packaged app is currently unsigned. Preferences stay on this Mac. Codex credentials remain owned by Codex.

## Updates and releases

The update check uses the latest published release of [Ved-Joshi/codex-switcher](https://github.com/Ved-Joshi/codex-switcher). Release tags should use semantic versions such as `v0.2.0`, and the version in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` must match. Until a release is published, Settings reports that no release is available. This project currently publishes source builds only. In-app installation would require Tauri updater signatures and a distribution pipeline.
