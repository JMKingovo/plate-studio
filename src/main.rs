// Windows 下不弹黑色控制台窗口
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod app;
mod generator;
mod netutil;
mod plate_number;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use eframe::egui;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use crate::app::PlateStudioApp;
use crate::generator::PlateGenerator;
use crate::state::{AppState, DEFAULT_PORT};

fn resolve_assets_dir() -> PathBuf {
    let candidates = [
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("assets"))),
        Some(PathBuf::from("assets")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")),
    ];
    for c in candidates.into_iter().flatten() {
        if c.join("plate_model").is_dir() && c.join("font_model").is_dir() {
            return c;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn resolve_output_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let api_only = std::env::args().any(|a| a == "--api-only");
    let assets_dir = resolve_assets_dir();
    let output_dir = resolve_output_dir();
    info!("assets: {}", assets_dir.display());
    info!("output: {}", output_dir.display());

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let handle = rt.handle().clone();

    let state = AppState::new(assets_dir.clone(), output_dir.clone(), DEFAULT_PORT);
    let generator = Arc::new(
        PlateGenerator::new(&assets_dir)
            .with_context(|| format!("加载素材失败: {}", assets_dir.display()))
            .unwrap_or_else(|e| {
                eprintln!("{e:#}");
                std::process::exit(1);
            }),
    );

    let api_state = state.clone();
    let api_gen = generator.clone();
    let port = DEFAULT_PORT;

    if api_only {
        info!("API-only mode on 0.0.0.0:{port} (LAN enabled)");
        for u in netutil::lan_base_urls(port) {
            info!("LAN URL: {u}");
        }
        // 前台运行，避免 spawn + 后台进程被 SIGHUP 杀掉
        if let Err(e) = rt.block_on(api::start_api_server(api_state.clone(), api_gen, port)) {
            error!("API server failed: {e:#}");
            eprintln!("API server failed: {e:#}");
            std::process::exit(1);
        }
        return Ok(());
    }

    rt.spawn(async move {
        if let Err(e) = api::start_api_server(api_state.clone(), api_gen, port).await {
            error!("API server failed: {e:#}");
            let mut data = api_state.inner.write().await;
            data.api_listening = false;
            data.api_error = Some(e.to_string());
            data.log(format!("API 启动失败: {e}"));
        }
    });

    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        });
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 560.0])
            .with_title("Plate Studio — 车牌生成"),
        ..Default::default()
    };

    eframe::run_native(
        "Plate Studio",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(PlateStudioApp::new(
                cc,
                state,
                handle,
                generator,
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plate_number;

    #[test]
    fn generate_blue_plate_image() {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let gen = PlateGenerator::new(&assets).expect("assets");
        let img = gen.generate("粤C12345", "blue").expect("generate");
        // 默认输出为居中场景图
        assert_eq!(img.width(), crate::generator::SCENE_W);
        assert_eq!(img.height(), crate::generator::SCENE_H);
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("output/test_粤C12345.png");
        gen.save_image(&img, &out).expect("save");
        assert!(out.exists());
        let plate = gen
            .generate_plate_only("粤C12345", "blue")
            .expect("plate only");
        let scale = crate::generator::RENDER_SCALE;
        assert_eq!(plate.width(), 440 * scale);
        assert_eq!(plate.height(), 140 * scale);
    }

    #[test]
    fn generate_green_plate_image() {
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
        let gen = PlateGenerator::new(&assets).expect("assets");
        let img = gen.generate("粤AD12345", "green_car").expect("generate");
        assert_eq!(img.width(), crate::generator::SCENE_W);
        assert_eq!(img.height(), crate::generator::SCENE_H);
    }

    #[test]
    fn random_plate_valid_length() {
        let mut rng = rand::rng();
        for _ in 0..20 {
            let (plate, color) = plate_number::generate_random(&mut rng, None);
            let n = plate.chars().count();
            assert!((7..=8).contains(&n), "{plate} / {color}");
        }
    }

    #[test]
    fn random_green_plate_has_df() {
        let mut rng = rand::rng();
        for _ in 0..50 {
            let (plate, _) = plate_number::generate_random(&mut rng, Some("green_car"));
            assert!(
                plate_number::is_valid_green_plate(&plate),
                "invalid green: {plate}"
            );
            let third = plate.chars().nth(2).unwrap();
            assert!(third == 'D' || third == 'F', "{plate}");
        }
    }

    #[test]
    fn random_blue_plate_second_is_letter() {
        let mut rng = rand::rng();
        for _ in 0..50 {
            let (plate, _) = plate_number::generate_random(&mut rng, Some("blue"));
            assert!(
                plate_number::is_valid_ordinary_plate(&plate),
                "invalid blue: {plate}"
            );
            let second = plate.chars().nth(1).unwrap();
            assert!(plate_number::is_letter(second), "pos1 not letter: {plate}");
        }
    }
}
