use chrono::DateTime;

pub fn format_iso_date(iso_string: &str) -> String {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(iso_string) {
        return datetime.format("%d %b %Y, %H:%M UTC").to_string();
    } else {
        iso_string.to_string()
    }
}
