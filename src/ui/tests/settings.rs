use super::*;

fn preferences() -> AppWindow {
    let app = fixture();
    app.set_selected_tab(2);
    app.set_theme_mode("dark".into());
    app.set_app_version("6.0.0".into());
    app.set_fetch_timeout_secs(180);
    app.set_http_timeout_secs(120);
    app.set_concurrent_downloads_index(2);
    app.set_filename_format("{date}_{handle}_{tweet_id}_{index}".into());
    app.set_folder_name_format("{handle}".into());
    app.global::<RendererSettings>().set_gpu_supported(true);
    app.on_settings_changed({
        let app = app.as_weak();
        move || {
            let app = app.upgrade().unwrap();
            app.set_filename_preview(crate::download::engine::filename_preview(
                &app.get_filename_format(),
            ));
            app.set_folder_name_preview(crate::download::engine::folder_name_preview(
                &app.get_folder_name_format(),
            ));
        }
    });
    app
}

#[test]
fn settings_keep_legacy_card_bounds_and_reset_scroll_between_categories() {
    let app = preferences();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for (label, y, height) in [
        ("Restore preferences settings card", 68.0, 125.0),
        ("Theme settings card", 205.0, 801.0),
    ] {
        let rect = h.get_by_label(label).rect();
        assert_eq!(
            rect,
            egui::Rect::from_min_size(egui::pos2(298.0, y), Vec2::new(714.0, height))
        );
    }
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    let thumb = painted_scroll_thumb(&h, bar).0;
    h.drag_at(thumb.center());
    h.run_steps(1);
    h.hover_at(thumb.center() + Vec2::new(0.0, 200.0));
    h.run_steps(3);
    h.drop_at(thumb.center() + Vec2::new(0.0, 200.0));
    h.run_steps(3);
    let scrolled =
        painted_scroll_thumb(&h, h.get_by_role(egui::accesskit::Role::ScrollBar).rect()).0;
    app.set_selected_tab(1);
    h.run();
    app.set_selected_tab(2);
    h.run();
    let restored =
        painted_scroll_thumb(&h, h.get_by_role(egui::accesskit::Role::ScrollBar).rect()).0;
    assert_eq!(
        restored, scrolled,
        "Returning to Settings reset its scroll position"
    );
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Downloads")
        .click();
    h.run();
    let first = h.get_by_label("Extraction defaults settings card").rect();
    assert_eq!(
        first,
        egui::Rect::from_min_size(egui::pos2(298.0, 68.0), Vec2::new(714.0, 298.0))
    );
    for label in ["Image", "Video", "GIF", "Text"] {
        assert_eq!(h.get_by_label(label).rect().top(), 146.0);
        assert_eq!(h.get_by_label(label).rect().height(), 32.0);
    }
    assert_eq!(h.get_by_label("Retweets").rect().height(), 32.0);
    h.set_size(Vec2::new(1920.0, 1080.0));
    h.run();
    let card = h.get_by_label("Extraction defaults settings card").rect();
    assert_eq!(card.left(), 693.0);
    assert_eq!(card.width(), 820.0);
}

#[test]
fn extraction_text_option_uses_the_saved_preference_and_allows_text_only_requests() {
    let app = preferences();
    app.set_settings_category(2);
    app.set_media_filters_custom(true);
    app.set_include_photos(true);
    app.set_include_videos(false);
    app.set_include_gifs(false);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(h.query_by_label("Include text-only posts").is_none());
    assert!(!app.get_text_tweets());
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Text")
        .click();
    h.run();
    assert!(app.get_text_tweets());
    assert_eq!(
        crate::fetch::request::media_flags(&app),
        (true, false, false)
    );
    assert!(crate::app::settings::settings_from_app(&app).text_tweets);
    h.get_by_label("Image").click();
    h.run();
    let profile =
        crate::fetch::request::profile_from_app(&app, "example", &Default::default()).unwrap();
    assert_eq!(profile.media_type, "none");
    assert!(profile.text_tweets);
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Text")
        .click();
    h.run();
    assert!(!app.get_text_tweets());
    assert!(crate::fetch::request::profile_from_app(&app, "example", &Default::default()).is_err());
    app.set_is_busy(true);
    h.run();
    let text = h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Text");
    assert!(text.accesskit_node().is_disabled());
    text.click();
    h.run();
    assert!(!app.get_text_tweets());
}

