use super::*;

fn jump_top_fixture_count(count: usize) -> AppWindow {
    let app = fixture();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..count)
            .map(|index| MediaItem {
                media_index: index as i32,
                title: format!("Scroll result {index}"),
                media_type: "photo".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    )));
    app
}

fn jump_top_fixture() -> AppWindow {
    jump_top_fixture_count(300)
}

fn scroll_thumb_top(h: &Harness<'_, Desktop>) -> f32 {
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    h.output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.fill.a() > 0
                    && rect.fill.a() < 255
                    && rect.rect.width() <= 6.1
                    && rect.rect.height() >= 8.0
                    && rect.rect.height() < bar.height()
                    && bar.contains(rect.rect.center()) =>
            {
                Some(rect.rect.top())
            }
            _ => None,
        })
        .expect("Expected the scroll thumb")
}

#[test]
fn jump_animation_moves_through_intermediate_positions_and_finishes_within_350ms() {
    for view in ["grid", "list"] {
        for count in [300, 3000] {
            let app = jump_top_fixture_count(count);
            app.set_results_view(view.into());
            let mut h = harness_at_frame_rate(app, 60.0);
            h.set_size(Vec2::new(1024.0, 600.0));
            h.run();
            h.state_mut().limit = count;
            h.run();
            let top_at_rest = scroll_thumb_top(&h);
            scroll_results_to_end(&mut h);
            let start = scroll_thumb_top(&h);
            h.get_by_label("Jump to top").click();
            h.step();
            assert!(
                (scroll_thumb_top(&h) - start).abs() < 1.0,
                "A click must start an animation, not teleport"
            );
            h.step();
            let started = h.ctx.input(|input| input.time);
            let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
            let mut previous = start;
            let mut intermediate = false;
            for _ in 0..21 {
                h.step();
                let top = scroll_thumb_top(&h);
                assert!(
                    top <= previous + 0.5,
                    "The jump must move toward the top continuously"
                );
                intermediate |= top < start - 5.0 && top > bar.top() + 5.0;
                previous = top;
            }
            assert!(intermediate);
            assert!(h.ctx.input(|input| input.time) - started <= 0.351);
            assert!(
                (scroll_thumb_top(&h) - top_at_rest).abs() < 1.0,
                "{view} {count}: expected {top_at_rest}, got {}; distance must not extend the duration",
                scroll_thumb_top(&h)
            );
            assert!(h.query_by_label("Jump to top").is_none());
            assert_eq!(h.state().limit, count);
        }
    }
}

#[test]
fn manual_wheel_and_scrollbar_input_cancel_the_jump_animation() {
    for drag in [false, true] {
        let app = jump_top_fixture_count(3000);
        let mut h = harness_at_frame_rate(app, 60.0);
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        h.state_mut().limit = 3000;
        h.run();
        scroll_results_to_end(&mut h);
        h.get_by_label("Jump to top").click();
        h.step();
        h.run_steps(7);
        if drag {
            let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
            let point = egui::pos2(bar.center().x, bar.top() + bar.height() * 0.7);
            h.drag_at(point);
            h.step();
            h.drop_at(point);
            h.step();
        } else {
            h.hover_at(egui::pos2(500.0, 300.0));
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -120.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            });
            h.step();
        }
        h.run_steps(35);
        let stopped = scroll_thumb_top(&h);
        let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
        assert!(
            stopped > bar.top() + 10.0,
            "Manual input must stop the animation before it reaches the top"
        );
        h.run_steps(10);
        assert!((scroll_thumb_top(&h) - stopped).abs() < 1.0);
        assert!(h.query_by_label("Jump to top").is_some());
    }
}

fn scroll_results_to_end(h: &mut Harness<'_, Desktop>) {
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    let bottom = egui::pos2(bar.center().x, bar.bottom() - 1.0);
    h.drag_at(bar.center());
    h.run_steps(1);
    h.hover_at(bottom);
    h.run_steps(3);
    h.drop_at(bottom);
    h.run();
}

