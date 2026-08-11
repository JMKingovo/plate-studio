//! 车牌图片合成（复用 plate_model / font_model 素材）
//! 高清硬贴字 + 可选居中场景图（便于相机识别）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use image::{imageops, DynamicImage, GrayImage, ImageEncoder, Rgb, RgbImage};

use crate::plate_number::{self, is_letter, DIGITS, LETTERS};

/// 高清：相对标准 440×140 / 480×140 放大倍数
pub const RENDER_SCALE: u32 = 3;

/// 场景图尺寸（车牌居中铺在暗底上，方便相机/识别器定位）
pub const SCENE_W: u32 = 1280;
pub const SCENE_H: u32 = 720;

#[derive(Clone)]
pub struct PlateGenerator {
    plate_model_dir: PathBuf,
    fonts: HashMap<String, GrayImage>,
}

impl PlateGenerator {
    pub fn new(assets_dir: impl AsRef<Path>) -> Result<Self> {
        let assets = assets_dir.as_ref();
        let plate_model_dir = assets.join("plate_model");
        let font_dir = assets.join("font_model");
        if !plate_model_dir.is_dir() {
            return Err(anyhow!("缺少底板素材目录: {}", plate_model_dir.display()));
        }
        if !font_dir.is_dir() {
            return Err(anyhow!("缺少字符素材目录: {}", font_dir.display()));
        }

        let mut fonts = HashMap::new();
        for entry in std::fs::read_dir(&font_dir).with_context(|| format!("读取 {}", font_dir.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("无效字体文件名: {}", path.display()))?
                .to_string();

            let mut img = image::open(&path)
                .with_context(|| format!("打开字体 {}", path.display()))?
                .to_luma8();

            // 与 Python 预缩放一致（再按 bbox 放大到高清）
            if stem.contains("140") {
                img = imageops::resize(&img, 45, 90, imageops::FilterType::Triangle);
            } else if stem.contains("220") {
                img = imageops::resize(&img, 65, 110, imageops::FilterType::Triangle);
            } else if let Some(ch) = stem.rsplit('_').next() {
                let c = ch.chars().next().unwrap_or('\0');
                if ch.len() == 1 && (DIGITS.contains(&c) || LETTERS.contains(&c)) {
                    img = imageops::resize(&img, 43, 90, imageops::FilterType::Triangle);
                }
            }

            fonts.insert(stem, img);
        }

        Ok(Self {
            plate_model_dir,
            fonts,
        })
    }

    /// 生成高清车牌本体（不含场景底）
    pub fn generate_plate_only(&self, plate: &str, bg_color: &str) -> Result<RgbImage> {
        plate_number::validate_plate(plate).map_err(|e| anyhow!(e))?;
        let chars: Vec<char> = plate.chars().collect();
        let length = chars.len();
        let base_h = 140u32;
        let base_w = if length == 7 { 440 } else { 480 };
        let scale = RENDER_SCALE;
        let height = base_h * scale;
        let width = base_w * scale;

        let model_path = self.plate_model_dir.join(format!("{bg_color}_{base_h}.PNG"));
        if !model_path.exists() {
            return Err(anyhow!("底板不存在: {}", model_path.display()));
        }
        let plate_img = image::open(&model_path)
            .with_context(|| format!("打开底板 {}", model_path.display()))?
            .to_rgb8();
        let mut plate_img =
            imageops::resize(&plate_img, width, height, imageops::FilterType::Lanczos3);

        let locations = location_data(length, split_id(plate), base_h as i32, scale);

        for (i, ch) in chars.iter().enumerate() {
            let font = self.load_font_for_char(plate, *ch, base_h)?;
            let bbox = locations[i];
            let is_red =
                (*ch == '警' || *ch == '使' || *ch == '领') || (i == 0 && is_letter(*ch));
            paste_font(&mut plate_img, font, bbox, bg_color, is_red);
        }

        Ok(plate_img)
    }

    /// 默认输出：高清车牌居中放在 1280×720 暗底场景上（更易被相机框选识别）
    pub fn generate(&self, plate: &str, bg_color: &str) -> Result<RgbImage> {
        let plate_img = self.generate_plate_only(plate, bg_color)?;
        Ok(compose_centered_scene(&plate_img))
    }

    fn load_font_for_char(&self, plate: &str, ch: char, base_h: u32) -> Result<&GrayImage> {
        let key = if plate.chars().count() == 8 {
            format!("green_{ch}")
        } else {
            format!("{base_h}_{ch}")
        };
        self.fonts
            .get(&key)
            .ok_or_else(|| anyhow!("缺少字符素材: {key}"))
    }

