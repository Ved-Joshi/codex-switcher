# Open source Codex switchers

Reviewed 2026-09-25 for two existing Codex desktop accounts in one macOS login.

| Project | Approach | Fit for this project |
|---|---|---|
| [edihasaj/codex-account-switcher](https://github.com/edihasaj/codex-account-switcher) | Native menu bar app launches a second unchanged Codex desktop process with a separate `CODEX_HOME` and Electron `--user-data-dir`. It offers focus and concurrent open actions. | Adapted its two directory launch method into the probe and Rust adapter. Preserved its MIT notice in `docs/legal/third-party-notices.md`. Open Both stays deferred until visible account and history checks. |
| [bartekczyz/ai-profiles](https://github.com/bartekczyz/ai-profiles) | Tauri profile manager launches separate ChatGPT desktop environments and can reopen a specific running process by PID. It also offers wrapper apps, CLI profiles, migration, usage, and session tools. | Imported the PID-addressed reopen event, adapted installed app discovery and its Codex app-server transport for per-profile account and usage reads. Preserved its MIT notice in `docs/legal/third-party-notices.md`. Its larger profile system and credential/session features do not fit the small menu bar scope. |
| [4LAU/codex-profile-switcher](https://github.com/4LAU/codex-profile-switcher) | Menu bar app saves credentials in Keychain and changes `~/.codex/auth.json` to switch profiles. | Useful fallback research, but it handles live credentials and does not establish isolated desktop histories. No implementation copied. |
| [liuzhao1225/codex-account-switcher](https://github.com/liuzhao1225/codex-account-switcher) | Native profile manager with saved account state, usage, and switch confirmation. | Similar credential snapshot direction. No implementation copied. |
| [JqyModi/codex-multi-launcher](https://github.com/JqyModi/codex-multi-launcher) | Describes multiple Codex desktop profiles using separate data locations. | Supporting idea, but the inspected repository does not provide reusable app source. No implementation copied. |
| [nhocconan/codex-multi](https://github.com/nhocconan/codex-multi) | CLI account profiles with separate `CODEX_HOME`, deliberately shared sessions, and no desktop switching. | No code imported because it does not isolate desktop history or switch the desktop app. |
| [ChArLiEdance/codex-switch](https://github.com/ChArLiEdance/codex-switch) | Tauri account switcher that replaces the global active auth state. | No code imported because global auth swapping does not support two simultaneous isolated desktops. |

## Adapted behavior

The same user experiment passes both `CODEX_HOME` and `--user-data-dir` while launching the unchanged Codex app with `open -n`. The Rust adapter now creates each A/B profile's owner only `codex` and `electron` directories on first use, checks the installed bundle and matching process, and sends a reopen event to the matching PID before activation. The probe records launch/process metadata but never treats a new process as proof of account isolation.

The next evidence is an A → B → A run with two actual accounts, their visible identities, and distinct tasks in history. If the app shares login or history through another location, the two directory method fails for this build and the fallback needs a separate design.
