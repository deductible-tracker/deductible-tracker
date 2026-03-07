// Service Worker for Deductible Tracker - offline asset caching
const CACHE_NAME = 'dt-cache-v1';

// Core assets to pre-cache on install
const PRECACHE_ASSETS = [
    '/',
    '/vendor/dexie-4.3.0.min.js',
    '/vendor/lucide.min.js',
    '/assets/tailwind.css'
];

self.addEventListener('install', event => {
    event.waitUntil(
        caches.open(CACHE_NAME).then(cache => {
            return cache.addAll(PRECACHE_ASSETS);
        }).then(() => self.skipWaiting())
    );
});

self.addEventListener('activate', event => {
    event.waitUntil(
        caches.keys().then(keys =>
            Promise.all(keys.filter(k => k !== CACHE_NAME).map(k => caches.delete(k)))
        ).then(() => self.clients.claim())
    );
});

self.addEventListener('fetch', event => {
    const url = new URL(event.request.url);

    // Skip non-GET requests and API/auth calls (these must hit the network)
    if (event.request.method !== 'GET') return;
    if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/auth/') || url.hostname === 'accounts.google.com') return;

    event.respondWith(
        caches.match(event.request).then(cached => {
            // Network-first for HTML (to pick up new fingerprinted asset URLs)
            if (event.request.headers.get('accept')?.includes('text/html')) {
                return fetch(event.request)
                    .then(response => {
                        if (response.ok) {
                            const clone = response.clone();
                            caches.open(CACHE_NAME).then(c => c.put(event.request, clone));
                        }
                        return response;
                    })
                    .catch(() => cached || new Response('Offline', { status: 503 }));
            }

            // Cache-first for static assets (JS, CSS, fonts, vendor)
            if (cached) return cached;

            return fetch(event.request).then(response => {
                if (response.ok && (
                    url.pathname.startsWith('/assets/') ||
                    url.pathname.startsWith('/vendor/') ||
                    url.pathname.startsWith('/css/')
                )) {
                    const clone = response.clone();
                    caches.open(CACHE_NAME).then(c => c.put(event.request, clone));
                }
                return response;
            }).catch(() => {
                return new Response('Offline', { status: 503 });
            });
        })
    );
});
