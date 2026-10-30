//! Native egui views sharing commands with the asynchronous workers.
use crate::AppTheme;
use crate::AppWindow;
use crate::I18n;
use crate::MediaItem;
use crate::MediaResults;
use crate::MediaSelection;
use crate::PreviewPlayback;
use crate::RendererSettings;
use crate::ui::runtime::{self as ui_runtime, Image, Model};
use appearance::ThemedScrollArea;
use egui::{Color32, RichText, Ui, Vec2};
use std::collections::{HashMap, HashSet};
mod components;
mod dialogs;
pub(crate) mod runtime;
mod screens;
#[cfg(test)]
mod tests;
mod theme;
use components::controls::{ActionStyle, ButtonTone};
use components::icons::Icon;
use screens::{history, settings};
pub(crate) use theme::schemes;
use theme::{appearance, swatches, typography};

pub fn run(app: AppWindow) -> Result<(), ui_runtime::PlatformError> {
    let preferred = app.global::<RendererSettings>().get_preferred();
    let result = run_with_renderer(app.clone(), preferred == "gpu");
    if let Err(error) = &result {
        if preferred == "gpu" {
            crate::platform::logging::warn(format!(
                "wgpu could not start; retrying with OpenGL: {error}"
            ));
            app.global::<RendererSettings>().set_active("cpu".into());
            app.global::<RendererSettings>()
                .set_error(format!("wgpu could not start. Using OpenGL: {error}"));
            return run_with_renderer(app, false);
        }
    }
    result
}

fn run_with_renderer(app: AppWindow, gpu: bool) -> Result<(), ui_runtime::PlatformError> {
    let icon = image::load_from_memory(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/logo/logo.png"
    )))
    .ok()
    .map(|image| {
        let rgba = image.to_rgba8();
        std::sync::Arc::new(egui::IconData {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        })
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1024.0, 600.0])
        .with_min_inner_size([1024.0, 600.0])
        .with_app_id(crate::APP_ID);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: if gpu {
            eframe::Renderer::Wgpu
        } else {
            eframe::Renderer::Glow
        },
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Twitter/X Media Batch Downloader",
        options,
        Box::new(move |cc| {
            setup(&cc.egui_ctx);
            crate::platform::appearance::watch({
                let ctx = cc.egui_ctx.clone();
                move || ctx.request_repaint()
            });
            ui_runtime::attach(&cc.egui_ctx);
            Ok(Box::new(Desktop::new(app)))
        }),
    )
    .map_err(|error| ui_runtime::PlatformError::Other(error.to_string()))
}

fn text_rendering() -> fastframe_text::TextRendering {
    // Slint's reference renderer uses linear glyph coverage; egui's dark
    // transfer function otherwise makes the same Cera Round fonts heavier.
    fastframe_text::TextRendering {
        coverage: fastframe_text::Coverage::Linear,
        ..fastframe_text::TextRendering::platform_default()
    }
}

fn setup(ctx: &egui::Context) {
    let mut fonts = fastframe_fonts::FontSetup::default().definitions();
    fonts.font_data.insert(
        "cera-round".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/Cera_Round_Regular.ttf"
        )))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "cera-round".into());
    for (name, data) in [
        (
            "cera-medium",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Cera_Round_Medium.ttf"
            ))
            .as_slice(),
        ),
        (
            "cera-bold",
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Cera_Round_Bold.ttf"
            ))
            .as_slice(),
        ),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(data)),
        );
        let mut family = fonts.families[&egui::FontFamily::Proportional].clone();
        family.insert(0, name.into());
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), family);
    }
    text_rendering().apply_to(&mut fonts);
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    fastframe_icons::install::<Icon>(ctx);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 10.0);
        style.spacing.button_padding = Vec2::new(12.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.combo_height = 300.0;
        style.spacing.scroll = egui::style::ScrollStyle {
            bar_width: 6.0,
            floating_width: 2.0,
            bar_outer_margin: 4.0,
            handle_min_length: 16.0,
            foreground_color: false,
            dormant_handle_opacity: 1.0,
            active_handle_opacity: 1.0,
            interact_handle_opacity: 1.0,
            dormant_background_opacity: 0.0,
            active_background_opacity: 0.0,
            interact_background_opacity: 1.0,
            ..egui::style::ScrollStyle::floating()
        };
        style.interaction.tooltip_delay = 0.45;
        style.interaction.tooltip_grace_time = 0.0;
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(typography::BODY),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(typography::BODY),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::new(
                typography::TITLE,
                egui::FontFamily::Name("cera-bold".into()),
            ),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}

