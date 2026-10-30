use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{cell::RefCell, rc::Rc};
mod account_search;
mod controls;
mod history;
mod home;
mod library;
mod preview;
mod settings;
mod support;
mod xtractor_setup;

fn harness(app: AppWindow) -> Harness<'static, Desktop> {
    harness_at_frame_rate(app, 4.0)
}

fn harness_at_frame_rate(app: AppWindow, frames_per_second: f32) -> Harness<'static, Desktop> {
    Harness::builder()
        .with_size([1120.0, 720.0])
        .with_step_dt(1.0 / frames_per_second)
        .with_max_steps((frames_per_second.ceil() as u64).max(12))
        .build_ui_state(
            |ui, state: &mut Desktop| {
                if state.frame == 0 {
                    setup(ui.ctx());
                    state.frame = 1;
                    ui.ctx().request_repaint();
                    return;
                }
                ui.set_clip_rect(ui.ctx().viewport_rect());
                let mut root = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("native-root")
                        .max_rect(ui.ctx().viewport_rect()),
                );
                root.set_clip_rect(ui.ctx().viewport_rect());
                state.render(&mut root);
            },
            Desktop::new(app),
        )
}
fn fixture() -> AppWindow {
    let app = AppWindow::new().unwrap();
    app.set_language_mode("en".into());
    app.set_theme_mode("light".into());
    app.set_app_version("test".into());
    app.set_saved_auth_token("test-only-token".into());
    crate::media::results::register(&app);
    let mut pixels = ui_runtime::SharedPixelBuffer::new(64, 48);
    for pixel in pixels.make_mut_bytes().chunks_exact_mut(4) {
        pixel.copy_from_slice(&[140, 100, 40, 255]);
    }
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        MediaItem {
            media_index: 42,
            title: "UI test fixture".into(),
            media_type: "photo".into(),
            status: "Ready".into(),
            preview_image: Image::from_rgba8(pixels),
            ..Default::default()
        },
    ])));
    app
}

fn original_home_fixture() -> AppWindow {
    let app = fixture();
    app.set_theme_mode("dark".into());
    app.set_source_url("xbatchdemo".into());
    app.set_has_output(true);
    app.set_account_profile(crate::AccountProfile{available:true,display_name:"xbatchdemo".into(),username:"xbatchdemo".into(),bio:"Unlock access to the Twitter/X Media Batch Downloader Pro script by becoming a paid member!".into(),website_label:"patreon.com/exyezed".into(),website_url:"https://patreon.com/exyezed".into(),joined:"Aug 2025".into(),posts:"11".into(),following:"2".into(),followers:"92".into(),media:"8".into(),has_banner:true,..Default::default()});
    let rows = (0..15)
        .map(|index| MediaItem {
            media_index: index,
            author_name: "xbatchdemo".into(),
            author_handle: "xbatchdemo".into(),
            title: format!("xbatchdemo · @xbatchdemo · item {index}"),
            media_type: ["animated_gif", "video", "photo"][index as usize % 3].into(),
            is_new: matches!(index, 0 | 2 | 4),
            status: if index == 4 {
                "Saved"
            } else if index == 5 {
                "Failed"
            } else {
                "Ready"
            }
            .into(),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(rows)));
    app.set_media_count(15);
    app
}

#[test]
fn home_control_positions_match_the_original_result_page() {
    let app = original_home_fixture();
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for label in [
        "Account profile",
        "Back to Home",
        "Paste from clipboard",
        "Search and extract",
        "Download all",
        "Export JSON",
        "Open folder",
        "Select filter",
        "Grid view",
        "List view",
        "xbatchdemo · @xbatchdemo · item 0",
    ] {
        let rect = h.get_by_label(label).rect();
        eprintln!("{label}: {rect:?}");
    }
    for (label, x, y, width, height) in [
        ("Account profile", 68.0, 66.0, 944.0, 239.0),
        ("Back to Home", 68.0, 12.0, 40.0, 40.0),
        ("Paste from clipboard", 789.0, 16.0, 32.0, 32.0),
        ("Search and extract", 827.0, 16.0, 32.0, 32.0),
        ("Download all", 582.0, 319.0, 148.0, 32.0),
        ("Export JSON", 738.0, 319.0, 140.0, 32.0),
        ("Open folder", 886.0, 319.0, 126.0, 32.0),
        ("Grid view", 942.0, 365.0, 32.0, 32.0),
        ("List view", 980.0, 365.0, 32.0, 32.0),
    ] {
        let rect = h.get_by_label(label).rect();
        assert!(
            (rect.left() - x).abs() < 2.0
                && (rect.top() - y).abs() < 2.0
                && (rect.width() - width).abs() < 2.0
                && (rect.height() - height).abs() < 2.0,
            "{label}: {rect:?}, expected [{x},{y},{width},{height}]"
        );
    }
    let tile = h.get_by_label("xbatchdemo · @xbatchdemo · item 7").rect();
    assert!(
        (tile.top() - 547.3).abs() < 1.0,
        "grid row spacing differs from Slint: {tile:?}"
    );
}

