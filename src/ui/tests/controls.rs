use super::*;

#[test]
fn forms_keep_shared_sizes_across_pages_and_dialogs() {
    let app = fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1120.0, 2600.0));
    let check = |h: &Harness<'_, Desktop>| {
        for picker in h.query_all_by_role(egui::accesskit::Role::ComboBox) {
            assert_eq!(picker.rect().height(), 32.0);
        }
        for input in h.query_all_by_role(egui::accesskit::Role::TextInput) {
            let input = input.rect();
            if input.height() <= 32.0 {
                let large_search = ["Search X/Twitter media URL", "Search accounts"]
                    .iter()
                    .any(|label| {
                        h.query_all_by_role_and_label(egui::accesskit::Role::TextInput, label)
                            .any(|node| node.rect() == input)
                    });
                let height = if large_search { 40.0 } else { 32.0 };
                assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(frame) if frame.rect.contains_rect(input) && frame.rect.height() == height)),
                    "Single-line input has no standard frame: {input:?}");
            }
        }
        for label in [
            "Download all",
            "Export JSON",
            "Open folder",
            "Select filter",
            "Save",
            "Clear",
            "Apply",
            "Reset",
            "Reset defaults",
            "Browse",
            "Refresh",
            "Cancel",
            "View",
            "Open post",
            "Download",
        ] {
            for button in h.query_all_by_label(label) {
                assert_eq!(button.rect().height(), 32.0, "{label}");
            }
        }
    };
    for tab in 0..4 {
        app.set_selected_tab(tab);
        if tab == 2 {
            for category in 0..5 {
                app.set_settings_category(category);
                h.run();
                check(&h);
            }
        } else {
            h.run();
            check(&h);
        }
    }
    app.set_selected_tab(0);
    for dialog in 0..5 {
        match dialog {
            0 => {
                app.set_date_filter_enabled(true);
                app.set_fetch_options_open(true);
            }
            1 => app.set_account_search_open(true),
            2 => app.set_account_labels_open(true),
            3 => app.set_selected_media_index(42),
            _ => app.set_xtractor_setup_required(true),
        }
        h.run();
        check(&h);
        app.set_fetch_options_open(false);
        app.set_account_search_open(false);
        app.set_account_labels_open(false);
        app.set_selected_media_index(-1);
        app.set_xtractor_setup_required(false);
        h.run();
    }
}

#[test]
#[ignore = "Renders text context-menu icons, hover and disabled states in both themes"]
fn render_text_context_menus() {
    let directory = std::path::Path::new("target/text-context-menu-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        let app = fixture();
        app.set_theme_mode(theme.into());
        app.set_media_items(Default::default());
        app.set_source_url("example_user".into());
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        h.get_by_label("Search X/Twitter media URL")
            .click_secondary();
        h.run();
        h.remove_cursor();
        h.run();
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-no-selection.png")))
            .unwrap();
        h.get_by_label("Select all").click();
        h.run();
        h.get_by_label("Search X/Twitter media URL")
            .click_secondary();
        h.run();
        h.hover_at(h.get_by_label("Copy").rect().center());
        h.run();
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-selection-hover.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run();
        app.set_log_viewer_open(true);
        app.set_log_text("Context menu review".into());
        h.run();
        h.get_by_label("Session log contents").click_secondary();
        h.run();
        h.remove_cursor();
        h.run();
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-readonly.png")))
            .unwrap();
    }
}

#[test]
fn text_menus_cut_unicode_update_the_model_and_preserve_undo() {
    let app = fixture();
    app.set_media_items(Default::default());
    app.set_source_url("café 日本".into());
    let mut h = harness(app.clone());
    h.run();
    h.get_by_label("Search X/Twitter media URL")
        .click_secondary();
    h.run();
    h.get_by_label("Select all").click();
    h.run();
    h.get_by_label("Search X/Twitter media URL")
        .click_secondary();
    h.run();
    assert!(!h.get_by_label("Cut").accesskit_node().is_disabled());
    h.get_by_label("Cut").click();
    h.run();
    assert!(app.get_source_url().is_empty());
    h.key_press_modifiers(
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
        egui::Key::Z,
    );
    h.run();
    assert_eq!(app.get_source_url(), "café 日本");
    app.set_selected_tab(2);
    app.set_settings_category(1);
    app.set_auth_token("fixture-token".into());
    h.run();
    h.get_by_label("X auth_token").click_secondary();
    h.run();
    h.get_by_label("Select all").click();
    h.run();
    h.get_by_label("X auth_token").click_secondary();
    h.run();
    h.get_by_label("Cut").click();
    h.run();
    assert!(app.get_auth_token().is_empty());
    h.get_by_label("X auth_token").click_secondary();
    h.run();
    h.get_by_label("Paste").click();
    h.run();
    h.event(egui::Event::Paste("fixture-pasted-token".into()));
    h.run();
    assert_eq!(app.get_auth_token(), "fixture-pasted-token");
}

