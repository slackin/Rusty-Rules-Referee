import 'dotenv/config';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');

export const paths = {
  root,
  out: path.join(root, 'out'),
  raw: path.join(root, 'out', 'raw'),
  audio: path.join(root, 'out', 'audio'),
  final: path.join(root, 'out', 'final'),
  script: path.join(root, 'script', 'narration.json'),
  timed: path.join(root, 'out', 'narration.timed.json'),
  db: process.env.R3_DB_PATH ?? path.join(root, 'out', 'demo.db'),
  config: process.env.R3_CONFIG_PATH ?? path.join(root, 'out', 'referee-demo.toml'),
  pidFile: path.join(root, 'out', 'demo-stack.pid'),
};

export const env = {
  url: process.env.R3_URL ?? 'http://127.0.0.1:2727',
  adminUser: process.env.R3_ADMIN_USER ?? 'admin',
  adminPass: process.env.R3_ADMIN_PASS ?? 'demo-password-r3',
  binary: process.env.R3_BINARY ?? path.resolve(root, '..', 'target', 'release', 'rusty-rules-referee.exe'),
  publishHost: process.env.R3_PUBLISH_HOST ?? 'r3.pugbot.net',
  publishUser: process.env.R3_PUBLISH_USER ?? 'bcmx',
  publishPath: process.env.R3_PUBLISH_PATH ?? '/home/bcmx/domains/r3.pugbot.net/public_html/media',
  // Upload transport — SSH key auth is configured for root@10.10.0.4 (the
  // internal LAN address of r3.pugbot.net) via deploy.ps1, so publish uses
  // that host and chowns files to bcmx:bcmx after upload.
  publishSshHost: process.env.R3_PUBLISH_SSH_HOST ?? '10.10.0.4',
  publishSshUser: process.env.R3_PUBLISH_SSH_USER ?? 'root',
  publishOwner: process.env.R3_PUBLISH_OWNER ?? 'bcmx:bcmx',
};

export interface Interaction {
  type: 'waitForSelector' | 'idle' | 'glide' | 'type' | 'clearInput' | 'press' | 'click' | 'scroll' | 'select' | 'hover';
  selector?: string;
  text?: string;
  value?: string;
  key?: string;
  delay?: number;
  ms?: number;
  y?: number;
  /** For click/type/select: don't fail (or log) if selector is missing. */
  ifExists?: boolean;
  /** For waitForSelector: max timeout in ms. */
  timeoutMs?: number;
  /** Pin this interaction to a specific offset from scene start (ms).
   * If set, the recorder waits until `sceneStart + atMs` before running it.
   * Used to align clicks with narration beats. */
  atMs?: number;
}

export interface Scene {
  id: string;
  route: string;
  narration: string;
  durationMs: number;
  interactions: Interaction[];
  audioMs?: number;
}

export interface Script {
  meta: {
    title: string;
    subtitle: string;
    targetLengthMs: number;
    hardCapMs: number;
    watermark: string;
    voice?: string;
    rate?: string;
    pitch?: string;
  };
  scenes: Scene[];
}