#[test]
fn settings_theme_choices_save_the_mode_and_accent_and_keep_hover_feedback() {
    let app = preferences();
    let saved = Rc::new(RefCell::new(Vec::new()));
    app.on_save_theme_mode({
        let saved = saved.clone();
        move |value| saved.borrow_mut().push(value)
    });
    app.on_save_theme_preset({
        let saved = saved.clone();
        move |value| saved.borrow_mut().push(value)
    });
    let mut h = harness(app.clone());
    h.run();
    let rect = h
        .get_by_role_and_label(egui::accesskit::Role::RadioButton, "GitHub")
        .rect();
    h.hover_at(rect.center());
    h.run_steps(4);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    h.get_by_role_and_label(egui::accesskit::Role::RadioButton, "GitHub")
        .click();
    h.run();
    assert_eq!(app.global::<AppTheme>().get_preset(), "github");
    h.get_by_role_and_label(egui::accesskit::Role::RadioButton, "Light")
        .click();
    h.run();
    assert_eq!(app.get_theme_mode(), "light");
    assert_eq!(*saved.borrow(), ["github", "light"]);
}

#[test]
fn settings_media_and_long_concurrency_menu_preserve_filter_semantics() {
    let app = preferences();
    app.set_settings_category(2);
    app.set_media_type_index(2);
    app.set_no_videos(true);
    let ffmpeg = Rc::new(std::cell::Cell::new(0));
    app.on_ensure_ffmpeg({
        let calls = ffmpeg.clone();
        move || calls.set(calls.get() + 1)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 1000.0));
    h.run();
    assert!(!app.get_media_filters_custom());
    h.get_by_label("Image").click();
    h.run();
    assert!(app.get_media_filters_custom());
    assert!(!app.get_no_videos());
    assert_eq!(
        (
            app.get_include_photos(),
            app.get_include_videos(),
            app.get_include_gifs()
        ),
        (true, false, false)
    );
    h.get_by_label("Retweets").click();
    h.run();
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_retweets_index(), 1);
    h.get_by_label("Concurrent downloads").click();
    h.run();
    for _ in 0..48 {
        h.key_press(egui::Key::ArrowDown);
        h.run();
    }
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_concurrent_downloads_index(), 50);
    h.get_by_label("Auto Convert GIF").click();
    h.run();
    assert!(app.get_convert_gif());
    assert_eq!(ffmpeg.get(), 1);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Better")
        .click();
    h.run();
    assert_eq!(app.get_gif_quality_index(), 1);
    app.set_is_busy(true);
    h.run();
    assert!(h.get_by_label("Image").accesskit_node().is_disabled());
}

#[test]
fn token_switching_is_offered_with_saved_tokens_and_can_change_during_a_fetch() {
    let app = preferences();
    app.set_settings_category(1);
    let saved = Rc::new(std::cell::Cell::new(0));
    app.on_settings_changed({
        let saved = saved.clone();
        move || saved.set(saved.get() + 1)
    });
    let label = "Switch tokens when rate limited";
    let mut h = harness(app.clone());
    h.run();
    assert!(
        h.query_by_label(label).is_none(),
        "nothing to switch to without saved tokens"
    );
    app.set_auth_token_history(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::AuthTokenItem {
            index: 0,
            token: "masked-fixture-token".into(),
            ..Default::default()
        },
    ])));
    // A fetch is running and has just been rate limited.
    app.set_is_busy(true);
    app.set_is_extracting(true);
    h.run();
    assert!(!app.get_rotate_auth_tokens(), "off until chosen");
    assert!(!h.get_by_label(label).accesskit_node().is_disabled());
    h.get_by_label(label).click();
    h.run();
    assert!(app.get_rotate_auth_tokens());
    assert_eq!(saved.get(), 1);
    h.get_by_label(label).click();
    h.run();
    assert!(!app.get_rotate_auth_tokens());
    assert_eq!(saved.get(), 2);
}

