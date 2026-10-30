use super::*;

fn library_fixture() -> AppWindow {
    let app = fixture();
    let image = app.get_media_items().row_data(0).unwrap().preview_image;
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        ["Saved", "Missing", "Ready"]
            .into_iter()
            .enumerate()
            .map(|(index, status)| MediaItem {
                title: format!("Library media {index}"),
                author_name: "Demo".into(),
                author_handle: "demo".into(),
                media_type: "photo".into(),
                status: status.into(),
                media_index: index as i32,
                preview_image: image.clone(),
                detail_image: image.clone(),
                file_extension: "JPG".into(),
                saved_path: if status == "Ready" {
                    String::new()
                } else {
                    format!("H:/Downloads/demo/{index}.jpg")
                },
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    )));
    app.update_library(|data| {
        data.items =
            ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![crate::HistoryItem {
                id: "demo".into(),
                title: "Demo".into(),
                handle: "demo".into(),
                media_count: 3,
                saved_count: 1,
                size_text: "12 KB".into(),
                output_dir: "H:/Downloads/demo".into(),
                ..Default::default()
            }]));
        data.title = "Demo".into();
        data.available = 1;
        data.missing = 1;
        data.pending = 1;
        data.folder = "H:/Downloads/demo".into();
    });
    app.on_library_open({
        let weak = app.as_weak();
        move |key| {
            weak.upgrade().unwrap().update_library(|data| {
                data.account = key;
                data.view_ready = true;
            });
        }
    });
    app.set_selected_tab(crate::library::TAB);
    app.on_load_thumbnail({
        let weak = app.as_weak();
        move |index| {
            let app = weak.upgrade().unwrap();
            let items = app.get_media_items();
            if let Some(position) = items.iter().position(|item| item.media_index == index) {
                let mut item = items.row_data(position).unwrap();
                item.preview_image = image.clone();
                items.set_row_data(position, item);
            }
        }
    });
    app
}

