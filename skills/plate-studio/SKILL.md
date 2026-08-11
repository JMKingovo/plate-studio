---
name: plate-studio
description: >-
  操作 Plate Studio（中国机动车号牌图像生成软件，默认局域网 API 端口 18765）。
  在生成/更换号牌、控制全屏预览、查询最近与历史记录、或对接脚本与 Agent 时使用。
  触发词：plate-studio、车牌生成、号牌 API、18765。
---

# Plate Studio

桌面端中国机动车号牌图像生成软件（Rust + egui），对外提供本机/局域网 HTTP 与 WebSocket API，供脚本与 Agent 自动化调用。

## 范围界定

| 范围内 | 范围外 |
|--------|--------|
| 号牌图像生成与界面预览 | 停车场业务库、道闸控制 |
| 本机/局域网 API 与事件推送 | 云端授权或远程鉴权 |
| 通过界面或 API 控制全屏展示 | 第三方相机协议模拟与上报 |

- 仓库：https://github.com/JMKingovo/plate-studio
- Windows 发行包：[Releases](https://github.com/JMKingovo/plate-studio/releases) → `plate-studio-windows.zip`
- Skill 发行包：[Releases](https://github.com/JMKingovo/plate-studio/releases) → `plate-studio-skills.zip`

## 运行拓扑

| 项 | 说明 |
|----|------|
| 监听地址 | `0.0.0.0:18765`（本机与局域网均可访问） |
| 服务基址 | `http://<主机>:18765`（界面标题栏显示局域网 URL） |
| 默认成图 | `1280×720` 深色场景、号牌居中（便于相机识别） |

发行目录须保持同级结构：

```
plate-studio.exe
assets/plate_model/
assets/font_model/
api-ms-win-core-path-l1-1-0.dll   # 打包时附带的 Windows 兼容 DLL
output/                          # 运行时自动创建
```

## Agent 操作规范

1. 确定 `BASE`：优先使用用户给定主机；否则取界面标题栏局域网地址；默认 `http://127.0.0.1:18765`。
2. 变更状态前先调用 `GET /health` 做存活检查。
3. 通过 REST 生成或查询号牌；自动化场景默认 `include_image: false`，除非明确需要图像载荷。
4. 向用户回报 JSON 中的 `plate` 与 `color`；勿在对话中输出完整 `image_base64`。

### 存活检查

```bash
curl -sS -m 5 "$BASE/health"
# {"ok":true,"service":"plate-studio","version":"..."}
```

失败时排查：进程是否运行、主机是否可达、TCP `18765` 是否放行。

### 生成号牌

```bash
# 按类型随机生成
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"random":true,"color":"green_car","include_image":false}'

# 指定号牌文本
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"plate":"粤C12345","color":"blue","include_image":false}'
```

### 查询

```bash
curl -sS -m 10 "$BASE/api/v1/plates/latest"
curl -sS -m 10 "$BASE/api/v1/plates?limit=20"
```

### 全屏控制（需 GUI 进程在运行）

```bash
curl -sS -m 5 -X POST "$BASE/api/v1/ui/fullscreen" \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true}'
# 退出：{"enabled":false}  |  切换：{"toggle":true}
# 查询：GET /api/v1/ui/fullscreen
```

### WebSocket

`ws://<主机>:18765/api/v1/events`  
推送事件包括 `plate_generated` 与全屏状态变更。

## HTTP API 参考

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 存活探测 |
| POST | `/api/v1/plates/generate` | 生成号牌 |
| GET | `/api/v1/plates/latest` | 最近一条记录 |
| GET | `/api/v1/plates?limit=N` | 历史记录（`N` ≤ 100） |
| GET | `/api/v1/ui/fullscreen` | 查询全屏状态 |
| POST | `/api/v1/ui/fullscreen` | 设置/切换全屏 |
| WS | `/api/v1/events` | 事件推送 |

### `POST /api/v1/plates/generate` 请求体

| 字段 | 类型 | 说明 |
|------|------|------|
| `random` | bool | 为 `true` 时按规则随机生成，忽略 `plate` |
| `plate` | string | 非随机模式下的号牌文本 |
| `color` | string | 号牌样式键（见下表） |
| `include_image` | bool | 默认 `true`；自动化建议 `false` |

### 响应字段

`plate`、`color`、`image_path`、`image_base64`（可选）、`created_at`、`source`

### `color` 取值

| 键 | 含义 |
|----|------|
| `blue` | 蓝牌（普通民用） |
| `yellow` | 黄牌 |
| `green_car` | 新能源小型汽车 |
| `green_truck` | 新能源大型汽车 |
| `white` | 警车 |
| `white_army` | 军车 |
| `black` | 港澳 |
| `black_shi` | 使领馆 |

## 号牌编码规则

| 类型 | 规则 |
|------|------|
| 普通号牌（如蓝牌） | 省简称 + **字母** + 5 位序号（以数字为主） |
| 新能源号牌（`green_*`） | 省简称 + 字母 + **D/F** + 5 位序号（共 8 位） |

不符合规则的文本将被生成器拒绝。

## 图形界面说明

| 操作 | 方式 |
|------|------|
| 生成 | 左侧：随机 / 指定 → 选择颜色 → **生成车牌** |
| 复制号牌 | 左侧按钮、点击大号号牌文字、双击历史项，或全屏 **复制** |
| 复制 API 基址 | 点击标题栏高亮 URL，或 **复制地址** |
| 进入全屏 | `F11` 或 **全屏查看** |
| 退出全屏 | 双击 **双击退出**，或 `Esc` |

预览区单击**不会**进入全屏。

## 本地开发

```bash
cargo run --release                 # 图形界面 + API
cargo run --release -- --api-only   # 仅 API
```

交叉编译与打包说明见仓库根目录 `README.md`。

## Skill 获取与安装

仓库路径：`skills/plate-studio/`  
发行附件：`plate-studio-skills.zip`（解压后得到 `plate-studio/SKILL.md`）

将整个 `plate-studio` 目录复制到所用 Agent 的 skills 目录即可（不同产品路径不同，按各自文档配置）。
