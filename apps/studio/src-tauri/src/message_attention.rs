//! Dock badge, menu-bar count, and one banner per conversation.
//! The renderer sends the unread total. This module only paints it.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const EVENT: &str = "message-attention-open";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageAttentionOpen {
    conversation_id: Option<String>,
}

/// `""` clears the badge. Counts above 99 share the conversation badge's `99+`.
pub fn message_badge_label(count: u32) -> String {
    if count == 0 {
        String::new()
    } else if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
}

static COUNT: AtomicU32 = AtomicU32::new(0);

pub fn set_count(app: &AppHandle, count: u32) {
    COUNT.store(count, Ordering::Relaxed);
    crate::desktop_settings::set_status_title(&message_badge_label(count));
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            macos::apply_count(&app, COUNT.load(Ordering::Relaxed));
        });
    }
}

pub fn publish(app: &AppHandle, id: &str, title: &str, body: &str) {
    let id = id.trim();
    let title = clip(title.trim(), 120);
    if id.is_empty() || title.is_empty() {
        return;
    }
    let id = id.to_owned();
    let body = clip(body.trim(), 180);
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let _ = app
            .clone()
            .run_on_main_thread(move || macos::publish(&app, &id, &title, &body));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, id, body);
    }
}

pub fn withdraw(app: &AppHandle, id: &str) {
    let id = id.trim().to_owned();
    if id.is_empty() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let app = app.clone();
        let _ = app.run_on_main_thread(move || macos::withdraw(&id));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, id);
    }
}

pub(crate) fn emit_open(app: &AppHandle, conversation_id: Option<String>) {
    let _ = app.emit(EVENT, MessageAttentionOpen { conversation_id });
}

fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::atomic::{AtomicBool, Ordering};

    use objc2::define_class;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObjectProtocol, ProtocolObject};
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSApplication, NSDockTile};
    use objc2_foundation::{NSArray, NSError, NSObject, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
        UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
        UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };
    use tauri::AppHandle;

    use super::message_badge_label;
    use crate::desktop_settings;

    struct NoticeIvars {
        app: AppHandle,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = NoticeIvars]
        struct MessageNoticeDelegate;

        unsafe impl NSObjectProtocol for MessageNoticeDelegate {}

        unsafe impl UNUserNotificationCenterDelegate for MessageNoticeDelegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                // A focused app stays silent unless the delegate asks for a banner.
                completion_handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                let id = response.notification().request().identifier().to_string();
                let app = self.ivars().app.clone();
                desktop_settings::show_main(&app);
                super::emit_open(&app, Some(id));
                completion_handler.call(());
            }
        }
    );

    pub(super) fn apply_count(app: &AppHandle, count: u32) {
        set_dock(&message_badge_label(count));
        if count > 0 {
            ensure_notifications(app);
        }
    }

    pub(super) fn publish(app: &AppHandle, id: &str, title: &str, body: &str) {
        ensure_notifications(app);
        let Some(_marker) = MainThreadMarker::new() else {
            return;
        };
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, None);
    }

    pub(super) fn withdraw(id: &str) {
        let Some(_marker) = MainThreadMarker::new() else {
            return;
        };
        let label = NSString::from_str(id);
        let ids = NSArray::from_slice(&[label.as_ref()]);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.removeDeliveredNotificationsWithIdentifiers(&ids);
        center.removePendingNotificationRequestsWithIdentifiers(&ids);
    }

    fn set_dock(label: &str) {
        let Some(marker) = MainThreadMarker::new() else {
            return;
        };
        let tile: Retained<NSDockTile> = NSApplication::sharedApplication(marker).dockTile();
        if label.is_empty() {
            tile.setBadgeLabel(None);
        } else {
            tile.setBadgeLabel(Some(&NSString::from_str(label)));
        }
    }

    fn ensure_notifications(app: &AppHandle) {
        static ASKED: AtomicBool = AtomicBool::new(false);
        if ASKED.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(marker) = MainThreadMarker::new() else {
            ASKED.store(false, Ordering::Release);
            return;
        };
        let allocated = marker
            .alloc::<MessageNoticeDelegate>()
            .set_ivars(NoticeIvars { app: app.clone() });
        let delegate: Retained<MessageNoticeDelegate> =
            unsafe { msg_send![super(allocated), init] };
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // The center keeps only a weak reference to its delegate.
        std::mem::forget(delegate);
        let block = block2::RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &block,
        );
    }
}

#[tauri::command]
pub fn set_message_attention(app: AppHandle, count: u32) {
    set_count(&app, count);
}

#[tauri::command]
pub fn publish_message_notification(app: AppHandle, id: String, title: String, body: String) {
    publish(&app, &id, &title, &body);
}

#[tauri::command]
pub fn withdraw_message_notification(app: AppHandle, id: String) {
    withdraw(&app, &id);
}

#[cfg(test)]
mod tests {
    use super::message_badge_label;

    #[test]
    fn badge_label_hides_zero_and_caps_at_ninety_nine() {
        assert_eq!(message_badge_label(0), "");
        assert_eq!(message_badge_label(4), "4");
        assert_eq!(message_badge_label(99), "99");
        assert_eq!(message_badge_label(100), "99+");
    }
}