#[test]
fn home_list_rows_keep_the_original_height_spacing_and_actions() {
    let app = original_home_fixture();
    app.set_results_view("list".into());
    let downloaded = Rc::new(std::cell::Cell::new(-1));
    let record = downloaded.clone();
    app.on_download_selected_media(move |index| record.set(index));
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let first = h.get_by_label("xbatchdemo · @xbatchdemo · item 0").rect();
    let second = h
        .get_all_by_label("xbatchdemo · @xbatchdemo · item 1")
        .find(|node| node.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .rect();
    assert_eq!(first.height(), 96.0);
    assert!((first.top() - 411.0).abs() < 1.0);
    assert!((second.top() - first.top() - 106.0).abs() < 1.0);
    let label = "Download file · xbatchdemo · @xbatchdemo · item 0";
    let rect = h.get_by_label(label).rect();
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(3);
    let idle = fill_for_rect(&h, rect);
    h.hover_at(rect.center());
    h.run_steps(4);
    assert_ne!(fill_for_rect(&h, rect), idle);
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(3);
    h.get_by_label(label).click();
    h.run_steps(2);
    assert_eq!(downloaded.get(), 0);
    assert_eq!(app.get_selected_media_index(), -1);
}

#[test]
fn home_profile_without_banner_keeps_the_original_compact_height() {
    let app = original_home_fixture();
    let mut profile = app.get_account_profile();
    profile.has_banner = false;
    app.set_account_profile(profile);
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let rect = h.get_by_label("Account profile").rect();
    assert_eq!(rect.top(), 66.0);
    assert_eq!(rect.height(), 183.0);
}

#[test]
fn home_text_media_wraps_and_stays_inside_its_tile() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "text".into();
    item.preview_image = Image::default();
    item.tweet_text = "Long text layout fixture. ".repeat(30).into();
    let text = item.tweet_text.to_string();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let tile = h.get_by_label("UI test fixture").rect();
    let shape = h
        .output()
        .shapes
        .iter()
        .find(|shape| matches!(&shape.shape,egui::Shape::Text(value)if value.galley.text()==text))
        .unwrap();
    assert!(tile.contains_rect(shape.clip_rect));
    if let egui::Shape::Text(value) = &shape.shape {
        assert!(value.galley.rows.len() > 1);
    }
}

#[test]
fn dormant_batch_items_do_not_move_the_empty_home_search() {
    let app = fixture();
    app.set_media_items(Default::default());
    app.set_batch_fetch_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::BatchFetchItem {
            source: "https://x.com/test-fixture".into(),
            status: "Queued".into(),
            ..Default::default()
        },
    ])));
    app.set_batch_mode(false);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(h.query_by_label("Batch queue").is_none());
    assert!(
        (h.get_by_label("Search X/Twitter media URL")
            .rect()
            .center()
            .y
            - 317.0)
            .abs()
            < 3.0
    );
    let single = h
        .get_by_role_and_label(egui::accesskit::Role::Button, "Single")
        .rect();
    let batch = h
        .get_by_role_and_label(egui::accesskit::Role::Button, "Batch")
        .rect();
    assert_eq!(single.height(), 32.0);
    assert_eq!(single.right(), batch.left());
    assert_eq!((single.left() + batch.right()) / 2.0, 540.0);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Batch")
        .click();
    h.run();
    assert!(h.query_by_label("Batch queue").is_some());
    assert!(h.query_by_label("Search X/Twitter media URL").is_none());
    assert!(h.query_by_label("Edit accounts").is_some());
    assert_eq!(
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Single")
            .rect(),
        single
    );
    assert_eq!(
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Batch")
            .rect(),
        batch
    );
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    assert!(!app.get_batch_mode());
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert!(app.get_batch_mode());
    app.set_is_busy(true);
    h.run_steps(4);
    for label in ["Single", "Batch"] {
        assert!(
            h.get_by_role_and_label(egui::accesskit::Role::Button, label)
                .accesskit_node()
                .is_disabled()
        );
    }
}

fn fill_for_rect(h: &Harness<'_, Desktop>, rect: egui::Rect) -> Color32 {
    h.output()
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(shape)
                if shape.rect == rect && shape.fill != Color32::TRANSPARENT =>
            {
                Some(shape.fill)
            }
            _ => None,
        })
        .unwrap_or(Color32::TRANSPARENT)
}

fn assert_painted_text_color(h: &Harness<'_, Desktop>, label: &str, expected: Color32) {
    let text = h
        .output()
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) if text.galley.text() == label => Some(text),
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing painted text: {label}"));
    for section in &text.galley.job.sections {
        assert_eq!(
            section.format.color, expected,
            "{label} has a baked-in text color that overrides its link color"
        );
    }
}

#[test]
fn home_links_paint_the_selected_accent_in_every_state() {
    let app = fixture();
    app.set_saved_auth_token(String::new());
    app.set_media_items(Default::default());
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        for preset in ["cacao", "github", "graphite"] {
            app.global::<AppTheme>().set_preset(preset.into());
            h.run_steps(3);
            let accent = palette_color(preset, theme == "dark");
            assert_painted_text_color(&h, "click here", accent);
            h.get_by_label("click here").hover();
            h.run_steps(3);
            assert_painted_text_color(&h, "click here", accent);
            for dialog in 0..2 {
                if dialog == 0 {
                    app.set_fetch_options_open(true);
                } else {
                    app.set_account_search_open(true);
                }
                h.run_steps(3);
                assert_painted_text_color(&h, "click here", accent);
                app.set_fetch_options_open(false);
                app.set_account_search_open(false);
                h.run_steps(2);
            }
        }
    }
    h.get_by_label("click here").click();
    h.run();
    assert_eq!(app.get_selected_tab(), 2);
    assert_eq!(app.get_settings_category(), 1);
    app.set_selected_tab(0);
    let populated = original_home_fixture();
    app.set_media_items(populated.get_media_items());
    app.set_account_profile(populated.get_account_profile());
    h.run_steps(3);
    assert_painted_text_color(&h, "patreon.com/exyezed", palette_color("graphite", false));
}

