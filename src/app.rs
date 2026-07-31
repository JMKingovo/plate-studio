//! egui 图形界面

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Frame, Key, Margin, RichText, Stroke, TextureHandle, Vec2,
};
use tokio::runtime::Handle;

use crate::api;
use crate::generator::PlateGenerator;
use crate::state::{AppState, PlateRecord};

// 视觉：沥青灰底 + 深蓝强调，避开常见 AI 紫/奶油陶土风
const BG: Color32 = Color32::from_rgb(236, 239, 243);
const PANEL: Color32 = Color32::from_rgb(250, 251, 253);
const INK: Color32 = Color32::from_rgb(22, 32, 48);
const MUTED: Color32 = Color32::from_rgb(110, 122, 140);
const ACCENT: Color32 = Color32::from_rgb(24, 78, 119);
const ACCENT_HOVER: Color32 = Color32::from_rgb(32, 98, 148);
const LINE: Color32 = Color32::from_rgb(214, 220, 230);
const OK: Color32 = Color32::from_rgb(36, 140, 96);
const BAD: Color32 = Color32::from_rgb(180, 64, 58);

const COLOR_OPTIONS: &[(&str, &str)] = &[
    ("blue", "蓝牌"),
    ("yellow", "黄牌"),
    ("green_car", "新能源"),
    ("green_truck", "新能源卡"),
    ("white", "警车"),
    ("white_army", "军车"),
    ("black", "港澳"),
    ("black_shi", "使领馆"),
];

pub struct PlateStudioApp {
    state: AppState,
    rt: Handle,
    generator: Arc<PlateGenerator>,
    preview_texture: Option<TextureHandle>,
    preview_path: Option<String>,
    last_seq: u64,
    status_msg: String,
    plate_fullscreen: bool,
}

