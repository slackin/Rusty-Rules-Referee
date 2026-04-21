/**
 * Bootstrap a self-signed CA plus server and client certs for the demo
 * master mode. Uses the openssl binary that ships with Git for Windows
 * (or the system one on other platforms).
 *
 * Output: video/out/certs/{ca,server,client}.{crt,key}
 *
 * If the certs already exist the script is a no-op.
 */
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { paths } from '../lib/config.js';

const certsDir = path.join(paths.out, 'certs');

function findOpenssl(): string {
  const candidates = [
    process.env.OPENSSL,
    'C:/Program Files/Git/usr/bin/openssl.exe',
    'C:/Program Files (x86)/Git/usr/bin/openssl.exe',
    'openssl',
  ].filter(Boolean) as string[];
  for (const c of candidates) {
    try {
      execFileSync(c, ['version'], { stdio: 'ignore' });
      return c;
    } catch { /* next */ }
  }
  throw new Error('openssl not found. Install Git for Windows or set $env:OPENSSL=/path/to/openssl');
}

function run(bin: string, args: string[]) {
  execFileSync(bin, args, { stdio: 'inherit' });
}

function exists(p: string) { return fs.existsSync(p); }

function main() {
  fs.mkdirSync(certsDir, { recursive: true });
  const openssl = findOpenssl();
  console.log(`[certs] using ${openssl}`);

  const caKey    = path.join(certsDir, 'ca.key');
  const caCrt    = path.join(certsDir, 'ca.crt');
  const srvKey   = path.join(certsDir, 'server.key');
  const srvCsr   = path.join(certsDir, 'server.csr');
  const srvCrt   = path.join(certsDir, 'server.crt');
  const cliKey   = path.join(certsDir, 'client.key');
  const cliCsr   = path.join(certsDir, 'client.csr');
  const cliCrt   = path.join(certsDir, 'client.crt');
  const extFile  = path.join(certsDir, 'server.ext');

  if (exists(caCrt) && exists(srvCrt) && exists(cliCrt)) {
    console.log('[certs] already present — skipping');
    return;
  }

  // 1. CA
  run(openssl, ['genrsa', '-out', caKey, '2048']);
  run(openssl, ['req', '-x509', '-new', '-nodes', '-key', caKey,
    '-sha256', '-days', '3650', '-out', caCrt,
    '-subj', '/CN=R3 Demo CA']);

  // 2. Server cert (SAN 127.0.0.1 + localhost)
  fs.writeFileSync(extFile,
    `subjectAltName=DNS:localhost,IP:127.0.0.1\n` +
    `basicConstraints=CA:FALSE\n` +
    `keyUsage=digitalSignature,keyEncipherment\n` +
    `extendedKeyUsage=serverAuth\n`);
  run(openssl, ['genrsa', '-out', srvKey, '2048']);
  run(openssl, ['req', '-new', '-key', srvKey, '-out', srvCsr,
    '-subj', '/CN=r3-master']);
  run(openssl, ['x509', '-req', '-in', srvCsr, '-CA', caCrt, '-CAkey', caKey,
    '-CAcreateserial', '-out', srvCrt, '-days', '3650', '-sha256',
    '-extfile', extFile]);

  // 3. Client cert (extendedKeyUsage=clientAuth)
  const cliExt = path.join(certsDir, 'client.ext');
  fs.writeFileSync(cliExt,
    `basicConstraints=CA:FALSE\n` +
    `keyUsage=digitalSignature,keyEncipherment\n` +
    `extendedKeyUsage=clientAuth\n`);
  run(openssl, ['genrsa', '-out', cliKey, '2048']);
  run(openssl, ['req', '-new', '-key', cliKey, '-out', cliCsr,
    '-subj', '/CN=r3-demo-client']);
  run(openssl, ['x509', '-req', '-in', cliCsr, '-CA', caCrt, '-CAkey', caKey,
    '-CAcreateserial', '-out', cliCrt, '-days', '3650', '-sha256',
    '-extfile', cliExt]);

  console.log(`[certs] wrote ca/server/client .crt/.key to ${certsDir}`);
}

main();