#[test]
#[ignore = "Renders link and scrollbar idle, hover, and drag states"]
fn render_home_color_details() {
    let app = original_home_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::env::var_os("TWITTERDL_UI_DETAIL_SNAPSHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/egui-detail-review".into());
    std::fs::create_dir_all(&directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        app.set_account_profile(Default::default());
        app.set_media_items(Default::default());
        app.set_saved_auth_token(String::new());
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-home-auth.png")))
            .unwrap();
        app.set_fetch_options_open(true);
        app.set_batch_mode(true);
        app.set_date_filter_enabled(true);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-fetch-expanded.png")))
            .unwrap();
        let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
        h.hover_at(bar.center());
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-fetch-scrollbar-hover.png")))
            .unwrap();
        app.set_fetch_options_open(false);
        app.set_batch_mode(false);
        app.set_date_filter_enabled(false);
        app.set_saved_auth_token("test-only-token".into());
        let fixture = original_home_fixture();
        app.set_media_items(fixture.get_media_items());
        app.set_account_profile(fixture.get_account_profile());
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-results-idle.png")))
            .unwrap();
        let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
        h.hover_at(bar.center());
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-results-scrollbar-hover.png")))
            .unwrap();
        let thumb = painted_scroll_thumb(&h, bar).0;
        h.drag_at(thumb.center());
        h.run_steps(1);
        let target = thumb.center() + Vec2::new(0.0, 80.0);
        h.hover_at(target);
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-results-scrollbar-drag.png")))
            .unwrap();
        h.drop_at(target);
        h.run_steps(3);
    }
}

fn painted_scroll_thumb(h: &Harness<'_, Desktop>, bar: egui::Rect) -> (egui::Rect, Color32) {
    h.output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill.a() > 0
                    && rect.fill.a() < 255
                    && rect.rect.width() <= 6.1
                    && rect.rect.height() > 16.0
                    && bar.contains(rect.rect.center()) =>
            {
                Some((rect.rect, rect.fill))
            }
            _ => None,
        })
        .expect("Expected a translucent scrollbar thumb, independently of text and button colors")
}

#[test]
fn scrollbars_match_legacy_transparency_and_still_drag_in_home_and_dialogs() {
    for theme in ["dark", "light"] {
        let app = original_home_fixture();
        app.set_theme_mode(theme.into());
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        let expected = if theme == "dark" {
            Color32::from_white_alpha(20)
        } else {
            Color32::from_black_alpha(115)
        };
        for view in 0..3 {
            if view == 1 {
                app.set_media_items(Default::default());
                app.set_account_profile(Default::default());
                h.set_size(Vec2::new(1024.0, 360.0));
                app.set_fetch_options_open(true);
                app.set_batch_mode(true);
                app.set_date_filter_enabled(true);
            } else if view == 2 {
                h.set_size(Vec2::new(1024.0, 600.0));
                app.set_fetch_options_open(false);
                app.set_batch_mode(false);
                app.set_date_filter_enabled(false);
                app.set_account_search_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
                    (0..30)
                        .map(|index| crate::AccountSearchItem {
                            name: format!("Scrollbar fixture {index}"),
                            handle: format!("fixture{index}"),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>(),
                )));
                app.set_account_search_open(true);
            }
            h.remove_cursor();
            h.run_steps(4);
            let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
            let (idle, color) = painted_scroll_thumb(&h, bar);
            assert_eq!(color, expected);
            assert_eq!(idle.width(), 2.0);
            if view == 0 {
                assert_eq!(idle.right(), 1020.0);
                assert_eq!(idle.top(), 16.0);
            }
            h.hover_at(bar.center());
            h.run_steps(4);
            let (hover, color) = painted_scroll_thumb(&h, bar);
            assert_eq!(color, expected);
            assert_eq!(hover.width(), 6.0);
            h.drag_at(hover.center());
            h.run_steps(1);
            let target = hover.center() + Vec2::new(0.0, 60.0);
            h.hover_at(target);
            h.run_steps(3);
            let (drag, color) = painted_scroll_thumb(&h, bar);
            assert_eq!(color, expected);
            assert!(
                drag.top() > idle.top() + 10.0,
                "Dragging failed in {theme} view {view}"
            );
            h.drop_at(target);
            h.run_steps(4);
            assert_eq!(painted_scroll_thumb(&h, bar).1, expected);
        }
    }
}

