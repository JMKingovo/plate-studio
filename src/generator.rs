//! 车牌图片合成（复用 plate_model / font_model 素材）
//! 默认按 3 倍分辨率渲染，无模糊，边缘更清晰。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use image::{imageops, DynamicImage, GrayImage, Rgb, RgbImage, ImageEncoder};

use crate::plate_number::{self, is_letter};

/// 相对标准 440×140 / 480×140 的放大倍数
pub const RENDER_SCALE: u32 = 3;

#[derive(Clone)]
pub struct PlateGenerator {
    plate_model_dir: PathBuf,
    /// 原始灰度字体（不做预缩放，粘贴时按目标尺寸高质量缩放）
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

            let img = image::open(&path)
                .with_context(|| format!("打开字体 {}", path.display()))?
                .to_luma8();
            fonts.insert(stem, img);
        }

        Ok(Self {
            plate_model_dir,
            fonts,
        })
    }

    pub fn generate(&self, plate: &str, bg_color: &str) -> Result<RgbImage> {
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

fn split_id(plate: &str) -> usize {
    if plate.contains('警') {
        1
    } else if plate.contains('使') {
        4
    } else {
        2
    }
}

/// 返回每个字符 [x1,y1,x2,y2]，已按 scale 放大
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

    // 软边缘抗锯齿：按灰度做 alpha 混合
    for fy in 0..h {
        for fx in 0..w {
            let px = font_resized.get_pixel(fx, fy)[0];
            if px >= 220 {
                continue;
            }
            let dx = x1 as u32 + fx;
            let dy = y1 as u32 + fy;
            if dx >= img.width() || dy >= img.height() {
                continue;
            }
            let alpha = (220u16.saturating_sub(px as u16)) as f32 / 220.0;
            let alpha = alpha.clamp(0.0, 1.0);
            let bg = img.get_pixel(dx, dy);
            let blended = Rgb([
                ((color[0] as f32) * alpha + (bg[0] as f32) * (1.0 - alpha)) as u8,
                ((color[1] as f32) * alpha + (bg[1] as f32) * (1.0 - alpha)) as u8,
                ((color[2] as f32) * alpha + (bg[2] as f32) * (1.0 - alpha)) as u8,
            ]);
            img.put_pixel(dx, dy, blended);
        }
    }
}
