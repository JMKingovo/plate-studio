---
name: plate-studio
description: Plate Studio 中国车牌生成桌面软件（Rust + HTTP API）。生成/预览车牌、局域网换牌、全屏展示、复制地址与车牌号。用户提到 plate-studio、车牌生成器、换车牌、18765、局域网可连、或要对 Windows/本机 Plate Studio 远程操作时使用。
---

# Plate Studio

中国车牌生成桌面软件。**不连接任何车场软件**；通过本机/局域网 HTTP API 供 AI 与脚本调用。

| 项 | 值 |
|----|-----|
| 仓库 | https://github.com/JMKingovo/plate-studio |
| 默认端口 | `18765`（监听 `0.0.0.0`） |
| Windows 包 | [Releases](https://github.com/JMKingovo/plate-studio/releases) 中的 `plate-studio-windows.zip` |
| 本 skill 路径 | 仓库内 `.cursor/skills/plate-studio/` |

## 安装本 Skill（给别人用）

**方式 A — 克隆/打开本仓库（推荐）**  
用 Cursor 打开 `plate-studio` 仓库后，项目 skill 自动可用。

**方式 B — 装到个人 skills（任意项目都能用）**

```bash
git clone --depth 1 https://github.com/JMKingovo/plate-studio.git /tmp/plate-studio
mkdir -p ~/.cursor/skills
cp -a /tmp/plate-studio/.cursor/skills/plate-studio ~/.cursor/skills/
```

Windows（PowerShell）：

```powershell
git clone --depth 1 https://github.com/JMKingovo/plate-studio.git $env:TEMP\plate-studio
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.cursor\skills" | Out-Null
Copy-Item -Recurse -Force "$env:TEMP\plate-studio\.cursor\skills\plate-studio" "$env:USERPROFILE\.cursor\skills\"
```

## 先确认连得上

用户说「换车牌 / 连上了吗」时，先探活再操作。基址以对方界面顶部「局域网可连」显示的为准：

```bash
# 本机
curl -sS -m 5 http://127.0.0.1:18765/health

# 局域网其他机器（替换为实际 IP）
curl -sS -m 5 http://192.168.x.x:18765/health
```

期望：`{"ok":true,"service":"plate-studio",...}`。失败则 ping 主机、确认程序已启动、防火墙放行 18765。

可点蓝色网址或「复制地址」拿到 API 根地址。

## 界面操作（Windows / Linux GUI）

1. 解压包后保持同目录：`plate-studio.exe`、`assets/`、兼容 DLL（若有）
2. 左侧选 **随机 / 指定**，选颜色，点 **生成车牌**
3. 预览为 **1280×720 居中场景图**（便于相机识别）
4. **复制车牌号**：左侧按钮 / 点大号车牌文字 / 历史项双击 / 全屏「复制」
5. **全屏**：`F11` 或「全屏查看」；退出：双击「双击退出」或 `Esc`（预览单击不会进全屏）

### 颜色键（界面 ↔ API）

| API `color` | 界面 |
|-------------|------|
| `blue` | 蓝牌 |
| `yellow` | 黄牌 |
| `green_car` | 新能源 |
| `green_truck` | 新能源卡 |
| `white` | 警车 |
| `white_army` | 军车 |
| `black` | 港澳 |
| `black_shi` | 使领馆 |

### 号牌规则（生成器会校验）

- **蓝牌等普通牌**：省简称 + **字母** + 5 位序号（数字为主）
- **新能源绿牌**：省 + 字母 + **D/F** + 5 位序号（共 8 位）

## 远程换牌（Agent 默认走 API）

基址：`http://<主机>:18765`。写操作成功后用一句话回报车牌号与颜色即可。

```bash
BASE=http://127.0.0.1:18765   # 或局域网 IP

# 随机（推荐带 color；include_image:false 避免巨大 base64）
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"random":true,"color":"green_car","include_image":false}'

# 指定号码
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"plate":"粤C12345","color":"blue","include_image":false}'

# 最近一次（响应可能含大图，解析时只取 plate/color）
curl -sS -m 10 "$BASE/api/v1/plates/latest" \
  | python3 -c 'import sys,json;d=json.load(sys.stdin);print(d["plate"],d["color"])'

# 全屏（需 GUI 在跑）
curl -sS -m 5 -X POST "$BASE/api/v1/ui/fullscreen" \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true}'
# 退出：{"enabled":false}  切换：{"toggle":true}
```

### API 一览

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/health` | 探活 |
| POST | `/api/v1/plates/generate` | 生成；body: `plate` / `color` / `random` / `include_image` |
| GET | `/api/v1/plates/latest` | 最近一次 |
| GET | `/api/v1/plates?limit=20` | 历史 |
| GET/POST | `/api/v1/ui/fullscreen` | 查询 / 设置全屏 |
| WS | `/api/v1/events` | `plate_generated` / 全屏变更推送 |

响应常用字段：`plate`、`color`、`image_path`、`image_base64`、`created_at`、`source`。

## 本机开发运行

```bash
cd plate-studio
cargo run --release                 # GUI + API
cargo run --release -- --api-only   # 仅 API
```

交叉编译与打包见仓库根目录 `README.md`。

## 边界

- Plate Studio **只产图 + 展示 + API**，不连接车场库或岗亭服务。
- 若要把生成图用于停车系统模拟相机上报，另走对应车场联调流程（与本软件解耦）。