    pub fn save_image(&self, img: &RgbImage, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        DynamicImage::ImageRgb8(img.clone())
            .save(path)
            .with_context(|| format!("保存 {}", path.display()))?;
        Ok(())
    }

    pub fn encode_png_base64(img: &RgbImage) -> Result<String> {
        use base64::Engine;
        let mut buf = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut buf);
        encoder.write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )?;
        Ok(base64::engine::general_purpose::STANDARD.encode(buf))
    }
}

/// 把车牌居中贴到暗色场景，宽度约占画面 72%
pub fn compose_centered_scene(plate: &RgbImage) -> RgbImage {
    let mut canvas = RgbImage::from_pixel(SCENE_W, SCENE_H, Rgb([18, 22, 28]));
    let target_w = ((SCENE_W as f32) * 0.72).round() as u32;
    let target_h =
        ((target_w as f32) * (plate.height() as f32) / (plate.width() as f32).max(1.0)).round() as u32;
    let target_h = target_h.max(1).min(SCENE_H - 40);
    let target_w = target_w.max(1);
    let resized = imageops::resize(plate, target_w, target_h, imageops::FilterType::Lanczos3);
    let x = ((SCENE_W - target_w) / 2) as i64;
    let y = ((SCENE_H - target_h) / 2) as i64;
    imageops::overlay(&mut canvas, &resized, x, y);
    canvas
}

fn split_id(plate: &str) -> usize {
    if plate.contains('警') {
        1
    } else if plate.contains('使') {
        4
    } else {
        2
    }
}

fn location_data(length: usize, split_id: usize, base_height: i32, scale: u32) -> Vec<[i32; 4]> {
    let mut location_xy = vec![[0i32; 4]; length];
    let s = scale as i32;
    if base_height == 140 {
        for row in &mut location_xy {
            row[1] = 25 * s;
            row[3] = 115 * s;
        }
        let step_split = if length == 7 { 34 } else { 49 };
        let step_font = if length == 7 { 12 } else { 9 };
        let mut width_font = 45;
        for i in 0..length {
            if i == 0 {
                location_xy[i][0] = 15 * s;
            } else if i == split_id {
                location_xy[i][0] = location_xy[i - 1][2] + step_split * s;
            } else {
                location_xy[i][0] = location_xy[i - 1][2] + step_font * s;
            }
            if length == 8 && i > 0 {
                width_font = 43;
            }
            location_xy[i][2] = location_xy[i][0] + width_font * s;
        }
    }
    location_xy
}

fn paste_font(img: &mut RgbImage, font: &GrayImage, bbox: [i32; 4], bg_color: &str, is_red: bool) {
    let (x1, y1, x2, y2) = (bbox[0], bbox[1], bbox[2], bbox[3]);
    let w = (x2 - x1).max(1) as u32;
    let h = (y2 - y1).max(1) as u32;
    let font_resized = imageops::resize(font, w, h, imageops::FilterType::Lanczos3);

    let color = if is_red {
        Rgb([255, 0, 0])
    } else if bg_color.contains("blue") || bg_color.contains("black") {
        Rgb([255, 255, 255])
    } else {
        Rgb([0, 0, 0])
    };

    // 硬阈值贴字（清晰锐利），边缘用轻度 alpha 仅抗锯齿
    for fy in 0..h {
        for fx in 0..w {
            let px = font_resized.get_pixel(fx, fy)[0];
            if px >= 200 {
                continue;
            }
            let dx = x1 as u32 + fx;
            let dy = y1 as u32 + fy;
            if dx >= img.width() || dy >= img.height() {
                continue;
            }
            if px < 160 {
                img.put_pixel(dx, dy, color);
            } else {
                // 窄边缘抗锯齿，保持主体清晰
                let alpha = (200u16.saturating_sub(px as u16)) as f32 / 40.0;
                let alpha = alpha.clamp(0.0, 1.0);
                let bg = img.get_pixel(dx, dy);
                img.put_pixel(
                    dx,
                    dy,
                    Rgb([
                        ((color[0] as f32) * alpha + (bg[0] as f32) * (1.0 - alpha)) as u8,
                        ((color[1] as f32) * alpha + (bg[1] as f32) * (1.0 - alpha)) as u8,
                        ((color[2] as f32) * alpha + (bg[2] as f32) * (1.0 - alpha)) as u8,
                    ]),
                );
            }
        }
    }
}
