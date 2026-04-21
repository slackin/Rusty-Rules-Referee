/**
 * Microsoft Edge TTS narration generator.
 *
 * Uses the free `msedge-tts` package, which drives Microsoft's public Azure
 * speech endpoint — no API key, no login, no account. Streams high-quality
 * neural voices directly as MP3.
 *
 * - Reads script/narration.json (voice/rate/pitch in meta)
 * - For each scene, synthesizes out/audio/<id>.mp3
 * - Probes each MP3 via ffprobe for exact duration
 * - Pads scene.durationMs to ≥ audio length + 500ms
 * - Writes out/narration.timed.json with final timings
 */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
// @ts-ignore — msedge-tts ships its own d.ts in some versions, JS-only in others
import { MsEdgeTTS, OUTPUT_FORMAT } from 'msedge-tts';
import { paths, Script } from './lib/config.js';

function log(msg: string) { console.log(`[narrate] ${msg}`); }

function probeDurationMs(file: string): number {
  const out = execFileSync('ffprobe', [
    '-v', 'error',
    '-show_entries', 'format=duration',
    '-of', 'default=noprint_wrappers=1:nokey=1',
    file,
  ], { encoding: 'utf8' });
  return Math.round(parseFloat(out.trim()) * 1000);
}

async function synthesizeOne(tts: any, text: string, outPath: string, opts: { rate?: string; pitch?: string }) {
  // v2 API: toFile(dirPath, input, options) → { audioFilePath, metadataFilePath }
  // We ask msedge-tts to write inside the audio/ directory, then rename the
  // generated file to our scene-named outPath.
  const dir = path.dirname(outPath);
  const result = await tts.toFile(dir, text, opts);
  const srcAudio = result?.audioFilePath;
  if (!srcAudio || !fs.existsSync(srcAudio)) {
    throw new Error(`msedge-tts returned no audio file (got: ${JSON.stringify(result)})`);
  }
  if (srcAudio !== outPath) {
    if (fs.existsSync(outPath)) fs.unlinkSync(outPath);
    fs.renameSync(srcAudio, outPath);
  }
  // Clean up the metadata sidecar if any.
  if (result?.metadataFilePath && fs.existsSync(result.metadataFilePath)) {
    try { fs.unlinkSync(result.metadataFilePath); } catch {}
  }
}

async function main() {
  const script: Script = JSON.parse(fs.readFileSync(paths.script, 'utf8'));
  fs.mkdirSync(paths.audio, { recursive: true });

  const voice = script.meta.voice ?? 'en-US-GuyNeural';
  const rate = script.meta.rate ?? '+0%';
  const pitch = script.meta.pitch ?? '+0Hz';

  const tts = new MsEdgeTTS();
  log(`voice: ${voice}  rate: ${rate}  pitch: ${pitch}`);
  await tts.setMetadata(voice, OUTPUT_FORMAT.AUDIO_24KHZ_96KBITRATE_MONO_MP3);

  const PAD_MS = 500;
  let total = 0;

  for (const scene of script.scenes) {
    const outPath = path.join(paths.audio, `${scene.id}.mp3`);
    if (fs.existsSync(outPath) && !process.env.FORCE_NARRATE) {
      log(`${scene.id}: cached`);
    } else {
      log(`${scene.id}: synthesizing (${scene.narration.length} chars)`);
      try {
        await synthesizeOne(tts, scene.narration, outPath, { rate, pitch });
      } catch (err: any) {
        throw new Error(`failed to synthesize ${scene.id}: ${err.message ?? err}`);
      }
    }
    const audioMs = probeDurationMs(outPath);
    scene.audioMs = audioMs;
    const needed = audioMs + PAD_MS;
    if (scene.durationMs < needed) {
      log(`${scene.id}: padding ${scene.durationMs}ms → ${needed}ms  (audio ${audioMs}ms)`);
      scene.durationMs = needed;
    } else {
      log(`${scene.id}: audio ${audioMs}ms, scene ${scene.durationMs}ms — ok`);
    }
    total += scene.durationMs;
  }

  log(`total duration: ${(total / 1000).toFixed(1)}s (target ${script.meta.targetLengthMs / 1000}s, cap ${script.meta.hardCapMs / 1000}s)`);
  if (total > script.meta.hardCapMs) {
    log(`WARNING: exceeds hard cap by ${((total - script.meta.hardCapMs) / 1000).toFixed(1)}s`);
  }

  fs.writeFileSync(paths.timed, JSON.stringify(script, null, 2));
  log(`wrote ${paths.timed}`);
  // Clean exit: msedge-tts keeps a websocket alive otherwise
  try { (tts as any).close?.(); } catch {}
}

main().catch((err) => { console.error(err); process.exit(1); });