#[test]
fn home_hover_changes_the_painted_controls_and_keeps_tooltips_and_clicks() {
    let app = original_home_fixture();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let record = calls.clone();
    app.on_start_downloads(move |scope| record.borrow_mut().push(scope));
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for label in [
        "History",
        "Search accounts",
        "Fetch options",
        "Paste from clipboard",
        "Search and extract",
        "Download all",
        "Export JSON",
        "Open folder",
        "Image  5",
        "List view",
        "Preview · xbatchdemo · @xbatchdemo · item 0",
        "Download file · xbatchdemo · @xbatchdemo · item 0",
    ] {
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(3);
        let rect = h.get_by_label(label).rect();
        let idle = fill_for_rect(&h, rect);
        h.hover_at(rect.center());
        h.run_steps(4);
        assert_ne!(
            fill_for_rect(&h, rect),
            idle,
            "{label} has no painted hover response"
        );
        assert_eq!(
            h.output().platform_output.cursor_icon,
            egui::CursorIcon::PointingHand,
            "{label} has no pointer cursor"
        );
    }
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(4);
    let rect = h.get_by_label("Fetch options").rect();
    h.hover_at(rect.center());
    h.run_steps(4);
    assert!(h.output().shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text)if text.galley.text()=="Fetch options")),"The delayed tooltip is missing");
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(4);
    let rect = h.get_by_label("Download all").rect();
    h.hover_at(rect.center());
    h.run_steps(3);
    let hovered = fill_for_rect(&h, rect);
    h.event(egui::Event::PointerButton {
        pos: rect.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    assert_ne!(fill_for_rect(&h, rect), hovered, "Pressed state is missing");
    h.event(egui::Event::PointerButton {
        pos: rect.center(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(2);
    assert_eq!(*calls.borrow(), ["all"]);
}

#[test]
fn home_profile_download_overlays_reveal_on_hover_and_respect_busy_state() {
    let app = original_home_fixture();
    let mut profile = app.get_account_profile();
    profile.avatar_image = fixture()
        .get_media_items()
        .row_data(0)
        .unwrap()
        .preview_image;
    profile.has_avatar = true;
    app.set_account_profile(profile);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let record = calls.clone();
    app.on_download_profile_image(move |kind| record.borrow_mut().push(kind));
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for label in ["Download avatar", "Download banner"] {
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(3);
        let rect = h.get_by_label(label).rect();
        assert_eq!(
            fill_for_rect(&h, rect),
            Color32::TRANSPARENT,
            "{label} should stay hidden at rest"
        );
        h.hover_at(rect.center());
        h.run_steps(4);
        assert_ne!(
            fill_for_rect(&h, rect),
            Color32::TRANSPARENT,
            "{label} did not reveal its download overlay"
        );
    }
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(4);
    h.get_by_label("Download avatar").click();
    h.run_steps(2);
    assert_eq!(*calls.borrow(), ["avatar"]);
    app.set_is_busy(true);
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(4);
    h.get_by_label("Download avatar").click();
    h.run_steps(2);
    assert_eq!(
        *calls.borrow(),
        ["avatar"],
        "A busy operation must disable profile downloads"
    );
}

#[test]
#[ignore = "Renders Home idle and hover states for visual review"]
fn render_home_hover_review() {
    let app = original_home_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run_steps(4);
    std::thread::sleep(std::time::Duration::from_millis(100));
    h.run_steps(3);
    let directory = std::path::Path::new("target/home-review");
    std::fs::create_dir_all(directory).unwrap();
    h.render()
        .unwrap()
        .save(directory.join("results-idle.png"))
        .unwrap();
    for (label, name) in [
        ("History", "rail-hover"),
        ("Search accounts", "search-accounts-hover"),
        ("Fetch options", "fetch-options-hover"),
        ("Paste from clipboard", "paste-hover"),
        ("Search and extract", "extract-hover"),
        ("Download all", "download-all-hover"),
        ("Export JSON", "export-hover"),
        ("Grid view", "grid-hover"),
        (
            "Preview · xbatchdemo · @xbatchdemo · item 0",
            "tile-preview-hover",
        ),
        (
            "Download file · xbatchdemo · @xbatchdemo · item 0",
            "tile-download-hover",
        ),
    ] {
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(3);
        h.get_by_label(label).hover();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{name}.png")))
            .unwrap();
    }
    app.set_results_view("list".into());
    h.hover_at(egui::pos2(500.0, 10.0));
    h.run_steps(3);
    h.render()
        .unwrap()
        .save(directory.join("list-idle.png"))
        .unwrap();
    h.get_by_label("Download file · xbatchdemo · @xbatchdemo · item 0")
        .hover();
    h.run_steps(4);
    h.render()
        .unwrap()
        .save(directory.join("list-download-hover.png"))
        .unwrap();
    app.set_results_view("grid".into());
    for theme in ["light", "dark"] {
        app.set_theme_mode(theme.into());
        h.set_size(Vec2::new(1440.0, 900.0));
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-1440-results.png")))
            .unwrap();
    }
    h.set_size(Vec2::new(1024.0, 600.0));
    let mut profile = app.get_account_profile();
    profile.has_avatar = true;
    profile.avatar_image = fixture()
        .get_media_items()
        .row_data(0)
        .unwrap()
        .preview_image;
    app.set_account_profile(profile);
    for (label, file) in [
        ("Download avatar", "avatar-download-hover.png"),
        ("Download banner", "banner-download-hover.png"),
    ] {
        h.hover_at(egui::pos2(500.0, 10.0));
        h.run_steps(3);
        h.get_by_label(label).hover();
        h.run_steps(4);
        h.render().unwrap().save(directory.join(file)).unwrap();
    }
}

#[test]
fn navigation_download_selection_and_preview_use_source_indices() {
    let app = fixture();
    let downloads = Rc::new(RefCell::new(Vec::new()));
    let record = downloads.clone();
    app.on_start_downloads(move |scope| record.borrow_mut().push(scope));
    let mut h = harness(app.clone());
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "UI test fixture")
        .click();
    h.run();
    assert_eq!(
        app.global::<MediaSelection>()
            .get_indices()
            .iter()
            .collect::<Vec<_>>(),
        [42]
    );
    assert_eq!(
        app.get_selected_media_index(),
        -1,
        "Selecting a tile must not open its preview"
    );
    h.get_by_label("Download selected (1)").click();
    h.run();
    assert_eq!(*downloads.borrow(), ["selected"]);
    h.get_by_label("History").click();
    h.run();
    assert_eq!(app.get_selected_tab(), 1);
    h.get_by_label("Settings").click();
    h.run();
    assert_eq!(app.get_selected_tab(), 2);
    h.get_by_label("Cookies").click();
    h.run();
    assert_eq!(app.get_settings_category(), 1);
    h.get_by_label("Support").click();
    h.run();
    assert_eq!(app.get_selected_tab(), 3);
    app.set_selected_tab(0);
    h.run();
    h.get_by_label("Preview · UI test fixture").click();
    h.run();
    assert_eq!(app.get_selected_media_index(), 42);
    assert!(h.query_by_label("Media preview").is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(app.get_selected_media_index(), -1);
}

#[test]
fn fetch_options_autosave_edits_and_escape_keeps_them() {
    let app = fixture();
    let owner = app.as_weak();
    app.on_apply_fetch_options(move |batch, dates, start, end, sources| {
        let app = owner.upgrade().unwrap();
        app.set_batch_mode(batch);
        app.set_date_filter_enabled(dates);
        app.set_fetch_start_date(start);
        app.set_fetch_end_date(end);
        app.set_batch_sources_text(sources);
        false // Invalid partial input still remains editable, as in Slint.
    });
    let mut h = harness(app.clone());
    h.run();
    h.get_by_label("Fetch options").click();
    h.run();
    assert!(h.query_by_label("Batch accounts").is_none());
    h.get_by_label("Date range").click();
    h.run();
    h.get_by_label("Start date").click();
    h.run();
    h.get_by_label("Start date").type_text("2026-10");
    h.run();
    assert_eq!(app.get_fetch_start_date(), "2026-10");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_fetch_options_open());
    assert!(app.get_date_filter_enabled());
    h.get_by_label("Back to Home").click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Batch")
        .click();
    h.run();
    h.get_by_label("Batch account list").click();
    h.run();
    h.get_by_label("Batch account list")
        .type_text("@fixture\n@another_fixture");
    h.run();
    assert_eq!(
        h.state().batch_sources_draft.as_deref(),
        Some("@fixture\n@another_fixture")
    );
    assert!(app.get_batch_sources_text().is_empty());
    h.get_by_label("Update queue").click();
    h.run();
    assert_eq!(app.get_batch_sources_text(), "@fixture\n@another_fixture");
    assert!(app.get_batch_mode());
}

#[test]
fn home_dialogs_keep_auth_links_private_source_rules_and_compact_account_search() {
    let app = fixture();
    app.set_saved_auth_token(String::new());
    app.on_close_account_search({
        let app = app.as_weak();
        move || app.upgrade().unwrap().set_account_search_open(false)
    });
    crate::fetch::sources::register(&app);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    app.set_account_search_open(true);
    h.run();
    assert_eq!(
        h.get_by_label("Account search dialog").rect(),
        egui::Rect::from_min_size(egui::pos2(167.0, 200.0), Vec2::new(690.0, 200.0))
    );
    assert!(h.query_by_label("Load more").is_none());
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(app.get_account_search_open());
    h.get_by_label("click here").click();
    h.run();
    assert!(!app.get_account_search_open());
    assert_eq!(app.get_selected_tab(), 2);
    assert_eq!(app.get_settings_category(), 1);
    app.set_selected_tab(0);
    app.set_fetch_options_open(true);
    h.run();
    assert_eq!(h.get_by_label("Fetch options dialog").rect().width(), 520.0);
    assert_eq!(
        h.get_by_label("Fetch options dialog").rect().height(),
        413.0
    );
    h.get_by_label("Reset fetch options").hover();
    h.run_steps(4);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    assert!(h.output().shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Text(text) if text.galley.text()=="Reset fetch options")));
    let saves = Rc::new(RefCell::new(0));
    app.on_settings_changed({
        let saves = saves.clone();
        move || *saves.borrow_mut() += 1
    });
    h.get_by_label("Quick refresh").click();
    h.run();
    assert_eq!(*saves.borrow(), 1);
    h.get_by_label("Fetch source").click();
    h.run();
    h.get_by_label("Private · My Bookmarks").click();
    h.run();
    assert_eq!(app.get_fetch_source_index(), 2);
    for label in ["Date range", "Quick refresh"] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_fetch_options_open());
    app.set_fetch_options_open(true);
    h.run();
    h.get_by_label("Fetch source").click();
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(
        app.get_fetch_options_open(),
        "Escape first closes the source menu"
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_fetch_options_open());
}

