use super::*;

fn sidepanel_fixture() -> AppWindow {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    let mut pixels = ui_runtime::SharedPixelBuffer::new(230, 408);
    for (index, pixel) in pixels.make_mut_bytes().chunks_exact_mut(4).enumerate() {
        let grid = index % 230 % 23 < 2 || index / 230 % 34 < 2;
        pixel.copy_from_slice(if grid {
            &[215, 225, 215, 255]
        } else {
            &[45, 100, 80, 255]
        });
    }
    item.preview_image = Image::from_rgba8(pixels);
    item.author_avatar_image = fixture()
        .get_media_items()
        .row_data(0)
        .unwrap()
        .preview_image;
    item.author_name = "Preview sample with a long author name".into();
    item.author_handle = "preview_sample".into();
    item.author_verified = true;
    item.file_extension = "JPG".into();
    item.tweet_text = "A portrait preview with post details beside the image.\nhttps://example.com/media?source=preview".into();
    item.alt_text = "A generated pattern for layout review.".into();
    item.preview_date = "Sep 30, 2026 · 22:32 UTC".into();
    item.source = "X for Android".into();
    item.dimensions = "2304 × 4080".into();
    item.replies = "12.3K".into();
    item.reposts = "45.6K".into();
    item.likes = "789K".into();
    item.views = "12.3M".into();
    item.quotes = "45.6K".into();
    item.bookmarks = "78.9K".into();
    item.url = "https://example.com/media.jpg".into();
    item.tweet_url = "https://x.com/example/status/1".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    app
}

