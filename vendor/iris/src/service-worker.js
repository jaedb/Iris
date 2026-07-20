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

    // Opens Cache objects that start with 'font'.
    caches.open('iris').then((cache) => cache.match(request)
      .then((response) => {
        if (response) {
          return response;
        }

        // Not cached, so we make the request, return that and also save response in cache
        return fetch(request)
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
              if (!/^https?:$/i.test(new URL(request.url).protocol)) return;

              cache.put(request, liveResponse.clone());
            } else {
              console.info(`Not caching ${isBlacklisted ? '(blacklisted) ' : ''}${request.method} ${request.url}`);
            }

            return liveResponse;
          });

        // Exceptions from match() or fetch()
      }).catch((error) => {
        throw error;
      })),
  );
});