#[test]
fn back_and_forward_preserve_results_selection_filters_and_scroll() {
    for view in ["grid", "list"] {
        for keyboard in [false, true] {
            let app = jump_top_fixture();
            app.set_results_view(view.into());
            app.set_results_sort(2);
            app.set_source_url("fixture-account".into());
            app.global::<MediaSelection>().invoke_toggle(3);
            let cleared = Rc::new(std::cell::Cell::new(0));
            app.on_clear_results({
                let calls = cleared.clone();
                move || calls.set(calls.get() + 1)
            });
            let items = app.get_media_items();
            let mut h = harness(app.clone());
            h.set_size(Vec2::new(1024.0, 600.0));
            h.run();
            h.state_mut().kind = "photo".into();
            h.state_mut().limit = 300;
            h.run();
            scroll_results_to_end(&mut h);
            let scroll = scroll_thumb_top(&h);
            assert!(
                h.get_by_label("Forward to results")
                    .accesskit_node()
                    .is_disabled()
            );
            for label in ["Back to Home", "Forward to results"] {
                if keyboard {
                    h.get_by_label(label).focus();
                    h.run();
                    h.key_press(egui::Key::Enter);
                } else {
                    h.get_by_label(label).click();
                }
                h.run();
                assert!(items.ptr_eq(&app.get_media_items()));
                assert_eq!(app.get_media_items().row_count(), 300);
                assert_eq!(
                    app.global::<MediaSelection>().get_indices().row_data(0),
                    Some(3)
                );
                assert_eq!(app.get_results_sort(), 2);
                assert_eq!(app.get_results_view(), view);
                assert_eq!(app.get_source_url(), "fixture-account");
                assert_eq!(h.state().kind, "photo");
                assert_eq!(h.state().limit, 300);
                assert_eq!(cleared.get(), 0);
                if label == "Back to Home" {
                    assert!(app.get_home_results_hidden());
                    assert!(h.query_by_label("Download selected (1)").is_none());
                    assert!(h.query_by_label("Jump to top").is_none());
                    assert!(
                        !h.get_by_label("Forward to results")
                            .accesskit_node()
                            .is_disabled()
                    );
                }
            }
            assert!(!app.get_home_results_hidden());
            assert!((scroll_thumb_top(&h) - scroll).abs() < 1.0);
            app.set_is_busy(true);
            h.run();
            assert!(
                !h.get_by_label("Back to Home")
                    .accesskit_node()
                    .is_disabled()
            );
        }
    }
}

#[test]
fn opening_history_or_starting_a_fetch_leaves_the_landing_view() {
    let app = fixture();
    let mut h = harness(app.clone());
    h.run();
    h.get_by_label("Back to Home").click();
    h.run();
    assert!(app.get_home_results_hidden());
    app.get_media_items().set_row_data(
        0,
        MediaItem {
            title: "Updated in background".into(),
            media_index: 42,
            media_type: "photo".into(),
            ..Default::default()
        },
    );
    h.run();
    assert!(app.get_home_results_hidden());
    app.invoke_open_history("fetch-fixture".into());
    h.run();
    assert!(!app.get_home_results_hidden());
    assert!(h.query_by_label("Updated in background").is_some());
    h.get_by_label("Back to Home").click();
    h.run();
    app.invoke_start_extraction();
    h.run();
    assert!(!app.get_home_results_hidden());
    h.get_by_label("Back to Home").click();
    h.run();
    app.invoke_open_batch_result(7);
    h.run();
    assert!(!app.get_home_results_hidden());
}

