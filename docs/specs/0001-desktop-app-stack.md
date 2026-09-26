# 0001. Desktop app stack

**Date**: 2026-09-22
**Status**: In Progress

## Summary

Use Tauri 2 for a small macOS menu bar app. Keep the interface in plain TypeScript and put operating system work in Rust. Codex remains responsible for its own sign in and credentials. A separate desktop adapter will handle account switching only after that behavior is verified.

## Context

> ⚠️ Premise note: A desktop shell does not by itself isolate Codex accounts. The user wants an experimental workaround because Codex desktop does not support switching. The desktop behavior decision must prove login and history isolation plus identity readback before the switcher promises that a requested account is active.

The product is an open source menu bar tool for personal and work Codex accounts. It targets macOS 14 and newer. The first useful path switches one active desktop account. A later mode should allow both accounts to remain open if their login and history can be kept separate.

The app should stay local, start when the user signs in to macOS, and expose no server or cloud account. The first public release will not rely on an Apple Developer signing identity. The codebase is empty, so this decision establishes the application shape without migrating existing code.

## Options considered

### Option 1: Tauri 2 with plain TypeScript and Rust

Tauri provides the tray and webview shell while Rust owns local state and operating system integration. It keeps the interface small and leaves the account adapter replaceable. The cost is a boundary between TypeScript and Rust that needs explicit permissions and tests. (basis: Tauri system tray and Store documentation; one local deployable unit)

### Option 2: Electron with TypeScript

Electron has a mature tray API and keeps most application code in one language. It would make early interface work familiar, but it ships its own runtime and adds more process and packaging weight for a small menu bar utility. (basis: Electron Tray documentation; small local utility constraint)

### Option 3: Native SwiftUI with AppKit where needed

Apple's frameworks fit macOS closely and avoid a webview. They would give a compact native app, but the user chose Tauri and plain TypeScript, and a Swift codebase would not serve that preference. (basis: Apple MenuBarExtra and NSStatusItem documentation; engineer's selected framework)

## Decision

