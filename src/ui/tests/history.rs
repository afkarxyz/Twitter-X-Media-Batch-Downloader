use super::*;

fn rows() -> Vec<crate::HistoryItem> {
    vec![
        crate::HistoryItem{id:"demo".into(),title:"xbatchdemo".into(),handle:"xbatchdemo".into(),media_count:1,fetched_at:"Sep 30, 2026".into(),session_count:2,group_label:"Timeline · Imported filters".into(),update_eligible:true,..Default::default()},
        crate::HistoryItem{id:"nasa".into(),title:"NASA".into(),handle:"NASA".into(),fetched_at:"Sep 29, 2026".into(),session_count:1,group_label:"Timeline · Imported filters".into(),update_eligible:true,..Default::default()},
        crate::HistoryItem{id:"long".into(),title:"An account with a long display name to check how history cards fit the desktop window".into(),handle:"example".into(),fetched_at:"Sep 28, 2026".into(),session_count:1,group_label:"Timeline · Imported filters".into(),..Default::default()},
    ]
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ui_runtime::ModelRc<T> {
    ui_runtime::ModelRc::new(ui_runtime::VecModel::from(rows))
}

fn history_fixture() -> AppWindow {
    let app = fixture();
    app.set_selected_tab(1);
    app.set_theme_mode("dark".into());
    app.set_history_items(model(rows()));
    app.set_history_fetch_count(3);
    app.set_history_saved_count(2);
    app
}

fn downloads(app: &AppWindow) {
    app.set_selected_tab(crate::library::TAB);
    app.set_history_show_saved(true);
    app.set_history_items(model(vec![
        crate::HistoryItem {
            id: "download".into(),
            title: "xbatchdemo".into(),
            handle: "xbatchdemo".into(),
            media_count: 2,
            saved_count: 2,
            output_dir: "H:\\Downloads\\xbatchdemo".into(),
            size_text: "26 B".into(),
            fetched_at: "2026-10-14".into(),
            ..Default::default()
        },
        crate::HistoryItem {
            id: "earlier".into(),
            title: "Earlier batch".into(),
            handle: "xbatchdemo".into(),
            media_count: 1,
            saved_count: 0,
            output_dir: "H:\\Downloads\\xbatchdemo".into(),
            size_text: "13 B".into(),
            earlier_download: true,
            ..Default::default()
        },
    ]));
    let items = app.get_history_items();
    app.update_library(|data| {
        data.items = items;
        data.account.clear();
        data.view_ready = false;
    });
}

fn download_details(app: &AppWindow) {
    app.set_selected_tab(crate::library::TAB);
    app.update_library(|data| {
        data.account = "xbatchdemo".into();
        data.view_ready = true;
        data.files_view = true;
    });
    app.set_download_batch_id("download".into());
    app.set_download_batch_title("xbatchdemo".into());
    app.set_download_batch_folder("H:\\Downloads\\xbatchdemo".into());
    app.set_download_batch_size("26 B".into());
    app.set_download_batch_files(model(vec![
        crate::DownloadedFile {
            filename: "2026_demo_photo_01.jpg".into(),
            path: "H:\\Downloads\\xbatchdemo\\2026_demo_photo_01.jpg".into(),
            folder: "H:\\Downloads\\xbatchdemo".into(),
            kind: "photo".into(),
            size_text: "13 B".into(),
            present: true,
            ..Default::default()
        },
        crate::DownloadedFile {
            filename: "2026_demo_video_02.mp4".into(),
            path: "H:\\Downloads\\xbatchdemo\\2026_demo_video_02.mp4".into(),
            folder: "H:\\Downloads\\xbatchdemo".into(),
            kind: "video".into(),
            size_text: "13 B".into(),
            present: false,
            ..Default::default()
        },
    ]));
}

fn sessions(app: &AppWindow) {
    app.set_fetch_sessions_key("demo".into());
    app.set_fetch_sessions_title("xbatchdemo".into());
    app.set_fetch_sessions_label("Timeline · Imported filters".into());
    let mut row = rows().remove(0);
    row.session_count = 0;
    let mut older = row.clone();
    older.id = "old-demo".into();
    older.fetched_at = "Sep 29, 2026".into();
    app.set_fetch_sessions_items(model(vec![row, older]));
}

#[test]
fn history_grid_and_detail_positions_follow_the_legacy_layout() {
    let app = history_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for (label, x, y, w, height) in [
        ("xbatchdemo", 68.0, 130.0, 466.0, 94.0),
        ("NASA", 546.0, 130.0, 466.0, 94.0),
    ] {
        assert_eq!(
            h.get_by_role_and_label(egui::accesskit::Role::Button, label)
                .rect(),
            egui::Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, height))
        );
    }
    assert_eq!(
        h.get_by_label("Clear history").rect().min,
        egui::pos2(980.0, 18.0)
    );
    assert_eq!(
        h.get_by_label("Import history JSON").rect().min,
        egui::pos2(940.0, 18.0)
    );
    h.set_size(Vec2::new(2200.0, 900.0));
    h.run();
    let first = h
        .get_by_role_and_label(egui::accesskit::Role::Button, "xbatchdemo")
        .rect();
    assert_eq!(first.left(), 588.0);
    assert_eq!(first.width(), 534.0);
    h.set_size(Vec2::new(900.0, 600.0));
    h.run();
    assert_eq!(
        h.get_by_role_and_label(egui::accesskit::Role::Button, "NASA")
            .rect()
            .top(),
        234.0
    );
    h.set_size(Vec2::new(1024.0, 600.0));
    sessions(&app);
    h.run();
    assert_eq!(
        h.get_all_by_role_and_label(egui::accesskit::Role::Button, "xbatchdemo")
            .next()
            .unwrap()
            .rect()
            .height(),
        70.0
    );
    download_details(&app);
    h.run();
    assert_eq!(
        h.get_by_label("Open file · 2026_demo_photo_01.jpg")
            .rect()
            .top(),
        159.0
    );
    assert!(
        !h.get_by_label("Download again · 2026_demo_video_02.mp4")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn history_search_and_card_icons_dispatch_without_account_management() {
    let app = history_fixture();
    let events = Rc::new(RefCell::new(Vec::new()));
    app.on_open_history({
        let events = events.clone();
        move |id| events.borrow_mut().push(format!("open:{id}"))
    });
    app.on_export_history({
        let events = events.clone();
        move |id| events.borrow_mut().push(format!("export:{id}"))
    });
    app.on_search_history({
        let events = events.clone();
        move |q, saved| events.borrow_mut().push(format!("search:{q}:{saved}"))
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Export JSON · xbatchdemo").click();
    h.run();
    assert_eq!(*events.borrow(), ["export:demo"]);
    assert!(h.query_by_label("Select accounts").is_none());
    assert!(h.query_by_label("Filter group").is_none());
    h.get_by_role_and_label(egui::accesskit::Role::Button, "xbatchdemo")
        .click();
    h.run();
    assert_eq!(events.borrow().last().unwrap(), "open:demo");
    assert_eq!(app.get_history_selection().row_count(), 0);
    assert!(h.query_by_label("Downloads (2)").is_none());
    assert!(!app.get_history_show_saved());
    h.get_by_label("Search accounts…").click();
    h.run();
    h.get_by_label("Search accounts…").type_text("NASA");
    h.run();
    assert_eq!(events.borrow().last().unwrap(), "search:NASA:false");
    h.get_by_label("Clear search").click();
    h.run();
    assert_eq!(app.get_history_search_query(), "");
}

#[test]
fn history_source_filter_keeps_only_url_likes_or_bookmark_fetches() {
    let app = history_fixture();
    let entries = [
        ("1", "demo", "https://x.com/demo"),
        ("2", "liker", "https://x.com/liker/likes"),
        ("3", "saver", "https://x.com/i/bookmarks"),
    ]
    .map(|(id, username, source)| crate::download::engine::HistoryEntry {
        id: id.into(),
        username: username.into(),
        source_url: source.into(),
        ..Default::default()
    });
    let searches = Rc::new(RefCell::new(0));
    app.on_search_history({
        let app = app.as_weak();
        let entries = entries.clone();
        let searches = searches.clone();
        move |query, saved| {
            *searches.borrow_mut() += 1;
            crate::history::view_model::refresh(&app.upgrade().unwrap(), &entries, &query, saved);
        }
    });
    let handles = |app: &AppWindow| {
        app.get_history_items()
            .iter()
            .map(|row| row.handle.to_string())
            .collect::<Vec<_>>()
    };
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for (choice, expected) in [
        ("Likes", vec!["liker"]),
        ("Bookmarks", vec!["saver"]),
        ("URL", vec!["demo"]),
    ] {
        h.get_by_label("Fetch source").click();
        h.run();
        h.get_by_label(choice).click();
        h.run();
        assert_eq!(handles(&app), expected, "{choice}");
    }
    h.get_by_label("Fetch source").click();
    h.run();
    h.get_by_label("All").click();
    h.run();
    assert_eq!(app.get_history_source_filter(), 0);
    assert_eq!(handles(&app).len(), 3);
    assert_eq!(*searches.borrow(), 4);
}

#[test]
fn history_removal_requires_confirmation_and_preserves_session_scope() {
    let app = history_fixture();
    let removed = Rc::new(RefCell::new(Vec::new()));
    app.on_remove_history({
        let removed = removed.clone();
        move |id| removed.borrow_mut().push(id)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Remove from history · xbatchdemo").click();
    h.run();
    assert!(h.state().history_confirmation.as_ref().unwrap().group);
    assert_eq!(
        h.get_by_label("History confirmation").rect().center().x,
        540.0
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(removed.borrow().is_empty());
    sessions(&app);
    h.run();
    h.get_all_by_label("Remove from history · xbatchdemo")
        .next()
        .unwrap()
        .click();
    h.run();
    assert!(!h.state().history_confirmation.as_ref().unwrap().group);
    h.get_by_role_and_label(egui::accesskit::Role::Button, "Remove")
        .click();
    h.run();
    assert_eq!(*removed.borrow(), ["demo"]);
}

#[test]
#[ignore = "Renders History reference states for visual review"]
fn render_history_review() {
    let app = history_fixture();
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let dir = std::path::Path::new("target/history-review");
    std::fs::create_dir_all(dir).unwrap();
    for theme in ["dark", "light"] {
        app.set_selected_tab(1);
        app.set_theme_mode(theme.into());
        app.set_download_batch_id(String::new());
        app.set_fetch_sessions_key(String::new());
        app.set_history_show_saved(false);
        app.set_history_selection(Default::default());
        app.set_history_items(model(rows()));
        h.remove_cursor();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-fetches.png")))
            .unwrap();
        h.get_by_label("Export JSON · xbatchdemo").hover();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-export-hover.png")))
            .unwrap();
        h.remove_cursor();
        h.get_by_label("Remove from history · xbatchdemo").click();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-remove.png")))
            .unwrap();
        h.key_press(egui::Key::Escape);
        h.run();
        sessions(&app);
        h.remove_cursor();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-sessions.png")))
            .unwrap();
        app.set_fetch_sessions_key(String::new());
        downloads(&app);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-downloads.png")))
            .unwrap();
        download_details(&app);
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-files.png")))
            .unwrap();
        app.set_download_batch_id(String::new());
        app.set_history_items(Default::default());
        app.set_selected_tab(1);
        app.set_history_search_query("missing".into());
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-empty.png")))
            .unwrap();
        app.set_history_search_query(String::new());
    }
    app.set_history_items(model(rows()));
    app.set_selected_tab(1);
    app.set_history_show_saved(false);
    h.set_size(Vec2::new(2200.0, 900.0));
    h.run_steps(4);
    h.render()
        .unwrap()
        .save(dir.join("light-ultrawide.png"))
        .unwrap();
}

#[test]
fn history_download_actions_and_missing_files_keep_their_paths_and_busy_guards() {
    let app = history_fixture();
    download_details(&app);
    let opened = Rc::new(RefCell::new(Vec::new()));
    app.on_open_download_path({
        let opened = opened.clone();
        move |path, folder| opened.borrow_mut().push((path, folder))
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Open file · 2026_demo_photo_01.jpg").click();
    h.run();
    assert_eq!(
        opened.borrow()[0],
        (
            "H:\\Downloads\\xbatchdemo\\2026_demo_photo_01.jpg".into(),
            false
        )
    );
    assert!(
        !h.get_by_label("Download again · 2026_demo_video_02.mp4")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Open folder · 2026_demo_video_02.mp4")
        .click();
    h.run();
    assert_eq!(
        opened.borrow()[1],
        ("H:\\Downloads\\xbatchdemo".into(), true)
    );
    app.set_is_busy(true);
    h.run();
    for label in [
        "Back",
        "Check files",
        "Open file · 2026_demo_photo_01.jpg",
        "Open folder · 2026_demo_video_02.mp4",
    ] {
        assert!(h.get_by_label(label).accesskit_node().is_disabled());
    }
}

#[test]
fn library_labels_keep_failed_edits_and_submit_the_selected_shade() {
    let app = history_fixture();
    app.set_selected_tab(crate::library::TAB);
    app.set_account_labels_open(true);
    let submitted = Rc::new(RefCell::new(Vec::new()));
    app.on_apply_account_labels({
        let submitted = submitted.clone();
        move |group, tags, color| {
            submitted.borrow_mut().push((group, tags, color));
            false
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert_eq!(
        h.get_by_label("Account labels dialog").rect().width(),
        460.0
    );
    h.get_by_label("Group").type_text("Favorites");
    h.run();
    h.get_by_label("Tags, separated by commas").click();
    h.run();
    h.get_by_label("Tags, separated by commas")
        .type_text("art, video");
    h.run();
    h.get_by_label("Group color").click();
    h.run();
    h.get_by_label("Blue").click_accesskit();
    h.run();
    h.get_by_label("Blue 700 · #1447e6").click();
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(
        *submitted.borrow(),
        [("Favorites".into(), "art, video".into(), "#1447e6".into())]
    );
    assert!(app.get_account_labels_open());
    assert_eq!(h.state().labels_group, "Favorites");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!app.get_account_labels_open());
    app.set_account_labels_open(true);
    h.run();
    assert!(h.state().labels_group.is_empty());
    assert_eq!(h.state().labels_family, 0);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "Renders an explicitly supplied large migrated database using offline UI callbacks"]
async fn render_large_lazy_history_review() {
    let path = std::path::PathBuf::from(
        std::env::var("LAZY_HISTORY_REVIEW_DB").expect("Set a migrated review database"),
    );
    let connection =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let entries =
        crate::history::db::read_summaries(&connection, std::sync::Arc::new(path.clone())).unwrap();
    let largest = entries
        .iter()
        .filter(|entry| entry.download.is_none())
        .max_by_key(|entry| entry.media_count())
        .unwrap()
        .id
        .clone();
    let directory = path.parent().unwrap().join("ui-review");
    std::fs::create_dir_all(&directory).unwrap();
    for theme in ["dark", "light"] {
        let app = fixture();
        app.set_theme_mode(theme.into());
        app.set_media_items(Default::default());
        app.set_selected_tab(1);
        let history = std::sync::Arc::new(std::sync::Mutex::new(entries.clone()));
        let backend = std::sync::Arc::new(std::sync::Mutex::new(
            crate::download::engine::BackendState::default(),
        ));
        crate::history::open::register(&app, history, backend.clone());
        crate::history::view_model::refresh(&app, &entries, "", false);
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.remove_cursor();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-history.png")))
            .unwrap();
        app.invoke_open_history(largest.clone());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.get_history_busy() {
            ui_runtime::pump(&h.ctx);
            assert!(std::time::Instant::now() < deadline);
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(backend.lock().unwrap().candidates.is_empty());
        assert!(app.get_media_count() > 60000);
        let items = app.get_media_items();
        let model = items
            .as_any()
            .downcast_ref::<crate::history::paged::PagedModel>()
            .unwrap();
        assert_eq!(
            model
                .decoded_pages
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        for _ in 0..20 {
            h.run_steps(1);
            ui_runtime::pump(&h.ctx);
            if !items.row_data(0).unwrap().payload_pending {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        h.run_steps(3);
        assert!(!items.row_data(0).unwrap().payload_pending);
        assert!(
            model
                .decoded_pages
                .load(std::sync::atomic::Ordering::Relaxed)
                <= 2,
            "Rendering the first viewport loaded unrelated pages"
        );
        h.render()
            .unwrap()
            .save(directory.join(format!("{theme}-gallery.png")))
            .unwrap();
        app.set_selected_media_index(-1);
    }
}