pub(super) struct Desktop {
    app: AppWindow,
    scrolling: fastframe_scroll::Scrolling,
    textures: HashMap<u64, (egui::TextureHandle, u64)>,
    banner_fade: Option<(Color32, egui::TextureHandle)>,
    about_logo: Option<egui::TextureHandle>,
    frame: u64,
    kind: String,
    limit: usize,
    results_jump_requested: bool,
    settings_key: String,
    proxy: String,
    speed: String,
    delay: String,
    jitter: String,
    fetch_delay: String,
    fetch_jitter: String,
    fetch_timeout: String,
    http_timeout: String,
    labels_group: String,
    labels_tags: String,
    labels_family: usize,
    labels_shade: usize,
    labels_dialog_opened: bool,
    keep_original_gifs: bool,
    archive_options: Option<(usize, usize)>,
    settings_sources: Option<settings::InputSources>,
    settings_category: Option<i32>,
    settings_output_seen: String,
    history_confirmation: Option<history::HistoryConfirmation>,
    history_scroll_key: String,
    support_network: usize,
    support_usdc_network: usize,
    support_opened: bool,
    support_codes: Option<[egui::TextureHandle; 3]>,
    account_dialog_opened: bool,
    account_result_index: usize,
    account_scroll_reset: bool,
    account_page_requested: Option<(String, String)>,
    fetch_dialog_opened: bool,
    preview_index: Option<i32>,
    preview_canvas: dialogs::preview_canvas::PreviewCanvas,
    preview_texture: dialogs::preview_canvas::PreviewTexture,
    result_view: crate::media::results::ViewCache,
    result_workspace: i32,
    result_workspaces: HashMap<i32, (crate::media::results::ViewCache, String, usize)>,
    page_seen: i32,
    history_revision: i32,
    library_account_seen: String,
    batch_list_editing: bool,
    batch_sources_draft: Option<String>,
    preview_view: crate::media::results::ViewCache,
    visible_media: HashSet<i32>,
}
impl Desktop {
    fn new(app: AppWindow) -> Self {
        Self {
            proxy: app.get_proxy_url(),
            speed: app.get_download_speed_kib().to_string(),
            delay: app.get_download_delay_ms().to_string(),
            jitter: app.get_download_jitter_ms().to_string(),
            fetch_delay: app.get_fetch_delay_ms().to_string(),
            fetch_jitter: app.get_fetch_jitter_ms().to_string(),
            fetch_timeout: app.get_fetch_timeout_secs().to_string(),
            http_timeout: app.get_http_timeout_secs().to_string(),
            app,
            scrolling: Default::default(),
            textures: HashMap::new(),
            banner_fade: None,
            about_logo: None,
            frame: 0,
            kind: "all".into(),
            limit: 30,
            results_jump_requested: false,
            settings_key: String::new(),
            labels_group: String::new(),
            labels_tags: String::new(),
            labels_family: 0,
            labels_shade: 4,
            labels_dialog_opened: false,
            keep_original_gifs: true,
            archive_options: None,
            settings_sources: None,
            settings_category: None,
            settings_output_seen: String::new(),
            history_confirmation: None,
            history_scroll_key: String::new(),
            support_network: 2,
            support_usdc_network: 0,
            support_opened: false,
            support_codes: None,
            account_dialog_opened: false,
            account_result_index: 0,
            account_scroll_reset: true,
            account_page_requested: None,
            fetch_dialog_opened: false,
            preview_index: None,
            preview_canvas: Default::default(),
            preview_texture: Default::default(),
            result_view: Default::default(),
            result_workspace: 0,
            result_workspaces: HashMap::new(),
            page_seen: -1,
            history_revision: 0,
            library_account_seen: String::new(),
            batch_list_editing: false,
            batch_sources_draft: None,
            preview_view: Default::default(),
            visible_media: HashSet::new(),
        }
    }
    fn theme(&mut self, ctx: &egui::Context) {
        let mode = self.app.get_theme_mode();
        let preset = self.app.global::<AppTheme>().get_preset();
        let dark = match mode.as_str() {
            "dark" => true,
            "light" => false,
            _ => ctx
                .input(|i| i.raw.system_theme)
                .map(|theme| theme == egui::Theme::Dark)
                .or_else(crate::platform::appearance::prefers_dark)
                .unwrap_or(false),
        };
        let key = format!("{mode}:{preset}:{dark}");
        if self.settings_key == key {
            return;
        }
        self.settings_key = key;
        let mut visuals = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        appearance::Palette::new(&preset, dark).apply(&mut visuals);
        visuals.weak_text_color = Some(if dark {
            Color32::from_gray(170)
        } else {
            Color32::from_gray(90)
        });
        visuals.error_fg_color = if dark {
            Color32::from_rgb(255, 153, 153)
        } else {
            Color32::from_rgb(180, 35, 24)
        };
        text_rendering().apply_to_visuals(&mut visuals);
        ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        ctx.set_visuals(visuals);
    }
    fn tr(&self, key: &str) -> String {
        crate::i18n::translate(&self.app.global::<I18n>().get_language(), key)
    }
    fn error(&self, ui: &mut Ui, error: &str) {
        if !error.is_empty() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
    fn busy(&self) -> bool {
        self.app.get_is_busy()
            || (self.app.get_selected_tab() == crate::library::TAB
                && !self.app.get_library().job.is_empty())
            || self.app.get_xtractor_busy()
            || self.app.get_tool_update_busy()
            || self.app.get_history_busy()
            || self.app.get_ffmpeg_busy()
    }
}
impl Desktop {
    fn render(&mut self, ui: &mut Ui) {
        self.visible_media.clear();
        if self.history_revision != self.app.get_history_revision() {
            self.history_revision = self.app.get_history_revision();
            self.result_workspaces.clear();
            self.result_view = Default::default();
            self.preview_view = Default::default();
            self.limit = 30;
            self.kind = "all".into();
            self.library_account_seen.clear();
        }

        if let Some(model) = self
            .app
            .get_media_items()
            .as_any()
            .downcast_ref::<crate::history::paged::PagedModel>()
        {
            model.set_language(self.app.global::<crate::I18n>().get_language());
        }
        let tab = self.app.get_selected_tab();
        let workspace = match tab {
            4 => 2,
            0 => i32::from(self.app.get_batch_mode()),
            _ => self.result_workspace,
        };
        if workspace != self.result_workspace {
            self.result_workspaces.insert(
                self.result_workspace,
                (
                    std::mem::take(&mut self.result_view),
                    std::mem::take(&mut self.kind),
                    self.limit,
                ),
            );
            let (view, kind, limit) = self
                .result_workspaces
                .remove(&workspace)
                .unwrap_or_else(|| (Default::default(), "all".into(), 30));
            self.result_view = view;
            self.kind = kind;
            self.limit = limit;
            self.result_workspace = workspace;
        }
        let displaying_media = (tab == 0
            && !self.app.get_home_results_hidden()
            && (!self.app.get_batch_mode() || self.app.get_batch_result_open()))
            || (tab == 4
                && self.app.get_library().view_ready
                && !self.app.get_library().files_view
                && !self.app.get_library().workspace_pending);
        if displaying_media && self.result_view.source_changed(&self.app.get_media_items()) {
            self.result_view = Default::default();
            self.preview_view = Default::default();
            self.preview_canvas.reset();
            if tab == 0 {
                self.limit = 30;
            }
            self.results_jump_requested = false;
        }
        if tab != 0 {
            self.results_jump_requested = false;
        }
        let ctx = ui.ctx().clone();
        if self.page_seen != tab {
            self.scrolling.stop();
            egui::Popup::close_all(&ctx);
            self.page_seen = tab;
        }
        self.frame += 1;
        self.theme(&ctx);
        self.scrolling.apply(&ctx);
        egui::Panel::left("navigation")
            .resizable(false)
            .exact_size(appearance::RAIL_WIDTH)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(self.colors(ui).sidebar)
                    .inner_margin(0),
            )
            .show(ui, |ui| {
                if self.app.get_xtractor_setup_required() {
                    ui.disable();
                }
                self.product_rail(ui);
            });
        if self.app.get_xtractor_setup_required() {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(self.colors(ui).page)
                        .inner_margin(0),
                )
                .show(ui, |_| {});
            self.xtractor_setup_dialog(&ctx);
            return;
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(self.colors(ui).page)
                    .inner_margin(0),
            )
            .show(ui, |ui| match self.app.get_selected_tab() {
                1 => self.history(ui),
                2 => self.settings(ui),
                3 => self.support_view(ui),
                4 => self.library_view(ui),
                _ => self.home_view(ui),
            });
        self.preview_overlay(&ctx);
        self.dialogs(&ctx);
        if self.app.get_selected_media_index() >= 0 {
            self.visible_media
                .insert(self.app.get_selected_media_index());
        }
        crate::media::images::retain_thumbnails(&self.app, &self.visible_media);
        self.textures
            .retain(|_, (_, used)| self.frame.saturating_sub(*used) < 3);
    }
}
impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        ui_runtime::pump(ctx);
    }
    fn ui(&mut self, ui: &mut Ui, _: &mut eframe::Frame) {
        self.render(ui);
    }
}
#[cfg(test)]
fn palette_color(preset: &str, dark: bool) -> Color32 {
    appearance::Palette::new(preset, dark).accent
}
