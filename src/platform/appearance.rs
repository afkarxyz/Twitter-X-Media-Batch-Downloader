//! The desktop's light or dark preference where the window system does not report it.
//!
//! winit reports the system theme on Windows and macOS only. On Linux the XDG desktop
//! portal's `org.freedesktop.appearance` `color-scheme` setting says it, on GNOME, KDE
//! and other desktops alike: 1 prefers dark, 2 prefers light, 0 has no preference.

use std::sync::atomic::{AtomicU8, Ordering};

const UNKNOWN: u8 = 0;
const DARK: u8 = 1;
const LIGHT: u8 = 2;

static SCHEME: AtomicU8 = AtomicU8::new(UNKNOWN);

/// `Some(true)` when the desktop prefers dark, `Some(false)` for light, `None` when
/// the desktop has not said (no portal, or another platform).
pub fn prefers_dark() -> Option<bool> {
    match SCHEME.load(Ordering::Relaxed) {
        DARK => Some(true),
        LIGHT => Some(false),
        _ => None,
    }
}

/// Reads the preference and keeps it current; `changed` runs on a worker after each
/// change so the interface can repaint.
pub fn watch(changed: impl Fn() + Send + 'static) {
    #[cfg(target_os = "linux")]
    {
        let _ = std::thread::Builder::new()
            .name("appearance".into())
            .spawn(move || portal::follow(changed));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = changed;
    }
}

/// The stored state for a portal `color-scheme` value.
#[cfg(any(target_os = "linux", test))]
fn scheme(value: u32) -> u8 {
    match value {
        1 => DARK,
        // "No preference" is the desktop's light default.
        _ => LIGHT,
    }
}

#[cfg(target_os = "linux")]
mod portal {
    use super::{SCHEME, scheme};
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use zbus::blocking::{Proxy, connection};
    use zbus::zvariant::{OwnedValue, Value};

    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    pub(super) fn follow(changed: impl Fn()) {
        let Ok(connection) = connection::Builder::session()
            .and_then(|builder| builder.method_timeout(Duration::from_secs(2)).build())
        else {
            return;
        };
        let Ok(proxy) = Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        ) else {
            return;
        };
        // Subscribe before the first read so a change in between is not missed.
        let signals = proxy.receive_signal("SettingChanged");
        // `ReadOne` (portal version 2), or the older `Read`, which wraps the value twice.
        if let Some(value) = proxy
            .call::<_, _, OwnedValue>("ReadOne", &(NAMESPACE, KEY))
            .or_else(|_| proxy.call::<_, _, OwnedValue>("Read", &(NAMESPACE, KEY)))
            .ok()
            .and_then(|value| number(&value))
        {
            SCHEME.store(scheme(value), Ordering::Relaxed);
            changed();
        }
        let Ok(signals) = signals else {
            return;
        };
        for message in signals {
            let Ok((namespace, key, value)) =
                message.body().deserialize::<(String, String, OwnedValue)>()
            else {
                continue;
            };
            if namespace == NAMESPACE
                && key == KEY
                && let Some(value) = number(&value)
            {
                SCHEME.store(scheme(value), Ordering::Relaxed);
                changed();
            }
        }
    }

    pub(super) fn number(value: &Value<'_>) -> Option<u32> {
        match value {
            Value::U32(number) => Some(*number),
            Value::Value(inner) => number(inner),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_values_map_to_the_desktop_preference() {
        assert_eq!(scheme(1), DARK);
        assert_eq!(scheme(2), LIGHT);
        assert_eq!(scheme(0), LIGHT, "no preference is the light default");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nested_portal_variants_unwrap_to_the_number() {
        use zbus::zvariant::Value;
        let nested = Value::Value(Box::new(Value::Value(Box::new(Value::U32(1)))));
        assert_eq!(portal::number(&nested), Some(1));
        assert_eq!(portal::number(&Value::Str("dark".into())), None);
    }
}
