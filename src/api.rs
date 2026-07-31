//! 本机 HTTP API + WebSocket，供其他 AI / 脚本连接

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Local;
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tracing::{error, info};

use crate::generator::PlateGenerator;
use crate::plate_number::{self, infer_bg_color};
use crate::state::{AppState, PlateRecord};

#[derive(Clone)]
pub(crate) struct ApiCtx {
    state: AppState,
    generator: Arc<PlateGenerator>,
}

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub plate: Option<String>,
    pub color: Option<String>,
    pub random: Option<bool>,
    /// 是否在响应中附带 base64 图片，默认 true
    pub include_image: Option<bool>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: &'static str,
    pub version: &'static str,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct FullscreenRequest {
    /// true=进入全屏，false=退出；也可传 toggle=true 切换
    pub enabled: Option<bool>,
    pub toggle: Option<bool>,
}

#[derive(Serialize)]
pub struct FullscreenResponse {
    pub enabled: bool,
}

pub async fn start_api_server(
    state: AppState,
    generator: Arc<PlateGenerator>,
    port: u16,
) -> anyhow::Result<()> {
    let ctx = ApiCtx {
        state: state.clone(),
        generator,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/plates/generate", post(generate))
        .route("/api/v1/plates/latest", get(latest))
        .route("/api/v1/plates", get(history))
        .route("/api/v1/ui/fullscreen", get(get_fullscreen).post(set_fullscreen))
        .route("/api/v1/events", get(ws_events))
        .layer(CorsLayer::permissive())
        .with_state(ctx);

    // 0.0.0.0：本机 + 局域网均可访问
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let lan_urls = crate::netutil::lan_base_urls(port);
    {
        let mut data = state.inner.write().await;
        data.api_listening = true;
        data.api_error = None;
        data.api_port = port;
        data.lan_urls = lan_urls.clone();
        data.log(format!("API 已监听 0.0.0.0:{port}（局域网可访问）"));
        for u in &lan_urls {
            data.log(format!("局域网地址 {u}"));
        }
    }
    info!("API listening on http://0.0.0.0:{port}");
    for u in &lan_urls {
        info!("LAN URL: {u}");
    }

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "plate-studio",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn get_fullscreen(State(ctx): State<ApiCtx>) -> Json<FullscreenResponse> {
    let enabled = ctx.state.inner.read().await.plate_fullscreen;
    Json(FullscreenResponse { enabled })
}

async fn set_fullscreen(
    State(ctx): State<ApiCtx>,
    Json(req): Json<FullscreenRequest>,
) -> Result<Json<FullscreenResponse>, ApiError> {
    let current = ctx.state.inner.read().await.plate_fullscreen;
    let enabled = if req.toggle.unwrap_or(false) {
        !current
    } else if let Some(v) = req.enabled {
        v
    } else {
        return Err(ApiError::bad_request(
            "请提供 enabled=true/false，或 toggle=true",
        ));
    };

    // 进全屏时至少要有一张图，否则提醒但不强制阻断（界面仍会切到全屏空状态）
    if enabled {
        let has_plate = ctx.state.inner.read().await.latest.is_some();
        if !has_plate {
            let mut data = ctx.state.inner.write().await;
            data.log("API 请求全屏，但尚无车牌图片");
        }
    }

    ctx.state.set_fullscreen(enabled, "api").await;
    Ok(Json(FullscreenResponse { enabled }))
}

async fn generate(
    State(ctx): State<ApiCtx>,
    Json(req): Json<GenerateRequest>,
) -> Result<Json<PlateRecord>, ApiError> {
    let include_image = req.include_image.unwrap_or(true);
    let record = do_generate(&ctx, req, "api", include_image).await?;
    {
        let mut data = ctx.state.inner.write().await;
        data.log(format!("API 生成车牌 {}", record.plate));
    }
    Ok(Json(record))
}

async fn latest(State(ctx): State<ApiCtx>) -> Result<Json<PlateRecord>, ApiError> {
    let data = ctx.state.inner.read().await;
    match &data.latest {
        Some(r) => {
            let mut out = r.clone();
            // 按需读盘补 base64
            if out.image_base64.is_none() {
                if let Ok(bytes) = std::fs::read(&out.image_path) {
                    out.image_base64 = Some(B64.encode(bytes));
                }
            }
            Ok(Json(out))
        }
        None => Err(ApiError::not_found("尚无生成记录")),
    }
}

async fn history(
    State(ctx): State<ApiCtx>,
    Query(q): Query<HistoryQuery>,
) -> Json<Vec<PlateRecord>> {
    let limit = q.limit.unwrap_or(20).min(100);
    let data = ctx.state.inner.read().await;
    Json(data.history.iter().take(limit).cloned().collect())
}

async fn ws_events(ws: WebSocketUpgrade, State(ctx): State<ApiCtx>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, ctx))
}