#[test]
#[ignore = "Renders Home and preserved results with back/forward navigation"]
fn render_home_navigation_review() {
    let app = original_home_fixture();
    let mut h = harness(app.clone());
    let directory = std::path::Path::new("target/home-navigation-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        for width in [900.0, 1024.0] {
            h.set_size(Vec2::new(width, 600.0));
            for hidden in [false, true] {
                app.set_home_results_hidden(hidden);
                h.remove_cursor();
                h.run_steps(4);
                if !hidden {
                    assert!(
                        h.get_by_label("Forward to results").rect().right()
                            < h.get_by_label("Search X/Twitter media URL").rect().left()
                    );
                }
                h.render()
                    .unwrap()
                    .save(directory.join(format!(
                        "{theme}-{}-{}.png",
                        width as u32,
                        if hidden { "home" } else { "results" }
                    )))
                    .unwrap();
                if !hidden {
                    let media = app.get_media_items();
                    app.set_is_busy(true);
                    app.set_is_extracting(true);
                    app.set_media_items(Default::default());
                    app.set_status("Running xtractor · page 1…".into());
                    h.run_steps(4);
                    h.render()
                        .unwrap()
                        .save(directory.join(format!("{theme}-{}-fetch-profile.png", width as u32)))
                        .unwrap();
                    app.set_is_extracting(false);
                    app.set_media_items(media);
                    app.set_status("Downloading media…".into());
                    app.set_progress(1.4);
                    app.set_progress_detail("8 / 576 files saved · 11.9 MB downloaded".into());
                    h.run_steps(4);
                    h.render()
                        .unwrap()
                        .save(directory.join(format!("{theme}-{}-download.png", width as u32)))
                        .unwrap();
                    app.set_is_busy(false);
                }
            }
        }
    }
}

