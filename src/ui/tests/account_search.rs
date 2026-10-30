use super::*;

fn accounts(count: usize) -> ui_runtime::ModelRc<crate::AccountSearchItem> {
    ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
        (0..count)
            .map(|index| crate::AccountSearchItem {
                name: format!("Account {index}"),
                handle: format!("user{index}"),
                bio: "Account search review fixture.".into(),
                ..Default::default()
            })
            .collect::<Vec<_>>(),
    ))
}

fn search_fixture(count: usize) -> AppWindow {
    let app = fixture();
    app.set_account_search_open(true);
    app.set_account_search_query("demo".into());
    if count > 0 {
        app.set_account_search_result_query("demo".into());
    }
    app.set_account_search_items(accounts(count));
    let weak = app.as_weak();
    app.on_close_account_search(move || {
        let app = weak.upgrade().unwrap();
        app.set_account_search_open(false);
        app.set_account_search_busy(false);
    });
    app
}

fn scroll_to_end(h: &mut Harness<'_, Desktop>) {
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
    h.run_steps(3);
}

#[test]
fn search_modal_keeps_its_width_and_grows_vertically_with_results() {
    let app = search_fixture(0);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let empty = h.get_by_label("Account search dialog").rect();
    assert_eq!(empty.size(), Vec2::new(690.0, 200.0));
    let input_width = h
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Search accounts")
        .rect()
        .width();
    assert!(h.query_by_label("Load more").is_none());
    for count in [2, 30] {
        app.set_account_search_items(accounts(count));
        h.run();
        let dialog = h.get_by_label("Account search dialog").rect();
        assert_eq!(dialog.width(), empty.width());
        assert_eq!(dialog.height(), if count == 2 { 250.0 } else { 552.0 });
        assert_eq!(
            h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Search accounts")
                .rect()
                .width(),
            input_width
        );
    }
    app.set_account_search_items(accounts(0));
    app.set_account_search_result_query("demo".into());
    h.run();
    assert_eq!(
        h.get_by_label("Account search dialog").rect().size(),
        empty.size()
    );
    assert!(h.output().shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("No accounts match"))));
    app.set_account_search_error("Network error. ".repeat(30));
    h.run();
    let error = h.get_by_label("Account search dialog").rect();
    assert!(error.height() > empty.height() && error.height() < 400.0);
    assert!(h.ctx.viewport_rect().contains_rect(error));

    let name = "An account with a display name much longer than its card";
    let handle = "long_username_that_must_be_truncated_in_the_card";
    app.set_account_search_error(String::new());
    app.set_account_search_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(vec![
        crate::AccountSearchItem {
            name: name.into(),
            handle: handle.into(),
            verified: true,
            ..Default::default()
        },
        crate::AccountSearchItem::default(),
    ])));
    h.run();
    assert_eq!(
        h.get_by_label("Account search dialog").rect().width(),
        empty.width()
    );
    let label = format!("{name} · @{handle}");
    let account = h.get_by_label(&label);
    let card = account.rect();
    assert_eq!(card.width(), 319.0);
    for prefix in ["An account", "@long_username"] {
        let text = h
            .output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text().starts_with(prefix) => Some(text),
                _ => None,
            })
            .expect("Account text should be painted");
        assert!(text.galley.elided);
        assert_eq!(text.galley.rows.len(), 1);
        assert!(card.contains_rect(egui::Rect::from_min_size(text.pos, text.galley.size())));
    }
    account.click();
    h.run();
    assert_eq!(app.get_source_url(), handle);
}

#[test]
fn short_account_results_wait_for_scroll_instead_of_chasing_each_new_cursor() {
    for count in [1, 2, 4, 6] {
        let app = search_fixture(count);
        app.set_account_search_cursor("page-1".into());
        let requests = Rc::new(RefCell::new(Vec::new()));
        app.on_search_accounts({
            let requests = requests.clone();
            let weak = app.as_weak();
            move |query, more| {
                requests.borrow_mut().push((query, more));
                weak.upgrade().unwrap().set_account_search_busy(true);
            }
        });
        let mut h = harness(app.clone());
        h.set_size(Vec2::new(1024.0, 600.0));
        h.run_steps(12);
        assert!(
            requests.borrow().is_empty(),
            "{count} results must not load another page just because they fit in the modal"
        );
        let results = h.get_by_label("Account results").rect();
        h.hover_at(results.center());
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(8);
        assert_eq!(*requests.borrow(), [("demo".into(), true)]);
        app.set_account_search_cursor("page-2".into());
        app.set_account_search_busy(false);
        h.run_steps(12);
        assert_eq!(
            requests.borrow().len(),
            1,
            "A new cursor alone must not cause another request"
        );
    }
}

