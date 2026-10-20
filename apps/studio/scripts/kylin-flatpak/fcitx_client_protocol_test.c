/* Exercise the shipped client library without a desktop or a real input method. */
#include <gio/gio.h>
#include <glib/gstdio.h>
#include <gmodule.h>
#include <string.h>

#define PORTAL_NAME "org.freedesktop.portal.Fcitx"
#define FACTORY_IFACE "org.fcitx.Fcitx.InputMethod1"
#define IC_IFACE "org.fcitx.Fcitx.InputContext1"
#define NEW_FACTORY "/org/freedesktop/portal/inputmethod"
#define OLD_FACTORY "/inputmethod"
#define NEW_IC "/org/freedesktop/portal/inputcontext/1"
#define OLD_IC "/inputcontext/1"
#define DBUS_ERROR(name) "org.freedesktop.DBus.Error." name

typedef struct {
    const char *name;
    const char *new_error;
    const char *old_error;
    unsigned old_calls;
    gboolean connects;
} Case;

static const Case cases[] = {
    {"new-success", NULL, NULL, 0, TRUE},
    {"unknown-object", DBUS_ERROR("UnknownObject"), NULL, 1, TRUE},
    {"unknown-method", DBUS_ERROR("UnknownMethod"), NULL, 1, TRUE},
    {"access-denied", DBUS_ERROR("AccessDenied"), NULL, 0, FALSE},
    {"failed", DBUS_ERROR("Failed"), NULL, 0, FALSE},
    {"both-unknown-method", DBUS_ERROR("UnknownMethod"), DBUS_ERROR("UnknownMethod"), 1, FALSE},
    {"cancelled", "cancelled", NULL, 0, FALSE},
    {"no-reply", DBUS_ERROR("NoReply"), NULL, 0, FALSE}
};

typedef struct {
    const Case *scenario;
    GMainLoop *loop;
    GObject *client;
    void (*focus_in)(gpointer client);
    unsigned new_calls, old_calls, connected, focused, committed;
    guint deadline;
    gboolean complete;
    gboolean failed;
} Test;

static const char factory_xml[] =
    "<node><interface name='" FACTORY_IFACE "'>"
    "<method name='CreateInputContext'><arg type='a(ss)' direction='in'/>"
    "<arg type='o' direction='out'/><arg type='ay' direction='out'/></method>"
    "</interface></node>";

static const char ic_xml[] =
    "<node><interface name='" IC_IFACE "'>"
    "<method name='FocusIn'/><signal name='CommitString'><arg type='s'/></signal>"
    "</interface></node>";

static void stop(Test *test, gboolean failed) {
    test->complete = TRUE;
    test->failed |= failed;
    g_main_loop_quit(test->loop);
}

static void finalized(gpointer data, GObject *object) {
    (void)object;
    Test *test = data;
    if (!test->scenario->connects) stop(test, FALSE);
}

static void committed(GObject *client, const char *text, gpointer data) {
    (void)client;
    Test *test = data;
    test->committed++;
    stop(test, !g_str_equal(text, "中文"));
}

static void connected(GObject *client, gpointer data) {
    Test *test = data;
    test->connected++;
    if (!test->scenario->connects) {
        stop(test, TRUE);
        return;
    }
    test->focus_in(client);
}

static void method_call(GDBusConnection *connection, const gchar *sender,
                        const gchar *path, const gchar *interface,
                        const gchar *method, GVariant *parameters,
                        GDBusMethodInvocation *invocation, gpointer data) {
    (void)parameters;
    Test *test = data;
    if (g_str_equal(interface, FACTORY_IFACE) && g_str_equal(method, "CreateInputContext")) {
        gboolean old = g_str_equal(path, OLD_FACTORY);
        unsigned *calls = old ? &test->old_calls : &test->new_calls;
        (*calls)++;
        if (*calls > 1) stop(test, TRUE);

        /* Finalization proves the async error callback finished; no fixed settling sleep. */
        if (!test->scenario->connects) g_clear_object(&test->client);
        const char *error = old ? test->scenario->old_error : test->scenario->new_error;
        if (g_strcmp0(error, "cancelled") == 0) {
            g_dbus_method_invocation_return_error_literal(invocation, G_IO_ERROR, G_IO_ERROR_CANCELLED,
                                                         "Test cancellation");
        } else if (error) {
            g_dbus_method_invocation_return_dbus_error(invocation, error, "Test factory error");
        } else {
            const guint8 uuid[16] = {0};
            g_dbus_method_invocation_return_value(
                invocation, g_variant_new("(o@ay)", old ? OLD_IC : NEW_IC,
                    g_variant_new_fixed_array(G_VARIANT_TYPE_BYTE, uuid, G_N_ELEMENTS(uuid), sizeof(uuid[0]))));
        }
        return;
    }

    if (g_str_equal(interface, IC_IFACE) && g_str_equal(method, "FocusIn")) {
        test->focused++;
        const char *expected = test->scenario->old_calls ? OLD_IC : NEW_IC;
        if (!g_str_equal(path, expected)) stop(test, TRUE);
        g_dbus_method_invocation_return_value(invocation, NULL);
        GError *error = NULL;
        if (!g_dbus_connection_emit_signal(connection, sender, path, IC_IFACE, "CommitString",
                                           g_variant_new("(s)", "中文"), &error)) {
            g_printerr("Could not emit commit: %s\n", error->message);
            g_clear_error(&error);
            stop(test, TRUE);
        }
        return;
    }
    g_dbus_method_invocation_return_dbus_error(invocation, DBUS_ERROR("UnknownMethod"), "Unexpected call");
    stop(test, TRUE);
}

static gboolean deadline(gpointer data) {
    Test *test = data;
    test->deadline = 0;
    stop(test, TRUE);
    return G_SOURCE_REMOVE;
}