#[test]
fn account_profile_is_visible_before_the_first_media_and_keeps_its_banner_space() {
    let app = fixture();
    let image = app.get_media_items().row_data(0).unwrap().preview_image;
    app.set_media_items(Default::default());
    app.set_is_busy(true);
    app.set_is_extracting(true);
    app.set_status("Running xtractor · page 2…".into());
    app.set_account_profile(crate::AccountProfile {
        available: true,
        username: "early_account".into(),
        display_name: "Early account".into(),
        bio: "Profile received before media".into(),
        avatar_url: "https://example.invalid/avatar.png".into(),
        banner_url: "https://example.invalid/banner.png".into(),
        ..Default::default()
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run_steps(4);
    let panel = h.get_by_label("Account profile").rect();
    assert!(panel.top() < 120.0);
    assert!(h.query_by_label("Early account").is_some());
    assert!(h.query_by_label("Download all").is_none());
    let mut profile = app.get_account_profile();
    profile.has_avatar = true;
    profile.avatar_image = image.clone();
    profile.has_banner = true;
    profile.banner_image = image;
    app.set_account_profile(profile);
    h.run_steps(4);
    assert_eq!(h.get_by_label("Account profile").rect(), panel);
    assert!(app.get_is_extracting());
    app.set_is_extracting(false);
    app.set_is_busy(false);
    h.run();
    assert!(h.query_by_label("Account profile").is_some());
    h.get_by_label("Back to Home").click();
    h.run();
    assert!(h.query_by_label("Account profile").is_none());
    h.get_by_label("Forward to results").click();
    h.run();
    assert!(h.query_by_label("Account profile").is_some());
}

#[test]
fn jump_to_top_returns_both_result_views_without_resetting_results_or_selection() {
    for view in ["grid", "list"] {
        for keyboard in [false, true] {
            let app = jump_top_fixture();
            app.set_results_view(view.into());
            app.set_results_sort(2);
            app.global::<MediaSelection>().invoke_toggle(3);
            let mut h = harness(app.clone());
            h.set_size(Vec2::new(1024.0, 600.0));
            h.run();
            assert!(h.query_by_label("Jump to top").is_none());
            h.state_mut().limit = 300;
            h.state_mut().kind = "photo".into();
            h.run();
            scroll_results_to_end(&mut h);
            assert!(h.state().visible_media.iter().any(|index| *index > 270));
            let jump = h.get_by_label("Jump to top").rect();
            assert_eq!(jump.size(), Vec2::splat(40.0));
            assert!(h.ctx.viewport_rect().contains_rect(jump));
            app.set_selected_media_index(0);
            h.run();
            assert!(h.query_by_label("Jump to top").is_none());
            h.get_by_label("Close preview").click();
            h.run();
            if keyboard {
                h.get_by_label("Jump to top").focus();
                h.run();
                h.key_press(egui::Key::Enter);
            } else {
                h.get_by_label("Jump to top").click();
            }
            h.run();
            h.run_steps(4);
            assert!(h.query_by_label("Jump to top").is_none());
            assert!(h.state().visible_media.contains(&0));
            assert!(h.state().visible_media.iter().all(|index| *index < 100));
            assert!(
                h.ctx.viewport_rect().contains_rect(
                    h.get_by_role_and_label(egui::accesskit::Role::Label, "#1")
                        .rect(),
                )
            );
            assert_eq!(h.state().limit, 300);
            assert_eq!(h.state().kind, "photo");
            assert_eq!(app.get_results_sort(), 2);
            assert_eq!(app.get_selected_media_index(), -1);
            assert_eq!(
                app.global::<MediaSelection>().get_indices().row_data(0),
                Some(3)
            );
        }
    }
}

#[test]
#[ignore = "Renders the result jump button in both views and themes"]
fn render_results_jump_top() {
    let app = jump_top_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.state_mut().limit = 300;
    let directory = std::path::Path::new("target/results-jump-top-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        for view in ["grid", "list"] {
            app.set_results_view(view.into());
            h.run();
            scroll_results_to_end(&mut h);
            h.remove_cursor();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-{view}.png")))
                .unwrap();
        }
    }
}

#[test]
fn media_numbers_follow_display_order_and_filters_without_changing_source_indices() {
    let app = fixture();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        MediaItem {
            media_index: 42,
            title: "Alpha".into(),
            media_type: "photo".into(),
            sort_key: "2026-10-11".into(),
            ..Default::default()
        },
        MediaItem {
            media_index: 7,
            title: "Beta".into(),
            media_type: "video".into(),
            sort_key: "2026-10-12".into(),
            ..Default::default()
        },
        MediaItem {
            media_index: 99,
            title: "Gamma".into(),
            media_type: "photo".into(),
            sort_key: "2026-10-13".into(),
            ..Default::default()
        },
    ])));
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    for view in ["grid", "list"] {
        app.set_results_view(view.into());
        app.set_results_sort(1);
        h.state_mut().kind = "all".into();
        h.run();
        for (number, title) in [(1, "Gamma"), (2, "Beta"), (3, "Alpha")] {
            let row = h
                .get_by_role_and_label(egui::accesskit::Role::Button, title)
                .rect();
            let number = h
                .get_by_role_and_label(egui::accesskit::Role::Label, &format!("#{number}"))
                .rect();
            assert!(row.contains_rect(number));
        }
        h.get_by_role_and_label(egui::accesskit::Role::Label, "#1")
            .click();
        h.run();
        assert_eq!(app.get_selected_media_index(), 99);
        h.get_by_label("Close preview").click();
        h.run();
        app.set_results_sort(2);
        h.run();
        h.get_by_label("Image  2").click();
        h.run();
        for (number, title) in [(1, "Alpha"), (2, "Gamma")] {
            let row = h
                .get_by_role_and_label(egui::accesskit::Role::Button, title)
                .rect();
            let number = h
                .get_by_role_and_label(egui::accesskit::Role::Label, &format!("#{number}"))
                .rect();
            assert!(row.contains_rect(number));
        }
        assert!(
            h.query_by_role_and_label(egui::accesskit::Role::Label, "#3")
                .is_none()
        );
    }
}

