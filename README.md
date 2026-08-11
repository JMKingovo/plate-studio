# Plate Studio

中国车牌生成桌面软件（Rust）。图形界面可生成/预览车牌并大字显示号码；内置本机 HTTP API，供其他 AI 或脚本连接。

**不连接任何车场软件。**

## 功能

- 生成蓝牌 / 黄牌 / 新能源绿牌 / 白牌 / 黑牌等单层车牌图片
- 界面大字显示最新车牌号
- HTTP API：默认监听 `0.0.0.0:18765`（本机 + 局域网可访问）
- WebSocket 推送：有新车牌时通知订阅方

## Agent Skill

中文操作规范：`skills/plate-studio/SKILL.md`。

也可从 [Releases](https://github.com/JMKingovo/plate-studio/releases) 下载 `plate-studio-skills.zip`，解压后将 `plate-studio` 目录放入所用 Agent 的 skills 目录。

| 附件 | 说明 |
|------|------|
| `plate-studio-windows.zip` | Windows 可执行包 |
| `plate-studio-skills.zip` | Agent Skill（中文） |

## 运行

依赖：Rust 1.75+，素材目录 `assets/plate_model` 与 `assets/font_model`（已随项目提供）。

```bash
cd plate-studio
cargo run --release
```

仅 API（无窗口，适合服务器/自动化）：

```bash
cargo run --release -- --api-only
```

## HTTP API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 探活 |
| POST | `/api/v1/plates/generate` | 生成车牌 |
| GET | `/api/v1/plates/latest` | 最近一次车牌 |
| GET | `/api/v1/plates?limit=20` | 历史 |
| GET | `/api/v1/ui/fullscreen` | 查询是否全屏 |
| POST | `/api/v1/ui/fullscreen` | 控制全屏：`{"enabled":true}` 或 `{"toggle":true}` |
| WS | `/api/v1/events` | 新车牌 / 全屏变更事件推送 |

### 生成示例

```bash
# 随机蓝牌
curl -s http://127.0.0.1:18765/api/v1/plates/generate \
  -H 'Content-Type: application/json' \
  -d '{"random": true, "color": "blue"}'

# 指定号码
curl -s http://127.0.0.1:18765/api/v1/plates/generate \
  -H 'Content-Type: application/json' \
  -d '{"plate": "粤C12345", "color": "blue"}'

# 只要元数据、不要 base64
curl -s http://127.0.0.1:18765/api/v1/plates/generate \
  -H 'Content-Type: application/json' \
  -d '{"random": true, "include_image": false}'

curl -s http://127.0.0.1:18765/api/v1/plates/latest

# 全屏控制（需 GUI 正在运行）
curl -s http://127.0.0.1:18765/api/v1/ui/fullscreen \
  -H 'Content-Type: application/json' \
  -d '{"enabled": true}'

curl -s http://127.0.0.1:18765/api/v1/ui/fullscreen
# → {"enabled":true}

# 切换全屏
curl -s http://127.0.0.1:18765/api/v1/ui/fullscreen \
  -H 'Content-Type: application/json' \
  -d '{"toggle": true}'
```

响应字段：`plate`、`color`、`image_path`、`image_base64`（可选）、`created_at`、`source`。

`color` 可选：`blue` / `yellow` / `green_car` / `green_truck` / `white` / `black` / `black_shi` / `white_army`。

### WebSocket

```bash
# 需 websocat 或类似工具
websocat ws://127.0.0.1:18765/api/v1/events
```

推送示例：

```json
{
  "type": "plate_generated",
  "plate": "粤C12345",
  "color": "blue",
  "image_path": ".../output/粤C12345_173000.jpg",
  "ts": "2026-07-30T17:30:00+08:00",
  "source": "api"
}
```

## Windows 构建

在 Windows（建议 MSVC 工具链）上：

```powershell
cd plate-studio
cargo build --release
```

产物：`target\release\plate-studio.exe`。把整个 `assets` 目录复制到 exe 同级，再运行。

在 Linux 上交叉编译（需安装目标与链接器）：

```bash
rustup target add x86_64-pc-windows-gnu
# Arch 示例：sudo pacman -S mingw-w64-gcc
cargo build --release --target x86_64-pc-windows-gnu
```

分发时打包：

```
plate-studio.exe
assets/plate_model/
assets/font_model/
output/          # 可空，运行时自动创建
```

## 目录

```
plate-studio/
  assets/           # 车牌底板与字符素材
  output/           # 生成的图片
  src/
    main.rs
    app.rs          # egui 界面
    api.rs          # HTTP / WebSocket
    generator.rs    # 图片合成
    plate_number.rs # 号码规则
    state.rs        # 共享状态
```

## 素材来源

底板与字体素材来自开源项目 [chinese_license_plate_generator](https://github.com/Pengfei8324/chinese_license_plate_generator)；本软件用 Rust 重写了合成与对外 API，不依赖 Python。
