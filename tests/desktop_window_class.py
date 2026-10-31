"""The window class a dock compares against the launcher entry.

A Kylin dock decides whether a running window is the Aster launcher entry by
matching `WM_CLASS` against `StartupWMClass`. When the two differ it falls back
to a generic gear tile. GDK derives the class field of `WM_CLASS` from the glib
program name inside `gtk_init`, upper-casing its first byte, so the bundle id
`dev.aster.studio.aster` reaches the window system as `Dev.aster.studio.aster`.

This test holds both sides down: the entry the installer writes, and the class
the host can actually claim for its window.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCT = ROOT / "apps/studio/scripts/studio_product.py"
INSTALLER = ROOT / "apps/studio/scripts/studio-desktop.py"
BUNDLE_ID = "dev.aster.studio.aster"

PROBE = r"""
#include <gtk/gtk.h>
#include <stdio.h>

/* argv[1]: program name. argv[2]: "1" to pin the program class after init. */
int main(int argc, char **argv) {
  const char *app_id = argv[1];
  int pin_after = argc > 2 && argv[2][0] == '1';
  setvbuf(stdout, NULL, _IONBF, 0);
  g_set_prgname(app_id);
  g_set_application_name("Aster");
  gtk_init(&argc, &argv);
  if (pin_after) gdk_set_program_class(app_id);
  printf("class=%s\n", gdk_get_program_class());
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_default_size(GTK_WINDOW(window), 120, 80);
  gtk_widget_show_all(window);
  for (int i = 0; i < 40; i++) {
    gtk_main_iteration_do(FALSE);
    g_usleep(100000);
  }
  return 0;
}
"""

WM_CLASS_LINE = re.compile(r'WM_CLASS\(STRING\) = "([^"]*)", "([^"]*)"')


def desktop_entry_wm_class() -> str:
    completed = subprocess.run(
        ["python3", str(PRODUCT), "aster", "desktop"],
        capture_output=True,
        text=True,
        check=True,
    )
    for line in completed.stdout.splitlines():
        if line.startswith("StartupWMClass="):
            return line.split("=", 1)[1]
    raise AssertionError(f"no StartupWMClass in the generated entry:\n{completed.stdout}")


def _window_class_for_pid(pid: int) -> tuple[str, str] | None:
    client_list = subprocess.run(
        ["xprop", "-root", "_NET_CLIENT_LIST"], capture_output=True, text=True
    ).stdout
    for window in re.findall(r"0x[0-9a-f]+", client_list):
        window_pid = subprocess.run(
            ["xprop", "-id", window, "_NET_WM_PID"], capture_output=True, text=True
        ).stdout
        if not window_pid.rstrip().endswith(str(pid)):
            continue
        # `endswith` can match a longer number; confirm the parsed value.
        fields = re.findall(r"\d+", window_pid)
        if not fields or int(fields[-1]) != pid:
            continue
        wm_class = subprocess.run(
            ["xprop", "-id", window, "WM_CLASS"], capture_output=True, text=True
        ).stdout
        match = WM_CLASS_LINE.search(wm_class)
        if match:
            return match.group(1), match.group(2)
    return None


class WindowClassTest(unittest.TestCase):
    def setUp(self):
        missing = [tool for tool in ("cc", "pkg-config", "xprop") if shutil.which(tool) is None]
        if missing:
            self.skipTest("missing tools: " + ", ".join(missing))
        if not Path("/tmp").exists() or not shutil.which("python3"):
            self.skipTest("no Python to generate the entry")
        if not shutil.os.environ.get("DISPLAY"):
            self.skipTest("no X display")
        if (
            subprocess.run(
                ["pkg-config", "--exists", "gtk+-3.0"], capture_output=True
            ).returncode
            != 0
        ):
            self.skipTest("GTK 3 development files are unavailable")
        workspace = tempfile.TemporaryDirectory()
        self.addCleanup(workspace.cleanup)
        self.workspace = Path(workspace.name)
        source = self.workspace / "probe.c"
        source.write_text(PROBE)
        self.probe = self.workspace / "probe"
        flags = subprocess.run(
            ["pkg-config", "--cflags", "--libs", "gtk+-3.0"],
            capture_output=True,
            text=True,
        ).stdout.split()
        built = subprocess.run(
            ["cc", str(source), "-o", str(self.probe), *flags],
            capture_output=True,
            text=True,
        )
        if built.returncode != 0:
            self.skipTest("cannot build the GTK probe: " + built.stderr[-400:])

    def _run_probe(self, app_id: str, pin_after_init: bool) -> tuple[str, str]:
        process = subprocess.Popen(
            [str(self.probe), app_id, "1" if pin_after_init else "0"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            for _ in range(40):
                found = _window_class_for_pid(process.pid)
                if found is not None:
                    return found
                process.poll()
                if process.returncode is not None:
                    break
                subprocess.run(["sleep", "0.1"], check=False)
            self.fail(f"probe window for {app_id} never appeared")
        finally:
            process.terminate()
            process.wait(timeout=10)

    def test_prgname_alone_upper_cases_the_class_field(self):
        # Documents the GDK behaviour the host has to correct, so a future GDK
        # that stops capitalising shows up here instead of as a dock regression.
        instance, window_class = self._run_probe(BUNDLE_ID, pin_after_init=False)
        self.assertEqual(instance, BUNDLE_ID)
        self.assertEqual(window_class, BUNDLE_ID[0].upper() + BUNDLE_ID[1:])

    def test_pinned_program_class_equals_the_launcher_entry(self):
        instance, window_class = self._run_probe(BUNDLE_ID, pin_after_init=True)
        self.assertEqual(instance, BUNDLE_ID)
        self.assertEqual(
            window_class,
            desktop_entry_wm_class(),
            "WM_CLASS must equal StartupWMClass or a dock paints a generic gear",
        )

    def test_launcher_entry_pins_the_bundle_identifier(self):
        self.assertEqual(desktop_entry_wm_class(), BUNDLE_ID)


class InstalledIconTest(unittest.TestCase):
    """The dev launcher has to name its icon by path, not by theme name.

    The dock reads `Icon=` straight from the entry. A bare name only resolves
    when the PNG sits in a system theme directory: an icon installed by this
    installer under `$XDG_DATA_HOME/icons` is not on the panel's search path,
    and the window then keeps the same generic gear the class mismatch caused.
    """

    def test_the_installed_entry_names_the_icon_by_path(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            subprocess.run(
                ["python3", str(INSTALLER), "install"],
                capture_output=True,
                text=True,
                check=True,
                env=dict(os.environ, XDG_DATA_HOME=str(home)),
            )
            entry = (home / "applications" / f"{BUNDLE_ID}.desktop").read_text(encoding="utf-8")
            installed = home / "icons" / "hicolor" / "256x256" / "apps" / f"{BUNDLE_ID}.png"

        icon_lines = [line for line in entry.splitlines() if line.startswith("Icon=")]
        self.assertEqual(len(icon_lines), 1, entry)
        icon_path = Path(icon_lines[0].split("=", 1)[1])
        self.assertTrue(icon_path.is_absolute(), "a dock will not search $XDG_DATA_HOME/icons")
        self.assertEqual(icon_path.name, installed.name)


class SystemInstallTest(unittest.TestCase):
    """Kylin's compositor never reads the user tree, so the system install exists.

    On kylin-wlcom (through 1.3.1-ok36) a window whose only launcher entry lives
    under `$HOME/.local/share/applications` keeps the generic gear even with the
    class, the entry, and the icon all correct — an identical entry resolves from
    `/usr/share/applications` and falls back from the user directory. The system
    install writes into the system prefix and names the icon by absolute path,
    because the hicolor theme is only scanned at compositor startup and the
    compositor's specific-path loader resolves a path on demand.
    """

    def test_the_system_entry_keeps_the_theme_name_icon(self):
        import importlib.util

        spec = importlib.util.spec_from_file_location("studio-desktop", INSTALLER)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            self.assertEqual(module.install_system(prefix=prefix), 0)
            entry = (prefix / "applications" / f"{BUNDLE_ID}.desktop").read_text(encoding="utf-8")
            icons = sorted((prefix / "icons" / "hicolor").glob(f"*/apps/{BUNDLE_ID}.png"))

            # The hicolor theme is scanned once at compositor startup, so a
            # mid-session install names the icon by absolute path: the compositor's
            # specific-path loader creates it from the file on demand.
            icon_lines = [line for line in entry.splitlines() if line.startswith("Icon=")]
            self.assertEqual(
                icon_lines,
                [f"Icon={prefix / 'icons' / 'hicolor' / '256x256' / 'apps' / f'{BUNDLE_ID}.png'}"],
                entry,
            )
            self.assertTrue(icon_lines[0].split("=", 1)[1])

    def test_remove_system_undoes_the_install(self):
        import importlib.util

        spec = importlib.util.spec_from_file_location("studio-desktop", INSTALLER)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        with tempfile.TemporaryDirectory() as directory:
            prefix = Path(directory)
            self.assertEqual(module.install_system(prefix=prefix), 0)
            self.assertEqual(module.remove_system(prefix=prefix), 0)
            self.assertFalse((prefix / "applications" / f"{BUNDLE_ID}.desktop").exists())
            self.assertFalse(list((prefix / "icons" / "hicolor").glob(f"*/apps/{BUNDLE_ID}.png")))


if __name__ == "__main__":
    unittest.main(verbosity=2)
