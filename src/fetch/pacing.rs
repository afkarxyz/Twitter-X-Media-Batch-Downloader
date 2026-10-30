//! Waits between fetch page requests (Settings → Fetch pacing).
//!
//! Each fetch page, whether the next page of one account, the first page of the next
//! account in a batch, or a retry, starts no sooner than the delay plus a random 0 to
//! the extra delay after the previous fetch page finished. Account search and auth
//! token checks are not paced: they are single requests the user waits for.
use std::{
    collections::hash_map::RandomState,
    hash::BuildHasher,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_MS: u32 = 3_600_000;

static GATE: Gate = Gate::new();

/// Tests that change the shared delay hold this, so they do not slow or skip each other's waits.
#[cfg(test)]
pub(crate) static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Applies the saved delay and extra delay, in milliseconds.
pub fn configure(delay_ms: u32, jitter_ms: u32) {
    GATE.configure(delay_ms, jitter_ms);
}

/// Waits until the next fetch page may start, showing the countdown as the status.
/// The returned guard records when the request finished as it is dropped.
pub async fn start(
    ui: &crate::ui::runtime::Weak<crate::AppWindow>,
    cancel: &AtomicBool,
) -> Result<Pace, String> {
    GATE.start(ui, cancel).await
}

/// Applies the values typed in Settings, or shows why they cannot be used.
pub fn apply(app: &crate::AppWindow, delay: &str, jitter: &str) -> bool {
    match parse(delay, jitter) {
        Ok((delay, jitter)) => {
            // A running fetch uses the new values from its next page on.
            configure(delay, jitter);
            app.set_fetch_delay_ms(delay as i32);
            app.set_fetch_jitter_ms(jitter as i32);
            app.set_fetch_pacing_error(String::new());
            app.invoke_settings_changed();
            true
        }
        Err(error) => {
            app.set_fetch_pacing_error(error.into());
            false
        }
    }
}

pub fn parse(delay: &str, jitter: &str) -> Result<(u32, u32), &'static str> {
    let parse = |text: &str| {
        text.trim()
            .parse::<u32>()
            .ok()
            .filter(|value| *value <= MAX_MS)
    };
    match (parse(delay), parse(jitter)) {
        (Some(delay), Some(jitter)) => Ok((delay, jitter)),
        _ => Err("Enter whole numbers from 0 to 3600000 ms."),
    }
}

struct Gate {
    delay_ms: AtomicU32,
    jitter_ms: AtomicU32,
    last: Mutex<Option<Instant>>,
    sequence: AtomicU64,
    random: OnceLock<RandomState>,
}

impl Gate {
    const fn new() -> Self {
        Self {
            delay_ms: AtomicU32::new(0),
            jitter_ms: AtomicU32::new(0),
            last: Mutex::new(None),
            sequence: AtomicU64::new(0),
            random: OnceLock::new(),
        }
    }

    fn configure(&self, delay_ms: u32, jitter_ms: u32) {
        self.delay_ms.store(delay_ms.min(MAX_MS), Ordering::Relaxed);
        self.jitter_ms
            .store(jitter_ms.min(MAX_MS), Ordering::Relaxed);
    }

    // Hashing a counter with a per-process seed avoids a shared RNG, as download jitter does.
    fn interval(&self) -> Duration {
        let delay = self.delay_ms.load(Ordering::Relaxed) as u64;
        let jitter = self.jitter_ms.load(Ordering::Relaxed) as u64;
        let extra = if jitter == 0 {
            0
        } else {
            self.random
                .get_or_init(RandomState::new)
                .hash_one(self.sequence.fetch_add(1, Ordering::Relaxed))
                % (jitter + 1)
        };
        Duration::from_millis(delay + extra)
    }

    async fn start(
        &'static self,
        ui: &crate::ui::runtime::Weak<crate::AppWindow>,
        cancel: &AtomicBool,
    ) -> Result<Pace, String> {
        let last = *self.last.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(last) = last {
            let deadline = last + self.interval();
            let mut shown = None;
            while Instant::now() < deadline {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Extraction cancelled.".into());
                }
                let seconds = deadline.saturating_duration_since(Instant::now()).as_secs() + 1;
                if shown != Some(seconds) {
                    shown = Some(seconds);
                    let status = format!("Fetch delay · next page in {seconds}s…");
                    let _ = ui.upgrade_in_event_loop(move |app| app.set_status(status));
                }
                let step = deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(25));
                tokio::time::sleep(step).await;
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Extraction cancelled.".into());
        }
        Ok(Pace { gate: self })
    }
}