#[test]
fn settings_cookies_save_clear_and_token_history_actions_use_backend_indices() {
    let app = preferences();
    app.set_settings_category(1);
    app.set_saved_auth_token(String::new());
    app.set_auth_token_history(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::AuthTokenItem {
            index: 3,
            token: "masked-fixture-token".into(),
            ..Default::default()
        },
    ])));
    let saved = Rc::new(RefCell::new(Vec::new()));
    let checked = Rc::new(RefCell::new(Vec::new()));
    let used = Rc::new(RefCell::new(Vec::new()));
    app.on_save_auth_token({
        let weak = app.as_weak();
        let saved = saved.clone();
        move || {
            let app = weak.upgrade().unwrap();
            saved.borrow_mut().push(app.get_auth_token());
            app.set_saved_auth_token(app.get_auth_token());
        }
    });
    app.on_check_auth_token({
        let calls = checked.clone();
        move |index| calls.borrow_mut().push(index)
    });
    app.on_use_auth_token_history({
        let calls = used.clone();
        move |index| calls.borrow_mut().push(index)
    });
    let mut h = harness(app.clone());
    h.run();
    assert!(h.get_by_label("Save").accesskit_node().is_disabled());
    let save = h.get_by_label("Save").rect();
    let input = h.get_by_label("X auth_token").rect();
    assert_eq!(save.height(), 32.0);
    assert!((save.center().y - input.center().y).abs() < 0.5);
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Rect(rect) if rect.rect.contains_rect(input) && rect.rect.height() == save.height() && rect.rect.width() > input.width())));
    h.get_by_label("X auth_token").click();
    h.run();
    h.get_by_label("X auth_token")
        .type_text("fixture-new-token");
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(app.get_saved_auth_token(), "fixture-new-token");
    h.remove_cursor();
    h.run();
    assert_eq!(
        fill_for_rect(&h, h.get_by_label("Clear").rect()),
        appearance::DANGER_FILL
    );
    h.get_by_label("Check active token").click();
    h.run();
    h.get_by_label("Check").click();
    h.run();
    assert_eq!(*checked.borrow(), [-1, 3]);
    h.get_by_label("Use").click();
    h.run();
    assert_eq!(*used.borrow(), [3]);
    h.get_by_label("Clear").click();
    h.run();
    assert_eq!(*saved.borrow(), ["fixture-new-token", ""]);
    assert!(
        h.get_by_label("Check active token")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn settings_drafts_survive_unrelated_updates_and_resets_resynchronize_values() {
    let app = preferences();
    let resets = Rc::new(RefCell::new(Vec::new()));
    app.on_reset_settings({
        let calls = resets.clone();
        let weak = app.as_weak();
        move |scope| {
            calls.borrow_mut().push(scope);
            weak.upgrade().unwrap().set_proxy_url(String::new());
            true
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1120.0, 2400.0));
    h.run();
    h.get_by_label("Proxy URL").click();
    h.run();
    h.get_by_label("Proxy URL").type_text("http://fixture:7890");
    h.run();
    app.set_download_speed_kib(1024);
    h.run();
    assert_eq!(h.state().proxy, "http://fixture:7890");
    assert_eq!(h.state().speed, "1024");
    app.set_proxy_url("http://changed:8080".into());
    h.run();
    assert_eq!(h.state().proxy, "http://changed:8080");
    h.get_by_label("Reset defaults").click();
    h.run();
    assert_eq!(*resets.borrow(), [0]);
    assert!(h.state().proxy.is_empty());
}

#[test]
fn settings_storage_refreshes_on_entry_and_path_changes_and_keeps_the_path_read_only() {
    let app = preferences();
    app.set_settings_category(3);
    app.set_output_dir("fixture/Downloads".into());
    let refresh = Rc::new(std::cell::Cell::new(0));
    let backups = Rc::new(std::cell::Cell::new(0));
    app.on_refresh_storage({
        let calls = refresh.clone();
        move || calls.set(calls.get() + 1)
    });
    app.on_backup_database({
        let calls = backups.clone();
        move || calls.set(calls.get() + 1)
    });
    let mut h = harness(app.clone());
    h.run();
    assert_eq!(refresh.get(), 1);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Download folder")
        .click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Download folder")
        .type_text("accidental-edit");
    h.run();
    assert_eq!(app.get_output_dir(), "fixture/Downloads");
    app.set_output_dir("fixture/Another".into());
    h.run();
    assert_eq!(refresh.get(), 2);
    h.get_by_label("Refresh").click();
    h.run();
    assert_eq!(refresh.get(), 3);
    h.get_by_label("Backup database").click();
    h.run();
    assert_eq!(backups.get(), 1);
}

#[test]
fn background_storage_refreshes_keep_the_previous_figures_and_layout() {
    let app = preferences();
    app.set_settings_category(3);
    app.set_output_dir("fixture/Downloads".into());
    app.on_refresh_storage(|| {});
    app.set_storage_used("1.2 GB".into());
    app.set_storage_free("80.0 GB".into());
    app.set_storage_files(42);
    let texts = |h: &Harness<'_, Desktop>| {
        h.output()
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let mut h = harness(app.clone());
    app.set_storage_busy(true);
    h.run();
    assert!(
        texts(&h).iter().any(|text| text == "Calculating…"),
        "The first scan has nothing to show yet"
    );
    app.set_storage_ready(true);
    h.run();
    let metric = h.get_by_label("Download folder usage metric").rect();
    let refreshing = texts(&h);
    assert!(refreshing.iter().any(|text| text == "1.2 GB"));
    assert!(!refreshing.iter().any(|text| text == "Calculating…"));
    assert!(!h.get_by_label("Refresh").accesskit_node().is_disabled());
    app.set_storage_busy(false);
    h.run();
    assert_eq!(h.get_by_label("Download folder usage metric").rect(), metric);
}

#[test]
fn settings_quality_groups_and_limit_fields_act_independently() {
    let app = preferences();
    app.set_settings_category(2);
    app.set_image_size_index(2);
    app.set_avatar_size_index(4);
    app.set_banner_size_index(3);
    let limits = Rc::new(RefCell::new(Vec::new()));
    app.on_apply_download_limits({
        let calls = limits.clone();
        move |speed, delay, jitter| {
            calls.borrow_mut().push((speed, delay, jitter));
            true
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 2600.0));
    h.run();
    assert_eq!(
        h.get_by_label("Media quality settings card")
            .rect()
            .height(),
        550.0
    );
    assert_eq!(
        h.get_by_label("Filename format settings card").rect().top(),
        940.0
    );
    h.get_by_label("Image size options")
        .get_by_label("Original")
        .click();
    h.run();
    assert_eq!(
        (
            app.get_image_size_index(),
            app.get_avatar_size_index(),
            app.get_banner_size_index()
        ),
        (0, 4, 3)
    );
    h.get_by_label("Avatar size options")
        .get_by_label("400px")
        .click();
    h.run();
    assert_eq!(app.get_avatar_size_index(), 1);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Random extra delay (ms)")
        .click();
    h.run();
    h.key_press(egui::Key::End);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Random extra delay (ms)")
        .type_text("25");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(*limits.borrow(), [("0".into(), "0".into(), "025".into())]);
    h.get_by_label("{media_id}").click();
    h.run();
    assert!(app.get_filename_format().ends_with("{media_id}"));
    assert!(!app.get_filename_preview().is_empty());
    assert!(
        !h.output()
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text().contains("use of widget ID")))
    );
}

#[test]
fn fetch_pacing_applies_during_a_fetch_and_rejects_bad_values() {
    let _shared = crate::fetch::pacing::TEST_LOCK.blocking_lock();
    let app = preferences();
    app.set_settings_category(2);
    // A fetch is running: the delay can still be raised once X starts rate limiting.
    app.set_is_busy(true);
    app.set_is_extracting(true);
    app.on_apply_fetch_pacing({
        let app = app.as_weak();
        move |delay, jitter| crate::fetch::pacing::apply(&app.upgrade().unwrap(), &delay, &jitter)
    });
    let saved = Rc::new(std::cell::Cell::new(0));
    app.on_settings_changed({
        let saved = saved.clone();
        move || saved.set(saved.get() + 1)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 3200.0));
    h.run();
    assert!(h.query_by_label("Fetch pacing settings card").is_some());
    let delay = "Delay between fetch pages (ms)";
    let jitter = "Random extra fetch delay (ms)";
    for (label, typed) in [(delay, "2000"), (jitter, "1000")] {
        let input = h.get_by_role_and_label(egui::accesskit::Role::TextInput, label);
        assert!(
            !input.accesskit_node().is_disabled(),
            "{label} is editable during a fetch"
        );
        input.click();
        h.run();
        h.key_press(egui::Key::End);
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, label)
            .type_text(typed);
        h.run();
    }
    h.key_press(egui::Key::Enter);
    h.run();
    // Typed after the shown 0: "02000" and "01000" are the same numbers.
    assert_eq!(
        (app.get_fetch_delay_ms(), app.get_fetch_jitter_ms()),
        (2000, 1000)
    );
    assert!(app.get_fetch_pacing_error().is_empty());
    assert_eq!(saved.get(), 1);

    h.get_by_role_and_label(egui::accesskit::Role::TextInput, delay)
        .click();
    h.run();
    h.key_press(egui::Key::End);
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, delay)
        .type_text("x");
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(
        app.get_fetch_pacing_error(),
        "Enter whole numbers from 0 to 3600000 ms."
    );
    assert!(
        h.query_by_label("Enter whole numbers from 0 to 3600000 ms.")
            .is_some()
    );
    assert_eq!(
        (app.get_fetch_delay_ms(), app.get_fetch_jitter_ms()),
        (2000, 1000),
        "a rejected value keeps the saved delay"
    );
    assert_eq!(saved.get(), 1);
    crate::fetch::pacing::configure(0, 0);
}

