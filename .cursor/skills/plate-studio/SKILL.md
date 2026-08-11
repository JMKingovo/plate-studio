---
name: plate-studio
description: >-
  Operates Plate Studio, a Chinese license-plate image generator with a local
  LAN HTTP/WebSocket API (default port 18765). Use when generating or changing
  plates, controlling fullscreen preview, querying latest/history records,
  integrating scripts or agents with Plate Studio, or when the user mentions
  plate-studio, 车牌生成, 18765, or plate API endpoints.
---

# Plate Studio

Desktop application (Rust + egui) that synthesizes single-layer Chinese license-plate images and exposes a LAN-accessible HTTP API for automation.

## Scope

| In scope | Out of scope |
|----------|----------------|
| Plate image generation and GUI preview | Parking-lot databases or barrier controllers |
| Local/LAN API and WebSocket events | Cloud licensing or remote auth |
| Fullscreen display control via GUI or API | Camera protocol simulation for third-party systems |

Repository: https://github.com/JMKingovo/plate-studio  
Windows binary: [Releases](https://github.com/JMKingovo/plate-studio/releases) → `plate-studio-windows.zip`

## Runtime topology

- Bind address: `0.0.0.0:18765` (loopback and LAN)
- Base URL: `http://<host>:18765` (host IP shown in the GUI title bar)
- Default output canvas: `1280×720` centered plate on a dark scene (camera-friendly)
- Distribution layout (must stay colocated):

```
plate-studio.exe
assets/plate_model/
assets/font_model/
api-ms-win-core-path-l1-1-0.dll   # Windows compatibility shim when packaged
output/                          # created at runtime
```

## Agent operating procedure

1. Resolve `BASE` from the user-provided host, or from the GUI “LAN URL”, else `http://127.0.0.1:18765`.
2. Probe `GET /health` before mutating state.
3. Generate or query plates via REST; set `include_image: false` unless the image payload is required.
4. Report `plate` and `color` from the JSON response. Do not dump `image_base64` into chat.

### Health check

```bash
curl -sS -m 5 "$BASE/health"
# {"ok":true,"service":"plate-studio","version":"..."}
```

On failure: verify process is running, host reachable, and TCP `18765` is allowed.

### Generate

```bash
# Random plate of a given type
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"random":true,"color":"green_car","include_image":false}'

# Explicit plate text
curl -sS -m 15 -X POST "$BASE/api/v1/plates/generate" \
  -H 'Content-Type: application/json' \
  -d '{"plate":"粤C12345","color":"blue","include_image":false}'
```

### Query

```bash
curl -sS -m 10 "$BASE/api/v1/plates/latest"
curl -sS -m 10 "$BASE/api/v1/plates?limit=20"
```

### Fullscreen (GUI must be running)

```bash
curl -sS -m 5 -X POST "$BASE/api/v1/ui/fullscreen" \
  -H 'Content-Type: application/json' \
  -d '{"enabled":true}'
# disable: {"enabled":false}  |  toggle: {"toggle":true}
# status: GET /api/v1/ui/fullscreen
```

### WebSocket

`ws://<host>:18765/api/v1/events` — events include `plate_generated` and fullscreen changes.

## HTTP API reference

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness |
| POST | `/api/v1/plates/generate` | Generate plate |
| GET | `/api/v1/plates/latest` | Latest record |
| GET | `/api/v1/plates?limit=N` | History (`N` ≤ 100) |
| GET | `/api/v1/ui/fullscreen` | Fullscreen state |
| POST | `/api/v1/ui/fullscreen` | Set/toggle fullscreen |
| WS | `/api/v1/events` | Push notifications |

### `POST /api/v1/plates/generate` body

| Field | Type | Notes |
|-------|------|--------|
| `random` | bool | When true, ignore `plate` and generate by rules |
| `plate` | string | Explicit plate text when not random |
| `color` | string | Plate style key (see below) |
| `include_image` | bool | Default `true`; prefer `false` for automation |

### Response fields

`plate`, `color`, `image_path`, `image_base64` (optional), `created_at`, `source`

### `color` values

| Key | Meaning |
|-----|---------|
| `blue` | Blue civil plate |
| `yellow` | Yellow plate |
| `green_car` | New-energy passenger |
| `green_truck` | New-energy commercial |
| `white` | Police |
| `white_army` | Military |
| `black` | HK/Macau |
| `black_shi` | Embassy / consular |

## Plate number rules

| Type | Pattern |
|------|---------|
| Ordinary (e.g. blue) | Province + **letter** + 5-char serial (digit-heavy) |
| New energy (`green_*`) | Province + letter + **D/F** + 5-char serial (8 chars total) |

Invalid patterns are rejected by the generator.

## GUI reference

| Action | How |
|--------|-----|
| Generate | Side panel: Random / Fixed → color → **生成车牌** |
| Copy plate | Side button, click large plate text, double-click history, or fullscreen **复制** |
| Copy API base URL | Click accent URL or **复制地址** in the title bar |
| Enter fullscreen | `F11` or **全屏查看** |
| Exit fullscreen | Double-click **双击退出**, or `Esc` |

Single-click on the preview does **not** enter fullscreen.

## Local development

```bash
cargo run --release                 # GUI + API
cargo run --release -- --api-only   # API only
```

Packaging and cross-compile notes: repository `README.md`.

## Installing this skill

**Project scope:** open this repository in Cursor (skill path `.cursor/skills/plate-studio/`).

**User scope** (available in all projects):

```bash
git clone --depth 1 https://github.com/JMKingovo/plate-studio.git /tmp/plate-studio
mkdir -p ~/.cursor/skills
cp -a /tmp/plate-studio/.cursor/skills/plate-studio ~/.cursor/skills/
```

```powershell
git clone --depth 1 https://github.com/JMKingovo/plate-studio.git $env:TEMP\plate-studio
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.cursor\skills" | Out-Null
Copy-Item -Recurse -Force `
  "$env:TEMP\plate-studio\.cursor\skills\plate-studio" `
  "$env:USERPROFILE\.cursor\skills\"
```
