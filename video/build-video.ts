/**
 * Assemble master videos from raw Playwright recording + per-scene narration.
 *
 * Steps:
 *   1. Concatenate per-scene audio with padding into a single narration.mp3
 *      matching the total video duration.
 *   2. Re-encode raw WebM to master MP4 at 2560×1440 libx264 CRF 16, 30fps.
 *   3. Mux audio, add fade in/out, overlay watermark text at start and end.
 *   4. Downscale/re-encode to 1080p and 720p variants with faststart for web.
 *   5. Extract poster JPG at ~3 s.
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { paths, Script } from './lib/config.js';

function log(msg: string) { console.log(`[build] ${msg}`); }

function ffmpeg(args: string[]) {
  log(`ffmpeg ${args.join(' ')}`);
  const r = spawnSync('ffmpeg', ['-y', ...args], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`ffmpeg failed (${r.status})`);
}

function buildAudioTrack(script: Script, outPath: string) {
  // Create a silence-padded concat list: audio for each scene, followed by
  // the remainder of that scene's durationMs as silence.
  const tmpDir = path.join(paths.out, 'audio-tmp');
  fs.mkdirSync(tmpDir, { recursive: true });
  const concatList: string[] = [];

  script.scenes.forEach((scene, idx) => {
    const src = path.join(paths.audio, `${scene.id}.mp3`);
    if (!fs.existsSync(src)) throw new Error(`missing narration for scene ${scene.id}`);
    const audioMs = scene.audioMs ?? 0;
    const silenceMs = Math.max(0, scene.durationMs - audioMs);
    // Normalize each scene to a fixed-format WAV so concat is lossless/reliable
    const normPath = path.join(tmpDir, `${String(idx).padStart(2, '0')}-${scene.id}.wav`);
    ffmpeg([
      '-i', src,
      '-af', `apad=pad_dur=${(silenceMs / 1000).toFixed(3)}`,
      '-ar', '48000', '-ac', '2', '-c:a', 'pcm_s16le',
      normPath,
    ]);
    concatList.push(`file '${normPath.replace(/\\/g, '/')}'`);
  });

  const listFile = path.join(tmpDir, 'concat.txt');
  fs.writeFileSync(listFile, concatList.join('\n'));
  ffmpeg([
    '-f', 'concat', '-safe', '0', '-i', listFile,
    '-c:a', 'aac', '-b:a', '192k',
    outPath,
  ]);
}

function masterEncode(rawVideo: string, audioTrack: string, outPath: string, watermark: string, durationSec: number) {
  // Filter graph: fade in first 1s, fade out last 1s; drawtext watermark visible
  // during first 3s and last 3s only.
  const fadeOutStart = Math.max(0, durationSec - 1);
  const wmShow1 = `between(t,0.5,3.5)`;
  const wmShow2 = `between(t,${(durationSec - 3.5).toFixed(2)},${(durationSec - 0.5).toFixed(2)})`;
  const vf = [
    `scale=2560:1440:flags=lanczos`,
    `fade=t=in:st=0:d=1`,
    `fade=t=out:st=${fadeOutStart.toFixed(2)}:d=1`,
    `drawtext=fontfile='${(process.env.WATERMARK_FONT || 'C\\:/Windows/Fonts/arial.ttf')}':text='${watermark}':fontcolor=white:fontsize=42:alpha=0.85:` +
      `x=w-tw-60:y=h-th-60:box=1:boxcolor=black@0.45:boxborderw=14:` +
      `enable='${wmShow1}+${wmShow2}'`,
  ].join(',');

  ffmpeg([
    '-i', rawVideo,
    '-i', audioTrack,
    '-map', '0:v:0', '-map', '1:a:0',
    '-vf', vf,
    '-c:v', 'libx264', '-preset', 'slow', '-crf', '16',
    '-pix_fmt', 'yuv420p',
    '-r', '30',
    '-c:a', 'aac', '-b:a', '192k',
    '-movflags', '+faststart',
    '-shortest',
    outPath,
  ]);
}

function downscale(src: string, dst: string, height: number, crf: number) {
  ffmpeg([
    '-i', src,
    '-vf', `scale=-2:${height}:flags=lanczos`,
    '-c:v', 'libx264', '-preset', 'slow', '-crf', String(crf),
    '-pix_fmt', 'yuv420p',
    '-c:a', 'aac', '-b:a', '160k',
    '-movflags', '+faststart',
    dst,
  ]);
}

function poster(src: string, dst: string) {
  ffmpeg(['-ss', '3', '-i', src, '-vframes', '1', '-q:v', '2', dst]);
}

function main() {
  const script: Script = JSON.parse(fs.readFileSync(paths.timed, 'utf8'));
  const rawVideo = path.join(paths.raw, 'walkthrough.webm');
  if (!fs.existsSync(rawVideo)) throw new Error(`raw video missing: ${rawVideo}`);
  fs.mkdirSync(paths.final, { recursive: true });

  const audioTrack = path.join(paths.out, 'narration.m4a');
  buildAudioTrack(script, audioTrack);

  const totalMs = script.scenes.reduce((s, sc) => s + sc.durationMs, 0);
  const totalSec = totalMs / 1000;

  const master1440 = path.join(paths.final, 'r3-demo-1440p.mp4');
  masterEncode(rawVideo, audioTrack, master1440, script.meta.watermark, totalSec);

  const master1080 = path.join(paths.final, 'r3-demo-1080p.mp4');
  downscale(master1440, master1080, 1080, 19);

  const master720 = path.join(paths.final, 'r3-demo-720p.mp4');
  downscale(master1440, master720, 720, 21);

  poster(master1440, path.join(paths.final, 'r3-demo-poster.jpg'));

  log('build complete:');
  for (const f of fs.readdirSync(paths.final)) {
    const st = fs.statSync(path.join(paths.final, f));
    log(`  ${f}  ${(st.size / 1024 / 1024).toFixed(1)} MB`);
  }
}

main();