#[test]
fn settings_dialogs_handle_busy_escape_and_log_actions() {
    let app = preferences();
    let canceled = Rc::new(std::cell::Cell::new(0));
    let copied = Rc::new(std::cell::Cell::new(0));
    app.on_cancel_operation({
        let calls = canceled.clone();
        move || calls.set(calls.get() + 1)
    });
    app.on_copy_log({
        let calls = copied.clone();
        move || calls.set(calls.get() + 1)
    });
    app.set_integrity_open(true);
    app.set_integrity_busy(true);
    let mut h = harness(app.clone());
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(canceled.get(), 1);
    assert!(app.get_integrity_open());
    app.set_integrity_busy(false);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_integrity_open());
    app.set_log_viewer_open(true);
    app.set_log_text("fixture log".into());
    h.run();
    h.get_by_label("Copy").click();
    h.run();
    assert_eq!(copied.get(), 1);
    h.get_by_label("Auto-scroll: On").click();
    h.run();
    assert!(!app.get_logs_follow());
    h.get_by_label("Live: On").click();
    h.run();
    assert!(!app.get_logs_live());
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_log_viewer_open());
    let renderer = app.global::<RendererSettings>();
    renderer.set_prompt_open(true);
    renderer.set_restarting(true);
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(renderer.get_prompt_open());
    renderer.set_restarting(false);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!renderer.get_prompt_open());
}

