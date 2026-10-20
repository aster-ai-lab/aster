"""Run the Kylin launch decision the Flatpak command actually execs."""

from pathlib import Path
import importlib.util
import os
import shutil
import shlex
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
        probes = tempfile.TemporaryDirectory()
        self.addCleanup(probes.cleanup)
        self.probes = Path(probes.name)
        for backend in ("x11", "wayland"):
            path = self.probes / backend
            path.write_text("#!/bin/sh\nprintf '%s\\n' " + backend + "\n")
            path.chmod(0o755)

    def _env(self, **updates):
        # A clean environment, so a developer GTK_IM_MODULE cannot leak in.
        base = {"PATH": "/usr/bin:/bin", "HOME": "/var/empty",
                "KYLIN_IM_BACKEND_PROBE": str(self.probes / "x11")}
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
            KYLIN_IM_BACKEND_PROBE=str(self.probes / "wayland"),
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
            KYLIN_IM_BACKEND_PROBE=str(self.probes / "wayland"),
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
        self.assertEqual(fcitx["NOTE"], "missing-fcitx4-module")
        fcitx_child, fcitx_err = self.launched_with_note(
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx"
        )
        self.assertNotIn("GTK_IM_MODULE", fcitx_child)
        self.assertNotIn("XMODIFIERS", fcitx_child)
        self.assertIn("kylin-im: missing-fcitx4-module", fcitx_err)

        fcitx5, fcitx5_text = self.decide(KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["DBUS"], "org.fcitx.Fcitx5")
        self.assertEqual(fcitx5["GTK_IM_MODULE"], "")
        self.assertEqual(fcitx5["XMODIFIERS"], "")
        self.assertEqual(fcitx5["NOTE"], "missing-fcitx5-module")
        self.assertNotIn("org.fcitx.Fcitx-5", fcitx5_text)
        self.assertNotIn("xim", fcitx5_text)
        wayland_fcitx5, _ = self.decide(
            KYLIN_IM_BACKEND_PROBE=str(self.probes / "wayland"),
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
                talks.add(stripped.split("=", 1)[1].strip("'\""))
            if stripped.startswith("--env="):
                envs.append(stripped)
            if stripped.startswith("--command="):
                command.append(stripped)
        self.assertIn("org.fcitx.*", talks)
        self.assertIn("org.freedesktop.IBus", talks)
        self.assertNotIn("org.fcitx.Fcitx-5", talks)
        self.assertEqual(command, ["--command=kylin-launch"])
        self.assertIn("Exec=kylin-launch", (SCRIPT_DIR / "dev.a3s.AsterDream.desktop").read_text())
        self.assertLess(text.index('"${script_dir}/test-input-method.sh" "$files"'), text.index('flatpak build-export'))
        self.assertIn('install -Dm755 "${script_dir}/kylin-launch.sh"', text)
        self.assertIn('"${files}/bin/kylin-im-backend"', text)
        self.assertIn('"${files}/share/licenses/aster-dream/kylin_im_backend.c"', text)
        self.assertIn("/app/lib/im/fcitx5/im-fcitx5.so", text)
        self.assertIn("immodules/fcitx4", text)
        self.assertNotIn("--socket=session-bus", text)
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

    def _fcitx_root(self, version="fcitx5", module_id="fcitx"):
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        module = Path(root.name) / version
        module.mkdir()
        (module / "immodules.cache").write_text("cache\n")
        (module / "module-id").write_text(module_id + "\n")
        return Path(root.name)

    def test_fcitx5_uses_the_bundled_module_on_x11(self):
        root = self._fcitx_root()
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
            KYLIN_IM_BACKEND_PROBE=str(self.probes / "wayland"),
            WAYLAND_DISPLAY="wayland-0",
            KYLIN_IM_BUS_OWNERS=owners,
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(wayland["GTK_IM_MODULE"], "")
        self.assertEqual(wayland["GTK_IM_MODULE_FILE"], "")
        self.assertEqual(wayland["NOTE"], "")

    def test_invalid_module_id_does_not_select_a_module(self):
        root = self._fcitx_root(module_id="fcitx xim")
        decided, text = self.decide(
            KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5",
            KYLIN_IM_MODULE_ROOT=str(root),
        )
        self.assertEqual(decided["GTK_IM_MODULE"], "")
        self.assertEqual(decided["NOTE"], "missing-fcitx5-module")
        self.assertNotIn("xim", text)

    def test_actual_backend_result_takes_precedence_over_session_variables(self):
        for version, owner in (("fcitx4", "org.fcitx.Fcitx-0"), ("fcitx5", "org.fcitx.Fcitx5")):
            with self.subTest(version=version):
                root = self._fcitx_root(version=version)
                env = dict(DISPLAY=":0", KYLIN_IM_BUS_OWNERS=owner, KYLIN_IM_MODULE_ROOT=str(root))
                for requested in ("x11", "x11,wayland", "wayland,x11", "*"):
                    x11, _ = self.decide(GDK_BACKEND=requested, WAYLAND_DISPLAY="stale-wayland", **env)
                    self.assertEqual(x11["GTK_IM_MODULE"], "fcitx")
                    self.assertEqual(x11["GTK_IM_MODULE_FILE"], str(root / version / "immodules.cache"))
                    child = self.launched(GDK_BACKEND=requested, WAYLAND_DISPLAY="stale-wayland", **env)
                    self.assertEqual(child["GTK_IM_MODULE"], "fcitx")
                wayland, _ = self.decide(KYLIN_IM_BACKEND_PROBE=str(self.probes / "wayland"),
                                         GDK_BACKEND="wayland,x11", **env)
                self.assertEqual(wayland["GTK_IM_MODULE"], "")
                self.assertEqual(wayland["GTK_IM_MODULE_FILE"], "")

    def test_backend_probe_failure_and_unknown_output_do_not_guess_or_block_app(self):
        root = self._fcitx_root()
        for body, note in (("exit 12", "backend-probe-failed"),
                           ("printf 'X11\\n'", "invalid-backend-probe-output")):
            with self.subTest(note=note):
                probe = self.probes / "broken"
                probe.write_text("#!/bin/sh\n" + body + "\n")
                probe.chmod(0o755)
                child, stderr = self.launched_with_note(
                    KYLIN_IM_BACKEND_PROBE=str(probe), KYLIN_IM_MODULE_ROOT=str(root),
                    KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5", GTK_IM_MODULE="stale",
                    WAYLAND_DISPLAY="stale-wayland", DISPLAY=":0",
                )
                self.assertNotIn("GTK_IM_MODULE", child)
                self.assertNotIn("GTK_IM_MODULE_FILE", child)
                self.assertIn(note, stderr)

    def test_child_fcitx_address_uses_only_the_filtered_session_bus(self):
        session = "unix:path=/fixture/private-session-bus"
        child = self.launched(KYLIN_IM_BUS_OWNERS="", DBUS_SESSION_BUS_ADDRESS=session,
                              FCITX_DBUS_ADDRESS="unix:path=/fixture/unrelated-bus")
        self.assertEqual(child["FCITX_DBUS_ADDRESS"], session)
        for values in ({}, {"DBUS_SESSION_BUS_ADDRESS": ""}):
            with self.subTest(values=values):
                child = self.launched(KYLIN_IM_BUS_OWNERS="",
                                      FCITX_DBUS_ADDRESS="unix:path=/fixture/unrelated-bus", **values)
                self.assertNotIn("FCITX_DBUS_ADDRESS", child)

    @unittest.skipUnless(shutil.which("xvfb-run") and shutil.which("cc") and shutil.which("pkg-config"),
                         "native backend test needs Xvfb and GDK development tools")
    def test_native_gdk_fallback_lists_select_the_actual_x11_module(self):
        if subprocess.run(["pkg-config", "--exists", "gdk-3.0"]).returncode:
            self.skipTest("native backend test needs gdk-3.0 development files")
        probe = self.probes / "native-backend"
        flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gdk-3.0"], text=True))
        subprocess.run(["cc", "-O2", "-Wall", "-Wextra", "-Werror", str(SCRIPT_DIR / "kylin_im_backend.c"),
                        "-o", str(probe), *flags], check=True, capture_output=True, text=True)
        root = self._fcitx_root()
        for requested in ("x11", "x11,wayland", "wayland,x11", "*"):
            with self.subTest(requested=requested):
                env = self._env(KYLIN_IM_BACKEND_PROBE=str(probe), KYLIN_IM_MODULE_ROOT=str(root),
                                KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx5", GDK_BACKEND=requested,
                                WAYLAND_DISPLAY="missing-socket", XDG_RUNTIME_DIR=str(self.probes),
                                GTK_IM_MODULE="deliberately-invalid", GTK_IM_MODULE_FILE="/absent/cache")
                actual = subprocess.run(["xvfb-run", "-a", str(probe)], env=env,
                                        check=True, capture_output=True, text=True, timeout=10)
                self.assertEqual(actual.stdout.strip(), "x11")
                result = subprocess.run(["xvfb-run", "-a", "bash", str(LAUNCH), "--print"], env=env,
                                        check=True, capture_output=True, text=True, timeout=10)
                self.assertEqual(_parse_lines(result.stdout)["GTK_IM_MODULE"], "fcitx")

    def test_fcitx4_uses_its_own_module_and_current_display_bus(self):
        root = self._fcitx_root(version="fcitx4")
        for display, service in ((":0", "org.fcitx.Fcitx-0"), (":12.0", "org.fcitx.Fcitx-12")):
            with self.subTest(display=display):
                env = dict(
                    DISPLAY=display,
                    KYLIN_IM_BUS_OWNERS=service + " org.freedesktop.IBus",
                    KYLIN_IM_MODULE_ROOT=str(root),
                )
                decided, _ = self.decide(**env)
                self.assertEqual(decided["DBUS"], service)
                self.assertEqual(decided["GTK_IM_MODULE"], "fcitx")
                self.assertEqual(decided["GTK_IM_MODULE_FILE"], str(root / "fcitx4/immodules.cache"))
                self.assertEqual(decided["NOTE"], "")
                child = self.launched(GTK_IM_MODULE="xim", **env)
                self.assertEqual(child["GTK_IM_MODULE"], "fcitx")
                self.assertEqual(child["GTK_IM_MODULE_FILE"], str(root / "fcitx4/immodules.cache"))
                self.assertNotIn("XMODIFIERS", child)

        missing, _ = self.decide(
            DISPLAY=":0", KYLIN_IM_BUS_OWNERS="org.fcitx.Fcitx-0",
            KYLIN_IM_MODULE_ROOT=str(root / "absent"),
        )
        self.assertEqual(missing["NOTE"], "missing-fcitx4-module")
        self.assertEqual(missing["GTK_IM_MODULE"], "")

    @unittest.skipUnless(
        shutil.which("dbus-run-session") and importlib.util.find_spec("gi"),
        "real session-bus test needs dbus-run-session and python3-gi",
    )
    def test_fcitx4_detection_on_a_real_isolated_session_bus(self):
        root = self._fcitx_root(version="fcitx4")
        program = """
import subprocess, sys
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
reply = bus.call_sync(
    'org.freedesktop.DBus', '/org/freedesktop/DBus',
    'org.freedesktop.DBus', 'RequestName',
    GLib.Variant('(su)', ('org.fcitx.Fcitx-12', 0)),
    GLib.VariantType.new('(u)'), Gio.DBusCallFlags.NONE, 1000, None,
)
assert reply.unpack() == (1,), reply
subprocess.run(['bash', sys.argv[1], '--print'], check=True)
"""
        result = subprocess.run(
            ["dbus-run-session", "--", sys.executable, "-c", program, str(LAUNCH)],
            env=self._env(DISPLAY=":12.0", KYLIN_IM_MODULE_ROOT=str(root)),
            check=True, capture_output=True, text=True, timeout=10,
        )
        decision = _parse_lines(result.stdout)
        self.assertEqual(decision["DBUS"], "org.fcitx.Fcitx-12")
        self.assertEqual(decision["GTK_IM_MODULE"], "fcitx")
        self.assertEqual(decision["GTK_IM_MODULE_FILE"], str(root / "fcitx4/immodules.cache"))
        self.assertEqual(decision["NOTE"], "")

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
