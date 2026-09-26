# Same-user Codex desktop probe, 2026-09-23

## Setup

- macOS user session: one user, with an existing Codex desktop process left running.
- Codex bundle: `/Applications/ChatGPT.app`, identifier `com.openai.codex`, version `26.917.62051` (build `10789`).
- Experimental roots: `~/Library/Application Support/Codex Switcher/probe/profiles/{A,B}/{codex,electron}`.
- Launch method: `open -n -a` with `CODEX_HOME` and `--user-data-dir` set per profile.
- The probe read path counts and process metadata only. It did not read or copy credential contents.

## Observations

| Check | Result |
|---|---|
| Original Codex process before launch | PID 95510 |
| Launch A while original remained running | New PID 20652; original PID remained. The A process command line contained the A Electron data argument. |
| A data roots | A `codex` and `electron` roots changed from empty to populated. A `auth.json` exists; `CODEX_HOME=A codex login status` reported `Logged in using ChatGPT`. |
| Launch B while original and A remained running | New PID 21699; both prior PIDs remained. The B process command line contained the B Electron data argument. |
| B data roots | B `codex` and `electron` roots changed from empty to populated. At the time of the check, `CODEX_HOME=B codex login status` reported `Not logged in`; its browser sign-in flow remained open. |
| B after user completed sign-in | `CODEX_HOME=B codex login status` reported `Logged in using ChatGPT`. The user designated A as the first account signed into and B as the latter. |
| Restart A while B remained running | A's original test PID exited. Relaunch produced PID 23204 with the A Electron argument; A and B both still reported signed in through their own `CODEX_HOME`. |
| Restart B while A remained running | B's original test PID exited. Relaunch produced PID 24334 with the B Electron argument; A and B both still reported signed in through their own `CODEX_HOME`. |
| Default and shared data | File counts and byte totals changed while the original app was active. These aggregate changes cannot be attributed to A, B, or ordinary activity in the original app. |
| Auth file metadata | The original `~/.codex/auth.json` modification time remained 2026-09-15 23:33:19; separate A and B auth files were created during this test. File contents were not read. |

The local metadata reports are under `~/Library/Application Support/Codex Switcher/probe/reports/`: baseline `1790211495825106000-snapshot.json`, A launch `1790211528614202000-launch.json`, post-A `1790211582112644000-snapshot.json`, B launch `1790211695748761000-launch.json`, A restart `1790211888038264000-launch.json`, and B restart `1790211951636588000-launch.json`.

## Result and remaining checks

The dual-root method can start two additional Codex processes in this macOS user session, and each process uses its own requested Electron data argument. After sign-in, the CLI reports both experimental roots signed in. Both states survived separate process restarts while the other experimental profile and the original Codex app remained running. This supports process and some state separation, but it does **not** prove distinct desktop identities or history isolation.

To complete the A → B → A acceptance check, read each instance's visible account identity and create a distinct test task in each. Then confirm that each instance retains its correct identity and task history after these restarts. The CLI login checks suggest that closing one instance did not sign out the other, but that also needs visible desktop confirmation. Until those checks pass, the menu must continue to show identity unknown and offer no verified switch action.

## Experimental menu adapter follow-up

The Tauri menu now has A and B **Open or Focus** actions. The native adapter checks the Codex bundle and exact profile data argument before activating a running process. In a live integration check, it focused both existing profile processes and the Codex process count stayed at three, including the original session. A second check stopped only B, called the adapter, and observed one new B process; B's own `CODEX_HOME` still reported signed in. These checks exercise the adapter behind the menu, not a visual click on the tray item. The menu bar UI and visible account identity and history still need a manual check.
