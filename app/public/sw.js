// Offline support. After each release build, `stamp-sw` replaces the two
// __BUILD_…__ placeholders with the build's files and a hash of their contents. Every deploy
// that changes anything (code, phrases, a re-recorded clip) gets a fresh cache,
// and the whole app, audio included, is stored at install so it works offline
// from the first launch.
const VERSION = '__BUILD_VERSION__';
const PRECACHE = '__BUILD_FILES__'.split('|').filter((u) => u && !u.startsWith('__'));
const CACHE = `baatcheet-${VERSION}`;
const FONTS = 'baatcheet-fonts';

self.addEventListener('install', (event) => {
  event.waitUntil((async () => {
    const cache = await caches.open(CACHE);
    await cache.addAll(PRECACHE.length ? PRECACHE : ['./']);
    await self.skipWaiting();
  })());
});

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key !== CACHE && key !== FONTS) await caches.delete(key);
    }
    await self.clients.claim();
  })());
});

self.addEventListener('fetch', (event) => {
  const req = event.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.hostname.endsWith('gstatic.com') || url.hostname.endsWith('googleapis.com')) {
    event.respondWith(cacheFirst(req, FONTS));
  } else if (url.origin === location.origin) {
    event.respondWith(fromBuild(req));
  }
});

// Files from this build come straight from the cache: fast on a weak signal,
// and they can't be stale because the cache is replaced on every deploy.
async function fromBuild(req) {
  const cache = await caches.open(CACHE);
  const hit = await cache.match(req, { ignoreSearch: true })
    || (req.mode === 'navigate' ? await cache.match('./') : undefined);
  if (hit) return hit;
  const res = await fetch(req);
  if (res.ok) cache.put(req, res.clone());
  return res;
}

async function cacheFirst(req, name) {
  const cache = await caches.open(name);
  const hit = await cache.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (res.ok || res.type === 'opaque') cache.put(req, res.clone());
  return res;
}