static int run_case(const char *library, const Case *scenario) {
    GError *error = NULL;
    gchar *config = g_dir_make_tmp("aster-fcitx-protocol-XXXXXX", &error);
    g_assert_no_error(error);
    gchar *socket_dir = g_build_filename(config, "fcitx", "dbus", NULL);
    g_assert_cmpint(g_mkdir_with_parents(socket_dir, 0700), ==, 0);
    g_setenv("XDG_CONFIG_HOME", config, TRUE);
    g_setenv("DISPLAY", ":61343", TRUE);

    GTestDBus *bus = g_test_dbus_new(G_TEST_DBUS_NONE);
    g_test_dbus_up(bus);
    const gchar *address = g_test_dbus_get_bus_address(bus);
    g_setenv("FCITX_DBUS_ADDRESS", address, TRUE);
    GDBusConnection *service = g_dbus_connection_new_for_address_sync(
        address, G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
        NULL, NULL, &error);
    g_assert_no_error(error);
    GVariant *owned = g_dbus_connection_call_sync(service, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "RequestName", g_variant_new("(su)", PORTAL_NAME, 0),
        G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, 1000, NULL, &error);
    g_assert_no_error(error);
    guint result = 0;
    g_variant_get(owned, "(u)", &result);
    g_assert_cmpuint(result, ==, 1);
    g_variant_unref(owned);

    Test test = {.scenario = scenario, .loop = g_main_loop_new(NULL, FALSE)};
    GDBusNodeInfo *factory = g_dbus_node_info_new_for_xml(factory_xml, &error);
    g_assert_no_error(error);
    GDBusNodeInfo *ic = g_dbus_node_info_new_for_xml(ic_xml, &error);
    g_assert_no_error(error);
    const GDBusInterfaceVTable vtable = {.method_call = method_call};
    const char *paths[] = {NEW_FACTORY, OLD_FACTORY, NEW_IC, OLD_IC};
    guint registrations[G_N_ELEMENTS(paths)];
    for (unsigned i = 0; i < G_N_ELEMENTS(paths); ++i) {
        registrations[i] = g_dbus_connection_register_object(service, paths[i],
            i < 2 ? factory->interfaces[0] : ic->interfaces[0], &vtable, &test, NULL, &error);
        g_assert_no_error(error);
        g_assert_cmpuint(registrations[i], !=, 0);
    }

    GModule *module = g_module_open(library, G_MODULE_BIND_LOCAL);
    if (!module) g_error("Cannot load client: %s", g_module_error());
    g_module_make_resident(module);
    GObject *(*new_client)(void) = NULL;
    g_assert_true(g_module_symbol(module, "fcitx_client_new", (gpointer *)&new_client));
    g_assert_true(g_module_symbol(module, "fcitx_client_focus_in", (gpointer *)&test.focus_in));
    test.client = new_client();
    g_object_weak_ref(test.client, finalized, &test);
    g_signal_connect(test.client, "connected", G_CALLBACK(connected), &test);
    g_signal_connect(test.client, "commit-string", G_CALLBACK(committed), &test);
    test.deadline = g_timeout_add_seconds(2, deadline, &test);
    g_main_loop_run(test.loop);
    if (test.deadline) g_source_remove(test.deadline);

    gboolean valid = test.complete && !test.failed && test.new_calls == 1 &&
        test.old_calls == scenario->old_calls && test.connected == (unsigned)scenario->connects &&
        test.focused == (unsigned)scenario->connects && test.committed == (unsigned)scenario->connects;
    g_print("%s: %s new=%u old=%u connected=%u focus=%u commit=%u\n", scenario->name,
        valid ? "PASS" : "FAIL", test.new_calls, test.old_calls, test.connected, test.focused, test.committed);

    if (test.client) {
        g_object_weak_unref(test.client, finalized, &test);
        g_clear_object(&test.client);
    }
    for (unsigned i = 0; i < G_N_ELEMENTS(registrations); ++i)
        g_dbus_connection_unregister_object(service, registrations[i]);
    g_dbus_connection_close_sync(service, NULL, NULL);
    g_object_unref(service);
    g_dbus_node_info_unref(factory);
    g_dbus_node_info_unref(ic);
    g_test_dbus_down(bus);
    g_object_unref(bus);
    g_main_loop_unref(test.loop);
    g_rmdir(socket_dir);
    g_free(socket_dir);
    socket_dir = g_build_filename(config, "fcitx", NULL);
    g_rmdir(socket_dir);
    g_free(socket_dir);
    g_rmdir(config);
    g_free(config);
    return valid ? 0 : 1;
}

int main(int argc, char **argv) {
    if (argc < 2 || argc > 3) {
        g_printerr("usage: %s LIBFCITX_GCLIENT [CASE]\n", argv[0]);
        return 2;
    }
    if (argc == 3) {
        for (unsigned i = 0; i < G_N_ELEMENTS(cases); ++i)
            if (g_str_equal(argv[2], cases[i].name)) return run_case(argv[1], &cases[i]);
        g_printerr("Unknown case: %s\n", argv[2]);
        return 2;
    }

    int result = 0;
    for (unsigned i = 0; i < G_N_ELEMENTS(cases); ++i) {
        gchar *args[] = {argv[0], argv[1], (gchar *)cases[i].name, NULL};
        gint status = 0;
        GError *error = NULL;
        if (!g_spawn_sync(NULL, args, NULL, 0, NULL, NULL, NULL, NULL, &status, &error) ||
            !g_spawn_check_wait_status(status, &error)) {
            g_printerr("Case %s failed: %s\n", cases[i].name, error->message);
            g_clear_error(&error);
            result = 1;
        }
    }
    return result;
}