#[test]
fn batch_fetch_options_use_the_queue_and_keep_public_options_enabled() {
    let app = fixture();
    app.set_batch_mode(true);
    app.set_fetch_options_open(true);
    app.set_batch_sources_text("first_account\nsecond_account".into());
    crate::app::preferences::register_with_writer(&app, |_| true);
    let applied = Rc::new(RefCell::new(Vec::new()));
    app.on_apply_fetch_options({
        let calls = applied.clone();
        let weak = app.as_weak();
        move |batch, dates, start, end, sources| {
            calls.borrow_mut().push((batch, dates, start, end, sources));
            weak.upgrade().unwrap().set_date_filter_enabled(dates);
            true
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(h.query_by_label("Fetch source").is_none());
    assert!(h.get_by_label("Fetch options dialog").rect().height() < 350.0);
    for label in ["Date range", "Quick refresh"] {
        assert!(!h.get_by_label(label).accesskit_node().is_disabled());
    }
    h.get_by_label("Date range").click();
    h.run();
    assert!(app.get_date_filter_enabled());
    assert_eq!(applied.borrow().len(), 1);
    assert!(applied.borrow()[0].0);
    assert_eq!(applied.borrow()[0].4, "first_account\nsecond_account");
    h.get_by_label("Reset fetch options").click();
    h.run();
    assert!(app.get_batch_mode());
    assert!(!app.get_date_filter_enabled());
    assert_eq!(
        app.get_batch_sources_text(),
        "first_account\nsecond_account"
    );
    assert_eq!(applied.borrow().len(), 2);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_fetch_options_open());
}

#[test]
#[ignore = "Writes GPU-rendered Home dialog comparison images"]
fn render_home_dialog_comparisons() {
    let app = fixture();
    app.set_media_items(Default::default());
    app.set_saved_auth_token(String::new());
    app.set_incremental_fetch(true);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::path::Path::new("target/egui-home-dialogs");
    std::fs::create_dir_all(directory).unwrap();
    for (preset, theme) in [("cacao", "dark"), ("graphite", "dark"), ("cacao", "light")] {
        app.global::<AppTheme>().set_preset(preset.into());
        app.set_theme_mode(theme.into());
        app.set_fetch_options_open(true);
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{preset}-{theme}-fetch.png")))
            .unwrap();
        app.set_batch_mode(true);
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{preset}-{theme}-batch-fetch.png")))
            .unwrap();
        app.set_batch_mode(false);
        app.set_fetch_options_open(false);
        app.set_account_search_open(true);
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{preset}-{theme}-search.png")))
            .unwrap();
        app.set_account_search_open(false);
    }
    app.set_theme_mode("dark".into());
    app.set_saved_auth_token("test-only-token".into());
    app.set_account_search_query("demo".into());
    app.set_account_search_result_query("demo".into());
    app.set_account_search_cursor("next".into());
    app.set_account_search_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..30)
            .map(|index| crate::AccountSearchItem {
                name: format!("Search fixture {index}"),
                handle: format!("fixture{index}"),
                bio: "Account search layout test fixture.".into(),
                followers: "123".into(),
                verified: index % 2 == 0,
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    )));
    app.set_account_search_open(true);
    h.run_steps(5);
    h.render()
        .unwrap()
        .save(directory.join("amber-dark-search-results.png"))
        .unwrap();
    app.set_account_search_open(false);
    app.set_fetch_options_open(true);
    app.set_batch_mode(true);
    app.set_date_filter_enabled(true);
    app.set_batch_sources_text("@fixture\n@another_fixture".into());
    h.run_steps(5);
    h.render()
        .unwrap()
        .save(directory.join("amber-dark-fetch-expanded.png"))
        .unwrap();
}