async fn handle_socket(mut socket: WebSocket, ctx: ApiCtx) {
    let mut rx = ctx.state.events.subscribe();
    {
        let mut data = ctx.state.inner.write().await;
        data.log("WebSocket 客户端已连接");
    }
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(event) => {
                        let text = match serde_json::to_string(&event) {
                            Ok(t) => t,
                            Err(_) => continue,
                        };
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => {
                        let _ = socket.send(Message::Pong(p)).await;
                    }
                    _ => {}
                }
            }
        }
    }
    let mut data = ctx.state.inner.write().await;
    data.log("WebSocket 客户端已断开");
}

pub async fn do_generate(
    ctx: &ApiCtx,
    req: GenerateRequest,
    source: &str,
    include_image: bool,
) -> Result<PlateRecord, ApiError> {
    let mut rng = StdRng::from_os_rng();
    let random = req.random.unwrap_or(req.plate.is_none());

    let (plate, color) = if random {
        plate_number::generate_random(&mut rng, req.color.as_deref())
    } else {
        let plate = req
            .plate
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::bad_request("请提供 plate 或设置 random=true"))?
            .to_string();
        plate_number::validate_plate(&plate).map_err(ApiError::bad_request)?;
        let color = req
            .color
            .clone()
            .unwrap_or_else(|| infer_bg_color(&plate));
        (plate, color)
    };

    let img = ctx
        .generator
        .generate(&plate, &color)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let safe_name: String = plate.chars().map(|c| if c == '/' { '_' } else { c }).collect();
    let filename = format!("{safe_name}_{}.png", Local::now().format("%H%M%S"));
    let path = ctx.state.output_dir.join(&filename);
    ctx.generator
        .save_image(&img, &path)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let image_base64 = if include_image {
        Some(
            PlateGenerator::encode_png_base64(&img)
                .map_err(|e| ApiError::internal(e.to_string()))?,
        )
    } else {
        None
    };

    let record = PlateRecord {
        plate,
        color,
        image_path: path.display().to_string(),
        image_base64,
        created_at: Local::now(),
        source: source.into(),
    };

    {
        let mut data = ctx.state.inner.write().await;
        data.push_record(record.clone());
    }
    ctx.state.publish_generated(&record).await;
    Ok(record)
}

/// GUI 侧调用的生成入口（不经 HTTP）
pub async fn generate_from_gui(
    state: &AppState,
    generator: Arc<PlateGenerator>,
    plate: Option<String>,
    color: Option<String>,
    random: bool,
) -> anyhow::Result<PlateRecord> {
    let ctx = ApiCtx { state: state.clone(), generator };
    do_generate(
        &ctx,
        GenerateRequest {
            plate,
            color,
            random: Some(random),
            include_image: Some(true),
        },
        "gui",
        true,
    )
    .await
    .map_err(|e| anyhow::anyhow!(e.message))
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            error!("API error: {}", self.message);
        }
        let body = Json(serde_json::json!({ "error": self.message }));
        (self.status, body).into_response()
    }
}