#[test]
fn account_pagination_requests_each_cursor_once_and_stops_for_busy_or_changed_queries() {
    let app = search_fixture(30);
    app.set_account_search_cursor("page-1".into());
    let requests = Rc::new(RefCell::new(Vec::new()));
    app.on_search_accounts({
        let requests = requests.clone();
        let weak = app.as_weak();
        move |query, more| {
            requests.borrow_mut().push((query, more));
            weak.upgrade().unwrap().set_account_search_busy(true);
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(requests.borrow().is_empty());
    scroll_to_end(&mut h);
    assert_eq!(*requests.borrow(), [("demo".into(), true)]);
    h.run_steps(8);
    assert_eq!(requests.borrow().len(), 1);
    app.set_account_search_items(accounts(50));
    app.set_account_search_cursor("page-2".into());
    app.set_account_search_busy(false);
    h.run();
    assert_eq!(requests.borrow().len(), 1);
    scroll_to_end(&mut h);
    assert_eq!(requests.borrow().len(), 2);
    app.set_account_search_busy(false);
    h.run_steps(8);
    assert_eq!(
        requests.borrow().len(),
        2,
        "An unchanged cursor must not repeat indefinitely"
    );
    app.set_account_search_query("edited".into());
    app.set_account_search_cursor("page-3".into());
    h.run_steps(4);
    assert_eq!(requests.borrow().len(), 2);
    app.set_account_search_query("demo".into());
    h.run_steps(4);
    assert_eq!(requests.borrow().len(), 3);
    app.set_account_search_cursor(String::new());
    app.set_account_search_busy(false);
    h.run_steps(4);
    assert_eq!(requests.borrow().len(), 3);
}

#[test]
fn pagination_errors_require_retry_and_existing_accounts_stay_selectable_while_loading() {
    let app = search_fixture(30);
    app.set_account_search_cursor("page-1".into());
    let requests = Rc::new(RefCell::new(Vec::new()));
    app.on_search_accounts({
        let requests = requests.clone();
        let weak = app.as_weak();
        move |query, more| {
            requests.borrow_mut().push((query, more));
            let app = weak.upgrade().unwrap();
            app.set_account_search_busy(true);
            app.set_account_search_error(String::new());
        }
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    scroll_to_end(&mut h);
    app.set_account_search_busy(false);
    app.set_account_search_error("Could not connect to X search.".into());
    h.run_steps(6);
    assert_eq!(requests.borrow().len(), 1);
    h.get_by_label("Try again").click();
    h.run_steps(2);
    assert_eq!(requests.borrow().len(), 2);
    assert!(app.get_account_search_busy());
    let results = h.get_by_label("Account results").rect();
    let account = h
        .get_all_by_role(egui::accesskit::Role::Button)
        .find(|node| {
            node.accesskit_node()
                .label()
                .is_some_and(|label| label.starts_with("Account "))
                && results.contains(node.rect().center())
        })
        .unwrap();
    let label = account.accesskit_node().label().unwrap().to_owned();
    assert!(!account.accesskit_node().is_disabled());
    account.click();
    h.run_steps(3);
    assert!(!app.get_account_search_open());
    assert_eq!(app.get_source_url(), label.split('@').next_back().unwrap());
}

#[test]
fn large_account_search_builds_only_nearby_cards() {
    struct Probe(Rc<std::cell::Cell<usize>>);
    impl ui_runtime::Model for Probe {
        type Data = crate::AccountSearchItem;
        fn row_count(&self) -> usize {
            5000
        }
        fn row_data(&self, row: usize) -> Option<Self::Data> {
            self.0.set(self.0.get() + 1);
            (row < 5000).then(|| crate::AccountSearchItem {
                name: format!("Account {row}"),
                handle: format!("user{row}"),
                ..Default::default()
            })
        }
    }
    let reads = Rc::new(std::cell::Cell::new(0));
    let app = search_fixture(0);
    app.set_account_search_items(ui_runtime::ModelRc::new(Probe(reads.clone())));
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    assert!(
        reads.get() < 200,
        "Opening account search must not clone every result"
    );
    reads.set(0);
    scroll_to_end(&mut h);
    assert!(
        reads.get() < 200,
        "Scrolling must render only the viewport and nearby rows"
    );
    assert!(h.query_by_label("Account 4999 · @user4999").is_some());
}

#[test]
#[ignore = "Renders compact, loading, error and result account-search states"]
fn render_compact_account_search() {
    let app = search_fixture(0);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let directory = std::path::Path::new("target/compact-account-search-review");
    std::fs::create_dir_all(directory).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        app.set_account_search_items(accounts(0));
        app.set_account_search_error(String::new());
        app.set_account_search_busy(false);
        for (name, count, busy, error) in [
            ("empty", 0, false, ""),
            ("loading", 0, true, ""),
            (
                "error",
                0,
                false,
                "Could not connect to X search. Check your connection or proxy and try again.",
            ),
            ("few-results", 2, false, ""),
            ("results", 30, false, ""),
        ] {
            app.set_account_search_items(accounts(count));
            if count == 2 {
                app.set_account_search_items(ui_runtime::ModelRc::new(ui_runtime::VecModel::from(
                    vec![
                        crate::AccountSearchItem {
                            name: "An account with a display name much longer than its card".into(),
                            handle: "long_username_that_must_be_truncated_in_the_card".into(),
                            verified: true,
                            ..Default::default()
                        },
                        crate::AccountSearchItem {
                            name: "Another account".into(),
                            handle: "anotheruser".into(),
                            ..Default::default()
                        },
                    ],
                )));
            }
            app.set_account_search_busy(busy);
            app.set_account_search_error(error.into());
            h.remove_cursor();
            h.run_steps(5);
            h.render()
                .unwrap()
                .save(directory.join(format!("{theme}-{name}.png")))
                .unwrap();
        }
    }
}