#[test]
fn account_search_keyboard_grid_pagination_and_submit_keep_original_actions() {
    let app = fixture();
    let owner = app.as_weak();
    app.on_close_account_search(move || owner.upgrade().unwrap().set_account_search_open(false));
    let requests = Rc::new(RefCell::new(Vec::new()));
    app.on_search_accounts({
        let requests = requests.clone();
        move |query, more| requests.borrow_mut().push((query, more))
    });
    app.set_account_search_open(true);
    app.set_account_search_query("demo".into());
    app.set_account_search_result_query("demo".into());
    app.set_account_search_cursor("next".into());
    app.set_account_search_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..30)
            .map(|index| crate::AccountSearchItem {
                name: format!("Search fixture {index}"),
                handle: format!("fixture{index}"),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    )));
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let first = h.get_by_label("Search fixture 0 · @fixture0").rect();
    let second = h.get_by_label("Search fixture 1 · @fixture1").rect();
    assert_eq!(first.size(), Vec2::new(319.0, 84.0));
    assert_eq!(second.min - first.min, Vec2::new(331.0, 0.0));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(*requests.borrow(), [("demo".into(), false)]);
    assert!(h.query_by_label("Load more").is_none());
    let dialog = h.get_by_label("Account search dialog").rect();
    let bar = h
        .get_all_by_role(egui::accesskit::Role::ScrollBar)
        .find(|bar| dialog.contains_rect(bar.rect()))
        .unwrap()
        .rect();
    let bottom = egui::pos2(bar.center().x, bar.bottom() - 1.0);
    h.drag_at(bar.center());
    h.run_steps(1);
    h.hover_at(bottom);
    h.run_steps(3);
    h.drop_at(bottom);
    h.run();
    assert_eq!(requests.borrow().last(), Some(&("demo".into(), true)));
    h.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new("account-search-query")));
    h.run();
    h.key_press(egui::Key::Tab);
    h.run();
    h.key_press(egui::Key::Tab);
    h.run();
    assert_eq!(
        h.ctx.memory(|m| m.focused()),
        Some(egui::Id::new("account-search-results"))
    );
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    assert_eq!(h.state().account_result_index, 3);
    for _ in 0..12 {
        h.key_press(egui::Key::ArrowDown);
        h.run();
    }
    assert_eq!(h.state().account_result_index, 27);
    let last = h.get_by_label("Search fixture 27 · @fixture27").rect();
    assert!(
        last.bottom() <= dialog.bottom() - 20.0,
        "Keyboard navigation must reveal the selected card"
    );
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_source_url(), "fixture27");
    assert!(!app.get_account_search_open());
}

#[test]
fn all_views_and_dialogs_render_in_both_themes_at_minimum_size() {
    let app = fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    for theme in ["light", "dark"] {
        app.set_theme_mode(theme.into());
        for tab in 0..4 {
            app.set_selected_tab(tab);
            h.run_steps(3);
        }
        app.set_selected_tab(2);
        for category in 0..5 {
            app.set_settings_category(category);
            h.run_steps(3);
        }
        for dialog in 0..8 {
            match dialog {
                0 => app.set_account_search_open(true),
                1 => app.set_integrity_open(true),
                2 => app.set_backup_restore_open(true),
                3 => app.set_account_labels_open(true),
                4 => app.set_archive_conversion_open(true),
                5 => app.set_log_viewer_open(true),
                6 => app.global::<RendererSettings>().set_prompt_open(true),
                _ => app.set_fetch_options_open(true),
            }
            h.run_steps(3);
            h.key_press(egui::Key::Escape);
            h.run_steps(3);
        }
    }
}

#[test]
fn opening_download_settings_preserves_legacy_media_filters() {
    let app = fixture();
    app.set_media_type_index(1);
    app.set_media_filters_custom(false);
    let original = crate::fetch::request::media_flags(&app);
    app.set_selected_tab(2);
    app.set_settings_category(2);
    let mut h = harness(app.clone());
    h.run();
    assert!(!app.get_media_filters_custom());
    assert_eq!(crate::fetch::request::media_flags(&app), original);
}

#[test]
fn original_rail_sidebar_and_search_geometry_fit_the_minimum_window() {
    let app = fixture();
    app.set_media_items(Default::default());
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let home = h.get_by_label("Home").rect();
    assert!((home.width() - 40.0).abs() < 1.0);
    assert!((home.left() - 8.0).abs() < 1.0);
    let search = h.get_by_label("Search X/Twitter media URL").rect();
    assert!(search.is_finite());
    assert!((search.left() - 241.0).abs() < 2.0);
    assert!(
        (search.center().y - 317.0).abs() < 3.0,
        "search position differs from the previous 1024×600 view: {search:?}"
    );
    h.get_by_label("Settings").click();
    h.run();
    let cookies = h.get_by_label("Cookies").rect();
    assert!((cookies.left() - 68.0).abs() < 1.0);
    assert!((cookies.width() - 206.0).abs() < 2.0);
    for label in ["System", "Dark", "Light", "GitHub", "Dracula"] {
        let rect = h.get_by_label(label).rect();
        assert!(
            rect.right() <= 1024.0,
            "{label} extends past the window: {rect:?}"
        );
    }
}

