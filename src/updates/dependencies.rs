use crate::AppWindow;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub fn register(app: &AppWindow) {
    let ui = app.as_weak();
    app.on_apply_proxy(move |value| {
        let Some(app) = ui.upgrade() else {
            return false;
        };
        if app.get_is_busy()
            || app.get_history_busy()
            || app.get_tool_update_busy()
            || app.get_xtractor_busy()
            || app.get_ffmpeg_busy()
        {
            return false;
        }
        if let Err(error) = crate::network::proxy::configure(&value) {
            app.set_proxy_error(error.into());
            return false;
        }
        app.set_proxy_url(value.trim().into());
        app.set_proxy_error("".into());
        app.invoke_settings_changed();
        crate::app::notifications::show_toast(&app, "Proxy settings saved.", "info");
        true
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    app.on_cancel_tool_update(move || flag.store(true, Ordering::Relaxed));
    let ui = app.as_weak();
    app.on_update_tool(move |name| {
        let Some(app) = ui.upgrade() else { return };
        if !matches!(name.as_str(), "FFmpeg" | "ExifTool")
            || app.get_is_busy()
            || app.get_history_busy()
            || app.get_tool_update_busy()
            || app.get_ffmpeg_busy()
            || app.get_xtractor_busy()
        {
            return;
        }
        cancel.store(false, Ordering::Relaxed);
        app.set_tool_update_busy(true);
        app.set_tool_update_progress(0.0);
        app.set_tool_update_detail("".into());
        app.set_tool_update_error("".into());
        app.set_tool_update_status(format!("Preparing {name}…"));
        let ui = ui.clone();
        let cancel = cancel.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            let result = if name == "FFmpeg" {
                crate::updates::ffmpeg::reinstall(ui.clone(), cancel.clone()).await
            } else {
                crate::download::metadata::reinstall(ui.clone(), cancel.clone()).await
            };
            let _ = ui.upgrade_in_event_loop(move |app| {
                app.set_tool_update_busy(false);
                app.set_tool_update_detail("".into());
                match result {
                    Ok(_) => {
                        app.set_tool_update_progress(100.0);
                        app.set_tool_update_status(format!("{name} installed in portable data."));
                        if name == "FFmpeg" {
                            app.set_ffmpeg_ready(true);
                        }
                        crate::app::notifications::show_toast(
                            &app,
                            format!("{name} update/reinstall completed."),
                            "info",
                        );
                        app.invoke_check_all_updates();
                    }
                    Err(error) => {
                        app.set_tool_update_status(if cancel.load(Ordering::Relaxed) {
                            "Cancelled".into()
                        } else {
                            "Update failed".into()
                        });
                        app.set_tool_update_error(error.clone());
                        crate::platform::logging::error(format!("{name} setup failed: {error}"));
                    }
                }
            });
        });
    });
}
