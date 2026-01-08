use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::data::shared_booking::TimeSlot;
use crate::settings::AlertConfig;

static NOTIFICATION_STATE: OnceLock<Arc<RwLock<NotificationState>>> = OnceLock::new();

fn get_notification_state() -> &'static Arc<RwLock<NotificationState>> {
    NOTIFICATION_STATE.get_or_init(|| Arc::new(RwLock::new(NotificationState::default())))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationState {
    /// Tracks which slots we've already notified about (location_id -> slot times)
    pub notified_slots: HashMap<String, Vec<String>>,
    /// Active alerts for the webpage
    pub active_alerts: Vec<SlotAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotAlert {
    pub location_id: String,
    pub location_name: String,
    pub slot_time: String,
    pub created_at: String,
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedSlot {
    pub location_id: String,
    pub location_name: String,
    pub slot: TimeSlot,
}

pub struct NotificationManager;

impl NotificationManager {
    /// Check available slots against alert configurations and return matches
    pub fn check_for_matches(
        alerts: &[AlertConfig],
        available_slots: &[(String, String, Vec<TimeSlot>)], // (location_id, location_name, slots)
    ) -> Vec<MatchedSlot> {
        let mut matches = Vec::new();

        for (location_id, location_name, slots) in available_slots {
            // Find alert config for this location
            let alert = alerts.iter().find(|a| &a.location_id == location_id && a.enabled);
            
            if let Some(alert_config) = alert {
                for slot in slots {
                    if slot.availability && Self::slot_matches_months(&slot.start_time, &alert_config.months) {
                        matches.push(MatchedSlot {
                            location_id: location_id.clone(),
                            location_name: location_name.clone(),
                            slot: slot.clone(),
                        });
                    }
                }
            }
        }

        matches
    }

    /// Check if a slot's date falls within the specified months
    fn slot_matches_months(start_time: &str, months: &[u32]) -> bool {
        if months.is_empty() {
            return true; // No month filter means all months match
        }

        // start_time format is "DD/MM/YYYY HH:MM"
        if let Some(date_str) = start_time.split(' ').next() {
            if let Ok(slot_date) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
                let slot_month = slot_date.format("%m").to_string().parse::<u32>().unwrap_or(0);
                return months.contains(&slot_month);
            }
        }
        false
    }

    /// Process new matches - create alerts and trigger notifications
    pub fn process_matches(matches: Vec<MatchedSlot>) -> Vec<SlotAlert> {
        let mut state = get_notification_state().write().unwrap();
        let mut new_alerts = Vec::new();

        for matched in matches {
            let notified = state.notified_slots
                .entry(matched.location_id.clone())
                .or_insert_with(Vec::new);

            // Only create alert if we haven't notified about this slot yet
            if !notified.contains(&matched.slot.start_time) {
                notified.push(matched.slot.start_time.clone());

                let alert = SlotAlert {
                    location_id: matched.location_id,
                    location_name: matched.location_name,
                    slot_time: matched.slot.start_time,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    dismissed: false,
                };

                state.active_alerts.push(alert.clone());
                new_alerts.push(alert);
            }
        }

        new_alerts
    }

    /// Get all active (non-dismissed) alerts
    pub fn get_active_alerts() -> Vec<SlotAlert> {
        let state = get_notification_state().read().unwrap();
        state.active_alerts
            .iter()
            .filter(|a| !a.dismissed)
            .cloned()
            .collect()
    }

    /// Dismiss an alert
    pub fn dismiss_alert(location_id: &str, slot_time: &str) {
        let mut state = get_notification_state().write().unwrap();
        for alert in &mut state.active_alerts {
            if alert.location_id == location_id && alert.slot_time == slot_time {
                alert.dismissed = true;
            }
        }
    }

    /// Dismiss all alerts
    pub fn dismiss_all_alerts() {
        let mut state = get_notification_state().write().unwrap();
        for alert in &mut state.active_alerts {
            alert.dismissed = true;
        }
    }

    /// Clear notification history (useful when changing alert settings)
    pub fn clear_notification_history() {
        let mut state = get_notification_state().write().unwrap();
        state.notified_slots.clear();
        state.active_alerts.clear();
    }

    /// Send Windows toast notification
    #[cfg(all(feature = "ssr", target_os = "windows"))]
    pub fn send_windows_notification(title: &str, body: &str) -> Result<(), String> {
        use std::process::Command;

        // Use PowerShell to show Windows toast notification
        let script = format!(
            r#"
            [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

            $template = @"
            <toast>
                <visual>
                    <binding template="ToastText02">
                        <text id="1">{}</text>
                        <text id="2">{}</text>
                    </binding>
                </visual>
                <audio src="ms-winsoundevent:Notification.Default"/>
            </toast>
"@

            $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
            $xml.LoadXml($template)
            $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("NSW Driver Test Alert").Show($toast)
            "#,
            title.replace('"', "'"),
            body.replace('"', "'")
        );

        let output = Command::new("powershell")
            .args(["-ExecutionPolicy", "Bypass", "-Command", &script])
            .output()
            .map_err(|e| format!("Failed to execute PowerShell: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("PowerShell notification failed: {}", stderr))
        }
    }

    #[cfg(not(all(feature = "ssr", target_os = "windows")))]
    pub fn send_windows_notification(_title: &str, _body: &str) -> Result<(), String> {
        // No-op on non-Windows or non-SSR builds
        Ok(())
    }

    /// Send notifications for new alerts
    #[cfg(feature = "ssr")]
    pub fn notify_new_alerts(alerts: &[SlotAlert]) {
        for alert in alerts {
            let title = format!("🚗 Slot Available at {}!", alert.location_name);
            let body = format!("Available slot: {}", alert.slot_time);
            
            if let Err(e) = Self::send_windows_notification(&title, &body) {
                eprintln!("WARN: Failed to send Windows notification: {}", e);
            }
        }
    }

    #[cfg(not(feature = "ssr"))]
    pub fn notify_new_alerts(_alerts: &[SlotAlert]) {
        // No-op on client side
    }
}
