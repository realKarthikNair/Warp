use crate::gettext::duration;
use adw::glib;
use simple_moving_average::{SingleSumSMA, SMA};
use std::fmt::{Debug, Formatter};
use std::ops::Add;
use std::time::{Duration, Instant};

const MOVING_AVG_MS: usize = 5000;
const SAMPLE_DURATION_MS: usize = 50;
const SAMPLE_COUNT: usize = MOVING_AVG_MS / SAMPLE_DURATION_MS;
const SAMPLES_PER_SECOND: usize = 1000 / SAMPLE_DURATION_MS;

pub struct FileTransferProgress {
    start_time: Instant,
    next_feed_offset: usize,
    avg: SingleSumSMA<usize, usize, SAMPLE_COUNT>,
    done_bytes: usize,
    total_bytes: usize,
}

impl Debug for FileTransferProgress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FileTransferProgress {} / {} bytes",
            self.avg.get_most_recent_sample().unwrap_or(0),
            self.total_bytes
        )
    }
}

impl FileTransferProgress {
    pub fn begin(total_bytes: usize) -> Self {
        Self {
            start_time: Instant::now(),
            next_feed_offset: 0,
            avg: SingleSumSMA::new(),
            done_bytes: 0,
            total_bytes,
        }
    }

    fn should_add_sample(&self) -> bool {
        // We only care about one record every 50ms
        let next_feed = self
            .start_time
            .add(Duration::from_millis(self.next_feed_offset as u64));
        let now = Instant::now();

        // saturating_duration returns 0 if next_feed is earlier than now
        next_feed.saturating_duration_since(now).as_millis() == 0
    }

    pub fn set_progress(&mut self, bytes: usize) -> bool {
        if self.should_add_sample() {
            let offset = bytes - self.done_bytes;
            self.done_bytes = bytes;
            self.avg.add_sample(offset);
            self.next_feed_offset += SAMPLE_DURATION_MS;

            true
        } else {
            false
        }
    }

    fn bytes_per_sample_size(&self) -> Option<usize> {
        if self.avg.get_num_samples() >= SAMPLES_PER_SECOND {
            Some(self.avg.get_average())
        } else {
            None
        }
    }

    pub fn get_bytes_s(&self) -> Option<usize> {
        self.bytes_per_sample_size()
            .map(|avg| avg * SAMPLES_PER_SECOND)
    }

    pub fn get_pretty_bytes_per_s_string(&self) -> Option<String> {
        self.get_bytes_s().map(|bytes_s| {
            let mut bytes_str = glib::format_size(bytes_s as u64).to_string();
            bytes_str.push_str(" / s");
            bytes_str
        })
    }

    pub fn get_time_remaining(&self) -> Option<Duration> {
        self.get_bytes_s().and_then(|bytes_s| {
            self.total_bytes.checked_sub(self.done_bytes).map_or(
                Some(Duration::ZERO),
                |remaining_bytes| {
                    let secs = remaining_bytes as f64 / bytes_s as f64;
                    Some(Duration::from_secs_f64(secs))
                },
            )
        })
    }

    pub fn get_pretty_time_remaining(&self) -> Option<String> {
        self.get_time_remaining()
            .and_then(|duration| chrono::Duration::from_std(duration).ok())
            .map(|d| duration::left(self.done_bytes, self.total_bytes, &d))
    }
}
