use super::*;

fn setup_fixture() -> AppWindow {
    let app = fixture();
    app.set_xtractor_setup_required(true);
    app
}

#[test]
fn required_setup_ignores_escape_and_backdrop_and_blocks_navigation() {
    for theme in ["dark", "light"] {
        let app = setup_fixture();
        app.set_theme_mode(theme.into());
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        for _ in 0..3 {
            h.key_press(egui::Key::Escape);
            h.run();
            assert!(h.query_by_label("Xtractor setup dialog").is_some());
        }
        let backdrop = egui::pos2(800.0, 500.0);
        h.hover_at(backdrop);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: backdrop,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run();
        assert!(h.query_by_label("Xtractor setup dialog").is_some());
        let settings = h.get_by_label("Settings");
        assert!(settings.accesskit_node().is_disabled());
        settings.click();
        h.run();
        assert_eq!(app.get_selected_tab(), 0);
        assert!(h.query_by_label("Search X/Twitter media URL").is_none());
        assert!(h.query_by_label("Close").is_none());
        let button = h
            .get_by_role_and_label(egui::accesskit::Role::Button, "Download")
            .rect();
        assert_eq!(button.size(), Vec2::new(160.0, 32.0));
        assert_eq!(
            button.right(),
            h.get_by_label("Xtractor setup dialog").rect().right()
        );
    }
}

#[test]
fn setup_download_disables_repeat_clicks_retries_and_unlocks_only_after_installation() {
    let app = setup_fixture();
    let calls = Rc::new(std::cell::Cell::new(0));
    app.on_update_xtractor({
        let calls = calls.clone();
        let weak = app.as_weak();
        move || {
            calls.set(calls.get() + 1);
            let app = weak.upgrade().unwrap();
            app.set_xtractor_busy(true);
            app.set_xtractor_status("Downloading xtractor…".into());
            app.set_xtractor_progress(35.0);
            app.set_xtractor_detail("2.0 MB / 5.7 MB".into());
        }
    });
    let mut h = harness(app.clone());
    h.run();
    assert!(h.ctx.memory(|memory| memory.focused().is_none()));
    h.key_press(egui::Key::Tab);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(calls.get(), 1);
    let busy = h.get_by_role_and_label(egui::accesskit::Role::Button, "Downloading…");
    assert!(busy.accesskit_node().is_disabled());
    busy.click();
    h.run_steps(3);
    assert_eq!(calls.get(), 1);
    assert!(h.query_by_label("Xtractor setup dialog").is_some());
    app.set_xtractor_busy(false);
    app.set_xtractor_status("Could not install xtractor. Please try again.".into());
    app.set_xtractor_detail("Could not connect to GitHub. ".repeat(20));
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Try again")
        .click();
    h.run_steps(4);
    assert_eq!(calls.get(), 2);
    assert!(h.query_by_label("Search X/Twitter media URL").is_none());
    app.set_xtractor_busy(false);
    app.set_xtractor_installed(true);
    app.set_xtractor_setup_required(false);
    h.run();
    assert!(h.query_by_label("Xtractor setup dialog").is_none());
    assert!(h.query_by_label("Search X/Twitter media URL").is_some());
    assert!(!h.get_by_label("Settings").accesskit_node().is_disabled());
}

#[test]
#[ignore = "Renders mandatory Xtractor setup, progress and retry in both themes"]
fn render_xtractor_setup_review() {
    let directory = std::path::Path::new("target/xtractor-setup-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        let app = setup_fixture();
        app.set_theme_mode(theme.into());
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        for state in ["ready", "downloading", "error"] {
            app.set_xtractor_busy(state == "downloading");
            app.set_xtractor_status(
                match state {
                    "downloading" => "Downloading xtractor…",
                    "error" => "Could not install xtractor. Please try again.",
                    _ => "",
                }
                .into(),
            );
            app.set_xtractor_detail(
                match state {
                    "downloading" => "2.0 MB / 5.7 MB",
                    "error" => "Could not connect to GitHub. Check your connection and try again.",
                    _ => "",
                }
                .into(),
            );
            app.set_xtractor_progress(35.0);
            h.remove_cursor();
            h.run_steps(5);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-{state}.png")))
                .unwrap();
        }
    }
}