/// Held for one fetch page request; dropping it starts the next delay.
pub struct Pace {
    gate: &'static Gate,
}

impl Drop for Pace {
    fn drop(&mut self) {
        *self
            .gate
            .last
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn gate(delay_ms: u32, jitter_ms: u32) -> &'static Gate {
        let gate = Box::leak(Box::new(Gate::new()));
        gate.configure(delay_ms, jitter_ms);
        gate
    }

    fn ui() -> crate::ui::runtime::Weak<crate::AppWindow> {
        crate::ui::runtime::Weak::default()
    }

    #[test]
    fn values_are_whole_milliseconds_within_an_hour() {
        assert_eq!(parse("0", "0"), Ok((0, 0)));
        assert_eq!(parse(" 2000 ", "1000"), Ok((2000, 1000)));
        assert_eq!(parse("3600000", "3600000"), Ok((MAX_MS, MAX_MS)));
        for (delay, jitter) in [
            ("-1", "0"),
            ("1.5", "0"),
            ("abc", "0"),
            ("", "0"),
            ("3600001", "0"),
            ("0", "x"),
        ] {
            assert!(parse(delay, jitter).is_err(), "{delay:?} {jitter:?}");
        }
    }

    #[test]
    fn the_extra_delay_varies_within_its_bound_and_is_off_at_zero() {
        let plain = gate(250, 0);
        assert!((0..20).all(|_| plain.interval() == Duration::from_millis(250)));
        let varied = gate(100, 400);
        let samples: std::collections::HashSet<_> = (0..200).map(|_| varied.interval()).collect();
        assert!(samples.len() > 20, "the extra delay should vary");
        assert!(samples.iter().all(|value| {
            *value >= Duration::from_millis(100) && *value <= Duration::from_millis(500)
        }));
    }

    #[tokio::test]
    async fn the_first_page_starts_at_once_and_later_pages_wait_after_the_previous_one() {
        let gate = gate(150, 0);
        let cancel = AtomicBool::new(false);
        let began = Instant::now();
        drop(gate.start(&ui(), &cancel).await.unwrap());
        assert!(
            began.elapsed() < Duration::from_millis(100),
            "no wait before the first page"
        );
        let began = Instant::now();
        let pace = gate.start(&ui(), &cancel).await.unwrap();
        assert!(began.elapsed() >= Duration::from_millis(140));
        // A slow request: the delay counts from when it finished, not when it started.
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(pace);
        let began = Instant::now();
        drop(gate.start(&ui(), &cancel).await.unwrap());
        assert!(began.elapsed() >= Duration::from_millis(140));
    }

    #[tokio::test]
    async fn no_delay_never_waits() {
        let gate = gate(0, 0);
        let cancel = AtomicBool::new(false);
        let began = Instant::now();
        for _ in 0..5 {
            drop(gate.start(&ui(), &cancel).await.unwrap());
        }
        assert!(began.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn stopping_the_fetch_ends_the_wait_promptly() {
        let gate = gate(MAX_MS, MAX_MS);
        let cancel = Arc::new(AtomicBool::new(false));
        drop(gate.start(&ui(), &cancel).await.unwrap());
        let flag = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            flag.store(true, Ordering::Relaxed);
        });
        let began = Instant::now();
        assert!(gate.start(&ui(), &cancel).await.is_err());
        assert!(began.elapsed() < Duration::from_millis(500));
        // A fetch that is already stopped does not start another page.
        cancel.store(true, Ordering::Relaxed);
        assert!(gate.start(&ui(), &cancel).await.is_err());
    }

    #[tokio::test]
    async fn a_new_setting_applies_to_the_next_wait() {
        let gate = gate(MAX_MS, 0);
        let cancel = AtomicBool::new(false);
        drop(gate.start(&ui(), &cancel).await.unwrap());
        gate.configure(0, 0);
        let began = Instant::now();
        drop(gate.start(&ui(), &cancel).await.unwrap());
        assert!(began.elapsed() < Duration::from_millis(100));
    }
}
