use super::*;

const TRON: &str = "THnzAAwZgp2Sq5CAXLP2njQDhTvgZG9EWs";
const EVM: &str = "0xB563a7F39770C151e2FacE26926081a00c5EF349";

fn qr_texture(h: &Harness<'_, Desktop>, label: &str) -> egui::TextureId {
    let rect = h.get_by_label(label).rect();
    h.output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(shape) if shape.rect == rect => {
                shape.brush.as_ref().map(|brush| brush.fill_texture_id)
            }
            _ => None,
        })
        .expect("QR must be painted from the wallet image asset")
}

fn choose(h: &mut Harness<'_, Desktop>, coin: &str, network: &str) {
    h.get_by_label(&format!("Network · {coin}")).click();
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::Button, network)
        .click();
    h.run();
}

#[test]
fn support_cards_match_legacy_bounds_and_keep_the_address_on_one_line() {
    let app = fixture();
    app.set_selected_tab(3);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    for (coin, y) in [("USDT", 125.0), ("USDC", 321.0)] {
        assert_eq!(
            h.get_by_label(&format!("{coin} support card")).rect(),
            egui::Rect::from_min_size(egui::pos2(268.0, y), Vec2::new(544.0, 184.0))
        );
        assert_eq!(
            h.get_by_label(&format!("Network · {coin}")).rect(),
            egui::Rect::from_min_size(egui::pos2(456.0, y + 66.0), Vec2::new(340.0, 32.0))
        );
        assert_eq!(
            h.get_by_label(&format!("Copy wallet address · {coin}"))
                .rect(),
            egui::Rect::from_min_size(egui::pos2(764.0, y + 116.0), Vec2::splat(28.0))
        );
    }
    let qr = format!("USDT · Tron (TRC20) · {TRON}");
    assert_eq!(
        h.get_by_label(&qr).rect(),
        egui::Rect::from_min_size(egui::pos2(292.0, 149.0), Vec2::splat(136.0))
    );
    let address = h
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == EVM => Some(text),
            _ => None,
        })
        .unwrap();
    assert_eq!(address.galley.rows.len(), 1);
    assert!(address.pos.x + address.galley.size().x <= 742.0);
    h.set_size(Vec2::new(1440.0, 900.0));
    h.run();
    assert_eq!(h.get_by_label("USDT support card").rect().left(), 476.0);
}

#[test]
fn support_networks_switch_the_qr_and_copy_address_independently() {
    let app = fixture();
    app.set_selected_tab(3);
    let copied = Rc::new(RefCell::new(Vec::new()));
    app.on_copy_address({
        let copied = copied.clone();
        move |address| copied.borrow_mut().push(address)
    });
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    let tron_qr = qr_texture(&h, &format!("USDT · Tron (TRC20) · {TRON}"));
    let mut evm_qr = None;
    for (index, network) in [
        "Ethereum (ERC20)",
        "BNB Smart Chain (BEP20)",
        "Tron (TRC20)",
        "Polygon",
    ]
    .iter()
    .enumerate()
    {
        choose(&mut h, "USDT", network);
        assert_eq!(h.state().support_network, index);
        assert_eq!(h.state().support_usdc_network, 0);
        let expected = if index == 2 { TRON } else { EVM };
        let qr = qr_texture(&h, &format!("USDT · {network} · {expected}"));
        if index == 2 {
            assert_eq!(qr, tron_qr);
        } else {
            assert_ne!(qr, tron_qr);
            if let Some(evm) = evm_qr {
                assert_eq!(qr, evm);
            } else {
                evm_qr = Some(qr);
            }
        }
        h.get_by_label("Copy wallet address · USDT").click();
        h.run();
        assert_eq!(copied.borrow().last().unwrap(), expected);
    }
    let usdc_qr = qr_texture(&h, &format!("USDC · BNB Smart Chain (BEP20) · {EVM}"));
    assert_ne!(usdc_qr, evm_qr.unwrap());
    for (index, network) in [
        "BNB Smart Chain (BEP20)",
        "Polygon",
        "Ethereum (ERC20)",
        "Base",
    ]
    .iter()
    .enumerate()
    {
        choose(&mut h, "USDC", network);
        assert_eq!(h.state().support_usdc_network, index);
        assert_eq!(h.state().support_network, 3);
        assert_eq!(
            qr_texture(&h, &format!("USDC · {network} · {EVM}")),
            usdc_qr
        );
        h.get_by_label("Copy wallet address · USDC").click();
        h.run();
        assert_eq!(copied.borrow().last().unwrap(), EVM);
    }
    assert_eq!(copied.borrow().len(), 8);
}