#[test]
fn five_thousand_results_render_and_request_thumbnails_only_near_the_viewport() {
    let app = fixture();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..5000)
            .map(|index| MediaItem {
                media_index: index,
                title: format!("Large result {index}"),
                media_type: "photo".into(),
                thumbnail_url: format!("https://example.invalid/{index}.jpg"),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    )));
    let requested = Rc::new(RefCell::new(Vec::new()));
    app.on_load_thumbnail({
        let requested = requested.clone();
        move |index| requested.borrow_mut().push(index)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let initial_limit = h.state().limit;
    assert!((30..=60).contains(&initial_limit));
    assert!(requested.borrow().iter().all(|index| *index < 100));
    assert!(h.query_by_label("Load more").is_none());
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    h.drag_at(bar.center());
    h.run_steps(1);
    h.hover_at(egui::pos2(bar.center().x, bar.bottom() - 1.0));
    h.run_steps(3);
    h.drop_at(egui::pos2(bar.center().x, bar.bottom() - 1.0));
    h.run();
    assert!(h.state().limit > initial_limit && h.state().limit < 150);
    h.state_mut().limit = 5000;
    h.run();
    assert!(h.state().visible_media.len() < 100);
    for (view, maximum) in [("grid", 100), ("list", 15)] {
        app.set_results_view(view.into());
        h.run();
        let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
        h.drag_at(bar.center());
        h.run_steps(1);
        h.hover_at(egui::pos2(bar.center().x, bar.bottom() - 1.0));
        h.run_steps(3);
        h.drop_at(egui::pos2(bar.center().x, bar.bottom() - 1.0));
        h.run_steps(3);
        eprintln!(
            "5000 results, {view}: {} rendered items at the bottom",
            h.state().visible_media.len()
        );
        assert!(h.state().visible_media.len() < maximum);
        assert!(h.state().visible_media.iter().any(|index| *index > 4900));
        assert!(
            h.query_by_role_and_label(egui::accesskit::Role::Label, "#5000")
                .is_some()
        );
    }
    let old_source = app.get_media_items();
    app.set_media_items(Default::default());
    h.run();
    assert!(h.state().result_view.source_changed(&old_source));
}

#[test]
fn results_load_more_automatically_in_both_views_and_stop_at_the_filtered_total() {
    for view in ["grid", "list"] {
        let app = fixture();
        app.set_results_view(view.into());
        app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
            (0..95)
                .map(|index| MediaItem {
                    media_index: index,
                    title: format!("Auto result {index}"),
                    media_type: if index < 8 { "video" } else { "photo" }.into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        )));
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run();
        assert!((30..=60).contains(&h.state().limit));
        for _ in 0..8 {
            let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
            h.drag_at(bar.center());
            h.run_steps(1);
            let bottom = egui::pos2(bar.center().x, bar.bottom() - 1.0);
            h.hover_at(bottom);
            h.run_steps(3);
            h.drop_at(bottom);
            h.run();
        }
        assert_eq!(
            h.state().limit,
            95,
            "{view} must reveal all results by scrolling"
        );
        assert!(h.state().visible_media.contains(&94));
        assert!(h.query_by_label("Load more").is_none());
        h.state_mut().kind = "video".into();
        h.state_mut().limit = 30;
        h.run();
        assert_eq!(
            h.state().limit,
            30,
            "A completed filter must not keep requesting repaints"
        );
        assert!(h.state().visible_media.iter().all(|index| *index < 8));
        h.state_mut().kind = "text".into();
        h.run();
        assert_eq!(h.state().limit, 30);
        assert!(h.query_by_label("Nothing in this filter.").is_some());
    }
}

