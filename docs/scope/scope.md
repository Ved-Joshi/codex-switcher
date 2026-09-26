# Scope: Codex Account Switcher

A macOS menu bar app for people who keep personal and work Codex accounts. You can select an account, open the Codex desktop app under that account, and recognize which account and history you are using. The project is a small open source tool for other Mac users too.

**Build approach:** Tracer Bullet (prove one real account switch from the menu bar through the desktop app before adding breadth).
**Workflow:** Beta (verify the real app, then test it after development). Account and credential work uses GA. You may run or skip any suggested step.

_These are recommendations to keep your build orderly. You decide when a feature is done._

## At a glance

| # | Feature | Phase | Status |
|---|---|---|---|
| 1 | Desktop account behavior | Foundation | in-progress |
| 2 | Stack and architecture | Foundation | in-progress |
| 3 | Coding standards and tooling | Foundation | planned |
| 4 | Profile data model | Foundation | planned |
| 5 | Menu bar interface foundation | Foundation | planned |
| 6 | One active account switch | Slice 1 | planned |
| 7 | Safe switching and recovery | Slice 2 | planned |
| 8 | Setup for other Mac users | Slice 3 | planned |
| 9 | Concurrent account sessions | Slice 4 | planned |
| 10 | Profile management | Slice 5 | planned |

## Foundations

### 1. Desktop account behavior · in-progress · GA
Establish which account actions the Codex desktop app supports and how account history behaves. This is the feasibility gate for the whole product, including the desired concurrent mode.

**Done when:** a recorded decision identifies a reliable path for switching one active account and opening Codex, shows whether two isolated desktop sessions can coexist, and states what to do if they cannot.

- [x] Design it (spec): `/architect desktop account behavior`
- [ ] Run the same user two account probe: `/develop desktop account behavior`

Spec [0002](../specs/0002-desktop-account-behavior.md). Probe harness: [desktop_probe.py](../../tools/desktop_probe.py).

### 2. Stack and architecture · in-progress
Choose the application shape and create a runnable macOS project that fits the verified desktop behavior.

**Done when:** the stack decision is recorded, and a fresh checkout can run a minimal menu bar app locally.

- [x] Decide the stack (spec): `/architect stack and architecture`
- [x] Scaffold from the decision: `/develop stack and architecture`
- [ ] Verify it: `/check verify stack and architecture`
- [ ] Test it: `/test stack and architecture`

Spec [0001](../specs/0001-desktop-app-stack.md)

Code [src-tauri](../../src-tauri) and [src](../../src).

### 3. Coding standards and tooling
Capture conventions from the runnable project, then put the chosen checks in place.

**Done when:** project guidance describes the actual app, and its chosen checks run cleanly.

- [ ] Capture conventions and tooling: `/audit`
- [ ] Install the tooling: `/develop tooling`

### 4. Profile data model · needs a decision
Define account labels, the selected profile, and the boundary between switcher data and Codex account state. Keep personal and work history distinct.

**Done when:** the model can represent two accounts without storing passwords or tokens in its ordinary profile list, and it can tell which account the user selected.

- [ ] Design it (spec): `/architect profile data model`

### 5. Menu bar interface foundation · needs a decision
Set the small interface pattern for showing the selected account, switching state, and clear failure messages.

**Done when:** the menu bar shell has accessible labels, keyboard access, and visible idle, switching, and error states.

- [ ] Design it (spec): `/architect menu bar interface foundation`

## Slice 1: One real account switch

### 6. One active account switch · needs a decision · GA
Connect a menu bar choice to a real Codex desktop account change and launch. Start with two named accounts and one active desktop session.

**Done when:** you can choose personal or work, complete the supported sign in flow when needed, open Codex under the chosen account, and see that account's history.

- [ ] Design it (spec): `/architect one active account switch`

## Slice 2: Make switching dependable

### 7. Safe switching and recovery · needs a decision · GA
Handle a canceled sign in, an unavailable account, or work still running under the old account. Make the current identity clear before work starts.

**Done when:** a failed or interrupted switch leaves the previous account usable, secrets stay out of logs and UI, and the app never claims a switch succeeded when identity is uncertain.

- [ ] Design it (spec): `/architect safe switching and recovery`

## Slice 3: First open source release

### 8. Setup for other Mac users
Make the global switching path installable and understandable without requiring knowledge of the developer's machine.

**Done when:** another Mac user can install, set up two accounts, switch between them, recover from a common sign in failure, and uninstall the switcher without losing Codex account data.

- [ ] Build it: `/develop setup for other Mac users`

## Slice 4: Two accounts open at once

### 9. Concurrent account sessions · needs a decision · GA
Add a mode that keeps personal and work Codex desktop sessions open at the same time, if the foundation proves that their login and history can be isolated reliably.

**Done when:** both accounts can be open together, actions and history remain tied to the correct account, and closing one session does not sign out the other.

- [ ] Design it (spec): `/architect concurrent account sessions`

## Slice 5: Manage saved profiles

### 10. Profile management
Let you rename or remove a saved profile and keep the menu useful as your accounts change.

**Done when:** changes affect only the chosen profile, and removal explains whether any account history would be affected before proceeding.

- [ ] Build it: `/develop profile management`

## Deferred

* **Automatic account choice per project:** suggest an account from the current project while keeping the selected identity visible.
* **Codex CLI switching:** extend the product to the CLI after the desktop flow is dependable.
* **Windows and Linux:** plan each platform when there is demand and a verified desktop integration path.
* **Usage and billing display:** add only if supported account data is available and useful.

## Legend

**The decision box:** A feature with a real decision starts with `/architect`. After its spec exists, that skill fills in build milestones and suggested checks.

**Status:** `planned` means work has not started. Later skills can move a feature to `in-progress` and then `done`.

**Workflow:** Beta suggests `/check verify` and `/test` after `/develop`. GA also suggests a fresh review and documentation for account work.
