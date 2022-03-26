use super::*;

/// From https://gitlab.gnome.org/World/pika-backup/-/blob/main/src/ui/utils/duration.rs
pub fn left(d: &chrono::Duration) -> String {
    if d.num_minutes() < 2 {
        ngettextf_(
            "One second left",
            "{} seconds left",
            (d.num_seconds() + 1) as u32,
        )
    } else if d.num_hours() < 2 {
        ngettextf_(
            "One minute left",
            "{} minutes left",
            (d.num_minutes() + 1) as u32,
        )
    } else if d.num_days() < 2 {
        ngettextf_("One hour left", "{} hours left", (d.num_hours() + 1) as u32)
    } else {
        ngettextf_("One day left", "{} days left", (d.num_days() + 1) as u32)
    }
}