#[test]
fn preview_sidepanel_scrolls_independently_keeps_actions_visible_and_can_collapse() {
    let app = sidepanel_fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.tweet_text = "Long post details with several lines.\n".repeat(40);
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    let downloads = Rc::new(RefCell::new(Vec::new()));
    app.on_download_selected_media({
        let downloads = downloads.clone();
        move |index| downloads.borrow_mut().push(index)
    });
    let links = Rc::new(RefCell::new(Vec::new()));
    app.on_open_link({
        let links = links.clone();
        move |url| links.borrow_mut().push(url)
    });
    app.on_open_post({
        let links = links.clone();
        move |url| links.borrow_mut().push(url)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let panel = h.get_by_label("Media preview").rect();
    let stage = h.get_by_label("Media canvas").rect();
    assert_eq!(stage.height(), panel.height());
    assert!(
        stage.height() > 500.0,
        "Portrait media must get the full viewer height"
    );
    assert_eq!(panel.right() - stage.right(), 320.0);
    let download = h.get_by_label("Download JPG").rect();
    assert!(download.left() > stage.right() && panel.contains_rect(download));
    let sidebar_bar = h
        .get_all_by_role(egui::accesskit::Role::ScrollBar)
        .find(|bar| bar.rect().left() > stage.right() && bar.rect().right() <= panel.right())
        .unwrap()
        .rect();
    h.drag_at(sidebar_bar.center());
    h.run_steps(1);
    h.hover_at(sidebar_bar.center_bottom() - Vec2::new(0.0, 1.0));
    h.run_steps(3);
    h.drop_at(sidebar_bar.center_bottom() - Vec2::new(0.0, 1.0));
    h.run();
    assert_eq!(h.get_by_label("Media canvas").rect(), stage);
    assert_eq!(h.get_by_label("Download JPG").rect(), download);
    h.get_by_label("Download JPG").click();
    h.run();
    assert_eq!(*downloads.borrow(), [42]);
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Open post").click();
    h.run();
    assert_eq!(links.borrow().len(), 2);
    h.get_by_label("Hide details").focus();
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(h.state().preview_canvas.info_hidden);
    assert_eq!(h.get_by_label("Media canvas").rect(), panel);
    assert!(h.query_by_label("Download JPG").is_none());
    h.get_by_label("Enter full screen").click();
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.state().preview_canvas.info_hidden);
    h.get_by_label("Show details").click();
    h.run();
    assert_eq!(h.get_by_label("Media canvas").rect(), stage);
    h.get_by_label("Close preview").click();
    h.run();
    app.set_selected_media_index(42);
    h.run();
    assert!(h.query_by_label("Hide details").is_some());
}

#[test]
#[ignore = "Renders the split viewer and collapsed info panel at desktop sizes"]
fn render_preview_sidepanel_review() {
    let app = sidepanel_fixture();
    let mut h = harness(app.clone());
    let directory = std::path::Path::new("target/preview-sidepanel-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        for size in [
            Vec2::new(1024.0, 600.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(1920.0, 1080.0),
        ] {
            h.set_size(size);
            h.remove_cursor();
            h.run_steps(5);
            if theme == "light" && size.x == 1024.0 {
                let readout = h.get_by_label("Media zoom").rect();
                let painted = h
                    .output()
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            (text.galley.text() == "100%").then_some((shape.clip_rect, text.pos))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                assert!(
                    !painted.is_empty(),
                    "Zoom readout must stay visible after resizing: {readout:?}"
                );
            }
            let rendered = h.render().unwrap();
            let zoom = h.get_by_label("Media zoom").rect();
            let bright = (zoom.top().ceil() as u32..zoom.bottom().floor() as u32)
                .flat_map(|y| {
                    (zoom.left().ceil() as u32..zoom.right().floor() as u32).map(move |x| (x, y))
                })
                .filter(|(x, y)| {
                    rendered.get_pixel(*x, *y).0[..3]
                        .iter()
                        .all(|channel| *channel > 220)
                })
                .count();
            eprintln!("{theme} {} zoom pixels: {bright}", size.x);
            assert!(bright > 2, "Zoom toolbar must be painted above the media");
            rendered
                .save(directory.join(format!("{theme}-{}-portrait.png", size.x)))
                .unwrap();
            h.get_by_label("Hide details").click();
            h.run_steps(5);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-{}-collapsed.png", size.x)))
                .unwrap();
            h.get_by_label("Show details").click();
            h.run_steps(5);
        }
    }
}

#[test]
fn preview_sidepanel_actions_fit_in_every_supported_language() {
    let app = sidepanel_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    for language in ["en", "id", "de", "es", "fr", "pt", "tr", "zh_Hans"] {
        app.global::<I18n>().set_language(language.into());
        h.run();
        let panel = h
            .get_by_label(&crate::i18n::translate(language, "Media preview"))
            .rect();
        let view = h
            .get_by_label(&crate::i18n::translate(language, "View"))
            .rect();
        let post = h
            .get_by_label(&crate::i18n::translate(language, "Open post"))
            .rect();
        assert_eq!(
            view.size(),
            post.size(),
            "Preview actions must have matching fixed sizes"
        );
        assert_eq!(view.top(), post.top());
        for key in ["Download {}", "View", "Open post"] {
            let caption = crate::i18n::translate(language, key).replace("{}", "JPG");
            let button = h.get_by_label(&caption).rect();
            assert!(panel.contains_rect(button));
            for shape in &h.output().shapes {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == caption
                {
                    let bounds = egui::Rect::from_min_size(text.pos, text.galley.size());
                    assert!(
                        button.contains_rect(bounds),
                        "{language}: {caption} must fit inside its button"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "Renders linked text on the black preview stage in both themes"]
fn render_text_preview_links_in_both_themes() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "text".into();
    item.author_name = "Preview sample".into();
    item.author_handle = "example".into();
    item.tweet_text = "A post with a web link:\nhttps://example.com/a-long-address/that-remains-clickable/when-wrapped?source=preview\n\nMore post text.".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::path::Path::new("target/preview-viewer-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-text-links.png")))
            .unwrap();
        h.get_all_by_role(egui::accesskit::Role::Link)
            .next()
            .unwrap()
            .hover();
        h.run_steps(1);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-text-links-hover.png")))
            .unwrap();
        h.hover_at(egui::pos2(0.0, 0.0));
    }
}

#[test]
fn preview_uses_the_posts_author_avatar_instead_of_the_fetched_accounts_avatar() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.author_handle = "post-author".into();
    item.author_avatar_image = Image::from_rgba8(ui_runtime::SharedPixelBuffer::new(40, 40));
    let author_image = item.author_avatar_image.id;
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    let mut profile = app.get_account_profile();
    profile.username = "fetched-account".into();
    profile.has_avatar = true;
    profile.avatar_image = Image::from_rgba8(ui_runtime::SharedPixelBuffer::new(40, 40));
    let wrong_image = profile.avatar_image.id;
    app.set_account_profile(profile);
    app.set_selected_media_index(42);
    let mut h = harness(app);
    h.run();
    assert!(h.state().textures.contains_key(&author_image));
    assert!(!h.state().textures.contains_key(&wrong_image));
}

#[test]
fn wrapped_links_open_the_browser_with_mouse_and_keyboard_and_use_the_accent_color() {
    let address = format!("https://example.com/{}", "long-path/".repeat(6));
    let text = format!("写真はこちら\n({address}), normal text.");
    let mut h = Harness::builder()
        .with_size([280.0, 260.0])
        .with_step_dt(0.25)
        .build_ui_state(
            |ui, desktop: &mut Desktop| {
                if desktop.frame == 0 {
                    setup(ui.ctx());
                    desktop.frame = 1;
                }
                desktop.theme(ui.ctx());
                desktop.linked_text(ui, &text, 14.04, desktop.colors(ui).heading);
            },
            Desktop::new(fixture()),
        );
    h.run();
    assert!(h.output().shapes.iter().all(|shape| !matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.underline.width > 0.0))));
    let link = h.get_by_role_and_label(egui::accesskit::Role::Link, &address);
    assert!(link.rect().width() < 280.0);
    let wrapped_row = egui::pos2(25.0, link.rect().bottom() + link.rect().height() / 2.0);
    link.click();
    h.run_steps(1);
    assert!(
        h.output().platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == address)
        )
    );
    h.hover_at(wrapped_row);
    h.run_steps(1);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Vec(highlights) if highlights.len() > 1 && highlights.iter().all(|shape| matches!(shape,
            egui::Shape::Rect(rect) if rect.fill.a() > 0 && rect.fill.a() < 255)))),
        "Hovering a wrapped URL must shade all its fragments");
    h.hover_at(egui::pos2(275.0, 250.0));
    h.run_steps(1);
    assert!(h.output().shapes.iter().all(|shape| !matches!(&shape.shape,
        egui::Shape::Vec(highlights) if !highlights.is_empty())));
    h.hover_at(wrapped_row);
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: wrapped_row,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: wrapped_row,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    assert!(
        h.output().platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == address)
        ),
        "The continuation of a wrapped URL must open the same address"
    );
    h.get_by_role_and_label(egui::accesskit::Role::Link, &address)
        .focus();
    h.run();
    h.key_down(egui::Key::Enter);
    h.run_steps(1);
    assert!(
        h.output().platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::OpenUrl(url) if url.url == address)
        )
    );
    h.key_up(egui::Key::Enter);
    h.run();
    let accent = h
        .state()
        .colors(&egui::Ui::new(
            h.ctx.clone(),
            egui::Id::new("test-colors"),
            egui::UiBuilder::new(),
        ))
        .accent;
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.job.sections.iter().any(|section| section.format.color == accent && section.format.underline.width == 0.0))));
}