#[test]
fn sidepanel_navigation_keeps_legacy_positions_on_every_page() {
    let app = fixture();
    let mut h = harness(app.clone());
    for (width, height) in [(1024.0, 600.0), (1440.0, 900.0)] {
        h.set_size(Vec2::new(width, height));
        for tab in 0..4 {
            app.set_selected_tab(tab);
            h.remove_cursor();
            h.run_steps(3);
            for (label, y) in [
                ("Home", 16.0),
                ("History", 72.0),
                ("Support", height - 112.0),
                ("Settings", height - 56.0),
            ] {
                let rect = h
                    .get_by_role_and_label(egui::accesskit::Role::Button, label)
                    .rect();
                assert_eq!(
                    rect,
                    egui::Rect::from_min_size(egui::pos2(8.0, y), Vec2::splat(40.0)),
                    "{label} moved on tab {tab}"
                );
            }
            let divider = egui::Rect::from_min_size(egui::pos2(55.0, 0.0), Vec2::new(1.0, height));
            assert_eq!(
                fill_for_rect(&h, divider),
                appearance::Palette::new(schemes::DEFAULT, false).border
            );
        }
    }
}

#[test]
fn settings_sidepanel_rows_keep_legacy_alignment_colors_and_hover() {
    let app = fixture();
    app.set_selected_tab(2);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        h.remove_cursor();
        h.run_steps(3);
        let colors = appearance::Palette::new(schemes::DEFAULT, theme == "dark");
        for (index, label) in ["General", "Cookies", "Downloads", "Storage", "About"]
            .iter()
            .enumerate()
        {
            let rect = h
                .get_by_role_and_label(egui::accesskit::Role::Button, label)
                .rect();
            assert_eq!(
                rect,
                egui::Rect::from_min_size(
                    egui::pos2(68.0, 42.0 + 44.0 * index as f32),
                    Vec2::new(206.0, 40.0)
                )
            );
            let text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == *label && text.pos.x < 286.0 =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(
                text.pos.x, 80.0,
                "Category labels must align to the left inset"
            );
            assert_eq!(text.galley.job.sections[0].format.color, colors.heading);
        }
        let cookies = h
            .get_by_role_and_label(egui::accesskit::Role::Button, "Cookies")
            .rect();
        assert_eq!(fill_for_rect(&h, cookies), Color32::TRANSPARENT);
        h.hover_at(cookies.center());
        h.run_steps(3);
        assert_eq!(fill_for_rect(&h, cookies), colors.accent_soft);
        assert_eq!(
            h.output().platform_output.cursor_icon,
            egui::CursorIcon::PointingHand
        );
        h.remove_cursor();
        h.run_steps(3);
    }
}

#[test]
#[ignore = "Renders sidepanel selection, hover, tooltip, and localized layouts"]
fn render_sidepanel_review() {
    let app = fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::env::var_os("TWITTERDL_SIDEPANEL_SNAPSHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/sidepanel-review".into());
    std::fs::create_dir_all(&directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        for tab in 0..4 {
            app.set_selected_tab(tab);
            h.remove_cursor();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-tab-{tab}.png")))
                .unwrap();
        }
        for (category, label) in [(0, "Cookies"), (1, "Downloads"), (3, "About")] {
            app.set_selected_tab(2);
            app.set_settings_category(category);
            h.remove_cursor();
            h.run_steps(4);
            h.get_by_role_and_label(egui::accesskit::Role::Button, label)
                .hover();
            h.run_steps(3);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-category-{category}-hover.png")))
                .unwrap();
        }
        for label in ["Home", "History", "Support", "Settings"] {
            h.get_by_role_and_label(egui::accesskit::Role::Button, label)
                .hover();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-rail-{}.png", label.to_lowercase())))
                .unwrap();
        }
    }
    app.global::<I18n>().set_language("fr".into());
    h.set_size(Vec2::new(1440.0, 900.0));
    h.remove_cursor();
    h.run_steps(4);
    h.render()
        .unwrap()
        .save(directory.join("light-french-1440.png"))
        .unwrap();
}

#[test]
fn sidepanel_tooltips_stay_to_the_right_and_hover_states_work_on_every_page() {
    let app = fixture();
    app.set_theme_mode("dark".into());
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let colors = appearance::Palette::new(schemes::DEFAULT, true);
    for tab in 0..4 {
        app.set_selected_tab(tab);
        h.remove_cursor();
        h.run_steps(3);
        for (destination, label) in [(0, "Home"), (1, "History"), (3, "Support"), (2, "Settings")] {
            h.remove_cursor();
            h.run_steps(3);
            let rect = h
                .get_by_role_and_label(egui::accesskit::Role::Button, label)
                .rect();
            assert_eq!(
                fill_for_rect(&h, rect),
                if tab == destination {
                    colors.accent_soft
                } else {
                    Color32::TRANSPARENT
                }
            );
            h.hover_at(rect.center());
            h.run_steps(1);
            let tip = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Rect(rect)
                        if rect.fill == Color32::from_gray(41) && rect.rect.height() == 28.0 =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .expect("Rail tooltips must appear immediately");
            assert_eq!(tip.left(), 64.0);
            assert_eq!(tip.center().y, rect.center().y);
            let tooltip_text = h
                .output()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text)
                        if text.galley.text() == label && tip.contains(text.pos) =>
                    {
                        Some(text)
                    }
                    _ => None,
                })
                .expect("Missing rail tooltip label");
            assert_eq!(
                tooltip_text.galley.job.sections[0].format.color,
                Color32::WHITE
            );
            h.run_steps(3);
            assert_eq!(
                fill_for_rect(&h, rect),
                if tab == destination {
                    colors.accent_soft
                } else {
                    colors.control
                }
            );
            assert_eq!(
                h.output().platform_output.cursor_icon,
                egui::CursorIcon::PointingHand
            );
        }
    }
}

fn navigation_has_focus_ring(h: &Harness<'_, Desktop>, label: &str) -> bool {
    let rect = h
        .get_by_role_and_label(egui::accesskit::Role::Button, label)
        .rect();
    h.output().shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(value) if value.rect==rect&&value.stroke.width==2.0))
}

