#!/usr/bin/env node
/**
 * Game of Peram static server.
 * /game/ → public/game/  |  /src/ → src/  (ESM pure session modules)
 */
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const port = Number(process.argv[2] || process.env.PORT || 4173);

const types = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.wasm': 'application/wasm',
  '.svg': 'image/svg+xml',
  '.map': 'application/json',
  '.ico': 'image/x-icon',
  '.png': 'image/png',
};

function mapUrl(urlPath) {
  // Canonical game mount — assets live under /game/*
  if (urlPath === '/game') return { redirect: '/game/' };
  if (urlPath === '/' || urlPath === '/game/') return '/public/game/index.html';
  if (urlPath.startsWith('/game/')) {
    return `/public/game/${urlPath.slice('/game/'.length)}`;
  }
  // Life-derived watch graph (CLI export) — same host as the game
  if (urlPath === '/watch' || urlPath === '/watch/') return '/public/watch/index.html';
  if (urlPath.startsWith('/watch/')) {
    return `/public/watch/${urlPath.slice('/watch/'.length)}`;
  }
  // Legacy absolute public paths still work
  if (urlPath.startsWith('/public/')) return urlPath;
  if (urlPath.startsWith('/src/')) return urlPath;
  // Bare asset mistakes from relative URLs without trailing slash
  if (
    [
      '/styles.css',
      '/main.js',
      '/engine.js',
      '/render.js',
      '/sample-graph.json',
      '/life-graph.json',
    ].includes(urlPath)
  ) {
    return `/public/game${urlPath}`;
  }
  if (urlPath.startsWith('/pkg/')) return `/public/game${urlPath}`;
  return urlPath;
}

const server = http.createServer((req, res) => {
  try {
    const raw = decodeURIComponent((req.url || '/').split('?')[0]);
    const mapped = mapUrl(raw);
    if (mapped && typeof mapped === 'object' && mapped.redirect) {
      res.writeHead(302, { Location: mapped.redirect, 'Cache-Control': 'no-store' });
      res.end();
      return;
    }
    let urlPath = mapped;
    let filePath = path.normalize(path.join(root, String(urlPath).replace(/^\//, '')));
    if (!filePath.startsWith(root)) {
      res.writeHead(403);
      res.end('forbidden');
      return;
    }
    if (fs.existsSync(filePath) && fs.statSync(filePath).isDirectory()) {
      const idx = path.join(filePath, 'index.html');
      if (fs.existsSync(idx)) filePath = idx;
    }
    if (!fs.existsSync(filePath) || fs.statSync(filePath).isDirectory()) {
      res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
      res.end(`not found: ${raw} → ${urlPath}`);
      return;
    }
    const ext = path.extname(filePath);
    res.writeHead(200, {
      'Content-Type': types[ext] || 'application/octet-stream',
      'Cache-Control': 'no-store',
    });
    fs.createReadStream(filePath).pipe(res);
  } catch (err) {
    res.writeHead(500);
    res.end(String(err));
  }
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Game of Peram → http://127.0.0.1:${port}/game/`);
});
