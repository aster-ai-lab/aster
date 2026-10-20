/* Exercise the packaged GTK client against a real, isolated Fcitx 4 daemon. */
#include <gtk/gtk.h>
#include <string.h>

static GtkWidget *entry;
static unsigned phase;
static unsigned english_key_presses[3];
static gboolean finished;
static int result = 1;

static gboolean observe_key_press(GtkWidget *widget, GdkEventKey *event, gpointer unused) {
    (void)widget;
    (void)unused;
    if (phase == 2 && event->keyval >= GDK_KEY_a && event->keyval <= GDK_KEY_c)
        english_key_presses[event->keyval - GDK_KEY_a]++;
    return FALSE;
}

static gboolean remote(const char *option, const char *value, gchar **output) {
    gchar *args[] = {"/probe/fcitx-remote", (gchar *)option, (gchar *)value, NULL};
    gint status = 0;
    GError *error = NULL;
    gchar **env = g_environ_setenv(g_get_environ(), "LD_LIBRARY_PATH", "/probe/remote-libs", TRUE);
    const gchar *host_display = g_getenv("KYLIN_IME_HOST_DISPLAY");
    if (host_display) env = g_environ_setenv(env, "DISPLAY", host_display, TRUE);
    const gchar *control_bus = g_getenv("KYLIN_IME_CONTROL_BUS_ADDRESS");
    if (control_bus) {
        env = g_environ_setenv(env, "DBUS_SESSION_BUS_ADDRESS", control_bus, TRUE);
        env = g_environ_setenv(env, "FCITX_DBUS_ADDRESS", control_bus, TRUE);
    }
    gboolean spawned = g_spawn_sync(NULL, args, env, 0, NULL, NULL, output, NULL, &status, &error);
    g_strfreev(env);
    if (!spawned || status != 0) {
        g_printerr("Fcitx control failed: %s\n", error ? error->message : option);
        g_clear_error(&error);
        gtk_main_quit();
        return FALSE;
    }
    return TRUE;
}

static void key(guint value) {
    GdkKeymapKey *map = NULL;
    gint count = 0;
    if (!gdk_keymap_get_entries_for_keyval(gdk_keymap_get_for_display(gdk_display_get_default()),
                                        value, &map, &count) || count == 0) {
        g_printerr("Key is absent from the test keymap\n");
        gtk_main_quit();
        return;
    }
    for (unsigned release = 0; release < 2; ++release) {
        GdkEvent *event = gdk_event_new(release ? GDK_KEY_RELEASE : GDK_KEY_PRESS);
        event->key.window = g_object_ref(gtk_widget_get_window(entry));
        event->key.time = GDK_CURRENT_TIME;
        event->key.keyval = value;
        event->key.hardware_keycode = map[0].keycode;
        event->key.group = map[0].group;
        gchar utf8[7] = {0};
        gunichar unicode = gdk_keyval_to_unicode(value);
        event->key.length = unicode ? g_unichar_to_utf8(unicode, utf8) : 0;
        event->key.string = g_strndup(utf8, event->key.length);
        gdk_event_set_device(event, gdk_seat_get_keyboard(gdk_display_get_default_seat(gdk_display_get_default())));
        /* Queue keys so the asynchronous IM client can replay unconsumed events. */
        gdk_event_put(event);
        gdk_event_free(event);
    }
    g_free(map);
}

static void keys(const char *text) {
    for (const char *next = text; *next; ++next) key((guint)*next);
}

static gboolean finish(gpointer unused) {
    (void)unused;
    g_print("PASS: Chinese commit, mixed English, Escape cancellation\n");
    result = 0;
    finished = TRUE;
    gtk_main_quit();
    return G_SOURCE_REMOVE;
}

static gboolean type_when_ready(gpointer unused) {
    (void)unused;
    gchar *state = NULL;
    if (!remote(NULL, NULL, &state)) return G_SOURCE_REMOVE;
    gboolean ready = g_str_equal(state, phase == 2 ? "1\n" : "2\n");
    g_free(state);
    if (!ready) {
        if (!remote(phase == 2 ? "-c" : "-o", NULL, NULL)) return G_SOURCE_REMOVE;
        return G_SOURCE_CONTINUE;
    }
    g_print("Fcitx state ready for phase %u\n", phase);
    if (phase == 0) {
        keys("ni ");
    } else if (phase == 2) {
        keys("abc");
    } else {
        keys("ni");
        key(GDK_KEY_Escape);
        keys("hao ");
    }
    return G_SOURCE_REMOVE;
}

static gboolean next_case(gpointer unused) {
    (void)unused;
    if (phase == 1) {
        gtk_entry_set_text(GTK_ENTRY(entry), "");
        keys("nihao ");
        return G_SOURCE_REMOVE;
    }
    if (phase == 2) {
        if (!remote("-c", NULL, NULL)) return G_SOURCE_REMOVE;
    } else {
        if (!remote("-s", "pinyin", NULL) || !remote("-o", NULL, NULL)) return G_SOURCE_REMOVE;
    }
    /* Fcitx control returns before the requested state reaches its input context. */
    g_timeout_add(10, type_when_ready, NULL);
    return G_SOURCE_REMOVE;
}

static void changed(GtkEditable *editable, gpointer unused) {
    (void)unused;
    const gchar *text = gtk_entry_get_text(GTK_ENTRY(editable));
    if (phase == 0 && g_str_has_suffix(text, "你")) {
        phase = 1;
        g_idle_add(next_case, NULL);
    } else if (phase == 1 && g_str_equal(text, "你好")) {
        g_print("Chinese text committed through GTK: %s\n", text);
        phase = 2;
        g_idle_add(next_case, NULL);
    } else if (phase == 2 && g_str_equal(text, "你好abc")) {
        g_print("GTK English key presses: a=%u b=%u c=%u\n",
                english_key_presses[0], english_key_presses[1], english_key_presses[2]);
        if (english_key_presses[0] != 1 || english_key_presses[1] != 1 || english_key_presses[2] != 1) {
            g_printerr("IME key delivery count mismatch: expected a=1 b=1 c=1\n");
            gtk_main_quit();
            return;
        }
        phase = 3;
        g_idle_add(next_case, NULL);
    } else if (phase == 3 && g_str_equal(text, "你好abc好")) {
        phase = 4;
        g_idle_add(finish, NULL);
    }
}

static gboolean deadline(gpointer unused) {
    (void)unused;
    if (!finished) {
        gchar *state = NULL;
        if (remote(NULL, NULL, &state)) g_printerr("Observed Fcitx state: %s", state);
        g_free(state);
        g_printerr("IME test timed out at phase %u; committed text: %s\n", phase,
                   gtk_entry_get_text(GTK_ENTRY(entry)));
        gtk_main_quit();
    }
    return G_SOURCE_REMOVE;
}

int main(int argc, char **argv) {
    gtk_init(&argc, &argv);
    g_print("GTK client DISPLAY=%s\n", gdk_display_get_name(gdk_display_get_default()));
    GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    entry = gtk_entry_new();
    gtk_container_add(GTK_CONTAINER(window), entry);
    g_signal_connect(entry, "key-press-event", G_CALLBACK(observe_key_press), NULL);
    g_signal_connect(entry, "changed", G_CALLBACK(changed), NULL);
    gtk_widget_show_all(window);
    gtk_widget_grab_focus(entry);
    gdk_window_focus(gtk_widget_get_window(window), GDK_CURRENT_TIME);
    g_timeout_add(500, next_case, NULL);
    g_timeout_add_seconds(15, deadline, NULL);
    gtk_main();
    gtk_widget_destroy(window);
    return result;
}
