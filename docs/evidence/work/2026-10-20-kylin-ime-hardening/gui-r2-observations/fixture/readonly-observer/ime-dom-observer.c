/* Test-only LD_PRELOAD observer. The application and its resources are unchanged. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <string.h>
#include <webkit2/webkit2.h>

#include "ime-dom-observer.h"

static GHashTable *observed_managers;
static unsigned int log_count;
static const char *channel = "asterImeReadonlyTrace20261020";

static void trace_message(WebKitUserContentManager *manager, WebKitJavascriptResult *result,
                          gpointer unused) {
    (void)manager;
    (void)unused;
    if (log_count++ >= 4000) return;
    JSCValue *value = webkit_javascript_result_get_js_value(result);
    if (!value || !jsc_value_is_string(value)) return;
    char *message = jsc_value_to_string(value);
    if (message && strlen(message) <= 4096 && !strchr(message, '\n'))
        fprintf(stderr, "[IME-DOM] %s\n", message);
    g_free(message);
}

static void manager_finalized(gpointer unused, GObject *manager) {
    (void)unused;
    g_hash_table_remove(observed_managers, manager);
}

static void observe_view(WebKitWebView *view, const char *entry) {
    WebKitUserContentManager *manager = webkit_web_view_get_user_content_manager(view);
    if (!manager) return;
    if (!observed_managers) observed_managers = g_hash_table_new(g_direct_hash, g_direct_equal);
    if (g_hash_table_contains(observed_managers, manager)) return;
    gulong signal = g_signal_connect(manager,
                                    "script-message-received::asterImeReadonlyTrace20261020",
                                    G_CALLBACK(trace_message), NULL);
    if (!webkit_user_content_manager_register_script_message_handler(manager, channel)) {
        g_signal_handler_disconnect(manager, signal);
        fprintf(stderr, "[IME-OBSERVER] channel registration failed; no observation script added\n");
        return;
    }
    WebKitUserScript *script = webkit_user_script_new(IME_OBSERVER_SCRIPT,
                                                     WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
                                                     WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
                                                     NULL, NULL);
    webkit_user_content_manager_add_script(manager, script);
    webkit_user_script_unref(script);
    g_hash_table_add(observed_managers, manager);
    g_object_weak_ref(G_OBJECT(manager), manager_finalized, NULL);
    fprintf(stderr, "[IME-OBSERVER] passive bounded script installed before original %s\n", entry);
}

void webkit_web_view_load_uri(WebKitWebView *view, const gchar *uri) {
    typedef void (*LoadUri)(WebKitWebView *, const gchar *);
    LoadUri original = (LoadUri)dlsym(RTLD_NEXT, "webkit_web_view_load_uri");
    if (!original) {
        fprintf(stderr, "[IME-OBSERVER] missing original load_uri: %s\n", dlerror());
        return;
    }
    observe_view(view, "load_uri");
    original(view, uri);
}

void webkit_web_view_load_html(WebKitWebView *view, const gchar *content, const gchar *base_uri) {
    typedef void (*LoadHtml)(WebKitWebView *, const gchar *, const gchar *);
    LoadHtml original = (LoadHtml)dlsym(RTLD_NEXT, "webkit_web_view_load_html");
    if (!original) {
        fprintf(stderr, "[IME-OBSERVER] missing original load_html: %s\n", dlerror());
        return;
    }
    observe_view(view, "load_html");
    original(view, content, base_uri);
}

void webkit_web_view_load_request(WebKitWebView *view, WebKitURIRequest *request) {
    typedef void (*LoadRequest)(WebKitWebView *, WebKitURIRequest *);
    LoadRequest original = (LoadRequest)dlsym(RTLD_NEXT, "webkit_web_view_load_request");
    if (!original) {
        fprintf(stderr, "[IME-OBSERVER] missing original load_request: %s\n", dlerror());
        return;
    }
    observe_view(view, "load_request");
    original(view, request);
}
