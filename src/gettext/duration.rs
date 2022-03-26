use super::*;
use gtk::glib;

/// From https://gitlab.gnome.org/World/pika-backup/-/blob/main/src/ui/utils/duration.rs
pub fn left(done_bytes: usize, total_bytes: usize, d: &chrono::Duration) -> String {
    let bytes_str = glib::format_size(done_bytes as u64);
    let total_str = glib::format_size(total_bytes as u64);

    if d.num_minutes() < 2 {
        ngettextf(
            // Translators: Example: 11.3MB / 20.7MB - 3 seconds left
            "{0} / {1} - One second left",
            "{0} / {1} - {2} seconds left",
            (d.num_seconds() + 1) as u32,
            &[&bytes_str, &total_str, &(d.num_seconds() + 1).to_string()],
        )
    } else if d.num_hours() < 2 {
        ngettextf(
            // Translators: Example: 110.3MB / 200.7MB - 5 minutes left
            "{0} / {1} - One minute left",
            "{0} / {1} - {2} minutes left",
            (d.num_minutes() + 1) as u32,
            &[&bytes_str, &total_str, &(d.num_minutes() + 1).to_string()],
        )
    } else if d.num_days() < 2 {
        ngettextf(
            // Translators: Example: 110.3GB / 200.7GB - 2 hours left
            "{0} / {1} - One hour left",
            "{0} / {1} - {2} hours left",
            (d.num_hours() + 1) as u32,
            &[&bytes_str, &total_str, &(d.num_hours() + 1).to_string()],
        )
    } else {
        ngettextf(
            // Translators: Example: 11.3TB / 20.7TB - 2 days left
            "{0} / {1} - One day left",
            "{0} / {1} - {2} days left",
            (d.num_days() + 1) as u32,
            &[&bytes_str, &total_str, &(d.num_days() + 1).to_string()],
        )
    }
}
