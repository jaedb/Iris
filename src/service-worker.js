
const noCacheList = [
  'jamesbarnsley.co.nz'
];
function inNoCacheList(url) {
  for (let item of noCacheList) {
    if (url.indexOf(item) >= 0){
      console.log(`${url} is in our no-cache list`);
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
  event.respondWith(
    
    // Opens Cache objects that start with 'font'.
    caches.open('iris').then(cache => {
      return cache.match(event.request)
        .then(response => {
          
          // Cached response exists, so just return that (unless it's in our no-cache list)
          if (response && !inNoCacheList(event.request.url)) {
            return response;
          }

          // Not cached, so we make the request, return that and also save response in cache
          return fetch(event.request)
            .then(networkResponse => {
              if (networkResponse.status >= 200 && networkResponse.status < 400) {
                cache.put(event.request, networkResponse.clone());
              }
              return networkResponse;
            })
            .catch(error => {
              throw error;
            });

        // Exceptions from match() or fetch()
        }).catch(error => {
          throw error;
        });
    })
  );
});