#[test]
fn video_texture_is_separate_from_posters_and_released_when_preview_closes() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "video".into();
    let poster_id = item.preview_image.id;
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    let playback = app.global::<PreviewPlayback>();
    let frame = std::sync::Arc::new(egui::ColorImage::filled([160, 96], Color32::RED));
    playback.set_frame(Some(frame.clone()));
    playback.set_ready(true);
    let mut h = harness(app.clone());
    h.run();
    let ctx = h.ctx.clone();
    let id = h.state_mut().preview_texture.update(&ctx, &frame).id();
    assert!(h.state().textures.contains_key(&poster_id));
    for color in [Color32::GREEN, Color32::BLUE] {
        let frame = std::sync::Arc::new(egui::ColorImage::filled([160, 96], color));
        playback.set_frame(Some(frame.clone()));
        h.run_steps(1);
        assert_eq!(h.state_mut().preview_texture.update(&ctx, &frame).id(), id);
    }
    h.get_by_label("Enter full screen").click();
    h.run();
    h.get_by_label("Zoom in").click();
    h.run();
    let frame = playback.get_frame().unwrap();
    assert_eq!(h.state_mut().preview_texture.update(&ctx, &frame).id(), id);
    h.get_by_label("Close preview").click();
    h.run_steps(1);
    assert_eq!(app.get_selected_media_index(), -1);
    assert!(ctx.tex_manager().read().meta(id).is_none());
}

#[test]
#[ignore = "Exercises partial video uploads and resolution changes on the GPU"]
fn render_video_texture_updates() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "video".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    let playback = app.global::<PreviewPlayback>();
    playback.set_ready(true);
    let mut h = harness(app);
    for (size, color) in [
        ([960, 540], Color32::RED),
        ([960, 540], Color32::GREEN),
        ([540, 960], Color32::BLUE),
    ] {
        playback.set_frame(Some(std::sync::Arc::new(egui::ColorImage::filled(
            size, color,
        ))));
        h.run_steps(5);
        let center = h.get_by_label("Media canvas").rect().center();
        let image = h.render().unwrap();
        let pixel = image.get_pixel(center.x as u32, center.y as u32);
        for channel in 0..3 {
            assert!(
                (pixel[channel] as i16 - color[channel] as i16).abs() < 3,
                "The GPU must show the current video frame: {pixel:?}, expected {color:?}"
            );
        }
    }
}

