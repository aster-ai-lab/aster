"""Run the Kylin launch decision the Flatpak command actually execs."""

from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest

SCRIPT_DIR = Path(__file__).resolve().parents[1] / "apps/work/scripts/kylin-flatpak"
sys.path.insert(0, str(SCRIPT_DIR))
import immodule_cache  # noqa: E402


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
            ["GTK_IM_MODULE", "GTK_IM_MODULE_FILE", "XMODIFIERS", "DBUS", "NOTE"],
            completed.stdout,
        )
        return parsed, completed.stdout

    def launched(self, **updates):
        parsed, stderr = self.launched_with_note(**updates)
        self.assertEqual(stderr, "")
        return parsed

    def launched_with_note(self, **updates):
        completed = subprocess.run(
            ["bash", str(LAUNCH)],
            env=self._env(KYLIN_LAUNCH_TARGET="/usr/bin/env", **updates),
            check=True,
            capture_output=True,
            text=True,
        )
        return _parse_lines(completed.stdout), completed.stderr

    def test_four_host_sessions(self):
        wayland, wayland_text = self.decide(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS="",
            GTK_IM_MODULE="xim",
            XMODIFIERS="@im=fcitx",
        )
        self.assertEqual(wayland["GTK_IM_MODULE"], "")
        self.assertEqual(wayland["GTK_IM_MODULE_FILE"], "")
        self.assertEqual(wayland["XMODIFIERS"], "")
        self.assertEqual(wayland["NOTE"], "")
        self.assertNotIn("org.fcitx.Fcitx-5", wayland_text)
        self.assertNotIn("xim", wayland_text)
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
        self.assertNotIn("GTK_IM_MODULE_FILE", ibus_child)
        self.assertNotIn("XMODIFIERS", ibus_child)

        fcitx, _fcitx_text = self.decide(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx")
        self.assertEqual(fcitx["GTK_IM_MODULE"], "")
        self.assertEqual(fcitx["GTK_IM_MODULE_FILE"], "")
        self.assertEqual(fcitx["XMODIFIERS"], "")
        self.assertEqual(fcitx["DBUS"], "org.fcitx.Fcitx")
        self.assertEqual(fcitx["NOTE"], "fcitx4-unsupported")
        fcitx_child, fcitx_err = self.launched_with_note(
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx"
        )
        self.assertNotIn("GTK_IM_MODULE", fcitx_child)
        self.assertNotIn("XMODIFIERS", fcitx_child)
        self.assertIn("kylin-im: fcitx4-unsupported", fcitx_err)

        fcitx5, fcitx5_text = self.decide(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["DBUS"], "org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["GTK_IM_MODULE"], "")
        self.assertEqual(fcitx5["XMODIFIERS"], "")
        self.assertEqual(fcitx5["NOTE"], "missing-fcitx5-module")
        self.assertNotIn("org.fcitx.Fcitx-5", fcitx5_text)
        self.assertNotIn("xim", fcitx5_text)
        wayland_fcitx5, _ = self.decide(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5",
        )
        self.assertEqual(wayland_fcitx5["GTK_IM_MODULE"], "")
        self.assertEqual(wayland_fcitx5["GTK_IM_MODULE_FILE"], "")
        self.assertEqual(wayland_fcitx5["DBUS"], "org.fcitx.Fcitx5")
        self.assertEqual(wayland_fcitx5["NOTE"], "")

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
        self.assertIn("/app/lib/im/fcitx5/im-fcitx5.so", text)
        self.assertIn("immodule_cache.py", text)
        self.assertIn('grep -q \'libgtk-3\\.so\\.0\'', text)
        self.assertIn("PR_SET_SECCOMP", text)
        self.assertIn("bwrap --die-with-parent", text)
        self.assertIn('"$im_so"', text)
        self.assertNotIn("gtk-3.0/3.0.0/immodules", text)
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
        self.assertIn("GTK_IM_MODULE_FILE", readme)
        self.assertIn("org.fcitx.Fcitx5", readme)
        self.assertIn("org.gnome.Platform//50", readme)
        self.assertIn("WebKitGTK 4.0", readme)
        self.assertIn("not a Kylin desktop IME run", readme)
        self.assertIn("fcitx4", readme)
        self.assertNotIn("org.fcitx.Fcitx-5", readme)
        self.assertNotIn("XMODIFIERS=@im=fcitx", readme)
        self.assertNotIn("xim", LAUNCH.read_text())

    def _fcitx5_root(self, module_id="fcitx"):
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        module = Path(root.name) / "fcitx5"
        module.mkdir()
        (module / "immodules.cache").write_text("cache\n")
        (module / "module-id").write_text(module_id + "\n")
        return Path(root.name)

    def test_fcitx5_uses_the_bundled_module_on_x11(self):
        root = self._fcitx5_root()
        owners = "org.fcitx.Fcitx org.fcitx.Fcitx5 org.freedesktop.IBus"
        decided, _text = self.decide(
            KYLIN_IM_BUS_OWNERS=owners,
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(decided["GTK_IM_MODULE"], "fcitx")
        self.assertEqual(
            decided["GTK_IM_MODULE_FILE"], str(root / "fcitx5" / "immodules.cache")
        )
        self.assertEqual(decided["XMODIFIERS"], "")
        self.assertEqual(decided["DBUS"], "org.fcitx.Fcitx5")
        self.assertEqual(decided["NOTE"], "")
        child = self.launched(
            KYLIN_IM_BUS_OWNERS=owners,
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(child["GTK_IM_MODULE"], "fcitx")
        self.assertEqual(
            child["GTK_IM_MODULE_FILE"], str(root / "fcitx5" / "immodules.cache")
        )
        self.assertNotIn("XMODIFIERS", child)
        self.assertNotIn("QT_IM_MODULE", child)

        wayland, _wayland_text = self.decide(
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS=owners,
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(wayland["GTK_IM_MODULE"], "")
        self.assertEqual(wayland["GTK_IM_MODULE_FILE"], "")
        self.assertEqual(wayland["NOTE"], "")

    def test_invalid_module_id_does_not_select_a_module(self):
        root = self._fcitx5_root(module_id="fcitx xim")
        decided, text = self.decide(
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5",
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(decided["GTK_IM_MODULE"], "")
        self.assertEqual(decided["NOTE"], "missing-fcitx5-module")
        self.assertNotIn("xim", text)

    def test_immodule_cache_rewrites_the_query_path(self):
        text = "\n".join(
            [
                "# created by query",
                '"/src/dist/kylin/immodules/fcitx5/im-fcitx5.so"',
                '"fcitx" "Fcitx 5" "fcitx" "" "ja:ko:zh:*"',
                "",
            ]
        )
        rewritten = immodule_cache.rewrite_immodule_cache(
            text,
            "/src/dist/kylin/immodules/fcitx5/im-fcitx5.so",
            immodule_cache.SANDBOX_MODULE,
            "fcitx",
        )
        self.assertIn(f'"{immodule_cache.SANDBOX_MODULE}"', rewritten)
        self.assertNotIn("/src/dist/kylin", rewritten)
        self.assertEqual(
            immodule_cache.module_id_from_cache(
                rewritten, immodule_cache.SANDBOX_MODULE
            ),
            "fcitx",
        )
        with self.assertRaises(ValueError):
            immodule_cache.rewrite_immodule_cache(
                text, "/missing.so", "/app/x.so", "fcitx"
            )
        with self.assertRaises(ValueError):
            immodule_cache.rewrite_immodule_cache(
                text,
                "/src/dist/kylin/immodules/fcitx5/im-fcitx5.so",
                immodule_cache.SANDBOX_MODULE,
                "not-the-id",
            )


if __name__ == "__main__":
    unittest.main()