fn install_cjk_fonts(ctx: &egui::Context) {
    let candidates = [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyhbd.ttc",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    let Some(path) = candidates.iter().find(|p| std::path::Path::new(p).exists()) else {
        return;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "cjk".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("cjk".to_owned());
    ctx.set_fonts(fonts);
}

fn apply_theme(ctx: &egui::Context) {
    // 必须从 light 起步，否则 ComboBox 展开态 widgets.open 仍是深色黑底
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.window_margin = Margin::same(0);

    let soft = Color32::from_rgb(244, 246, 249);
    let soft_hover = Color32::from_rgb(232, 238, 246);
    let soft_active = Color32::from_rgb(220, 230, 242);

    style.visuals = egui::Visuals::light();
    style.visuals.dark_mode = false;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.panel_fill = PANEL;
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.faint_bg_color = soft;
    style.visuals.code_bg_color = Color32::WHITE;
    style.visuals.override_text_color = Some(INK);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.selection.bg_fill = Color32::from_rgb(210, 228, 245);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);

    let mk = |bg: Color32, stroke: Color32, fg: Color32| egui::style::WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: bg,
        bg_stroke: Stroke::new(1.0_f32, stroke),
        fg_stroke: Stroke::new(1.0_f32, fg),
        corner_radius: CornerRadius::same(6),
        expansion: 0.0,
    };
    style.visuals.widgets.noninteractive = mk(soft, LINE, MUTED);
    style.visuals.widgets.inactive = mk(soft, LINE, INK);
    style.visuals.widgets.hovered = mk(soft_hover, Color32::from_rgb(180, 196, 214), INK);
    style.visuals.widgets.active = mk(soft_active, ACCENT, INK);
    // ComboBox 展开时用的就是 open，之前漏设所以发黑
    style.visuals.widgets.open = mk(Color32::WHITE, ACCENT, INK);

    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(22.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::new(15.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(15.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        FontId::new(12.5, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

fn color_label(key: &str) -> String {
    COLOR_OPTIONS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| key.to_string())
}

fn primary_button(ui: &mut egui::Ui, text: &str, height: f32) -> egui::Response {
    let w = ui.available_width();
    let size = Vec2::new(w, height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let hovered = response.hovered();
    let fill = if hovered { ACCENT_HOVER } else { ACCENT };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::new(16.0, egui::FontFamily::Proportional),
        Color32::WHITE,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn ghost_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let response = ui.add_sized(
        Vec2::new(ui.available_width(), 34.0),
        egui::Button::new(RichText::new(text).color(ACCENT)).frame(true),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn panel_frame() -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .inner_margin(Margin::symmetric(18, 16))
        .corner_radius(CornerRadius::same(0))
}

impl PlateStudioApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        state: AppState,
        rt: Handle,
        generator: Arc<PlateGenerator>,
    ) -> Self {
        install_cjk_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx);

        Self {
            state,
            rt,
            generator,
            preview_texture: None,
            preview_path: None,
            last_seq: 0,
            status_msg: "准备就绪".into(),
            plate_fullscreen: false,
        }
    }

    fn sync_from_state(&mut self, ctx: &egui::Context) {
        let (seq, latest, listening, _port, err, fullscreen) = self.rt.block_on(async {
            let data = self.state.inner.read().await;
            (
                data.generation_seq,
                data.latest.clone(),
                data.api_listening,
                data.api_port,
                data.api_error.clone(),
                data.plate_fullscreen,
            )
        });

        if fullscreen != self.plate_fullscreen {
            self.plate_fullscreen = fullscreen;
        }

        if seq != self.last_seq {
            self.last_seq = seq;
            if let Some(record) = latest {
                self.status_msg = record.plate.clone();
                self.update_preview(ctx, &record);
            }
        }

        if let Some(e) = err {
            if !listening {
                self.status_msg = format!("服务异常：{e}");
            }
        }
    }

    fn set_fullscreen(&mut self, enabled: bool) {
        if self.plate_fullscreen == enabled {
            return;
        }
        self.plate_fullscreen = enabled;
        self.rt.block_on(self.state.set_fullscreen(enabled, "gui"));
    }

    fn update_preview(&mut self, ctx: &egui::Context, record: &PlateRecord) {
        if self.preview_path.as_deref() == Some(record.image_path.as_str()) {
            return;
        }
        let bytes = if let Some(b64) = &record.image_base64 {
            match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
                Ok(b) => b,
                Err(_) => return,
            }
        } else {
            match std::fs::read(&record.image_path) {
                Ok(b) => b,
                Err(_) => return,
            }
        };
        let img = match image::load_from_memory(&bytes) {
            Ok(i) => i.to_rgba8(),
            Err(_) => return,
        };
        let size = [img.width() as usize, img.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        self.preview_texture = Some(ctx.load_texture(
            "plate_preview",
            color_image,
            egui::TextureOptions::LINEAR,
        ));
        self.preview_path = Some(record.image_path.clone());
    }

    fn on_generate(&mut self) {
        let (random, custom, color) = self.rt.block_on(async {
            let data = self.state.inner.read().await;
            (
                data.use_random,
                data.custom_plate.clone(),
                data.selected_color.clone(),
            )
        });

        let result = self.rt.block_on(api::generate_from_gui(
            &self.state,
            self.generator.clone(),
            if random { None } else { Some(custom) },
            Some(color),
            random,
        ));

        match result {
            Ok(record) => self.status_msg = record.plate,
            Err(e) => self.status_msg = format!("生成失败：{e}"),
        }
    }
}

impl eframe::App for PlateStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_from_state(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        if ctx.input(|i| i.key_pressed(Key::F11)) && self.preview_texture.is_some() {
            let next = !self.plate_fullscreen;
            self.set_fullscreen(next);
        }
        if self.plate_fullscreen && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.set_fullscreen(false);
        }

        if self.plate_fullscreen {
            egui::CentralPanel::default()
                .frame(Frame::NONE.fill(Color32::from_rgb(14, 18, 24)))
                .show(ctx, |ui| {
                    let plate_text = self.rt.block_on(async {
                        self.state
                            .inner
                            .read()
                            .await
                            .latest
                            .as_ref()
                            .map(|r| r.plate.clone())
                            .unwrap_or_default()
                    });

                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.0;
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("退出").color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(40, 48, 60)),
                                )
                                .clicked()
                            {
                                self.set_fullscreen(false);
                            }
                            ui.label(
                                RichText::new(&plate_text)
                                    .size(36.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            );
                            ui.label(RichText::new("Esc").size(13.0).color(Color32::from_gray(140)));
                        });
                        ui.add_space(28.0);

                        let avail = ui.available_size();
                        let max_w = (avail.x - 48.0).max(200.0);
                        let max_h = (avail.y - 80.0).max(120.0);
                        if let Some(tex) = &self.preview_texture {
                            let aspect = tex.size()[0] as f32 / (tex.size()[1] as f32).max(1.0);
                            let mut w = max_w;
                            let mut h = w / aspect;
                            if h > max_h {
                                h = max_h;
                                w = h * aspect;
                            }
                            ui.image((tex.id(), Vec2::new(w, h)));
                        }
                    });
                });
            return;
        }

        // 顶栏
        egui::TopBottomPanel::top("top")
            .exact_height(56.0)
            .frame(
                Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0, LINE))
                    .inner_margin(Margin::symmetric(20, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("Plate Studio")
                            .size(20.0)
                            .strong()
                            .color(INK),
                    );
                    ui.add_space(18.0);

                    let (listening, port, lan_urls) = self.rt.block_on(async {
                        let d = self.state.inner.read().await;
                        (d.api_listening, d.api_port, d.lan_urls.clone())
                    });
                    // 优先显示局域网地址，方便其他设备连接
                    let display_url = lan_urls
                        .first()
                        .cloned()
                        .unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
                    let (dot, text) = if listening {
                        (OK, format!("局域网可连  {display_url}"))
                    } else {
                        (BAD, "服务未启动".into())
                    };
                    ui.painter().circle_filled(
                        ui.cursor().left_center() + Vec2::new(5.0, 0.0),
                        4.0,
                        dot,
                    );
                    ui.add_space(14.0);
                    ui.label(RichText::new(text).size(13.5).color(MUTED));

                    if ui
                        .add(
                            egui::Button::new(RichText::new("复制").size(13.0).color(ACCENT))
                                .frame(false),
                        )
                        .on_hover_text(&display_url)
                        .clicked()
                    {
                        ui.ctx().copy_text(display_url.clone());
                        self.status_msg = format!("已复制 {display_url}");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(self.preview_texture.is_some(), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("全屏").size(13.5).color(ACCENT),
                                    )
                                    .fill(Color32::from_rgb(236, 242, 248)),
                                )
                                .clicked()
                            {
                                self.set_fullscreen(true);
                            }
                        });
                    });
                });
            });

        // 左侧控制
        egui::SidePanel::left("controls")
            .exact_width(268.0)
            .frame(panel_frame())
            .show(ctx, |ui| {
                ui.label(RichText::new("生成").size(13.0).color(MUTED));
                ui.add_space(10.0);

                let mut use_random = self
                    .rt
                    .block_on(async { self.state.inner.read().await.use_random });
                let mut custom = self
                    .rt
                    .block_on(async { self.state.inner.read().await.custom_plate.clone() });
                let mut color = self
                    .rt
                    .block_on(async { self.state.inner.read().await.selected_color.clone() });

                ui.horizontal(|ui| {
                    let random_sel = use_random;
                    let fixed_sel = !use_random;
                    if ui
                        .selectable_label(random_sel, RichText::new("随机").size(14.0))
                        .clicked()
                    {
                        use_random = true;
                    }
                    if ui
                        .selectable_label(fixed_sel, RichText::new("指定").size(14.0))
                        .clicked()
                    {
                        use_random = false;
                    }
                });

                ui.add_space(10.0);
                if !use_random {
                    ui.label(RichText::new("车牌号").size(12.5).color(MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut custom)
                            .desired_width(ui.available_width())
                            .hint_text("如 粤C12345")
                            .text_color(INK)
                            .font(FontId::new(16.0, egui::FontFamily::Proportional))
                            .frame(true)
                            .margin(Margin::symmetric(10, 8)),
                    );
                    ui.add_space(8.0);
                }

                ui.label(RichText::new("颜色").size(12.5).color(MUTED));
                egui::ComboBox::from_id_salt("color")
                    .width(ui.available_width())
                    .selected_text(color_label(&color))
                    .show_ui(ui, |ui| {
                        for (key, label) in COLOR_OPTIONS {
                            ui.selectable_value(&mut color, (*key).to_string(), *label);
                        }
                    });

                self.rt.block_on(async {
                    let mut d = self.state.inner.write().await;
                    d.use_random = use_random;
                    d.custom_plate = custom;
                    d.selected_color = color;
                });

                ui.add_space(18.0);
                if primary_button(ui, "生成车牌", 42.0).clicked() {
                    self.on_generate();
                }

                ui.add_space(8.0);
                ui.add_enabled_ui(self.preview_texture.is_some(), |ui| {
                    if ghost_button(ui, "全屏查看").clicked() {
                        self.set_fullscreen(true);
                    }
                });

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(12.0);
                ui.label(RichText::new("最近").size(13.0).color(MUTED));
                ui.add_space(6.0);

                let history = self.rt.block_on(async {
                    self.state
                        .inner
                        .read()
                        .await
                        .history
                        .iter()
                        .take(12)
                        .cloned()
                        .collect::<Vec<_>>()
                });

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if history.is_empty() {
                        ui.label(RichText::new("还没有记录").size(13.0).color(MUTED));
                    }
                    for r in &history {
                        let selected = self.preview_path.as_deref() == Some(r.image_path.as_str());
                        let label = format!("{}  ·  {}", r.plate, color_label(&r.color));
                        let response = ui.selectable_label(
                            selected,
                            RichText::new(label)
                                .size(13.5)
                                .color(if selected { ACCENT } else { INK }),
                        );
                        if response.clicked() {
                            self.status_msg = r.plate.clone();
                            self.update_preview(ctx, r);
                        }
                    }
                });
            });

        // 主预览区
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(28, 24)),
            )
            .show(ctx, |ui| {
                let plate_text = self.rt.block_on(async {
                    self.state
                        .inner
                        .read()
                        .await
                        .latest
                        .as_ref()
                        .map(|r| r.plate.clone())
                        .unwrap_or_else(|| "等待生成".into())
                });
                let meta = self.rt.block_on(async {
                    self.state
                        .inner
                        .read()
                        .await
                        .latest
                        .as_ref()
                        .map(|r| {
                            format!(
                                "{}  ·  {}",
                                color_label(&r.color),
                                r.created_at.format("%H:%M:%S")
                            )
                        })
                        .unwrap_or_else(|| self.status_msg.clone())
                });

                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&plate_text)
                            .size(48.0)
                            .strong()
                            .color(INK),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new(meta).size(14.0).color(MUTED));
                    ui.add_space(22.0);

                    let max_w = ui.available_width();
                    let max_h = ui.available_height() - 28.0;

                    if let Some(tex) = &self.preview_texture {
                        let aspect = tex.size()[0] as f32 / (tex.size()[1] as f32).max(1.0);
                        let mut w = max_w.min(920.0);
                        let mut h = w / aspect;
                        if h > max_h {
                            h = max_h;
                            w = h * aspect;
                        }

                        // 轻微底板衬托车牌
                        let pad = 18.0;
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(w + pad * 2.0, h + pad * 2.0),
                            egui::Sense::click(),
                        );
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(12),
                            Color32::from_rgb(248, 249, 251),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(12),
                            Stroke::new(1.0, LINE),
                            egui::StrokeKind::Inside,
                        );
                        let img_rect = egui::Rect::from_center_size(rect.center(), Vec2::new(w, h));
                        ui.painter().image(
                            tex.id(),
                            img_rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            Color32::WHITE,
                        );
                        if response.clicked() || response.double_clicked() {
                            self.set_fullscreen(true);
                        }
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        ui.add_space(10.0);
                        ui.label(RichText::new("点击放大 · F11 全屏").size(12.5).color(MUTED));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(max_w.min(640.0), 180.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(12),
                            Color32::from_rgb(248, 249, 251),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            CornerRadius::same(12),
                            Stroke::new(1.0, LINE),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "点击左侧「生成车牌」开始",
                            FontId::new(15.0, egui::FontFamily::Proportional),
                            MUTED,
                        );
                    }
                });
            });
    }
}