#[test]
fn support_keyboard_selects_or_cancels_networks_and_leaves_no_popup_on_navigation() {
    let app = fixture();
    app.set_selected_tab(3);
    let copied = Rc::new(RefCell::new(Vec::new()));
    app.on_copy_address({
        let copied = copied.clone();
        move |address| copied.borrow_mut().push(address)
    });
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    h.run();
    h.get_by_label("Network · USDT").focus();
    h.run();
    h.key_press(egui::Key::ArrowDown);
    h.run();
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key_press(egui::Key::ArrowDown);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().support_network, 3);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    h.key_press(egui::Key::ArrowUp);
    h.run();
    h.key_press(egui::Key::ArrowUp);
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().support_network, 3);
    h.key_press(egui::Key::Tab);
    h.run();
    assert!(
        h.get_by_label("Copy wallet address · USDT")
            .accesskit_node()
            .is_focused()
    );
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(*copied.borrow(), [EVM]);
    h.get_by_label("Network · USDC").click();
    h.run();
    let networks = (h.state().support_network, h.state().support_usdc_network);
    h.get_by_label("History").click();
    h.run();
    assert!(!egui::Popup::is_any_open(&h.ctx));
    h.get_by_label("Support").click();
    h.run();
    assert_eq!(
        (h.state().support_network, h.state().support_usdc_network),
        networks
    );
    h.get_by_label("Copy wallet address · USDT").hover();
    h.run_steps(4);
    assert_painted_text_color(&h, "Copy wallet address", Color32::WHITE);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
}

#[test]
fn support_scrolls_without_stretching_cards_or_losing_copy_actions() {
    let app = fixture();
    app.set_selected_tab(3);
    app.set_theme_mode("dark".into());
    let copied = Rc::new(RefCell::new(String::new()));
    app.on_copy_address({
        let copied = copied.clone();
        move |address| *copied.borrow_mut() = address
    });
    let mut h = harness(app);
    h.set_size(Vec2::new(1024.0, 450.0));
    h.run();
    let bar = h.get_by_role(egui::accesskit::Role::ScrollBar).rect();
    let thumb = painted_scroll_thumb(&h, bar).0;
    h.drag_at(thumb.center());
    h.run_steps(1);
    let target = thumb.center() + Vec2::new(0.0, 80.0);
    h.hover_at(target);
    h.run_steps(3);
    h.drop_at(target);
    h.run_steps(3);
    let card = h.get_by_label("USDC support card").rect();
    assert_eq!(card.size(), Vec2::new(544.0, 184.0));
    assert!(card.bottom() < 450.0);
    h.get_by_label("Copy wallet address · USDC").click();
    h.run();
    assert_eq!(&*copied.borrow(), EVM);
}

#[test]
#[ignore = "Renders Support cards, wallet networks, and hover states"]
fn render_support_review() {
    let app = fixture();
    app.set_selected_tab(3);
    let mut h = harness(app.clone());
    h.set_size(Vec2::new(1024.0, 600.0));
    let dir = std::env::var_os("TWITTERDL_SUPPORT_SNAPSHOTS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "target/support-review".into());
    std::fs::create_dir_all(&dir).unwrap();
    for theme in ["dark", "light"] {
        app.set_theme_mode(theme.into());
        h.remove_cursor();
        h.run_steps(4);
        std::thread::sleep(std::time::Duration::from_millis(50));
        h.run_steps(3);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-support.png")))
            .unwrap();
        h.get_all_by_role(egui::accesskit::Role::ComboBox)
            .next()
            .unwrap()
            .click();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-network-menu.png")))
            .unwrap();
        h.get_by_role_and_label(egui::accesskit::Role::Button, "Ethereum (ERC20)")
            .click();
        h.run_steps(4);
        h.remove_cursor();
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-usdt-ethereum.png")))
            .unwrap();
        choose(&mut h, "USDT", "Tron (TRC20)");
        h.get_by_label("Copy wallet address · USDT").hover();
        h.run_steps(4);
        h.render()
            .unwrap()
            .save(dir.join(format!("{theme}-copy-hover.png")))
            .unwrap();
    }
    app.global::<I18n>().set_language("fr".into());
    h.set_size(Vec2::new(1440.0, 900.0));
    h.remove_cursor();
    h.run_steps(4);
    h.render()
        .unwrap()
        .save(dir.join("light-french-1440.png"))
        .unwrap();
}
