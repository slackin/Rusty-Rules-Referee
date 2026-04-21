# R3 Demo Video Pipeline

Automated end-to-end pipeline that produces a narrated video tour of the R3
web UI at 720p / 1080p / 1440p, and publishes it to `r3.pugbot.net/media/`
where the VitePress homepage embeds it.

## Architecture

```
seed/           → populate SQLite fixture DB + generate referee-demo.toml
mock/           → UDP UrT RCON responder (stands in for a real game server)
scripts/        → start/stop demo stack, publish to r3.pugbot.net
script/         → narration.json: single source of truth for scenes + copy
record.ts       → Playwright @2560×1440, injects synthetic cursor, WebM out
narrate.ts      → ElevenLabs TTS per scene, auto-pads scene timings
build-video.ts  → ffmpeg: mux audio, watermark, fade, downscale to 3 resolutions
```

All outputs land under `video/out/`:

```
out/
  demo.db                     # seeded SQLite
  referee-demo.toml           # bot config
  raw/walkthrough.webm        # raw Playwright recording
  audio/<scene>.mp3           # ElevenLabs per-scene narration
  narration.m4a               # stitched narration track
  narration.timed.json        # script with per-scene durations after padding
  final/
    r3-demo-1440p.mp4         # master (CRF 16)
    r3-demo-1080p.mp4         # homepage default (CRF 19)
    r3-demo-720p.mp4          # mobile (CRF 21)
    r3-demo-poster.jpg        # poster at t=3s
```

## Prerequisites

- **Node 20+**
- **Rust toolchain** — R3 binary must be built: `cargo build --release` at the
  repo root. Default binary path is `../target/release/rusty-rules-referee(.exe)`
  (override via `R3_BINARY` in `.env`).
- **ffmpeg** on PATH (with `libx264`). `ffprobe` is bundled with ffmpeg.
- **Playwright** browser binaries: `npx playwright install chromium`
- **Internet access** — narration uses Microsoft Edge's public TTS endpoint via
  [`msedge-tts`](https://www.npmjs.com/package/msedge-tts). No API key, no login.
- **ssh key auth** to `bcmx@r3.pugbot.net` for `npm run publish` (same key the
  main `deploy.ps1` uses).

## First-time setup

```bash
cd video
npm install
npx playwright install chromium
cp .env.example .env        # optional — defaults work for local dev
```

## One-shot render

```bash
# In terminal 1: start the demo stack (seed + bot + mock RCON)
npm run seed
npm run stack:start

# In terminal 2: record, narrate, build
npm run record
npm run narrate
npm run build
```

Or the composite (if you already have the stack running):

```bash
npm run video     # seed + record + narrate + build
```

## Publish

```bash
npm run publish
```

Uploads the three MP4s and poster to
`/home/bcmx/domains/r3.pugbot.net/public_html/media/` via rsync+ssh.

## Homepage embed

Already wired into [`docs/index.md`](../docs/index.md): a `<video>` block
below the hero with three `<source>` elements selected by viewport width:
phones get 720p, desktops get 1080p, large displays get 1440p. Poster
preloads at `https://r3.pugbot.net/media/r3-demo-poster.jpg`.

After publishing:

```bash
cd ../docs
npm run docs:build
# (deploy docs as usual via deploy-docs.sh)
```

## Editing the tour

- **Scene copy / timing** — edit [`script/narration.json`](script/narration.json).
  Each scene has a route, narration text, baseline duration, and an array of
  interactions (`waitForSelector` / `idle` / `glide` / `type` / `clearInput` /
  `press` / `click` / `scroll`). `narrate.ts` will automatically extend
  `durationMs` if the synthesized audio is longer than the scene's baseline.
- **Resolution** — change `VIEWPORT` in [`record.ts`](record.ts) and the
  `scale=…` filters in [`build-video.ts`](build-video.ts).
- **Voice** — swap `ELEVENLABS_VOICE_ID` in `.env`.
- **Watermark** — edit `meta.watermark` in `narration.json`.

## Troubleshooting

- **`clients table missing after migrations`** — the `migrations/*.sql` may
  contain MySQL-only statements. Rerun seed with `DEBUG=1` and inspect. The
  seeder tolerates per-file errors but requires at least `clients`.
- **Playwright timeout on `/api/v1/server/status`** — the R3 binary isn't
  binding to `127.0.0.1:8080`. Check `video/out/referee-demo.log`.
- **msedge-tts websocket error** — Microsoft's TTS endpoint has rate limits.
  Wait ~30s and retry; cached MP3s in `out/audio/` are skipped automatically.
- **ffmpeg drawtext error** — install a system font, or remove the watermark
  filter from `masterEncode()` in `build-video.ts`.
- **Raw video is blank** — login failed. Verify the seeded admin credentials
  in `.env` match what was written to the DB.

## Cost & runtime

- Render: ~5–7 min on a modern laptop (Playwright walk ~3:40 + ffmpeg ~2 min).
- Edge TTS: **free** (public Microsoft endpoint, no account required).
- Output size: 1440p ≈ 90 MB, 1080p ≈ 40 MB, 720p ≈ 18 MB.
