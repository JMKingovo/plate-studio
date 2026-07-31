//! 共享状态：最新车牌、历史、API 日志、WebSocket 广播

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Local};
use serde::Serialize;
use tokio::sync::{broadcast, RwLock};

pub const DEFAULT_PORT: u16 = 18765;
pub const HISTORY_LIMIT: usize = 100;
pub const LOG_LIMIT: usize = 200;

#[derive(Clone, Debug, Serialize)]
pub struct PlateRecord {
    pub plate: String,
    pub color: String,
    pub image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_base64: Option<String>,
    pub created_at: DateTime<Local>,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiLogEntry {
    pub time: DateTime<Local>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    PlateGenerated {
        plate: String,
        color: String,
        image_path: String,
        ts: DateTime<Local>,
        source: String,
    },
    FullscreenChanged {
        enabled: bool,
        ts: DateTime<Local>,
        source: String,
    },
}

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<SharedData>>,
    pub events: broadcast::Sender<WsEvent>,
    #[allow(dead_code)]
    pub assets_dir: PathBuf,
    pub output_dir: PathBuf,
}

pub struct SharedData {
    pub latest: Option<PlateRecord>,
    pub history: VecDeque<PlateRecord>,
    pub api_logs: VecDeque<ApiLogEntry>,
    pub api_port: u16,
    pub api_listening: bool,
    pub api_error: Option<String>,
    /// 局域网可访问的 http://IP:port 列表
    pub lan_urls: Vec<String>,
    /// GUI 轮询用：每次生成递增
    pub generation_seq: u64,
    pub custom_plate: String,
    pub selected_color: String,
    pub use_random: bool,
    /// 界面车牌全屏预览（可由 API / GUI 共同控制）
    pub plate_fullscreen: bool,
}

impl SharedData {
    pub fn new(port: u16) -> Self {
        Self {
            latest: None,
            history: VecDeque::new(),
            api_logs: VecDeque::new(),
            api_port: port,
            api_listening: false,
            api_error: None,
            lan_urls: Vec::new(),
            generation_seq: 0,
            custom_plate: "粤C12345".into(),
            selected_color: "blue".into(),
            use_random: true,
            plate_fullscreen: false,
        }
    }

    pub fn push_record(&mut self, record: PlateRecord) {
        self.generation_seq += 1;
        // 历史列表不存 base64，减小内存
        let mut hist = record.clone();
        hist.image_base64 = None;
        self.history.push_front(hist);
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_back();
        }
        self.latest = Some(record);
    }

    pub fn log(&mut self, message: impl Into<String>) {
        self.api_logs.push_front(ApiLogEntry {
            time: Local::now(),
            message: message.into(),
        });
        while self.api_logs.len() > LOG_LIMIT {
            self.api_logs.pop_back();
        }
    }
}

impl AppState {
    pub fn new(assets_dir: PathBuf, output_dir: PathBuf, port: u16) -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(RwLock::new(SharedData::new(port))),
            events,
            assets_dir,
            output_dir,
        }
    }

    pub async fn publish_generated(&self, record: &PlateRecord) {
        let _ = self.events.send(WsEvent::PlateGenerated {
            plate: record.plate.clone(),
            color: record.color.clone(),
            image_path: record.image_path.clone(),
            ts: record.created_at,
            source: record.source.clone(),
        });
    }

    pub async fn set_fullscreen(&self, enabled: bool, source: &str) {
        {
            let mut data = self.inner.write().await;
            if data.plate_fullscreen == enabled {
                return;
            }
            data.plate_fullscreen = enabled;
            data.log(format!(
                "{} 全屏：{}",
                source,
                if enabled { "开启" } else { "关闭" }
            ));
        }
        let _ = self.events.send(WsEvent::FullscreenChanged {
            enabled,
            ts: Local::now(),
            source: source.into(),
        });
    }
}