#[test]
fn pointer_navigation_never_outlines_the_previously_clicked_item() {
    for theme in ["dark", "light"] {
        let app = fixture();
        app.set_theme_mode(theme.into());
        app.set_selected_tab(2);
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        for (previous, next) in [("Storage", "About"), ("History", "Home")] {
            h.get_by_role_and_label(egui::accesskit::Role::Button, previous)
                .click();
            h.run();
            assert!(!navigation_has_focus_ring(&h, previous));
            h.get_by_role_and_label(egui::accesskit::Role::Button, next)
                .hover();
            h.run_steps(4);
            assert!(!navigation_has_focus_ring(&h, previous));
            let pos = h
                .get_by_role_and_label(egui::accesskit::Role::Button, next)
                .rect()
                .center();
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(3);
            assert!(
                !navigation_has_focus_ring(&h, previous),
                "{previous} gained an outline while pressing {next} in {theme} mode"
            );
            h.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
            assert!(!navigation_has_focus_ring(&h, previous));
            assert!(!navigation_has_focus_ring(&h, next));
        }
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(navigation_has_focus_ring(&h, "History"));
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Home")
            .hover();
        h.run_steps(4);
        assert!(navigation_has_focus_ring(&h, "History"));
    }
}

#[test]
fn sidepanel_keyboard_navigation_keeps_focus_and_pointer_clicks_clear_the_ring() {
    let app = fixture();
    let refreshes = Rc::new(RefCell::new(0));
    app.on_refresh_storage({
        let refreshes = refreshes.clone();
        move || *refreshes.borrow_mut() += 1
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Settings")
        .click();
    h.run();
    assert_eq!(app.get_selected_tab(), 2);
    assert!(!navigation_has_focus_ring(&h, "Settings"));
    h.key_press(egui::Key::Tab);
    h.run();
    assert!(navigation_has_focus_ring(&h, "General"));
    h.key_press(egui::Key::Tab);
    h.run();
    assert!(navigation_has_focus_ring(&h, "Cookies"));
    h.key_press(egui::Key::Space);
    h.run();
    assert_eq!(app.get_settings_category(), 1);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Storage")
        .click();
    h.run();
    assert_eq!(*refreshes.borrow(), 1);
    assert!(!navigation_has_focus_ring(&h, "Storage"));
    h.key_press(egui::Key::Tab);
    h.run();
    assert!(navigation_has_focus_ring(&h, "About"));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_settings_category(), 4);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Home")
        .focus();
    h.run();
    assert!(navigation_has_focus_ring(&h, "Home"));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(app.get_selected_tab(), 0);
    for (label, tab) in [
        ("History", 1),
        ("Library", 4),
        ("Support", 3),
        ("Settings", 2),
    ] {
        h.key_press(egui::Key::Tab);
        h.run();
        assert!(navigation_has_focus_ring(&h, label));
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(app.get_selected_tab(), tab);
        assert!(
            navigation_has_focus_ring(&h, label),
            "Focus disappeared after changing pages"
        );
    }
}

#[test]
fn palette_selection_text_has_aa_contrast_in_every_preset() {
    fn luminance(c: Color32) -> f32 {
        let linear = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }
    for scheme in schemes::SCHEMES {
        let contrast = 1.05 / (luminance(palette_color(scheme.id, false)) + 0.05);
        assert!(contrast >= 4.5, "{}: {contrast}", scheme.id);
        for dark in [false, true] {
            let colors = appearance::Palette::new(scheme.id, dark);
            let contrast = appearance::contrast(colors.accent, colors.on_accent);
            assert!(contrast >= 4.5, "{} dark={dark}: {contrast}", scheme.id);
            let overlay = appearance::contrast(colors.primary, Color32::WHITE);
            assert!(overlay >= 4.5, "{} overlay: {overlay}", scheme.id);
        }
    }
}

#[test]
#[ignore = "Requires an available graphics adapter; writes reviewable PNGs under target/egui-qa"]
fn render_ui_review_images() {
    let app = fixture();
    let mut h = harness(app.clone());
    let directory = std::path::Path::new("target/egui-qa");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["light", "dark"] {
        app.set_theme_mode(theme.into());
        for (width, height) in [(1024, 600), (1440, 900)] {
            h.set_size(Vec2::new(width as f32, height as f32));
            for tab in 0..4 {
                app.set_selected_tab(tab);
                h.run_steps(4);
                std::thread::sleep(std::time::Duration::from_millis(80));
                h.run_steps(3);
                h.render()
                    .unwrap()
                    .save(directory.join(format!("{theme}-{width}-{tab}.png")))
                    .unwrap();
            }
        }
        h.set_size(Vec2::new(1024.0, 600.0));
        app.set_selected_tab(0);
        app.set_media_items(Default::default());
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-empty-home.png")))
            .unwrap();
        let item = fixture().get_media_items().row_data(0).unwrap();
        let rows = (0..15)
            .map(|index| MediaItem {
                media_index: index,
                media_type: ["animated_gif", "video", "photo"][index as usize % 3].into(),
                preview_image: Image::default(),
                is_new: index % 5 == 0,
                status: if index == 4 {
                    "Saved".into()
                } else if index == 5 {
                    "Failed".into()
                } else {
                    "Ready".into()
                },
                ..item.clone()
            })
            .collect::<Vec<_>>();
        app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(rows)));
        let mut profile = crate::AccountProfile::default();
        profile.available = true;
        profile.display_name = "UI test fixture".into();
        profile.username = "fixture".into();
        profile.bio =
            "Profile layout fixture for comparison with the previous desktop interface.".into();
        profile.has_banner = true;
        profile.posts = "11".into();
        profile.following = "2".into();
        profile.followers = "92".into();
        profile.media = "15".into();
        app.set_account_profile(profile);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-populated-home.png")))
            .unwrap();
        app.set_selected_media_index(2);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-preview.png")))
            .unwrap();
        app.set_selected_media_index(-1);
        app.set_account_profile(Default::default());
    }
}
