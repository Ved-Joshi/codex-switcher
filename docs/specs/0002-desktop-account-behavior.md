# 0002. Desktop account behavior

**Date**: 2026-09-22
**Status**: In Progress

## Summary

Build an experimental Codex desktop switcher instead of waiting for official account support. First prove that separate local desktop environments can keep two accounts and their histories apart. If that works, the menu bar app can launch the chosen environment and check its identity before reporting success. Moving cached login state is a possible fallback that needs a separate security decision.

## Context

The project aims to switch personal and work Codex desktop accounts on one Mac. The user clarified that working around the absence of official desktop switching is the point of the project. This replaces the earlier supported interfaces only constraint for this feature. Codex still owns interactive sign in, and the switcher must protect local account state.

OpenAI says account switching is available on ChatGPT web and is not yet supported in Codex desktop. Its desktop authentication guide documents sign out, browser sign in, and checking the active account in the profile menu. Login state may include live access tokens. The public `CODEX_HOME` documentation lists the CLI, IDE extension, app server, and installers, but not the desktop app. This leaves the desktop storage and identity behavior to verify experimentally. (basis: OpenAI account switching, authentication, and environment variables guidance)

On the developer's Mac, the installed app is an Electron bundle with identifier `com.openai.codex` and version `26.915.31945`. Local state directories exist under `~/Library/Application Support/Codex`, `~/Library/Application Support/com.openai.codex`, and `~/.codex`. Their existence alone does not show which data controls login or history. No credential or session content was inspected. (basis: read only local metadata inspection on 2026-09-22)

macOS can launch a new app instance with a specified environment. Electron documents a separate `userData` path for app configuration and browser session data; overriding that path is normally an app startup decision. A new instance or environment variable therefore proves only that a new process started, not that Codex account state is separate. (basis: Apple launch configuration and Electron app path documentation)

## Requirements

**User stories**:

* As a person with personal and work accounts, I want one menu choice to open Codex in the intended account environment.
* As a person doing work in Codex, I want account identity and history tied to the correct profile.
* As a person recovering from a failed switch, I want my previous desktop state intact.

**Acceptance criteria**:

* **AC-1**: On the recorded Codex version, a two account experiment in one macOS user session identifies the state locations observed during sign in, relaunch, sign out, and history checks, and shows whether each location is isolated.
* **AC-2**: Switching between experiment profiles, including after a full app restart, opens Codex under the intended account without exposing a token or password in the switcher UI, logs, or ordinary preferences.
* **AC-3**: A switch reports success only when the active desktop identity is read from a proven source and matches the selected profile. If no reliable source exists, it reports identity unknown.
* **AC-4**: A failed, canceled, or interrupted switch does not move, copy, delete, or overwrite either profile environment through the switcher. Cancellation stops switcher tracking but may leave Codex sign in running. It reports the previous profile restored only after that profile's identity is read back again; otherwise it reports identity unknown.
* **AC-5**: Concurrent mode remains unavailable until two running instances have independent login and history, and closing one does not sign out the other.
* **AC-6**: The experiment records the Codex version, macOS version, launch method, state locations, observed account and history behavior, and repeatable steps.

## Options considered

### Option 1: Isolated desktop environments

Launch Codex with separate local app data for each profile. Probe app data, `CODEX_HOME`, Chromium user data, and keychain behavior as distinct surfaces. This could keep credentials under Codex's control, but shared macOS storage or login redirect routing may defeat isolation.

### Option 2: Snapshot and restore desktop state

Quit Codex, save the active state, restore the selected profile's state, then relaunch. This could work if isolation fails. It may involve live access tokens, large histories, and version dependent files. It needs a separate decision on credential handling and tested rollback.

### Option 3: Separate macOS users

Run each account in a different macOS user session. The operating system provides a strong storage boundary, but this fails the user's requirement to use two existing accounts from one macOS login. It is not a required test setup or a product fallback.

## Decision

**Chosen approach for the feasibility slice**: Option 1, isolated desktop environments within the same macOS user session. The first implementation uses two accounts the user already controls and separate test roots beneath that user's home directory, not a release claim. Record metadata about the current Codex state before launching either test profile. Start with sequential launches after Codex is fully quit; test concurrent instances only after the sequential result is understood. Probe each isolation surface independently, then combine only those proven necessary. Keep Option 2 as a fallback decision if isolation cannot satisfy **AC-2** through **AC-4**. (basis: user's same user requirement; minimize handling of live credentials)

The Tauri app keeps a replaceable Rust desktop adapter. Its result states are `unavailable`, `needsSignIn`, `switching`, `verified(accountId)`, `identityUnknown`, and `failed(reason)`. A selected profile is user intent, not proof. The adapter may return `verified` only after the experiment demonstrates repeatable identity readback that does not require reading secret token content. The Codex profile menu, read through macOS Accessibility if its identity text is exposed, is the first candidate to test.