#[test]
fn library_actions_filter_preview_and_retry_without_card_clickthrough() {
    let app = library_fixture();
    let events = Rc::new(RefCell::new(Vec::new()));
    app.on_library_refetch({
        let events = events.clone();
        move |key| events.borrow_mut().push(format!("fetch:{key}"))
    });
    app.on_library_download({
        let events = events.clone();
        move |index| events.borrow_mut().push(format!("download:{index}"))
    });
    app.on_open_download_path({
        let events = events.clone();
        move |path, folder| events.borrow_mut().push(format!("open:{path}:{folder}"))
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for width in [900.0, 1024.0, 1440.0] {
        h.set_size(Vec2::new(width, 600.0));
        h.run();
        let library_search = h.get_by_label("Search library").rect();
        app.set_selected_tab(1);
        h.run();
        assert_eq!(h.get_by_label("Search accounts…").rect(), library_search);
        let token = app.get_saved_auth_token();
        app.set_history_select_mode(true);
        app.set_saved_auth_token(String::new());
        h.run();
        assert_eq!(h.get_by_label("Search accounts…").rect(), library_search);
        app.set_history_select_mode(false);
        app.set_saved_auth_token(token);
        app.set_selected_tab(crate::library::TAB);
        h.run();
    }
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Refetch · Demo").click();
    h.run();
    assert_eq!(*events.borrow(), ["fetch:demo"]);
    assert!(!app.get_library().view_ready);
    h.get_by_label("Open library account · Demo").click();
    h.run();
    h.get_by_label("Download pending").click();
    h.run();
    assert_eq!(events.borrow().last().unwrap(), "download:-1");
    h.get_by_label("Open folder · Library media 0").click();
    h.run();
    assert_eq!(
        events.borrow().last().unwrap(),
        "open:H:/Downloads/demo:true"
    );
    h.get_by_label("File missing  1").click();
    h.run();
    assert!(h.query_by_label("Library media 0").is_none());
    h.get_by_label("Download again · Library media 1").click();
    h.run();
    assert_eq!(events.borrow().last().unwrap(), "download:1");
    assert_eq!(
        app.get_selected_media_index(),
        -1,
        "An inline action also opened the preview"
    );
    h.get_by_label("Library media 1").click();
    h.run();
    assert!(h.query_by_label("Media preview").is_some());
    h.get_by_label("Download again").click();
    h.run();
    assert_eq!(events.borrow().last().unwrap(), "download:1");
    h.key_press(egui::Key::Escape);
    h.run();
    h.get_by_label("Back to library").click();
    h.run();
    assert!(!app.get_library().view_ready);
    assert!(h.query_by_label("Open library account · Demo").is_some());
    h.get_by_label("Search library").click();
    h.get_by_label("Search library").type_text("absent");
    h.run();
    assert!(h.query_by_label("Open library account · Demo").is_none());
    h.get_by_label("Clear search").click();
    h.run();
    assert!(app.get_library().query.is_empty());
    assert!(h.query_by_label("Open library account · Demo").is_some());
}

#[test]
fn library_selection_filters_labels_and_bulk_update_stay_in_library() {
    let app = library_fixture();
    let mut demo = app.get_library().items.row_data(0).unwrap();
    demo.group_label = "Artists".into();
    demo.account_labels = "Artists · #art".into();
    let mut other = demo.clone();
    other.id = "other".into();
    other.handle = "other".into();
    other.title = "Other".into();
    other.group_label = "Favorites".into();
    other.account_labels = "Favorites · #photo".into();
    app.update_library(|data| {
        data.items = ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![demo, other]));
        data.groups = vec!["Artists".into(), "Favorites".into()];
    });
    let updates = Rc::new(RefCell::new(Vec::new()));
    app.on_library_refetch_selected({
        let weak = app.as_weak();
        let updates = updates.clone();
        move || {
            updates
                .borrow_mut()
                .push(weak.upgrade().unwrap().get_library().selection)
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Select accounts").click();
    h.run();
    h.get_by_label("Open library account · Demo").click();
    h.run();
    assert_eq!(app.get_library().selection, ["demo"]);
    assert!(!app.get_library().view_ready);
    h.get_by_label("Filter group").click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Favorites")
        .click();
    h.run();
    assert!(h.query_by_label("Open library account · Demo").is_none());
    h.get_by_label("Select visible").click();
    h.run();
    assert_eq!(app.get_library().selection, ["demo", "other"]);
    h.get_by_label("Update selected (2)").click();
    h.run();
    assert_eq!(
        updates.borrow().as_slice(),
        &[vec![String::from("demo"), String::from("other")]]
    );
    assert_eq!(app.get_selected_tab(), crate::library::TAB);
    h.get_by_label("Assign group and tags").click();
    h.run();
    assert!(h.query_by_label("Account labels dialog").is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    app.set_saved_auth_token(String::new());
    h.run();
    assert!(
        h.get_by_label("Update selected (2)")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Cancel selection").click();
    h.run();
    assert!(app.get_library().selection.is_empty());
    app.update_library(|data| data.group_filter.clear());
    h.run();
    h.get_by_label("Search library").click();
    h.get_by_label("Search library").type_text("#art");
    h.run();
    assert!(h.query_by_label("Open library account · Demo").is_some());
    assert!(h.query_by_label("Open library account · Other").is_none());
}

#[test]
fn library_feedback_stays_in_library_during_navigation_and_background_completion() {
    let app = library_fixture();
    app.set_status("Home is ready".into());
    app.set_progress(12.0);
    app.set_progress_detail("Home progress".into());
    app.update_library(|data| {
        data.job = "demo".into();
        data.feedback_active = true;
    });
    app.set_is_busy(true);
    app.set_is_extracting(true);
    app.set_status("Updating library account…".into());
    app.set_progress(65.0);
    app.set_progress_detail("Library progress".into());
    crate::app::notifications::show_library_toast(&app, "Library-only failure", "error");
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run_steps(4);
    assert!(h.query_by_label("Library-only failure").is_some());
    app.set_selected_tab(0);
    app.set_home_results_hidden(true);
    h.run_steps(4);
    assert!(h.query_by_label("Library-only failure").is_none());
    assert!(h.query_by_label("Cancel operation").is_none());
    assert!(h.query_by_label("Updating library account…").is_none());
    assert!(
        h.get_by_label("Search and extract")
            .accesskit_node()
            .is_disabled()
    );
    assert_eq!(app.get_status(), "Home is ready");
    assert_eq!(app.get_progress(), 12.0);
    assert_eq!(app.get_progress_detail(), "Home progress");
    assert!(app.get_last_error().is_empty());
    app.set_is_busy(false);
    app.set_is_extracting(false);
    app.update_library(|data| data.job.clear());
    crate::app::notifications::show_library_toast(&app, "Library completion failure", "error");
    app.update_library(|data| data.feedback_active = false);
    h.run_steps(4);
    assert!(h.query_by_label("Library completion failure").is_none());
    assert!(app.get_last_error().is_empty());
    app.set_selected_tab(crate::library::TAB);
    h.run_steps(4);
    assert!(h.query_by_label("Library completion failure").is_some());
    app.set_selected_tab(0);
    crate::app::notifications::show_toast(&app, "Home-only failure", "error");
    h.run_steps(4);
    assert_eq!(app.get_last_error(), "Home-only failure");
    app.set_selected_tab(crate::library::TAB);
    crate::app::notifications::show_library_toast(&app, "A later Library failure", "error");
    assert_eq!(app.get_last_error(), "Home-only failure");
    assert_eq!(app.get_library().error, "A later Library failure");
}

#[test]
fn page_navigation_preserves_each_media_filter_and_visible_limit() {
    let app = library_fixture();
    let base = app.get_media_items().row_data(0).unwrap();
    app.set_media_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..50)
            .map(|index| {
                let mut item = base.clone();
                item.title = format!("Media {index}");
                item.media_index = index;
                item.status = "Missing".into();
                item
            })
            .collect::<Vec<_>>(),
    )));

    app.invoke_library_open("demo".into());
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.state_mut().kind = "library-missing".into();
    h.state_mut().limit = 90;
    h.run();
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    let thumb = painted_scroll_thumb(&h, bar).0;
    h.drag_at(thumb.center());
    h.run_steps(1);
    h.hover_at(thumb.center() + Vec2::new(0.0, 100.0));
    h.run_steps(3);
    h.drop_at(thumb.center() + Vec2::new(0.0, 100.0));
    h.run_steps(3);
    let scrolled =
        painted_scroll_thumb(&h, h.get_by_role(egui::accesskit::Role::ScrollBar).rect()).0;
    app.set_selected_tab(2);
    h.run();
    app.set_selected_tab(crate::library::TAB);
    h.run();
    assert_eq!(h.state().kind, "library-missing");
    assert_eq!(h.state().limit, 90);
    assert_eq!(
        painted_scroll_thumb(&h, h.get_by_role(egui::accesskit::Role::ScrollBar).rect()).0,
        scrolled
    );

    app.set_selected_tab(0);
    h.run();
    h.state_mut().kind = "photo".into();
    h.state_mut().limit = 60;
    h.run();
    app.set_selected_tab(1);
    h.run();
    app.set_selected_tab(0);
    h.run();
    assert_eq!(h.state().kind, "photo");
    assert_eq!(h.state().limit, 60);
    app.set_batch_mode(true);
    h.run();
    h.state_mut().kind = "video".into();
    h.state_mut().limit = 120;
    app.set_selected_tab(crate::library::TAB);
    h.run();
    assert_eq!(h.state().kind, "library-missing");
    assert_eq!(h.state().limit, 90);
    app.set_selected_tab(0);
    h.run();
    assert_eq!(h.state().kind, "video");
    assert_eq!(h.state().limit, 120);
    app.set_batch_mode(false);
    h.run();
    assert_eq!(h.state().kind, "photo");
    assert_eq!(h.state().limit, 60);
}

#[test]
#[ignore = "Renders Library accounts, media states, progress, preview and file management"]
fn render_library_review() {
    let directory = std::path::Path::new("target/library-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        for width in [900.0, 1024.0, 1440.0] {
            let app = library_fixture();
            app.set_theme_mode(theme.into());
            let mut h = harness(app.clone());
            h.set_size(Vec2::new(width, 600.0));
            let accounts = app.get_library().items;
            for state in [
                "accounts",
                "selection",
                "labels",
                "queue",
                "media",
                "missing",
                "preview",
                "running",
            ] {
                match state {
                    "selection" => app.update_library(|data| {
                        data.select_mode = true;
                        data.selection = vec!["demo".into()];
                    }),
                    "labels" => app.set_account_labels_open(true),
                    "queue" => {
                        app.set_account_labels_open(false);
                        let mut rows = Vec::new();
                        let mut jobs = Vec::new();
                        for (key, status, count) in [
                            ("demo", "Fetching", 12),
                            ("other", "Completed", 20),
                            ("third", "Failed", 0),
                            ("fourth", "Queued", 0),
                        ] {
                            let mut row = accounts.row_data(0).unwrap();
                            row.id = key.into();
                            row.handle = key.into();
                            row.title = key.into();
                            row.account_labels = "Artists · #photo".into();
                            row.account_color = Color32::from_rgb(20, 71, 230);
                            rows.push(row);
                            jobs.push(crate::LibraryJob {
                                key: key.into(),
                                status: status.into(),
                                media_count: count,
                                error: if status == "Failed" {
                                    "Fixture error".into()
                                } else {
                                    String::new()
                                },
                            });
                        }
                        app.update_library(|data| {
                            data.items = ui_runtime::ModelRc::new(ui_runtime::VecModel::from(rows));
                            data.queue = jobs;
                            data.job = "demo".into();
                            data.feedback_active = true;
                        });
                        app.set_is_busy(true);
                        app.set_status("Updating library account… · @demo (3/4)".into());
                        app.set_progress(40.0);
                    }
                    "media" => {
                        app.set_is_busy(false);
                        app.update_library(|data| {
                            data.job.clear();
                            data.feedback_active = false;
                            data.queue.clear();
                            data.items = accounts.clone();
                        });
                        app.invoke_library_open("demo".into());
                    }
                    "missing" => h.state_mut().kind = "library-missing".into(),
                    "preview" => app.set_selected_media_index(1),
                    "running" => {
                        app.set_selected_media_index(-1);
                        app.update_library(|data| {
                            data.job = "demo".into();
                            data.feedback_active = true;
                        });
                        app.set_is_busy(true);
                        app.set_status("Updating library account…".into());
                    }
                    _ => {}
                }
                h.remove_cursor();
                h.run_steps(4);
                h.render()
                    .unwrap()
                    .save(directory.join(format!("{theme}-{}-{state}.png", width as u32)))
                    .unwrap();
            }
        }
    }
}

#[test]
#[ignore = "Renders Library feedback before and after returning Home"]
fn render_library_feedback_review() {
    let directory = std::path::Path::new("target/library-feedback-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        let app = library_fixture();
        app.set_theme_mode(theme.into());
        app.set_home_results_hidden(true);
        app.set_status("Ready for an X/Twitter URL.".into());
        app.update_library(|data| {
            data.job = "demo".into();
            data.feedback_active = true;
        });
        app.set_is_busy(true);
        app.set_status("Downloading library media…".into());
        app.set_progress(65.0);
        crate::app::notifications::show_library_toast(
            &app,
            "11 library files downloaded.\nFixture download error: connection interrupted.",
            "error",
        );
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        for state in [
            "library-running",
            "home-running",
            "home-finished",
            "library-finished",
        ] {
            match state {
                "home-running" => app.set_selected_tab(0),
                "home-finished" => {
                    app.set_is_busy(false);
                    app.update_library(|data| {
                        data.job.clear();
                        data.feedback_active = false;
                    });
                }
                "library-finished" => app.set_selected_tab(crate::library::TAB),
                _ => {}
            }
            h.remove_cursor();
            h.run_steps(4);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-{state}.png")))
                .unwrap();
        }
    }
}
