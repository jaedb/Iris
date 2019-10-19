
const blacklist = [
  'jamesbarnsley.co.nz',
  'following/contains',
  'followers/contains',
  'me/tracks',
  'me/albums',
  'me/following',
  'refresh_spotify_token'
];
function inBlacklist(url) {
  for (let item of blacklist) {
    if (url.indexOf(item) >= 0){
      return true;
    }
  }
}

self.addEventListener('activate', function(event) {
  event.waitUntil(
    caches.keys().then(cacheNames => {
      return Promise.all(
        cacheNames.filter(cacheName => {
          // Return true if you want to remove this cache,
          // but remember that caches are shared across
          // the whole origin
          return true;
        }).map(cacheName => {
          return caches.delete(cacheName);
        })
      );
    })
  );
});

self.addEventListener('fetch', event => {
  const { request } = event;

  event.respondWith(

    // Opens Cache objects that start with 'font'.
    caches.open('iris').then(cache => {
      return cache.match(request)
        .then(response => {
          if (response) {
            return response;
          }

          // Not cached, so we make the request, return that and also save response in cache
          return fetch(request)
            .then(liveResponse => {

              const isBlacklisted = inBlacklist(request.url);

              // Only cache successful GET requests
              if (!isBlacklisted &&
                  request.method === 'GET' &&
                  liveResponse.status >= 200 &&
                  liveResponse.status < 400
                ) {
                cache.put(request, liveResponse.clone());
              } else {
                console.info(`Not caching ${isBlacklisted ? '(blacklisted) ' : ''}${request.method} ${request.url}`);
              }

              return liveResponse;
            });

        // Exceptions from match() or fetch()
        }).catch(error => {
          throw error;
        });
    })
  );
});