#[test]
fn readonly_log_context_menu_offers_copy_without_edit_actions() {
    let app = fixture();
    app.set_log_viewer_open(true);
    app.set_log_text("Fixture log café 日本".into());
    let mut h = harness(app.clone());
    h.run();
    h.get_by_label("Session log contents").click_secondary();
    h.run();
    assert!(h.query_by_label("Cut").is_none());
    assert!(h.query_by_label("Paste").is_none());
    h.get_by_label("Select all").click();
    h.run();
    h.get_by_label("Session log contents").click_secondary();
    h.run();
    assert!(
        h.get_all_by_role_and_label(egui::accesskit::Role::Button, "Copy")
            .filter(|node| !node.accesskit_node().is_disabled())
            .count()
            >= 1
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(app.get_log_text(), "Fixture log café 日本");
}

#[test]
fn dropdowns_in_home_library_and_fetch_options_use_the_same_keyboard_selection() {
    let app = original_home_fixture();
    app.update_library(|data| data.groups = vec!["Favorites".into()]);
    app.on_choose_fetch_source({
        let weak = app.as_weak();
        move |index| {
            weak.upgrade().unwrap().set_fetch_source_index(index);
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Result order").click();
    h.run();
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_results_sort(), 1);
    app.set_selected_tab(crate::library::TAB);
    h.run();
    h.get_by_label("Filter group").click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Favorites")
        .click();
    h.run();
    assert_eq!(app.get_library().group_filter, "Favorites");
    app.set_selected_tab(0);
    app.set_fetch_options_open(true);
    h.run();
    h.get_by_label("Fetch source").click();
    h.run();
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_fetch_source_index(), 1);
    app.set_is_busy(true);
    h.run_steps(4);
    assert!(
        h.get_by_label("Fetch source")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn dropdown_long_labels_stay_inside_the_option_and_remain_selectable() {
    let app = fixture();
    app.set_selected_tab(crate::library::TAB);
    let name = "A library group with a name that is much longer than its dropdown width";
    app.update_library(|data| data.groups = vec![name.into()]);
    let mut h = harness(app.clone());
    h.run();
    h.get_by_label("Filter group").click();
    h.run();
    let option = h
        .get_by_role_and_label(egui::accesskit::Role::Button, name)
        .rect();
    let text = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == name => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.galley.rows.len(), 1);
    assert!(text.pos.x + text.galley.size().x <= option.right() - 24.0);
    h.get_by_role_and_label(egui::accesskit::Role::Button, name)
        .click();
    h.run();
    assert_eq!(app.get_library().group_filter, name);
}

#[test]
fn preview_buttons_obey_saved_busy_and_url_rules() {
    let app = fixture();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        MediaItem {
            media_index: 0,
            author_name: "Fixture".into(),
            author_handle: "fixture".into(),
            tweet_text: "Post fixture".into(),
            media_type: "photo".into(),
            file_extension: "JPG".into(),
            status: "Saved".into(),
            url: "https://example.invalid/original.jpg".into(),
            preview_url: "https://example.invalid/thumb.jpg".into(),
            ..Default::default()
        },
    ])));
    let links = Rc::new(RefCell::new(Vec::new()));
    app.on_open_link({
        let calls = links.clone();
        move |url| calls.borrow_mut().push(url)
    });
    app.set_selected_media_index(0);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Saved")
            .accesskit_node()
            .is_disabled()
    );
    assert!(h.get_by_label("Open post").accesskit_node().is_disabled());
    h.get_by_label("View").click();
    h.run();
    assert_eq!(*links.borrow(), ["https://example.invalid/original.jpg"]);
    let mut row = app.get_media_items().row_data(0).unwrap();
    row.status = "Ready".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        row,
    ])));
    app.set_is_busy(true);
    h.run_steps(5);
    assert!(
        h.get_by_label("Download JPG")
            .accesskit_node()
            .is_disabled()
    );
    app.set_is_busy(false);
    h.run_steps(4);
    assert!(
        !h.get_by_label("Download JPG")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn playback_buttons_and_seek_keep_their_commands() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "video".into();
    item.file_extension = "MP4".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    let playback = app.global::<PreviewPlayback>();
    playback.set_ready(true);
    playback.set_can_seek(true);
    playback.set_duration(60.0);
    let toggles = Rc::new(std::cell::Cell::new(0));
    let seeks = Rc::new(RefCell::new(Vec::new()));
    playback.on_toggle({
        let calls = toggles.clone();
        move || calls.set(calls.get() + 1)
    });
    playback.on_seek({
        let calls = seeks.clone();
        move |value| calls.borrow_mut().push(value)
    });
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Play").click();
    h.run();
    assert_eq!(toggles.get(), 1);
    let seek = h.get_by_label("Playback position").rect();
    h.drag_at(seek.center());
    h.run_steps(1);
    h.hover_at(seek.center() + Vec2::new(80.0, 0.0));
    h.run_steps(3);
    h.drop_at(seek.center() + Vec2::new(80.0, 0.0));
    h.run_steps(3);
    assert!(seeks.borrow().iter().any(|value| *value > 30.0));
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert!((seeks.borrow().last().unwrap() - 0.1).abs() < 0.001);
}

