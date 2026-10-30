use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::time::{Instant, sleep_until};

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub separate_types: bool,
    pub embed_metadata: bool,
    pub speed_kib: u32,
    pub delay_ms: u32,
    pub jitter_ms: u32,
}
pub fn from_app(app: &crate::AppWindow) -> Options {
    Options {
        separate_types: app.get_separate_media_folders(),
        embed_metadata: app.get_embed_metadata(),
        speed_kib: app.get_download_speed_kib().max(0) as u32,
        delay_ms: app.get_download_delay_ms().max(0) as u32,
        jitter_ms: app.get_download_jitter_ms().max(0) as u32,
    }
}
pub fn parse_limits(
    speed: &str,
    delay: &str,
    jitter: &str,
) -> Result<(u32, u32, u32), &'static str> {
    let parse = |text: &str, max: u32| {
        text.trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value <= max)
    };
    match (
        parse(speed, 1_048_576),
        parse(delay, 3_600_000),
        parse(jitter, 3_600_000),
    ) {
        (Some(speed), Some(delay), Some(jitter)) => Ok((speed, delay, jitter)),
        _ => Err("Enter whole numbers: speed 0–1048576 KiB/s, delay/jitter 0–3600000 ms."),
    }
}
pub fn media_folder(kind: &str) -> &'static str {
    match kind {
        "photo" => "Image",
        "video" => "Video",
        "animated_gif" => "GIF",
        "text" => "Text",
        _ => "Other",
    }
}
pub struct Throttle {
    speed: u64,
    delay: Duration,
    jitter_ms: u64,
    random: RandomState,
    sequence: AtomicU64,
    requests: Mutex<Instant>,
    bytes: Mutex<Instant>,
}
impl Throttle {
    pub fn new(options: Options) -> Arc<Self> {
        Arc::new(Self {
            speed: options.speed_kib.min(1_048_576) as u64 * 1024,
            delay: Duration::from_millis(options.delay_ms.min(3_600_000) as u64),
            jitter_ms: options.jitter_ms.min(3_600_000) as u64,
            random: RandomState::new(),
            sequence: AtomicU64::new(0),
            requests: Mutex::new(Instant::now()),
            bytes: Mutex::new(Instant::now()),
        })
    }
    // Independently seeded per download batch. Hashing a counter avoids shared RNG locks.
    fn interval(&self) -> Duration {
        let extra = if self.jitter_ms == 0 {
            0
        } else {
            self.random
                .hash_one(self.sequence.fetch_add(1, Ordering::Relaxed))
                % (self.jitter_ms + 1)
        };
        self.delay + Duration::from_millis(extra)
    }
    async fn wait(deadline: Instant, cancel: &AtomicBool) -> bool {
        while !cancel.load(Ordering::Relaxed) {
            if Instant::now() >= deadline {
                return true;
            }
            tokio::select! { _ = sleep_until(deadline) => return !cancel.load(Ordering::Relaxed), _ = tokio::time::sleep(Duration::from_millis(25)) => {} }
        }
        false
    }
    pub async fn request(&self, cancel: &AtomicBool) -> bool {
        let deadline = {
            let mut next = self.requests.lock().unwrap();
            let deadline = (*next).max(Instant::now());
            *next = deadline + self.interval();
            deadline
        };
        Self::wait(deadline, cancel).await
    }
    pub async fn consume(&self, bytes: usize, cancel: &AtomicBool) -> bool {
        if self.speed == 0 {
            return !cancel.load(Ordering::Relaxed);
        }
        let deadline = {
            let mut next = self.bytes.lock().unwrap();
            let deadline = (*next).max(Instant::now())
                + Duration::from_secs_f64(bytes as f64 / self.speed as f64);
            *next = deadline;
            deadline
        };
        Self::wait(deadline, cancel).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_limits_are_rejected_instead_of_disabling_a_saved_cap() {
        assert_eq!(parse_limits("0", "0", "0"), Ok((0, 0, 0)));
        assert_eq!(parse_limits(" 512 ", "1500", "250"), Ok((512, 1500, 250)));
        for (speed, delay) in [
            ("-1", "0"),
            ("1.5", "0"),
            ("0", "oops"),
            ("1048577", "0"),
            ("0", "3600001"),
        ] {
            assert!(parse_limits(speed, delay, "0").is_err());
        }
    }
    #[test]
    fn jitter_is_bounded_varied_and_disabled_by_default() {
        let plain = Throttle::new(Options {
            delay_ms: 100,
            ..Default::default()
        });
        assert_eq!(plain.interval(), Duration::from_millis(100));
        let varied = Throttle::new(Options {
            delay_ms: 100,
            jitter_ms: 500,
            ..Default::default()
        });
        let samples: std::collections::HashSet<_> = (0..100).map(|_| varied.interval()).collect();
        assert!(samples.len() > 10);
        assert!(
            samples
                .iter()
                .all(|value| *value >= Duration::from_millis(100)
                    && *value <= Duration::from_millis(600))
        );
        for bad in ["-1", "oops", "1.2", "3600001"] {
            assert!(parse_limits("0", "0", bad).is_err());
        }
    }
    #[tokio::test]
    async fn jitter_waits_share_the_request_gate_and_can_be_cancelled() {
        let throttle = Throttle::new(Options {
            delay_ms: 3_600_000,
            jitter_ms: 3_600_000,
            ..Default::default()
        });
        let cancel = Arc::new(AtomicBool::new(false));
        assert!(throttle.request(&cancel).await);
        let flag = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            flag.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        assert!(!throttle.request(&cancel).await);
        assert!(start.elapsed() < Duration::from_millis(250));
    }
    #[tokio::test]
    async fn concurrent_transfers_share_one_limit_and_waits_cancel_promptly() {
        let throttle = Throttle::new(Options {
            speed_kib: 64,
            delay_ms: 100,
            ..Default::default()
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let (first, second) = tokio::join!(
            throttle.consume(8192, &cancel),
            throttle.consume(8192, &cancel)
        );
        assert!(first && second && start.elapsed() >= Duration::from_millis(240));
        let start = Instant::now();
        assert!(throttle.request(&cancel).await);
        assert!(throttle.request(&cancel).await);
        assert!(start.elapsed() >= Duration::from_millis(90));
        let flag = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            flag.store(true, Ordering::Relaxed);
        });
        let start = Instant::now();
        assert!(!throttle.consume(1024 * 1024, &cancel).await);
        assert!(start.elapsed() < Duration::from_millis(250));
    }
}

/// Auto caps network transfers by media mix; explicit preferences remain authoritative.
pub fn worker_count<'a>(requested: usize, kinds: impl Iterator<Item = &'a str>) -> usize {
    if requested > 0 {
        return requested.clamp(1, 50);
    }
    let kinds = kinds.collect::<Vec<_>>();
    let video = kinds
        .iter()
        .filter(|kind| matches!(**kind, "video" | "animated_gif"))
        .count();
    if !kinds.is_empty() && video == kinds.len() {
        3
    } else if video == 0 {
        8
    } else {
        10
    }
}
#[cfg(test)]
mod auto_tests {
    #[test]
    fn auto_matches_media_mix_and_does_not_override_manual_limits() {
        assert_eq!(super::worker_count(0, ["photo", "photo"].into_iter()), 8);
        assert_eq!(
            super::worker_count(0, ["video", "animated_gif"].into_iter()),
            3
        );
        assert_eq!(super::worker_count(0, ["photo", "video"].into_iter()), 10);
        assert_eq!(super::worker_count(50, ["video"].into_iter()), 50);
    }
}
