# Same user desktop probe

Use this with two existing Codex accounts in your current macOS user session. The probe creates separate Codex homes and Electron data directories for A and B. It records app version, candidate process IDs, and file counts for known state locations. It never reads file contents or copies login files. A new process ID or file count does not prove account isolation.

Preparation and snapshots do not launch Codex or sign you out. Run these first:

```sh
python3 tools/desktop_probe.py prepare --test-root "$HOME/Library/Application Support/Codex Switcher/probe"
python3 tools/desktop_probe.py snapshot --test-root "$HOME/Library/Application Support/Codex Switcher/probe"
```

Before a launch, finish active Codex work and quit Codex fully. The launch flag is explicit because Codex may use shared app data or keychain entries and could change your current login even with a separate `CODEX_HOME`. To start the A probe:

```sh
python3 tools/desktop_probe.py launch --test-root "$HOME/Library/Application Support/Codex Switcher/probe" --profile A --allow-live-probe
```

Sign in to account A and create a clearly named test task. Copy [the observation template](observation-template.md) into the private probe directory and record its visible identity and history there. Run `snapshot` again. Quit Codex fully, run a fresh `snapshot` just before launching B with `--profile B`, sign in to your second account, and create a different test task. Repeat B to A and restart checks. Compare each snapshot to the baseline. The reports are saved with owner only permissions below `reports/` in the test root.

The probe requires a snapshot from the last 30 minutes for the same Codex build and unchanged process list. Run `snapshot` again after quitting Codex. A candidate process ID, file count change, or completed sign in does not prove isolation. The visible identity and history observations decide that. Concurrent testing has a separate `--allow-concurrent` flag. Use that only after the sequential A → B → A result is understood.