For the first quick access menu, selecting A or B means **Open or Focus**. If exactly one running Codex process has the expected bundle and an exact `--user-data-dir` argument for that profile's canonical Electron directory, activate that process. If none exists, launch a new instance using that profile's Codex home and Electron directory. If several processes match, or a launch cannot be tied back to its profile, show a failure or identity unknown and do not guess which window to focus. Process identifiers are refreshed from the running process list after switcher restart, since saved identifiers can be reused by macOS. A successful activation means only that the intended process was focused; it never means the account identity was verified. Keep the other profile running. Add an **Open Both** convenience action only after visible identity and history checks confirm concurrent isolation.

The first launch probe on this Mac uses `/usr/bin/open -n -a /Applications/ChatGPT.app --env CODEX_HOME=<profile root>/codex --args --user-data-dir=<profile root>/electron` with existing empty directories. This adapts the two directory launch approach from an MIT licensed macOS menu bar switcher. On another Mac, the harness resolves the installed bundle by identifier `com.openai.codex`. It must prove that the app actually writes login and history state beneath the intended roots. If it does not, that candidate fails even when a new process opens. The probe also needs evidence that Codex's own `userData`, keychain behavior, and background processes are isolated. The production adapter uses macOS `NSWorkspaceOpenConfiguration` to request a new instance with the proven environment and obtain a process reference. If Codex redirects the launch or sign in callback to another instance, the result is identity unknown.

## Rationale

Account separation is the product's defining behavior, so it must be shown with two real accounts and distinct visible history, not inferred from a directory name or a successful process launch. An isolated environment is the first path to test because it may let each profile retain its own state without copying live files during every switch. Snapshotting is more invasive and needs explicit safeguards.

## Feature design

**Data model sketch**: An experiment profile has a stable local identifier, user label, separate test root, expected account email and, when shown by Codex, expected workspace label, plus last verified app version. These are experiment inputs, not credentials. Comparison trims surrounding whitespace and ignores email case; it does not guess an identity from a display name alone. An ambiguous or missing email is identity unknown. The final profile schema belongs in the profile data model spec. The switch attempt records the previous profile identifier only if its identity was verified immediately before the attempt.

**State transitions**: `unavailable` → `needsSignIn` or `switching` → `verified(accountId)` only after readback. Sign in completion means matching identity readback, not a callback or window change. A user cancel action or a ten minute timeout ends switcher tracking but does not terminate a running Codex sign in. A later completion requires a fresh identity check. Process exit, launch failure, version change, or ambiguous identity leads to `identityUnknown` or `failed(reason)`. Restart clears prior verification until identity is checked again.

**Interface surface**:

| Action | Input | Output | Failure |
|---|---|---|---|
| Create experiment environment | Profile identifier, environment root | Empty isolated environment | Path is outside the test root, is a symlink, is owned by another user, or creation fails |
| Launch selected environment | Profile identifier, resolved Codex app path | Instance identifier and process identifier, or identity unknown | Existing session, redirect routing, launch failure |
| Check active identity | Tracked instance identifier, expected email and workspace | Matching account identifier or identity unknown | Missing, ambiguous, or mismatched readback |
| Restore prior environment | Previously verified profile identifier | Verified prior identity or identity unknown | Prior process unavailable, identity mismatch, or readback failure |

**Value sourcing**:

| Produced or displayed value | Source |
|---|---|
| Selected profile label | Switcher's local preferences |
| Expected account | User supplied during experiment setup |
| Actual desktop identity | Accessibility text from the tracked Codex profile menu, only if the experiment proves it repeatable |
| Codex version | Installed app bundle metadata |
| Codex app path | User selected app bundle validated by identifier `com.openai.codex`, or a discovered installed bundle with that identifier |
| Environment root | Switcher created path under its dedicated test root, owned by the local user and checked against symlink escape |
| Running instance | `NSWorkspace` launch result and process identifier, matched to the expected bundle and launch attempt |
| Previous profile | A profile whose desktop identity was verified immediately before switching |
| Launch result | macOS launch result plus the tracked running instance |
| History separation | Visible profile specific test chats or tasks created in the isolated test session |

**Key invariants**:

* A chosen profile, process, directory, or user acknowledgement is never treated as verified identity.
* The switcher never deliberately writes, moves, deletes, or copies the user's existing Codex state or cached credentials. A same user launch may still cause Codex itself to change shared login or history state; the probe must disclose this risk before launch and compare metadata against the baseline afterward.
* The adapter never deletes an old environment during a switch. Cleanup is a separate user action.
* Only one experimental global switch runs at a time. An interprocess lock in the switcher's app data directory rejects a second switcher instance's switch request. An app update clears prior verification until retested.
* The switcher never sends work to a profile whose identity is unknown.
* A launch redirected to an existing Codex instance does not inherit the selected profile's label or expected identity.
* A menu click never starts a duplicate instance when exactly one matching profile process is already running. A process match is based on the validated Codex bundle plus an exact profile data argument, not a window title or a stale process identifier.