**Chosen option**: Option 1, Tauri 2 with plain TypeScript and Rust. (basis: engineer's selected framework; Tauri system tray documentation)

Build one local desktop app. Rust owns the native tray menu, profile operations, saved settings, desktop integration, and the switch result. Plain TypeScript renders only a small settings window and calls named Rust commands. The app has no Dock icon during normal use. A tray click opens the menu, Settings opens or restores the window, closing that window hides it, and Quit terminates the app. Rust refreshes the menu after each state change.

The Codex adapter is a replaceable Rust boundary. It reports `unavailable`, `needsSignIn`, `switching`, `verified(accountId)`, `identityUnknown`, or `failed(reason)`. Only a repeatable account readback proven in the desktop behavior experiment can produce `verified`. Opening Codex alone is not proof. The selected profile is a saved user choice, while verified identity is a separate runtime value that must be checked again after restart. Until the desktop behavior experiment identifies and validates a readback source, the adapter reports `identityUnknown` and the UI does not claim success. (basis: desktop behavior spec 0002; verify identity before reporting success)

The settings window is the only webview with permission to call the named Rust commands it needs. It loads local bundled content only. No broad shell, filesystem, or network permission is exposed to the webview. Rust alone reads and writes the preferences store. (basis: least privilege; Tauri capability model)

No Codex password, API key, access token, or copied session belongs in the switcher's settings store. Codex handles authentication through its own supported flow. (basis: official OpenAI authentication guidance; least privilege)

## Rationale

The app has one user, one Mac, and a small amount of local preference data. A local monolith avoids the setup and privacy cost of a server. Tauri fits the requested menu bar shape and the user's preference for a small app. Plain TypeScript keeps the interface simple, while Rust gives a clear place to contain any verified macOS interaction. (basis: one local deployable unit; Tauri system tray documentation)

The riskiest part is Codex account control, not interface rendering. Keeping the workaround in one adapter limits the impact of Codex updates. It also lets the first end to end slice exercise a real menu action without presenting a false account state. (basis: Tracer Bullet approach in project scope; verify identity before reporting success)

## Proposed stack

| Layer | Choice | Reason |
|---|---|---|
| Product shape | One local Tauri 2 app | No server is needed for one person's local profiles. |
| Native core | Rust | Owns desktop actions and the account adapter boundary. |
| Interface | Plain TypeScript with Vite | A small settings window does not need a larger UI framework. |
| Package manager | npm | Keeps the frontend setup familiar and avoids another required tool. |
| Menu bar | Rust owned Tauri tray menu | Provides the primary macOS entry point and one owner for menu state. |
| Local preferences | Tauri Store plugin, called from Rust | Persists labels and settings, never credentials. |
| Start at login | Tauri Autostart plugin | Registers on first launch by default and supports opt out. |
| Authentication | Codex's own sign in flow | The switcher does not become a credential manager. |
| Account integration | Replaceable Rust adapter | Supported behavior and identity readback can be added after verification. |
| Tests | Rust logic tests and one interactive macOS tray smoke flow | Covers the state boundary and a real visible path. |
| Release checks | GitHub Actions on macOS plus local checks | Confirms the app builds and tests on its target platform. |
| Distribution | Source build instructions for the first public release | No signed download is planned. |

Minimum macOS version: 14. Source builds support both Apple silicon and Intel Macs. Use bundle identifier `app.codexswitcher.desktop` consistently across source builds so macOS treats preferences and login registration as one app. Commit `package-lock.json` and a pinned `rust-toolchain.toml`; the scaffold records the tested Node, Rust, Tauri, and Xcode versions in the source build instructions. The app has no database, cloud sync, account service, or remote telemetry in this decision.

The Store file is `preferences.json` in a Tauri managed application data directory. It has a `schemaVersion` starting at 1. Rust is its only reader and writer. It saves profile labels, selected profile, and app settings, never Codex credentials or verified identity. The profile data model spec defines the full fields and migration rules. If the file is unreadable or malformed, the app preserves it for recovery, shows an error, and does not overwrite it silently. If a save fails, the previous settings remain the effective state and the menu reports the error.

On first launch, the app requests autostart registration. At macOS login it starts in the menu bar with the settings window hidden. A visible setting can disable registration, and a later launch must respect that choice. Registration failure is shown in settings and does not prevent manual use.

The first scaffold should make a real tray action open a small settings view, call Rust, and display a result. That proves the app shell only. The first real account switch remains gated by the desktop behavior experiment. If account isolation or readback fails, the menu reports the limitation rather than presenting a fake success state. (basis: Tracer Bullet approach in project scope; verify identity before reporting success)

The first public release is a source build. Instructions use the locked dependencies and pinned toolchain to build the app locally on macOS 14 or newer. GitHub Actions checks compilation and logic tests on macOS. A separate interactive smoke check launches the built app and confirms the tray appears, Settings opens and restores, a profile label persists after restart, launch at login can be disabled, Quit exits, and an unavailable adapter never appears verified. This smoke check does not stand in for the later real account switching check.

## Consequences

**Positive**:

* One local app is simple to build and inspect.
* The Codex integration can be replaced without rewriting the menu.
* Profile labels can be persisted without handling Codex secrets.

**Negative and tradeoffs**:

* The team must maintain both TypeScript and Rust.
* Tauri plugin permissions and the boundary between the webview and Rust need careful review.
* Rust now owns both the tray and preferences, which reduces split state but adds more Rust code.
* A source build asks each early user to install development tools. An unsigned download would create a poor macOS install experience.
* This stack cannot guarantee account switching until the desktop behavior decision is proven.

## Follow-up

* [ ] Complete `/architect desktop account behavior` before implementing the Codex adapter. Confirm the active account source, history boundary, and whether concurrent sessions are supported.
* [ ] Check that the scope's setup feature covers a source build release. The user no longer plans a signed download.
* [ ] Decide whether to add the community Tauri skills after reviewing the discovery results. No extra skill is required to scaffold this stack.

## References

**Project sources**:

* [Project scope](../scope/scope.md): product target and Tracer Bullet approach.
* Engineer's selections during architecture discussion: Tauri 2, plain TypeScript, macOS 14, local only, Codex owned credentials, and no signed download.

**Practices and standards**:

* One local deployable unit for a small desktop tool.
* Least privilege for credentials and desktop access.
* Tauri capability boundaries for the settings window.
* Verify identity before reporting an account switch as complete.

**Links**:

* [Tauri system tray](https://v2.tauri.app/learn/system-tray/)
* [Tauri Store plugin](https://v2.tauri.app/plugin/store/)
* [Tauri Autostart plugin](https://v2.tauri.app/plugin/autostart/)
* [Tauri WebDriver testing](https://v2.tauri.app/develop/tests/webdriver/)
* [Tauri GitHub Actions guidance](https://v2.tauri.app/distribute/pipelines/github/)
* [Electron Tray API](https://www.electronjs.org/docs/latest/api/tray)
* [Apple MenuBarExtra](https://developer.apple.com/documentation/swiftui/menubarextra)
* [Apple NSStatusItem](https://developer.apple.com/documentation/appkit/nsstatusitem)
* [OpenAI authentication guidance](https://learn.chatgpt.com/docs/auth)
