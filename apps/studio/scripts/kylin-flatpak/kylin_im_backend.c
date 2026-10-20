/* SPDX-License-Identifier: AGPL-3.0-only */
#include <gdk/gdk.h>
#include <gdk/gdkwayland.h>
#include <gdk/gdkx.h>
#include <stdio.h>
#include <unistd.h>

int main(int argc, char **argv) {
    /* Leave stalled-display error handling to the main application after five seconds. */
    alarm(5);
    if (!gdk_init_check(&argc, &argv)) {
        fputs("kylin-im-backend: cannot open a GDK display\n", stderr);
        return 1;
    }
    alarm(0);
    GdkDisplay *display = gdk_display_get_default();
    const char *backend = NULL;
    if (GDK_IS_X11_DISPLAY(display)) backend = "x11";
    else if (GDK_IS_WAYLAND_DISPLAY(display)) backend = "wayland";
    if (!backend) {
        fputs("kylin-im-backend: unsupported GDK display backend\n", stderr);
        gdk_display_close(display);
        return 1;
    }
    puts(backend);
    gdk_display_close(display);
    return 0;
}