#[test]
fn batch_queue_centers_rows_preserves_actions_and_hides_empty_results() {
    let app = fixture();
    app.set_media_items(Default::default());
    app.set_batch_mode(true);
    app.set_batch_pending_count(1);
    app.set_batch_fetch_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::BatchFetchItem {
            index: 7,
            source: "https://x.com/first".into(),
            status: "Completed".into(),
            history_id: "first-result".into(),
            media_count: 20,
            ..Default::default()
        },
        crate::BatchFetchItem {
            index: 9,
            source: "https://x.com/second".into(),
            status: "Queued".into(),
            ..Default::default()
        },
    ])));
    let opened = Rc::new(RefCell::new(Vec::new()));
    app.on_open_batch_result({
        let opened = opened.clone();
        move |index| opened.borrow_mut().push(index)
    });
    let started = Rc::new(std::cell::Cell::new(0));
    app.on_start_batch({
        let started = started.clone();
        move || started.set(started.get() + 1)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let queue = h.get_by_label("Batch queue").rect();
    assert_eq!(queue.left(), 130.0);
    assert_eq!(queue.size(), Vec2::new(820.0, 202.0));
    assert!(queue.top() > h.get_by_label("Edit accounts").rect().bottom());
    assert!(h.query_by_label("Back to Home").is_none());
    assert!(h.query_by_label("Download all").is_none());
    let buttons = h.get_all_by_label("View results").collect::<Vec<_>>();
    assert_eq!(buttons.len(), 2);
    assert!(!buttons[0].accesskit_node().is_disabled());
    assert!(buttons[1].accesskit_node().is_disabled());
    buttons[0].click();
    h.run();
    assert_eq!(*opened.borrow(), [7]);
    h.get_by_label("Fetch pending").click();
    h.run();
    assert_eq!(started.get(), 1);
    app.set_is_busy(true);
    h.run_steps(4);
    for label in ["Fetch pending", "Reset queue", "Clear queue"] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
}

#[test]
fn preview_keeps_post_details_beside_the_stage_and_closes_on_backdrop() {
    let app = fixture();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        MediaItem {
            media_index: 5,
            media_type: "text".into(),
            tweet_text: "こんにちは café\nSecond line.".into(),
            ..Default::default()
        },
    ])));
    app.set_selected_media_index(5);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert_eq!(h.get_by_label("Media preview").rect().height(), 552.0);
    assert!(h.get_by_label("View").accesskit_node().is_disabled());
    h.event(egui::Event::PointerMoved(egui::pos2(1000.0, 590.0)));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: egui::pos2(1000.0, 590.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run();
    assert_eq!(app.get_selected_media_index(), -1);
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "photo".into();
    item.tweet_text = "Sample post used to check the desktop result preview.".into();
    item.alt_text = "A sample image description for checking wrapped preview details.".into();
    item.preview_date = "Aug 31, 2025 · 17:52 UTC".into();
    item.source = "Twitter Web App".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    app.set_selected_media_index(5);
    h.run();
    let panel = h.get_by_label("Media preview").rect();
    assert_eq!(panel.height(), 552.0);
    let date = h.get_by_label("Aug 31, 2025 · 17:52 UTC").rect();
    assert!(date.bottom() <= panel.bottom() - 62.0);
}

fn batch_page_fixture() -> AppWindow {
    let app = fixture();
    app.set_batch_mode(true);
    app.set_batch_pending_count(2);
    app.set_batch_sources_text("first_account\nsecond_account\nthird_account".into());
    app.set_batch_fetch_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::BatchFetchItem {
            index: 0,
            source: "https://x.com/first_account".into(),
            status: "Completed".into(),
            history_id: "first".into(),
            media_count: 579,
            ..Default::default()
        },
        crate::BatchFetchItem {
            index: 1,
            source: "https://x.com/second_account".into(),
            status: "Queued".into(),
            ..Default::default()
        },
        crate::BatchFetchItem {
            index: 2,
            source: "https://x.com/third_account".into(),
            status: "Queued".into(),
            ..Default::default()
        },
    ])));
    app.on_open_batch_result({
        let weak = app.as_weak();
        move |index| {
            let app = weak.upgrade().unwrap();
            app.set_batch_result_index(index);
            app.set_batch_result_open(true);
            app.set_account_profile(crate::AccountProfile {
                available: true,
                display_name: "First account".into(),
                username: "first_account".into(),
                ..Default::default()
            });
            app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
                MediaItem {
                    media_index: 0,
                    title: "Viewed batch media".into(),
                    media_type: "photo".into(),
                    ..Default::default()
                },
            ])));
            app.set_has_output(true);
        }
    });
    app
}