#[test]
fn archive_dropdowns_and_switch_send_selected_conversion_options() {
    let app = fixture();
    app.set_archive_gif_count(2);
    app.set_archive_conversion_open(true);
    let converted = Rc::new(RefCell::new(Vec::new()));
    app.on_convert_archived_gifs({
        let calls = converted.clone();
        move |keep, quality, resolution| calls.borrow_mut().push((keep, quality, resolution))
    });
    let mut h = harness(app.clone());
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Conversion mode")
        .click();
    h.run();
    h.get_by_label("Better").click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Resolution")
        .click();
    h.run();
    h.get_by_label("400px").click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, "Keep original MP4 files")
        .click();
    h.run();
    h.get_by_label("Convert").click();
    h.run();
    assert_eq!(*converted.borrow(), [(false, 1, 3)]);
    assert!(!app.get_archive_conversion_open());
}

#[test]
fn action_buttons_share_hover_focus_and_disabled_visuals() {
    let app = fixture();
    let mut h = Harness::builder().with_size([600.0, 300.0]).build_ui_state(
        |ui, state: &mut Desktop| {
            if state.frame == 0 {
                setup(ui.ctx());
                state.frame = 1;
                ui.ctx().request_repaint();
                return;
            }
            state.home_action(ui, "Home action", None, false, Vec2::new(160.0, 32.0), 6);
            state.setting_action(ui, "Settings action", None, 0, Vec2::new(160.0, 32.0), true);
            state.setting_action(
                ui,
                "Disabled action",
                None,
                0,
                Vec2::new(160.0, 32.0),
                false,
            );
        },
        Desktop::new(app),
    );
    h.run();
    let idle = fill_for_rect(&h, h.get_by_label("Home action").rect());
    assert_eq!(
        idle,
        fill_for_rect(&h, h.get_by_label("Settings action").rect())
    );
    let mut hover = None;
    for label in ["Home action", "Settings action"] {
        h.remove_cursor();
        h.run_steps(10);
        h.get_by_label(label).hover();
        h.run_steps(10);
        let fill = fill_for_rect(&h, h.get_by_label(label).rect());
        assert_ne!(fill, idle);
        if let Some(expected) = hover {
            assert_eq!(fill, expected);
        } else {
            hover = Some(fill);
        }
    }
    let disabled = h.get_by_label("Disabled action").rect();
    assert!(
        h.get_by_label("Disabled action")
            .accesskit_node()
            .is_disabled()
    );
    let before = fill_for_rect(&h, disabled);
    assert!(
        (before.a() as i32 - 128).abs() <= 1,
        "Disabled controls must fade once, not twice: {before:?}"
    );
    h.hover_at(disabled.center());
    h.run_steps(10);
    assert_eq!(fill_for_rect(&h, disabled), before);
    h.get_by_label("Home action").click();
    h.run();
    assert!(!navigation_has_focus_ring(&h, "Home action"));
    h.key_press(egui::Key::Tab);
    h.run();
    assert!(navigation_has_focus_ring(&h, "Settings action"));
}

#[test]
#[ignore = "Renders shared controls in actual pages and dialogs"]
fn render_component_review() {
    let app = original_home_fixture();
    app.global::<RendererSettings>().set_gpu_supported(true);
    let rows = app
        .get_media_items()
        .iter()
        .map(|mut item| {
            item.author_verified = true;
            item.tweet_text = "Sample post used to check the desktop result preview.".into();
            item.alt_text =
                "A sample image description for checking wrapped preview details.".into();
            item.replies = "0".into();
            item.reposts = "0".into();
            item.likes = "0".into();
            item.views = "1,372".into();
            item.quotes = "12".into();
            item.bookmarks = "24".into();
            item.preview_date = "Aug 31, 2025 · 17:52 UTC".into();
            item.dimensions = "450 × 674".into();
            item.source = "Twitter Web App".into();
            item.file_extension = if item.media_type == "photo" {
                "JPG"
            } else {
                "MP4"
            }
            .into();
            item.url = "https://example.invalid/original".into();
            item.tweet_url = "https://example.invalid/post".into();
            item
        })
        .collect::<Vec<_>>();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(rows)));
    app.update_library(|data| data.groups = vec!["Favorites".into()]);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::path::Path::new("target/components-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        app.set_selected_tab(0);
        h.run_steps(5);
        h.get_by_label("Result order").click();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-home-dropdown.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        app.set_selected_tab(crate::library::TAB);
        h.run_steps(4);
        h.get_by_label("Filter group").click();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-library-dropdown.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        app.set_selected_tab(2);
        app.set_settings_category(0);
        h.set_size(Vec2::new(1440.0, 900.0));
        h.run_steps(4);
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Renderer")
            .click();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-settings-dropdown.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        h.set_size(Vec2::new(1024.0, 600.0));
        app.set_archive_gif_count(2);
        app.set_archive_conversion_open(true);
        h.run_steps(4);
        h.get_by_role_and_label(egui::accesskit::Role::ComboBox, "Conversion mode")
            .click();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-archive-dropdown.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        app.set_selected_tab(0);
        app.set_selected_media_index(0);
        h.remove_cursor();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-preview.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        app.set_startup_update_version("99.0.0".into());
        app.set_notify_updates(true);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-update.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
    }
}