#[test]
#[ignore = "Renders normal, zoomed and fullscreen preview states"]
fn render_preview_viewer_review() {
    let app = fixture();
    let mut pixels = ui_runtime::SharedPixelBuffer::new(1600, 900);
    for (index, pixel) in pixels.make_mut_bytes().chunks_exact_mut(4).enumerate() {
        let (x, y) = (index % 1600, index / 1600);
        let line = x % 100 < 3 || y % 100 < 3;
        let color = if line {
            [225, 230, 240, 255]
        } else if x < 800 {
            [55, 100, 145, 255]
        } else {
            [165, 110, 55, 255]
        };
        pixel.copy_from_slice(&color);
    }
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.preview_image = Image::from_rgba8(pixels);
    item.author_name = "Preview sample".into();
    item.author_handle = "example".into();
    item.author_avatar_image = fixture()
        .get_media_items()
        .row_data(0)
        .unwrap()
        .preview_image;
    item.tweet_text = "Preview sample\nhttps://example.com/media?source=preview".into();
    item.file_extension = "JPG".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1280.0, 720.0));
    let directory = std::path::Path::new("target/preview-viewer-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-normal.png")))
            .unwrap();
        h.state_mut().preview_canvas.zoom = 2.0;
        h.state_mut().preview_canvas.pan = Vec2::new(50.0, 60.0);
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-zoomed.png")))
            .unwrap();
        h.get_by_label("Enter full screen").click();
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-fullscreen.png")))
            .unwrap();
        h.state_mut().preview_canvas.zoom = 2.0;
        h.state_mut().preview_canvas.pan = Vec2::new(120.0, 80.0);
        h.run_steps(5);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-fullscreen-zoomed.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run_steps(5);
    }
}

fn fullscreen_requested(h: &Harness<'_, Desktop>, enabled: bool) -> bool {
    h.output().viewport_output.values().any(|viewport| viewport.commands.iter().any(|command| matches!(command, egui::ViewportCommand::Fullscreen(value) if *value == enabled)))
}

#[test]
fn preview_shortcuts_do_not_capture_input_from_a_dialog_above_it() {
    let app = fixture();
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.run();
    app.set_startup_update_version("99.0.0".into());
    h.run();
    h.key_press(egui::Key::F);
    h.run();
    assert!(!h.state().preview_canvas.fullscreen);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(app.get_startup_update_version().is_empty());
    assert_eq!(app.get_selected_media_index(), 42);
    h.key_press(egui::Key::F);
    h.run();
    assert!(h.state().preview_canvas.fullscreen);
}

#[test]
fn double_click_toggles_between_zoomed_and_fitted_media() {
    let app = fixture();
    app.set_selected_media_index(42);
    let mut h = harness_at_frame_rate(app, 60.0);
    h.run();
    for expected in [2.0, 1.0] {
        h.get_by_label("Media canvas").click();
        h.run();
        h.get_by_label("Media canvas").click();
        h.run();
        assert_eq!(h.state().preview_canvas.zoom, expected);
        h.run_steps(40);
    }
}

