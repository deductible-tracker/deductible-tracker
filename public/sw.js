const CACHE_NAME='dt-cache-15d9d7c0f62c';function createOfflineResponse(){const offlineHtml=`
        <!doctype html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <title>Offline</title>
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <style>
                body { font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 0; padding: 2rem; background: #0f172a; color: #e5e7eb; display: flex; align-items: center; justify-content: center; min-height: 100vh; }
                .container { max-width: 28rem; text-align: center; }
                h1 { font-size: 1.5rem; margin-bottom: 0.75rem; }
                p { margin: 0.25rem 0; color: #9ca3af; }
                small { display: block; margin-top: 1rem; color: #6b7280; }
            </style>
        </head>
        <body>
            <main class="container">
                <h1>You’re offline</h1>
                <p>We couldn’t reach the server. Some data may be unavailable until you’re back online.</p>
                <p>Please check your internet connection and try again.</p>
                <small>Deductible Tracker</small>
            </main>
        </body>
        </html>
    `;return new Response(offlineHtml,{status:503,headers:{'Content-Type':'text/html; charset=utf-8',},});}const PRECACHE_ASSETS=["/","/assets/boot-e9a42cb64cc6.js","/assets/css/fonts-35484224e7dd.css","/assets/tailwind-6c7c646605e8.css","/assets/vendor/dexie-4.3.0.min-a5e8788ebb05.js"];self.addEventListener('install',(event)=>{event.waitUntil(caches.open(CACHE_NAME).then((cache)=>{return cache.addAll(PRECACHE_ASSETS);}).then(()=>self.skipWaiting()));});self.addEventListener('activate',(event)=>{event.waitUntil(caches.keys().then((keys)=>Promise.all(keys.filter((k)=>k!==CACHE_NAME).map((k)=>caches.delete(k)))).then(()=>self.clients.claim()));});self.addEventListener('fetch',(event)=>{const url=new URL(event.request.url);if(event.request.method!=='GET'){event.respondWith(fetch(event.request));return;}if(url.pathname.startsWith('/api/')||url.pathname.startsWith('/auth/')||url.hostname==='accounts.google.com'){event.respondWith(fetch(event.request));return;}event.respondWith(caches.match(event.request).then((cached)=>{if(event.request.headers.get('accept')?.includes('text/html')){return fetch(event.request).then((response)=>{if(response.ok){const clone=response.clone();caches.open(CACHE_NAME).then((c)=>c.put(event.request,clone));}return response;}).catch(()=>cached||createOfflineResponse());}if(cached)return cached;return fetch(event.request).then((response)=>{if(response.ok&&(url.pathname.startsWith('/assets/')||url.pathname.startsWith('/vendor/')||url.pathname.startsWith('/css/')||url.pathname.startsWith('/fonts/'))){const clone=response.clone();caches.open(CACHE_NAME).then((c)=>c.put(event.request,clone));}return response;}).catch(()=>{return createOfflineResponse();});}));});