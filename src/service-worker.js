const blacklist = [
  'jamesbarnsley.co.nz',
  'following/contains',
  'followers/contains',
  'me/tracks',
  'me/albums',
  'me/following',
  'refresh_spotify_token',
];
function inBlacklist(url) {
  for (const item of blacklist) {
    if (url.indexOf(item) >= 0) {
      return true;
    }
  }
  return false;
}

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((cacheNames) => Promise.all(cacheNames.map((cacheName) => caches.delete(cacheName)))),
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;

  event.respondWith(
    caches.open('iris').then((cache) => fetch(request)
      // Network-first: always prefer a live response so app updates (new
      // JS/CSS bundles, fresh API data) are visible immediately, rather
      // than being masked by a stale cache indefinitely. This cache is
      // now purely an offline fallback, not a source of truth.
      .then((liveResponse) => {
        const isBlacklisted = inBlacklist(request.url);

        // Only cache successful GET requests
        if (
          !isBlacklisted
            && request.method === 'GET'
            && liveResponse.status >= 200
            && liveResponse.status < 400
        ) {
          // Fixes Edge browser "'chrome-extension' is unsupported" issue
          // See https://stackoverflow.com/questions/49157622/service-worker-typeerror-when-opening-chrome-extension
          if (!/^https?:$/i.test(new URL(request.url).protocol)) return liveResponse;

          cache.put(request, liveResponse.clone());
        } else {
          console.info(`Not caching ${isBlacklisted ? '(blacklisted) ' : ''}${request.method} ${request.url}`);
        }

        return liveResponse;
      })
      // Network unavailable (offline): fall back to whatever we have cached.
      .catch(() => cache.match(request).then((cachedResponse) => {
        if (cachedResponse) {
          return cachedResponse;
        }

        throw new Error(`Network request failed and no cached response is available for ${request.url}`);
      }))),
  );
});