#[test]
fn preview_zoom_pan_fullscreen_and_escape_keep_the_normal_preview() {
    let app = fixture();
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let normal = h.get_by_label("Media preview").rect();
    h.get_by_label("Zoom in").click();
    h.run();
    assert_eq!(h.state().preview_canvas.zoom, 1.25);
    h.key_press(egui::Key::Plus);
    h.run();
    assert_eq!(h.state().preview_canvas.zoom, 1.5625);
    let stage = h.get_by_label("Media canvas").rect();
    h.drag_at(stage.center());
    h.run_steps(1);
    h.hover_at(stage.center() + Vec2::new(0.0, 60.0));
    h.run_steps(2);
    h.drop_at(stage.center() + Vec2::new(0.0, 60.0));
    h.run_steps(2);
    assert!(h.state().preview_canvas.pan.y > 1.0);
    let before_wheel = h.state().preview_canvas.zoom;
    h.get_by_label("Media canvas").hover();
    h.run_steps(1);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, 120.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    h.run_steps(4);
    assert!(h.state().preview_canvas.zoom > before_wheel);
    h.get_by_label("Fit to screen").click();
    h.run();
    assert_eq!(h.state().preview_canvas.zoom, 1.0);
    assert_eq!(h.state().preview_canvas.pan, Vec2::ZERO);
    h.get_by_label("Enter full screen").click();
    h.run_steps(1);
    assert!(fullscreen_requested(&h, true));
    h.run();
    assert_eq!(
        h.get_by_label("Media preview").rect(),
        h.ctx.viewport_rect()
    );
    assert!(h.query_by_label("Open post").is_none());
    h.key_press(egui::Key::Minus);
    h.run();
    assert_eq!(h.state().preview_canvas.zoom, 0.8);
    h.key_press(egui::Key::Num0);
    h.run();
    assert_eq!(h.state().preview_canvas.zoom, 1.0);
    h.key_down(egui::Key::Escape);
    h.run_steps(1);
    assert!(fullscreen_requested(&h, false));
    h.key_up(egui::Key::Escape);
    h.run();
    assert_eq!(app.get_selected_media_index(), 42);
    assert_eq!(h.get_by_label("Media preview").rect(), normal);
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(app.get_selected_media_index(), -1);
}

#[test]
fn fullscreen_video_and_gif_keep_playback_and_reset_zoom_when_navigating() {
    for kind in ["video", "animated_gif"] {
        let app = fixture();
        let mut first = app.get_media_items().row_data(0).unwrap();
        first.media_type = kind.into();
        let mut second = first.clone();
        second.media_index = 84;
        app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
            first, second,
        ])));
        app.set_selected_media_index(42);
        let playback = app.global::<PreviewPlayback>();
        playback.set_ready(true);
        playback.set_playing(true);
        playback.set_position(12.0);
        playback.set_duration(60.0);
        playback.set_can_seek(true);
        playback.on_seek({
            let playback = playback.clone();
            move |value| playback.set_position(value)
        });
        playback.on_toggle({
            let playback = playback.clone();
            move || playback.set_playing(!playback.get_playing())
        });
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        h.key_press(egui::Key::F);
        h.run();
        assert!(h.state().preview_canvas.fullscreen);
        h.get_by_label("Zoom in").click();
        h.run();
        assert_eq!(h.state().preview_canvas.zoom, 1.25);
        assert_eq!(playback.get_position(), 12.0);
        h.get_by_label("Playback position").focus();
        h.run();
        h.key_press(egui::Key::ArrowRight);
        h.run();
        h.key_press(egui::Key::ArrowRight);
        h.run();
        assert!((playback.get_position() - 12.2).abs() < 0.001);
        assert_eq!(app.get_selected_media_index(), 42);
        h.get_by_label("Pause").click();
        h.run();
        assert!(!playback.get_playing());
        h.get_by_label("Next").click();
        h.run();
        assert_eq!(app.get_selected_media_index(), 84);
        assert_eq!(h.state().preview_canvas.zoom, 1.0);
        assert!(h.state().preview_canvas.fullscreen);
        h.key_press(egui::Key::ArrowLeft);
        h.run();
        assert_eq!(app.get_selected_media_index(), 42);
        h.key_press(egui::Key::F11);
        h.run();
        assert!(!h.state().preview_canvas.fullscreen);
        h.key_press(egui::Key::F11);
        h.run();
        h.get_by_label("Close preview").click();
        h.run();
        assert_eq!(app.get_selected_media_index(), -1);
        assert!(!h.state().preview_canvas.fullscreen);
    }
}

#[test]
fn unavailable_media_disables_zoom_and_text_preview_can_still_leave_fullscreen() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.preview_image = Image::default();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item.clone(),
    ])));
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.run();
    assert!(h.get_by_label("Zoom in").accesskit_node().is_disabled());
    item.media_type = "text".into();
    item.tweet_text = "A text post".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    h.run();
    assert!(h.query_by_label("Zoom in").is_none());
    h.get_by_label("Enter full screen").click();
    h.run();
    assert!(h.state().preview_canvas.fullscreen);
    h.get_by_label("Exit full screen").click();
    h.run();
    assert!(!h.state().preview_canvas.fullscreen);
    app.set_selected_media_index(-1);
    h.run();
    assert!(!h.state().preview_canvas.fullscreen);
}
