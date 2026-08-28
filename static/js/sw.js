// ============================================================
// Louis Space v2.3.0 — Service Worker
// ============================================================
// Chiến lược:
//   - /static/* và /uploads/* (immutable, URL có ?v=hash hoặc UUID):
//     CACHE-FIRST với fallback network. Visit sau → serve từ cache
//     ngay lập tức (0 round-trip, FCP cực nhanh).
//   - HTML routes (/, /games/*, /news/*, /repos...):
//     NETWORK-FIRST với fallback cache (offline). Khi online → luôn
//     lấy HTML fresh từ server (đảm bảo content mới nhất, không stale).
//     Khi offline → fallback về cache gần nhất.
//   - API (/api/*, /chat/*): NETWORK-ONLY (không cache).
//   - POST/PUT/PATCH/DELETE: BYPASS (không intercept, gửi thẳng).
//
// AN TOÀN:
//   - Cache version key 'ls-sw-v2.7.0' — bump khi cần invalidate cache
//     (vd: schema thay đổi). Activate handler xoá cache cũ.
//   - Cache 50 entry tối đa cho HTML (LRU-eviction ngầm).
//   - Static cache KHÔNG giới hạn (immutable, không stale).
//   - KHÔNG cache response 5xx/4xx (chỉ cache 2xx).
//   - Opaque responses (CORS) không cache (anonymous origin).
// ============================================================

var CACHE_VERSION = 'ls-sw-v2.8.0';
var STATIC_CACHE = CACHE_VERSION + '-static';
var HTML_CACHE = CACHE_VERSION + '-html';
var HTML_CACHE_MAX = 50;

// Static asset prefixes — cache-first
var STATIC_PREFIXES = [
    '/static/',
    '/uploads/',
    '/manifest.json'
];

// HTML route patterns — network-first with fallback
function isHtmlRequest(url) {
    // Loại trừ /api/*, /chat/*, /opensearch*, /rss*, /sitemap*
    if (/^\/api\//.test(url.pathname)) return false;
    if (/^\/chat\//.test(url.pathname)) return false;
    if (/^\/ai\//.test(url.pathname)) return false;
    if (/^\/opensearch/.test(url.pathname)) return false;
    if (/^\/rss/.test(url.pathname)) return false;
    if (/^\/sitemap/.test(url.pathname)) return false;
    if (/^\/robots/.test(url.pathname)) return false;
    if (/^\/health/.test(url.pathname)) return false;
    if (/^\/.well-known\//.test(url.pathname)) return false;
    // File có extension (vd: .css, .js, .png) → không phải HTML
    if (/\.[a-z0-9]{2,5}$/i.test(url.pathname)) return false;
    return true;
}

function isStaticAsset(url) {
    return STATIC_PREFIXES.some(function(p) {
        return url.pathname.startsWith(p);
    });
}

// Install: pre-cache các critical assets (homepage, htmx, style.css)
// để LCP install-first cũng có dữ liệu.
self.addEventListener('install', function(event) {
    event.waitUntil(
        caches.open(STATIC_CACHE).then(function(cache) {
            return cache.addAll([
                '/static/js/htmx.min.js?v=2.8.0',
                '/static/css/style.css?v=2.8.0',
                '/static/css/fonts.css?v=2.8.0',
                '/static/js/app.js?v=2.8.0',
                '/static/img/favicon.svg'
            ]).catch(function() {
                // Critical pre-cache fail (vd: file chưa tồn tại) → không
                // block install. SW sẽ cache lazy khi fetch đầu tiên.
            });
        }).then(function() {
            return self.skipWaiting();
        })
    );
});

// Activate: clear cache cũ (của version trước) + claim clients
self.addEventListener('activate', function(event) {
    var allowedCaches = [STATIC_CACHE, HTML_CACHE];
    event.waitUntil(
        caches.keys().then(function(names) {
            return Promise.all(
                names.map(function(name) {
                    if (allowedCaches.indexOf(name) === -1) {
                        return caches.delete(name);
                    }
                    return undefined;
                })
            );
        }).then(function() {
            return self.clients.claim();
        })
    );
});

// Fetch handler
self.addEventListener('fetch', function(event) {
    var req = event.request;
    // Chỉ intercept GET — POST/PUT/PATCH/DELETE bypass thẳng
    if (req.method !== 'GET') return;
    var url = new URL(req.url);
    // Chỉ cache same-origin (cross-origin không có CORS sẽ fail)
    if (url.origin !== self.location.origin) return;

    // === 1) Static assets (cache-first) ===
    if (isStaticAsset(url)) {
        event.respondWith(
            caches.open(STATIC_CACHE).then(function(cache) {
                return cache.match(req).then(function(cached) {
                    if (cached) {
                        // Revalidate nền (stale-while-revalidate): update cache
                        // ngầm cho request sau, không block response.
                        fetch(req).then(function(resp) {
                            if (resp && resp.status === 200) {
                                cache.put(req, resp.clone());
                            }
                        }).catch(function() {
                            // Network fail → giữ nguyên cached version
                        });
                        return cached;
                    }
                    // Not cached → fetch + cache
                    return fetch(req).then(function(resp) {
                        if (resp && resp.status === 200) {
                            cache.put(req, resp.clone());
                        }
                        return resp;
                    }).catch(function() {
                        // Network fail + không cache → trả 504
                        return new Response('Offline — resource not cached', {
                            status: 504,
                            statusText: 'Gateway Timeout'
                        });
                    });
                });
            })
        );
        return;
    }

    // === 2) HTML routes (network-first with cache fallback) ===
    if (isHtmlRequest(url)) {
        event.respondWith(
            caches.open(HTML_CACHE).then(function(cache) {
                return fetch(req).then(function(resp) {
                    // Chỉ cache 2xx
                    if (resp && resp.status === 200) {
                        var clone = resp.clone();
                        cache.put(req, clone).then(function() {
                            // LRU eviction: nếu cache > max → xoá entry cũ nhất
                            cache.keys().then(function(keys) {
                                if (keys.length > HTML_CACHE_MAX) {
                                    // Xoá 1 entry đầu (FIFO ~ LRU vì key order theo insertion)
                                    cache.delete(keys[0]);
                                }
                            });
                        });
                    }
                    return resp;
                }).catch(function() {
                    // Network fail → fallback cache
                    return cache.match(req).then(function(cached) {
                        if (cached) return cached;
                        return new Response(
                            '<!DOCTYPE html><meta charset="utf-8"><title>Offline</title>' +
                            '<h1>Mất kết nối</h1><p>Trang này chưa được cache. ' +
                            'Vui lòng kết nối lại internet.</p>',
                            { status: 503, headers: { 'Content-Type': 'text/html; charset=utf-8' } }
                        );
                    });
                });
            })
        );
        return;
    }

    // === 3) API/chat/opensearch (network-only) ===
    // Không intercept — request đi thẳng đến network.
});

// Message handler — cho phép page chủ động trigger update
self.addEventListener('message', function(event) {
    if (event.data === 'skipWaiting') {
        self.skipWaiting();
    }
});
