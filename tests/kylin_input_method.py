"""Run the Kylin launch decision the Flatpak command actually execs."""

from pathlib import Path
import os
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[1]
LAUNCH = ROOT / "apps/work/scripts/kylin-flatpak/kylin-launch.sh"
BUNDLE = ROOT / "apps/work/scripts/kylin-flatpak/package-bundle.sh"
INSTALLER = ROOT / "apps/work/scripts/kylin-flatpak/install-on-kylin.sh"
NOTES = ROOT / "apps/work/scripts/kylin-flatpak/安装说明.txt"
README = ROOT / "apps/work/scripts/kylin-flatpak/README.md"


def _parse_lines(text):
    parsed = {}
    for line in text.splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        parsed[key] = value
    return parsed


class KylinInputMethodTest(unittest.TestCase):
    def setUp(self):
        self.assertTrue(LAUNCH.is_file(), LAUNCH)

    def _env(self, **updates):
        # A clean environment, so a developer GTK_IM_MODULE cannot leak in.
        base = {"PATH": "/usr/bin:/bin", "HOME": "/var/empty"}
        base.update(updates)
        return base

    def decide(self, **updates):
        completed = subprocess.run(
            ["bash", str(LAUNCH), "--print"],
            env=self._env(**updates),
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.stderr, "")
        parsed = _parse_lines(completed.stdout)
        self.assertEqual(
            list(parsed),
            ["GTK_IM_MODULE", "XMODIFIERS", "DBUS"],
            completed.stdout,
        )
        return parsed, completed.stdout

    def launched(self, **updates):
        completed = subprocess.run(
            ["bash", str(LAUNCH)],
            env=self._env(KYLIN_LAUNCH_TARGET="/usr/bin/env", **updates),
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.stderr, "")
        return _parse_lines(completed.stdout)

    def test_four_host_sessions(self):
        wayland, wayland_text = self.decide(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS="",
            GTK_IM_MODULE="xim",
            XMODIFIERS="@im=fcitx",
        )
        self.assertEqual(wayland["GTK_IM_MODULE"], "")
        self.assertEqual(wayland["XMODIFIERS"], "")
        self.assertNotIn("org.fcitx.Fcitx-5", wayland_text)
        launched = self.launched(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS="",
            GTK_IM_MODULE="xim",
            XMODIFIERS="@im=fcitx",
            QT_IM_MODULE="xim",
        )
        self.assertNotIn("GTK_IM_MODULE", launched)
        self.assertNotIn("XMODIFIERS", launched)
        self.assertNotIn("QT_IM_MODULE", launched)

        ibus, _ibus_text = self.decide(KYLIN_IM_BUS_OWNERS="org.freedesktop.IBus")
        self.assertEqual(ibus["GTK_IM_MODULE"], "ibus")
        self.assertEqual(ibus["XMODIFIERS"], "")
        self.assertEqual(ibus["DBUS"], "org.freedesktop.IBus")
        ibus_child = self.launched(
            KYLIN_IM_BUS_OWNERS="org.freedesktop.IBus",
            GTK_IM_MODULE="xim",
            XMODIFIERS="@im=fcitx",
        )
        self.assertEqual(ibus_child["GTK_IM_MODULE"], "ibus")
        self.assertNotIn("XMODIFIERS", ibus_child)

        fcitx, _fcitx_text = self.decide(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx")
        self.assertEqual(fcitx["GTK_IM_MODULE"], "xim")
        self.assertEqual(fcitx["XMODIFIERS"], "@im=fcitx")
        self.assertEqual(fcitx["DBUS"], "org.fcitx.Fcitx")
        fcitx_child = self.launched(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx")
        self.assertEqual(fcitx_child["GTK_IM_MODULE"], "xim")
        self.assertEqual(fcitx_child["XMODIFIERS"], "@im=fcitx")

        fcitx5, fcitx5_text = self.decide(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["DBUS"], "org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["GTK_IM_MODULE"], "xim")
        self.assertEqual(fcitx5["XMODIFIERS"], "@im=fcitx")
        self.assertNotIn("org.fcitx.Fcitx-5", fcitx5_text)
        wayland_fcitx5, _ = self.decide(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5",
        )
        self.assertEqual(wayland_fcitx5["GTK_IM_MODULE"], "")
        self.assertEqual(wayland_fcitx5["DBUS"], "org.fcitx.Fcitx5")

    def test_finish_args_grant_bus_names_without_a_fixed_module(self):
        text = BUNDLE.read_text()
        talks = set()
        envs = []
        command = []
        for line in text.splitlines():
            stripped = line.strip().rstrip("\\").strip()
            if stripped.startswith("--talk-name="):
                talks.add(stripped.split("=", 1)[1])
            if stripped.startswith("--env="):
                envs.append(stripped)
            if stripped.startswith("--command="):
                command.append(stripped)
        self.assertIn("org.fcitx.Fcitx", talks)
        self.assertIn("org.fcitx.Fcitx5", talks)
        self.assertIn("org.freedesktop.IBus", talks)
        self.assertNotIn("org.fcitx.Fcitx-5", talks)
        self.assertEqual(command, ["--command=kylin-launch"])
        self.assertIn('install -Dm755 "${script_dir}/kylin-launch.sh"', text)
        self.assertFalse(any(item.startswith("--env=GTK_IM_MODULE=") for item in envs))
        self.assertFalse(any(item.startswith("--env=XMODIFIERS=") for item in envs))
        self.assertFalse(any(item.startswith("--env=QT_IM_MODULE=") for item in envs))
        self.assertNotIn("GTK_IM_MODULE=xim", text)
        self.assertNotIn("--talk-name=org.fcitx.Fcitx-5", text)

    def test_installer_hint_removes_a_stale_override(self):
        for path in (INSTALLER, NOTES):
            text = path.read_text()
            self.assertNotIn("GTK_IM_MODULE=xim", text, path)
            self.assertIn("GTK_IM_MODULE", text, path)
            self.assertIn("覆盖", text, path)
        installer = INSTALLER.read_text()
        self.assertIn("请用桌面登录用户运行本脚本，不要 sudo", installer)
        readme = README.read_text()
        self.assertIn("WAYLAND_DISPLAY", readme)
        self.assertIn("GTK_IM_MODULE=ibus", readme)
        self.assertIn("XMODIFIERS=@im=fcitx", readme)
        self.assertIn("org.fcitx.Fcitx5", readme)
        self.assertIn("org.gnome.Platform//50", readme)
        self.assertIn("WebKitGTK 4.0", readme)
        self.assertIn("not a Kylin desktop IME run", readme)
        self.assertNotIn("org.fcitx.Fcitx-5", readme)


if __name__ == "__main__":
    unittest.main()
