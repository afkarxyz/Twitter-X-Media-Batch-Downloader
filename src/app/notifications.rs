use crate::AppWindow;
use crate::platform::logging as session_log;

pub(crate) fn show_toast(app: &AppWindow, message: impl Into<String>, kind: &str) {
    let library = app.get_library().feedback_active;
    toast(app, message.into(), kind, library);
}

pub(crate) fn show_library_toast(app: &AppWindow, message: impl Into<String>, kind: &str) {
    toast(app, message.into(), kind, true);
}

fn toast(app: &AppWindow, message: String, kind: &str, library: bool) {
    let generation = app.get_toast_generation().wrapping_add(1);
    if kind == "error" {
        if library {
            app.update_library(|data| data.error = message.clone());
        } else {
            app.set_last_error(message.clone());
        }
    }
    session_log::write(
        match kind {
            "error" => session_log::Level::Error,
            "warning" | "warn" => session_log::Level::Warn,
            _ => session_log::Level::Info,
        },
        format!("Toast: {message}"),
    );
    let message = if message.chars().count() > 360 {
        format!("{}…", message.chars().take(359).collect::<String>())
    } else {
        message
    };
    app.set_toast_page(if library { crate::library::TAB } else { -1 });
    app.set_toast_message(message);
    app.set_toast_kind(kind.into());
    app.set_toast_generation(generation);
    app.set_toast_visible(true);

    let ui = app.as_weak();
    let lifetime = if kind == "error" {
        std::time::Duration::from_secs(6)
    } else {
        std::time::Duration::from_secs(4)
    };
    crate::ui::runtime::Timer::single_shot(lifetime, move || {
        if let Some(app) = ui.upgrade()
            && app.get_toast_generation() == generation
        {
            hide_toast(&app, generation);
        }
    });
}

pub(crate) fn dismiss_toast(app: &AppWindow) {
    let generation = app.get_toast_generation().wrapping_add(1);
    app.set_toast_generation(generation);
    hide_toast(app, generation);
}

pub(crate) fn hide_toast(app: &AppWindow, generation: i32) {
    app.set_toast_visible(false);
    let ui = app.as_weak();
    crate::ui::runtime::Timer::single_shot(std::time::Duration::from_millis(200), move || {
        if let Some(app) = ui.upgrade()
            && app.get_toast_generation() == generation
            && !app.get_toast_visible()
        {
            app.set_toast_message("".into());
        }
    });
}