#[test]
#[ignore = "Renders Settings categories, controls, and related dialogs"]
fn render_settings_review() {
    let app = preferences();
    let mut h = harness(app.clone());
    let dir = std::path::Path::new("target/settings-review");
    std::fs::create_dir_all(dir).unwrap();
    for (theme, width, height) in [
        ("dark", 1024.0, 600.0),
        ("light", 1440.0, 900.0),
        ("dark", 1920.0, 1080.0),
    ] {
        app.set_theme_mode(theme.into());
        h.set_size(Vec2::new(width, height));
        for category in 0..5 {
            app.set_settings_category(category);
            h.remove_cursor();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(dir.join(format!("{theme}-{}-category-{category}.png", width as u32)))
                .unwrap();
        }
        app.set_settings_category(2);
        app.set_text_tweets(true);
        h.run_steps(4);
        let _ = h.render().unwrap();
        h.run_steps(4);
        for label in ["Image", "Video", "GIF", "Text"] {
            let button = h.get_by_label(label).rect();
            let text_color = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        (text.galley.text() == label && button.contains(text.pos))
                            .then_some(text.galley.job.sections[0].format.color)
                    } else {
                        None
                    }
                })
                .unwrap();
            let icon_color = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Rect(rect) = &shape.shape {
                        (rect.brush.is_some() && button.contains_rect(rect.rect))
                            .then_some(rect.fill)
                    } else {
                        None
                    }
                })
                .expect("Each media type must paint its icon");
            assert_eq!(
                icon_color, text_color,
                "{theme}: {label} icon and text must have the same color"
            );
        }
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-{}-media-types-selected.png", width as u32)))
            .unwrap();
        app.set_text_tweets(false);
    }
    h.set_size(Vec2::new(1024.0, 2600.0));
    for category in [0, 2, 3] {
        app.set_settings_category(category);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("full-category-{category}.png")))
            .unwrap();
    }
    h.set_size(Vec2::new(1024.0, 600.0));
    app.set_settings_category(0);
    h.run_steps(4);
    for index in 0..4 {
        match index {
            0 => app.global::<RendererSettings>().set_prompt_open(true),
            1 => app.set_backup_restore_open(true),
            2 => app.set_integrity_open(true),
            _ => {
                app.set_log_sessions(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
                    "Session 2026-10-14".into(),
                ])));
                app.set_log_session_index(0);
                app.set_log_text(
                    "[INFO] Settings review fixture\n[WARN] Example diagnostic entry".into(),
                );
                app.set_log_viewer_open(true);
            }
        }
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("dialog-{index}.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
    }
}
