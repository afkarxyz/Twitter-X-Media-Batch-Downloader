use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};
static FETCH: AtomicU32 = AtomicU32::new(330);
static HTTP: AtomicU32 = AtomicU32::new(120);
pub fn configure(fetch: u32, http: u32) {
    FETCH.store(fetch.clamp(5, 3600), Ordering::Relaxed);
    HTTP.store(http.clamp(5, 3600), Ordering::Relaxed);
}
pub fn fetch() -> Duration {
    Duration::from_secs(FETCH.load(Ordering::Relaxed) as u64)
}
pub fn http() -> Duration {
    Duration::from_secs(HTTP.load(Ordering::Relaxed) as u64)
}
pub fn connect() -> Duration {
    http().min(Duration::from_secs(30))
}
pub fn parse(fetch: &str, http: &str) -> Result<(u32, u32), &'static str> {
    let parse = |text: &str| {
        text.trim()
            .parse::<u32>()
            .ok()
            .filter(|value| (5..=3600).contains(value))
    };
    match (parse(fetch), parse(http)) {
        (Some(fetch), Some(http)) => Ok((fetch, http)),
        _ => Err("Enter timeout values from 5 to 3600 seconds."),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_timeouts_do_not_turn_off_network_limits() {
        assert_eq!(parse("330", "120"), Ok((330, 120)));
        for value in ["0", "-1", "3601", "abc"] {
            assert!(parse(value, "120").is_err());
        }
    }
}