**Security model**: The experiment runs in the user's existing macOS session with accounts they control. Test roots and reports use owner only permissions. Preparation and snapshots read file metadata only and do not change Codex state. Launch is a separate, explicit step after a baseline report and a user choice to proceed; the probe refuses the first sequential launch while Codex is already running. A new instance may still use shared app data or keychain entries and may change the current Codex session, so a successful process launch is never proof of isolation. The switcher sends no profile data to its own server and logs no secrets. Codex's own sign in and test work use its normal network service. A later credential snapshot design must specify storage, permissions, atomicity, backup, recovery, and deletion before implementation.

**Observation protocol**: The probe's aggregate file counts and candidate process IDs are an early signal only. For each A → B → A launch, a local manual observation record captures the visible account email, a distinct test task in each account's history, whether the other account's task appears, the behavior after fully quitting and relaunching, and any sign in routing or keychain prompt. Never inspect or export keychain item contents. If these observations do not establish separate login and history, **AC-1** and **AC-2** remain unverified even if the metadata changes.

**Critical test scenarios**:

* Switch A → B → A, restart, and see the correct identity and distinct history each time, verifies **AC-1**, **AC-2**, and **AC-3**.
* Cancel sign in or force a launch failure, then recheck the prior profile. Report restored only on matching readback, verifies **AC-4**.
* Trigger a timeout, allow Codex sign in to finish later, and require a new readback before reporting success, verifies **AC-3** and **AC-4**.
* Open an existing Codex instance, then test a second launch and browser sign in callback routing, verifies **AC-2** and **AC-3**.
* Try to run A and B together and check whether closing one signs out the other, verifies **AC-5**.
* Repeat the recorded experiment after a Codex update, verifies **AC-6**.

## Build plan

1. In the current macOS user session, add a harness that prepares two owner only test roots and records a recent baseline of app version, resolved bundle path, process state, and state location metadata. Keep launch behind an explicit live probe flag and a check for an already running Codex instance, begins **AC-1** and **AC-6**.
2. Prove one end to end switch A → B → A with distinct sign in and visible history, then document the required state surfaces, satisfies **AC-1** and **AC-2**.
3. Identify and validate identity readback, then connect adapter states to the menu without false success, satisfies **AC-3**.
4. Test canceled sign in, failed launch, restart, and rollback with the separate test roots, satisfies **AC-4**.
5. Probe two simultaneous instances and keep the menu mode disabled unless isolation is proven, satisfies **AC-5**.
6. Add experimental A and B **Open or Focus** tray actions using the observed process and profile roots. Reattach to matching running processes after switcher restart, activate exactly one match, or launch when no match exists. Keep identity unknown until independent readback works. Verify repeated clicks create no duplicate and one profile's close does not sign out the other. The user authorized this experimental menu work before the visible identity and history check; those checks remain required for **AC-2**, **AC-3**, and **AC-5**.

## Consequences

**Positive**:

* The project directly tests the workaround that motivated it.
* The experiment tests the same macOS user boundary the finished menu bar tool must use.

**Negative and tradeoffs**:

* The integration may break when Codex updates. The release must state the versions it has tested.
* The desktop app may share data through locations or services that launch flags cannot isolate.
* A live probe may affect the existing Codex session if Codex uses shared state. The baseline records metadata for detecting this but cannot itself prevent the change.
* Reliable identity readback may be harder than isolated launching.
* Source builds for other Mac users need a repeatable compatibility check.

## Follow-up

* [ ] Decide whether credential snapshots are allowed if isolated environments fail.
* [ ] Run the same user two account experiment before designing the final switch operation.
* [ ] Design profile persistence and the menu interface after the experiment fixes the state and identity contract.

## References

**Project sources**:

* [Project scope](../scope/scope.md): desktop account goals and Tracer Bullet approach.
* [Desktop stack spec](0001-desktop-app-stack.md): adapter states and verification rule.
* User correction: work around missing official desktop account switching.

**Links**:

* [OpenAI account switching help](https://help.openai.com/en/articles/20001068)
* [OpenAI authentication guidance](https://learn.chatgpt.com/docs/auth)
* [OpenAI environment variables](https://learn.chatgpt.com/docs/config-file/environment-variables)
* [Apple new application instance](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/createsnewapplicationinstance)
* [Apple launch environment](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/environment)
* [Electron app data paths](https://www.electronjs.org/docs/latest/api/app)
* [Edi Hasaj's MIT licensed menu bar switcher](https://github.com/edihasaj/codex-account-switcher): working example of separate `CODEX_HOME` and Electron user data roots in one macOS login, used as the basis for the first launch probe.
