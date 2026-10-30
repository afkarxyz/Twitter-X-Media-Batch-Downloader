use crate::ui::components::dialog::child;
use crate::ui::components::icon_button::IconStyle;
use crate::ui::*;

const TRON: &str = "THnzAAwZgp2Sq5CAXLP2njQDhTvgZG9EWs";
const EVM: &str = "0xB563a7F39770C151e2FacE26926081a00c5EF349";
const USDT_NETWORKS: [&str; 4] = [
    "Ethereum (ERC20)",
    "BNB Smart Chain (BEP20)",
    "Tron (TRC20)",
    "Polygon",
];
const USDC_NETWORKS: [&str; 4] = [
    "BNB Smart Chain (BEP20)",
    "Polygon",
    "Ethereum (ERC20)",
    "Base",
];

impl Desktop {
    pub(in crate::ui) fn support_view(&mut self, ui: &mut Ui) {
        if self.support_codes.is_none() {
            let assets: [(&str, &[u8]); 3] = [
                (
                    "support-usdt-tron",
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/assets/support/usdt.jpg"
                    )),
                ),
                (
                    "support-usdt-evm",
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/assets/support/usdt_evm.jpg"
                    )),
                ),
                (
                    "support-usdc",
                    include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/assets/support/usdc.jpg"
                    )),
                ),
            ];
            self.support_codes = Some(assets.map(|(name, bytes)| {
                let image = image::load_from_memory(bytes)
                    .expect("embedded wallet QR must be a valid image")
                    .to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                ui.ctx().load_texture(
                    name,
                    egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
                    egui::TextureOptions::LINEAR,
                )
            }));
        }
        if !self.support_opened {
            self.support_network = 2;
            self.support_usdc_network = 0;
            self.support_opened = true;
        }
        let bounds = ui.max_rect();
        let colors = self.colors(ui);
        egui::ScrollArea::vertical()
            .id_salt("support-page")
            .scroll_bar_rect(bounds.shrink2(Vec2::new(0.0, 16.0)))
            .auto_shrink([false, false])
            .show_themed(ui, |ui| {
                let width = (ui.available_width() - 24.0).min(appearance::SUPPORT_WIDTH);
                let description =
                    self.tr("Scan the QR code or copy the address to support the project.");
                let mut job = egui::text::LayoutJob::simple(
                    description,
                    egui::FontId::proportional(12.0),
                    colors.secondary,
                    width,
                );
                job.sections[0].format.line_height = Some(17.0);
                let description = ui.painter().layout_job(job);
                let first_y = 108.0 + description.size().y;
                let height = (first_y + 392.0).max(bounds.height());
                let (page, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), height),
                    egui::Sense::hover(),
                );
                let left = page.center().x - width / 2.0;
                ui.painter().text(
                    egui::pos2(left + 6.0, page.top() + 34.0),
                    egui::Align2::LEFT_CENTER,
                    self.tr("Support"),
                    egui::FontId::new(
                        typography::TITLE,
                        egui::FontFamily::Name("cera-bold".into()),
                    ),
                    colors.title,
                );
                ui.painter().text(
                    egui::pos2(left, page.top() + 79.0),
                    egui::Align2::LEFT_CENTER,
                    self.tr("Buy Me a Coffee"),
                    egui::FontId::new(
                        typography::SUBTITLE,
                        egui::FontFamily::Name("cera-bold".into()),
                    ),
                    colors.heading,
                );
                ui.painter().galley(
                    egui::pos2(left, page.top() + 95.0),
                    description,
                    colors.secondary,
                );
                let card = egui::Rect::from_min_size(
                    egui::pos2(left, page.top() + first_y),
                    Vec2::new(width, 184.0),
                );
                self.support_network =
                    self.support_card(ui, card, "USDT", &USDT_NETWORKS, self.support_network);
                self.support_usdc_network = self.support_card(
                    ui,
                    card.translate(Vec2::new(0.0, 196.0)),
                    "USDC",
                    &USDC_NETWORKS,
                    self.support_usdc_network,
                );
            });
    }

    fn support_card(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        coin: &str,
        networks: &[&str; 4],
        network: usize,
    ) -> usize {
        let colors = self.colors(ui);
        let mut card = child(ui, coin, rect);
        let response = card.allocate_rect(rect, egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                true,
                format!("{coin} support card"),
            )
        });
        card.painter().rect_filled(rect, 8, colors.surface);
        card.painter().rect_stroke(
            rect,
            8,
            egui::Stroke::new(1.0, colors.border),
            egui::StrokeKind::Inside,
        );
        let left = rect.left() + 188.0;
        let controls_width = (rect.width() - 204.0).min(340.0);
        card.painter().text(
            egui::pos2(left, rect.top() + 41.0),
            egui::Align2::LEFT_CENTER,
            coin,
            egui::FontId::new(
                typography::SUBTITLE,
                egui::FontFamily::Name("cera-bold".into()),
            ),
            colors.heading,
        );
        let picker = egui::Rect::from_min_size(
            egui::pos2(left, rect.top() + 66.0),
            Vec2::new(controls_width, appearance::CONTROL_HEIGHT),
        );
        let network = self.support_network_picker(&mut card, picker, coin, networks, network);
        let (address, code) = if coin == "USDT" && network == 2 {
            (TRON, 0)
        } else if coin == "USDT" {
            (EVM, 1)
        } else {
            (EVM, 2)
        };
        let qr = egui::Rect::from_min_size(rect.min + Vec2::splat(16.0), Vec2::splat(152.0));
        card.painter().rect_filled(qr, 8, Color32::WHITE);
        let mut image = child(&mut card, "wallet-qr", qr.shrink(8.0));
        image.add(
            egui::Image::new(&self.support_codes.as_ref().unwrap()[code])
                .fit_to_exact_size(Vec2::splat(136.0))
                .alt_text(format!("{coin} · {} · {address}", networks[network])),
        );
        let address_rect = egui::Rect::from_min_size(
            egui::pos2(left, rect.top() + 114.0),
            Vec2::new(controls_width, appearance::CONTROL_HEIGHT),
        );
        card.painter().rect_filled(address_rect, 6, colors.surface);
        card.painter().rect_stroke(
            address_rect,
            6,
            egui::Stroke::new(1.0, colors.border),
            egui::StrokeKind::Inside,
        );
        let mut job = egui::text::LayoutJob::simple(
            address.into(),
            egui::FontId::proportional(12.0),
            colors.body,
            controls_width - 54.0,
        );
        job.wrap.max_rows = 1;
        let text = card.painter().layout_job(job);
        card.painter().galley(
            egui::pos2(
                address_rect.left() + 10.0,
                address_rect.center().y - text.size().y / 2.0,
            ),
            text,
            colors.body,
        );
        let copy = egui::Rect::from_center_size(
            address_rect.right_center() - Vec2::new(18.0, 0.0),
            Vec2::splat(appearance::COMPACT_CONTROL_SIZE),
        );
        let response = self.home_icon(
            &mut child(&mut card, "copy-wallet", copy),
            Icon::Copy,
            "Copy wallet address",
            appearance::COMPACT_CONTROL_SIZE,
            IconStyle {
                idle: Color32::TRANSPARENT,
                hover: Color32::TRANSPARENT,
                pressed: Color32::TRANSPARENT,
                foreground: colors.body,
                border: egui::Stroke::NONE,
                radius: 6,
                icon_size: 16.0,
                visible: true,
            },
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("{} · {coin}", self.tr("Copy wallet address")),
            )
        });
        if response.clicked() {
            self.app.invoke_copy_address(address.into());
        }
        network
    }

    fn support_network_picker(
        &self,
        ui: &mut Ui,
        rect: egui::Rect,
        coin: &str,
        choices: &[&str; 4],
        selected: usize,
    ) -> usize {
        let choices = choices.map(str::to_owned);
        let label = format!("{} · {coin}", self.tr("Network"));
        self.choice_picker(
            ui,
            rect,
            &crate::ui::components::dropdown::SelectOptions::new(&label, &choices),
            selected,
        )
    }
}