#[test]
fn running_batch_opens_completed_results_and_back_keeps_the_queue() {
    let app = batch_page_fixture();
    app.set_batch_running(true);
    app.set_is_busy(true);
    app.set_is_extracting(true);
    let single = app.get_media_items();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run_steps(4);
    assert!(single.ptr_eq(&app.get_media_items()));
    assert!(h.query_by_label("Search X/Twitter media URL").is_none());
    assert!(h.query_by_label("Download all").is_none());
    let results = h.get_all_by_label("View results").collect::<Vec<_>>();
    assert!(!results[0].accesskit_node().is_disabled());
    assert!(results[1].accesskit_node().is_disabled());
    results[0].click();
    h.run_steps(4);
    assert!(app.get_batch_result_open());
    assert!(h.query_by_label("Batch queue").is_none());
    assert!(h.query_by_label("Viewed batch media").is_some());
    let viewed = app.get_media_items();
    let rows = app.get_batch_fetch_items();
    let mut row = rows.row_data(1).unwrap();
    row.status = "Fetching".into();
    row.media_count = 1117;
    rows.set_row_data(1, row);
    h.run_steps(4);
    assert!(viewed.ptr_eq(&app.get_media_items()));
    h.get_by_label("Back to batch").click();
    h.run_steps(4);
    assert!(!app.get_batch_result_open());
    assert!(app.get_batch_mode());
    assert_eq!(app.get_batch_fetch_items().row_count(), 3);
    assert_eq!(
        app.get_batch_fetch_items().row_data(1).unwrap().media_count,
        1117
    );
    assert!(h.query_by_label("Batch queue").is_some());
    assert!(h.query_by_label("Viewed batch media").is_none());
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text().contains("Fetching · 1117 media"))));
}

#[test]
#[ignore = "Renders the batch queue, editor, running state and result navigation"]
fn render_batch_page_review() {
    let directory = std::path::Path::new("target/batch-page-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        for width in [900.0, 1280.0] {
            let app = batch_page_fixture();
            app.set_theme_mode(theme.into());
            let mut h = harness(app.clone());
            h.set_size(Vec2::new(width, 600.0));
            for state in ["queue", "editing", "running", "result", "returned"] {
                match state {
                    "editing" => h.state_mut().batch_list_editing = true,
                    "running" => {
                        h.state_mut().batch_list_editing = false;
                        let rows = app.get_batch_fetch_items();
                        let mut row = rows.row_data(1).unwrap();
                        row.status = "Fetching".into();
                        row.media_count = 1117;
                        rows.set_row_data(1, row);
                        app.set_status("Running xtractor · page 12…".into());
                        app.set_batch_running(true);
                        app.set_is_busy(true);
                        app.set_is_extracting(true);
                    }
                    "result" => app.invoke_open_batch_result(0),
                    "returned" => app.set_batch_result_open(false),
                    _ => {}
                }
                h.remove_cursor();
                h.run_steps(4);
                h.render()
                    .unwrap()
                    .save(directory.join(format!("{theme}-{}-{state}.png", width as u32)))
                    .unwrap();
                if state == "queue" {
                    h.get_by_role_and_label(egui::accesskit::Role::Button, "Single")
                        .hover();
                    h.run_steps(4);
                    h.render()
                        .unwrap()
                        .save(directory.join(format!("{theme}-{}-mode-hover.png", width as u32)))
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn gif_loops_and_video_audio_controls_follow_playback_readiness() {
    let app = fixture();
    let mut item = app.get_media_items().row_data(0).unwrap();
    item.media_type = "animated_gif".into();
    item.tweet_text = "GIF preview".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item.clone(),
    ])));
    app.set_selected_media_index(42);
    let mut h = harness(app.clone());
    h.run();
    assert!(h.query_by_label("Looping GIF").is_some());
    assert!(h.query_by_label("Volume").is_none());
    assert!(h.get_by_label("Play").accesskit_node().is_disabled());
    item.media_type = "video".into();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        item,
    ])));
    let playback = app.global::<PreviewPlayback>();
    playback.set_ready(true);
    let muted = Rc::new(RefCell::new(Vec::new()));
    playback.on_mute({
        let muted = muted.clone();
        move |value| muted.borrow_mut().push(value)
    });
    h.run();
    assert!(h.query_by_label("Looping GIF").is_none());
    assert!(!h.get_by_label("Volume").accesskit_node().is_disabled());
    h.get_by_label("Mute").click();
    h.run();
    assert_eq!(*muted.borrow(), [true]);
}
