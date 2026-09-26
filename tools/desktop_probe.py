#!/usr/bin/env python3
"""Probe Codex desktop launch behavior within one macOS user session."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import time
from typing import Optional

BUNDLE_ID = "com.openai.codex"


def fail(message: str) -> None:
    raise SystemExit(message)


def validate_test_root(test_root: Path) -> Path:
    home = Path.home().resolve()
    root = test_root.expanduser().absolute()
    expected = home / "Library" / "Application Support" / "Codex Switcher" / "probe"
    if ".." in root.parts:
        fail("The test root cannot contain parent directory components.")
    if root != expected:
        fail(f"The test root must be the dedicated probe path: {expected}")
    if any(part.is_symlink() for part in (root, *root.parents) if part != home):
        fail("The test root cannot contain a symlink component.")
    if root.exists():
        if not root.is_dir() or root.stat().st_uid != os.getuid():
            fail("The test root must be a directory owned by this user.")
        if root.stat().st_mode & 0o077:
            fail("The test root must be accessible only to its owner.")
    else:
        root.mkdir(mode=0o700, parents=True)
        root.chmod(0o700)
    return root


def app_metadata(bundle: Path) -> dict:
    if bundle.is_symlink() or not bundle.is_dir():
        fail("The Codex app bundle must be a directory, not a symlink.")
    info_file = bundle / "Contents" / "Info.plist"
    try:
        with info_file.open("rb") as file:
            info = plistlib.load(file)
    except (OSError, ValueError) as error:
        fail(f"Could not read the app bundle metadata: {error}")
    if info.get("CFBundleIdentifier") != BUNDLE_ID:
        fail(f"Expected bundle identifier {BUNDLE_ID}.")
    if not isinstance(info.get("CFBundleExecutable"), str):
        fail("The app bundle has no executable name.")
    return {
        "bundlePath": str(bundle.resolve()),
        "bundleIdentifier": BUNDLE_ID,
        "version": info.get("CFBundleShortVersionString"),
        "build": info.get("CFBundleVersion"),
        "executable": info.get("CFBundleExecutable"),
    }


def resolve_app_path(override: Optional[Path]) -> Path:
    if override is not None:
        return override
    candidates = (Path("/Applications/ChatGPT.app"), Path.home() / "Applications/ChatGPT.app")
    for candidate in candidates:
        if candidate.is_dir() and not candidate.is_symlink():
            return candidate
    fail("Codex desktop was not found. Pass its app bundle with --app.")


def path_stats(path: Path) -> dict:
    if path.is_symlink():
        return {"exists": True, "symlink": True}
    if not path.exists():
        return {"exists": False}
    if path.is_file():
        return {"exists": True, "files": 1, "bytes": path.stat().st_size}
    files = 0
    size = 0
    for directory, names, filenames in os.walk(path, followlinks=False):
        names[:] = [name for name in names if not (Path(directory) / name).is_symlink()]
        for name in filenames:
            entry = Path(directory) / name
            if not entry.is_symlink():
                try:
                    size += entry.stat().st_size
                    files += 1
                except OSError:
                    pass
    return {"exists": True, "files": files, "bytes": size}


def observed_paths(home: Path, root: Path) -> dict:
    paths = {
        "codexHomeDefault": home / ".codex",
        "supportCodex": home / "Library/Application Support/Codex",
        "supportBundle": home / "Library/Application Support/com.openai.codex",
        "profileA": root / "profiles/A/codex",
        "profileB": root / "profiles/B/codex",
        "electronA": root / "profiles/A/electron",
        "electronB": root / "profiles/B/electron",
    }
    return {name: path_stats(path) for name, path in paths.items()}


def app_pids(executable: str) -> list[int]:
    result = subprocess.run(["/bin/ps", "-axo", "pid=,comm="], capture_output=True, text=True, check=True)
    pids = []
    for line in result.stdout.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) == 2 and Path(parts[1]).name == executable:
            pids.append(int(parts[0]))
    return sorted(pids)


def matching_user_data_pids(pids: list[int], user_data: Path) -> list[int]:
    if not pids:
        return []
    result = subprocess.run(["/bin/ps", "-axo", "pid=,command="], capture_output=True, text=True, check=True)
    expected = f"--user-data-dir={user_data}"
    matches = []
    for line in result.stdout.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) == 2 and parts[0].isdigit() and int(parts[0]) in pids and expected in parts[1]:
            matches.append(int(parts[0]))
    return sorted(matches)


def launch_command(app_path: Path, codex_home: Path, user_data: Path) -> list[str]:
    return [
        "/usr/bin/open", "-n", "-a", str(app_path),
        "--env", f"CODEX_HOME={codex_home}",
        "--args", f"--user-data-dir={user_data}",
    ]


def profile_directory(root: Path, profile: str, leaf: str, create: bool) -> Path:
    path = root
    for name in ("profiles", profile, leaf):
        path = path / name
        if path.is_symlink():
            fail("A profile path cannot contain a symlink.")
        if path.exists():
            if not path.is_dir() or path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o077:
                fail("A profile path must be an owner only directory.")
        elif create:
            path.mkdir(mode=0o700)
            path.chmod(0o700)
        else:
            fail("Run prepare before launch.")
    return path


def save_report(root: Path, report: dict) -> Path:
    reports = root / "reports"
    if reports.is_symlink() or (reports.exists() and reports.stat().st_uid != os.getuid()):
        fail("The reports directory cannot be a symlink or owned by another user.")
    reports.mkdir(mode=0o700, exist_ok=True)
    reports.chmod(0o700)
    destination = reports / f"{time.time_ns()}-{report['action']}.json"
    with open(destination, "x", encoding="utf-8", opener=lambda path, flags: os.open(path, flags, 0o600)) as file:
        json.dump(report, file, indent=2)
        file.write("\n")
    return destination


def recent_baseline(root: Path, app: dict) -> tuple[Path, dict]:
    reports = root / "reports"
    snapshots = sorted(reports.glob("*-snapshot.json")) if reports.is_dir() and not reports.is_symlink() else []
    if not snapshots:
        fail("Run snapshot immediately before launch to record current Codex state metadata.")
    path = snapshots[-1]
    stats = path.lstat()
    if path.is_symlink() or stats.st_uid != os.getuid() or stats.st_mode & 0o077:
        fail("The baseline report must be an owner only regular file.")
    try:
        created_ns = int(path.name.split("-", 1)[0])
        with path.open(encoding="utf-8") as file:
            baseline = json.load(file)
    except (OSError, ValueError, json.JSONDecodeError):
        fail("The latest baseline report is unreadable or invalid.")
    age_ns = time.time_ns() - created_ns
    if not 0 <= age_ns <= 30 * 60 * 1_000_000_000:
        fail("The baseline is older than 30 minutes. Run snapshot again before launch.")
    if baseline.get("action") != "snapshot" or baseline.get("app") != app:
        fail("The baseline was recorded for a different Codex app build.")
    if not isinstance(baseline.get("runningCandidatePids"), list):
        fail("The baseline has no process observation. Run snapshot again.")
    return path, baseline


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "snapshot", "launch"))
    parser.add_argument("--test-root", type=Path, required=True)
    parser.add_argument("--app", type=Path)
    parser.add_argument("--profile", choices=("A", "B"))
    parser.add_argument("--allow-live-probe", action="store_true", help="Allow Codex launch in this macOS user session")
    parser.add_argument("--allow-concurrent", action="store_true", help="Allow a second Codex instance while one is running")
    args = parser.parse_args()
    if sys.platform != "darwin":
        fail("This probe runs on macOS only.")
    root = validate_test_root(args.test_root)
    app_path = resolve_app_path(args.app)
    bundle = app_metadata(app_path)
    if args.action == "launch" and args.profile is None:
        parser.error("launch requires --profile A or --profile B")
    if args.action == "launch" and not args.allow_live_probe:
        parser.error("launch requires --allow-live-probe because Codex may change shared account state")
    if args.allow_concurrent and args.action != "launch":
        parser.error("--allow-concurrent applies only to launch")

    report = {
        "action": args.action,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "macOS": subprocess.run(["/usr/bin/sw_vers", "-productVersion"], capture_output=True, text=True, check=True).stdout.strip(),
        "app": bundle,
        "state": observed_paths(Path.home(), root),
        "runningCandidatePids": app_pids(bundle["executable"]),
    }
    if args.action == "prepare":
        for profile in ("A", "B"):
            profile_directory(root, profile, "codex", create=True)
            profile_directory(root, profile, "electron", create=True)
        report["state"] = observed_paths(Path.home(), root)
    elif args.action == "launch":
        baseline_path, baseline = recent_baseline(root, bundle)
        codex_home = profile_directory(root, args.profile, "codex", create=False)
        user_data = profile_directory(root, args.profile, "electron", create=False)
        before = set(app_pids(bundle["executable"]))
        if before != set(baseline["runningCandidatePids"]):
            fail("Running Codex processes changed since the baseline. Run snapshot again.")
        if before and not args.allow_concurrent:
            fail("Codex is running. Quit it before a sequential probe, or explicitly use --allow-concurrent later.")
        command = launch_command(app_path, codex_home, user_data)
        result = subprocess.run(command, capture_output=True, text=True)
        new_pids = []
        if result.returncode == 0:
            for _ in range(10):
                new_pids = sorted(set(app_pids(bundle["executable"])) - before)
                if new_pids:
                    break
                time.sleep(1)
        report["launch"] = {
            "profile": args.profile,
            "method": "open -n -a with CODEX_HOME and --user-data-dir",
            "environmentRoot": str(codex_home),
            "electronDataRoot": str(user_data),
            "exitCode": result.returncode,
            "newCandidatePids": new_pids,
            "userDataArgumentObservedPids": matching_user_data_pids(new_pids, user_data),
            "processIdentityVerified": False,
            "concurrentRequested": args.allow_concurrent,
            "baselineReport": baseline_path.name,
        }
        report["state"] = observed_paths(Path.home(), root)
        report["runningCandidatePids"] = app_pids(bundle["executable"])
    destination = save_report(root, report)
    print(destination)


if __name__ == "__main__":
    main()
