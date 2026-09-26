import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from desktop_probe import launch_command, profile_directory, recent_baseline, save_report, validate_test_root


class SameUserProbeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name).resolve()
        self.home_patch = patch("desktop_probe.Path.home", return_value=self.home)
        self.home_patch.start()
        self.addCleanup(self.home_patch.stop)
        self.probe = self.home / "Library" / "Application Support" / "Codex Switcher" / "probe"

    def test_rejects_root_outside_home(self):
        with self.assertRaises(SystemExit):
            validate_test_root(self.home.parent)
        self.assertFalse(self.probe.exists())

    def test_rejects_live_codex_state_as_root(self):
        with self.assertRaises(SystemExit):
            validate_test_root(self.home / ".codex")
        with self.assertRaises(SystemExit):
            validate_test_root(self.home / "Library/Application Support/Codex")

    def test_creates_owner_only_root(self):
        root = validate_test_root(self.probe)
        self.assertEqual(root.stat().st_mode & 0o777, 0o700)

    def test_rejects_symlink_parent(self):
        parent = self.probe.parent
        parent.mkdir(parents=True)
        actual = self.home / "actual"
        actual.mkdir()
        self.probe.symlink_to(actual, target_is_directory=True)
        with self.assertRaises(SystemExit):
            validate_test_root(self.probe)

    def test_rejects_parent_escape(self):
        with self.assertRaises(SystemExit):
            validate_test_root(self.probe / ".." / "escape")

    def test_rejects_symlink_in_profile(self):
        root = validate_test_root(self.probe)
        (root / "profiles").symlink_to(self.home, target_is_directory=True)
        with self.assertRaises(SystemExit):
            profile_directory(root, "A", "codex", create=True)

    def test_launch_command_isolates_both_directories(self):
        command = launch_command(
            Path("/Applications/ChatGPT.app"),
            Path("/tmp/profile A/codex"),
            Path("/tmp/profile A/electron"),
        )
        self.assertEqual(command, [
            "/usr/bin/open", "-n", "-a", "/Applications/ChatGPT.app",
            "--env", "CODEX_HOME=/tmp/profile A/codex",
            "--args", "--user-data-dir=/tmp/profile A/electron",
        ])

    def test_baseline_matches_current_app(self):
        root = validate_test_root(self.probe)
        app = {"bundleIdentifier": "com.openai.codex", "version": "test"}
        report = {"action": "snapshot", "app": app, "runningCandidatePids": []}
        saved = save_report(root, report)
        path, loaded = recent_baseline(root, app)
        self.assertEqual(path, saved)
        self.assertEqual(loaded, report)
        with self.assertRaises(SystemExit):
            recent_baseline(root, {"bundleIdentifier": "com.openai.codex", "version": "new"})

    def test_rejects_stale_baseline(self):
        root = validate_test_root(self.probe)
        reports = root / "reports"
        reports.mkdir(mode=0o700)
        stale = reports / "1-snapshot.json"
        stale.write_text('{"action":"snapshot"}', encoding="utf-8")
        stale.chmod(0o600)
        with self.assertRaises(SystemExit):
            recent_baseline(root, {"bundleIdentifier": "com.openai.codex"})


if __name__ == "__main__":
    unittest.main()
