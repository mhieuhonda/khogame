// ============================================
// Louis Space v2.0 — Frontend JavaScript
// ============================================
// Kiến trúc: 1 IIFE, chia module rõ ràng:
//   1. Toast system          — thông báo nổi góc phải
//   2. Theme                 — light/dark + sync server + multi-tab
//   3. HTMX enhancements     — progress bar, error toast, loading
//   4. Search                — autocomplete + phím tắt "/"
//   5. Mega menu             — hamburger + click-outside + Esc
//   6. Announcement          — banner toàn site, dismiss theo session
//   7. Duplicate check       — cảnh báo trùng tiêu đề game/tin
//   8. Upload generic        — 1 handler cho mọi .upload-zone
//   9. Share                 — delegation qua data-share-platform
//  10. Forms UX              — char counter, confirm, double-submit
//  11. Misc                  — auto-resize, notif auto-read, smooth scroll
// ============================================

(function() {
    'use strict';

    // ==========================================
    // 1. TOAST SYSTEM
    // ==========================================
    var toastContainer = null;

    function getToastContainer() {
        if (!toastContainer || !document.body.contains(toastContainer)) {
            toastContainer = document.createElement('div');
            toastContainer.className = 'toast-container';
            toastContainer.setAttribute('aria-live', 'polite');
            document.body.appendChild(toastContainer);
        }
        return toastContainer;
    }

    function toast(message, type, duration) {
        type = type || 'info';
        duration = duration || 3200;
        var t = document.createElement('div');
        t.className = 'toast toast-' + type;
        t.setAttribute('role', 'status');
        t.textContent = message;
        getToastContainer().appendChild(t);
        setTimeout(function() {
            t.classList.add('toast-out');
            setTimeout(function() { t.remove(); }, 220);
        }, duration);
        // Không stack quá 4 toast cùng lúc
        var all = toastContainer.children;
        while (all.length > 4) {
            toastContainer.removeChild(toastContainer.firstChild);
        }
    }

    // Expose cho inline script & HTMX events
    window.lsToast = toast;

    // ==========================================
    // 2. THEME — light/dark
    // localStorage key 'ls-theme'. Tương thích lùi 'kg-theme'.
    // ==========================================
    function getStoredTheme() {
        var ls = localStorage.getItem('ls-theme');
        if (ls === 'dark' || ls === 'light') {
            return ls;
        }
        var legacy = localStorage.getItem('kg-theme');
        if (legacy === 'dark' || legacy === 'light') {
            localStorage.setItem('ls-theme', legacy);
            localStorage.removeItem('kg-theme');
            return legacy;
        }
        return null;
    }

    function applyTheme(theme) {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('ls-theme', theme);
    }

    // Lần đầu (chưa chọn): theo hệ điều hành. Louis Space ưu tiên light.
    function initialTheme() {
        var stored = getStoredTheme();
        if (stored === 'dark' || stored === 'light') return stored;
        if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) {
            return 'dark';
        }
        return 'light';
    }

    function toggleTheme() {
        var current = document.documentElement.getAttribute('data-theme') || 'light';
        var next = current === 'dark' ? 'light' : 'dark';
        applyTheme(next);
        // Đồng bộ theme lên server (nếu đã đăng nhập) — fail-safe im lặng
        fetch('/api/preferences/theme', {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: 'theme=' + encodeURIComponent(next)
        }).catch(function() {});
    }

    applyTheme(initialTheme());

    // Đồng bộ theme giữa các tab cùng site qua storage event
    window.addEventListener('storage', function(e) {
        if (e.key === 'ls-theme' && (e.newValue === 'dark' || e.newValue === 'light')) {
            document.documentElement.setAttribute('data-theme', e.newValue);
        }
        if (e.key === 'kg-theme' && (e.newValue === 'dark' || e.newValue === 'light')) {
            localStorage.setItem('ls-theme', e.newValue);
            localStorage.removeItem('kg-theme');
            document.documentElement.setAttribute('data-theme', e.newValue);
        }
    });

    // ==========================================
    // 3. HTMX ENHANCEMENTS
    // ==========================================
    var progressBar = null;
    // v3.6.2 — đếm số request ĐANG chạy: nhiều request chồng nhau (page
    // load bắn 5-10 HTMX song song) trước đây cùng toggle 1 bar → nhấp
    // nháy liên tục ("thanh tím giật giật trên đầu trang"). Giờ bar chỉ
    // tắt khi request CUỐI CÙNG kết thúc.
    var pendingRequests = 0;
    var progressHideTimer = null;

    // v3.6.2 — CHỈ hiện progress bar cho thao tác người dùng chủ động
    // (click/submit/change). Background request (hx-trigger="load" của
    // widget điểm danh chạy trên MỌI trang, "revealed" lazy-load reply,
    // "every Ns" polling admin) KHÔNG còn nháy thanh tím nữa — nguyên
    // nhân chính gây "thanh mỏng màu tím ở trên cùng giật giật liên
    // tục rồi biến mất".
    function isUserInitiated(evt) {
        var elt = evt.detail && evt.detail.elt;
        if (!elt || !elt.getAttribute) return true;
        var trigger = elt.getAttribute('hx-trigger') || '';
        if (/every\s+\d/.test(trigger)) return false;  // polling nền
        if (/\bload\b/.test(trigger)) return false;    // load trang
        if (/\brevealed\b/.test(trigger)) return false; // lazy-load khi cuộn
        return true;
    }

    function showProgress() {
        if (!progressBar) progressBar = document.getElementById('htmx-progress');
        if (!progressBar) return;
        if (progressHideTimer) { clearTimeout(progressHideTimer); progressHideTimer = null; }
        pendingRequests++;
        requestAnimationFrame(function() {
            progressBar.classList.remove('done');
            progressBar.classList.add('active');
        });
    }

    function finishProgress() {
        pendingRequests = Math.max(0, pendingRequests - 1);
        if (pendingRequests > 0) return; // vẫn còn request khác đang chạy
        if (!progressBar) progressBar = document.getElementById('htmx-progress');
        if (progressBar) {
            progressBar.classList.add('done');
            progressHideTimer = setTimeout(function() {
                pendingRequests = 0;
                progressBar.classList.remove('active', 'done');
            }, 250);
        }
    }

    function initHtmx() {
        if (!window.htmx) return;

        // Progress bar mỏng dưới header khi có request người dùng chủ động
        document.body.addEventListener('htmx:beforeRequest', function(evt) {
            if (!isUserInitiated(evt)) return;
            showProgress();
        });

        document.body.addEventListener('htmx:afterRequest', function(evt) {
            if (!isUserInitiated(evt)) return;
            finishProgress();
        });

        // v3.6.2 — lỗi HTMX → toast THÂN THIỆN: đọc message tiếng Việt
        // server render trong partial lỗi (.error-message) thay vì bắn
        // "Lỗi kết nối (HTTP 400)" vô nghĩa. Từ giờ mua Hộp XP thiếu tiền
        // hay hết lượt trong ngày sẽ hiện ĐÚNG lý do ("Không đủ XP — cần
        // 100 XP...") thay vì báo lỗi kết nối đáng ngờ.
        function extractServerMessage(xhr) {
            if (!xhr || !xhr.responseText) return null;
            try {
                var doc = new DOMParser().parseFromString(xhr.responseText, 'text/html');
                var el = doc.querySelector('.error-message');
                var txt = el ? el.textContent.trim() : '';
                return txt || null;
            } catch (e) { return null; }
        }

        document.body.addEventListener('htmx:responseError', function(evt) {
            var xhr = evt.detail.xhr;
            var status = xhr ? xhr.status : 0;
            var serverMsg = extractServerMessage(xhr);
            // Message từ server luôn ưu tiên — là lý do NGHIÊM MÔN của lỗi
            if (serverMsg) { toast(serverMsg, 'error', 4600); return; }
            var msg = 'Lỗi kết nối (HTTP ' + status + '). Vui lòng thử lại.';
            if (status === 401) {
                msg = 'Cần đăng nhập để thực hiện hành động này.';
            } else if (status === 403) {
                msg = 'Bạn không có quyền thực hiện hành động này.';
            } else if (status === 429) {
                msg = 'Thao tác quá nhanh — vui lòng đợi chút rồi thử lại.';
            } else if (status >= 500) {
                msg = 'Máy chủ gặp sự cố (HTTP ' + status + '). Vui lòng thử lại sau.';
            }
            toast(msg, 'error', 4200);
        });

        // Swap thành công chứa .error-partial → toast lỗi (kể cả 200 OK
        // mà server trả partial lỗi — handler validation tự render)
        document.body.addEventListener('htmx:afterSwap', function(evt) {
            var errBox = evt.detail && evt.detail.target ?
                evt.detail.target.querySelector : null;
            if (typeof errBox === 'function') {
                var err = evt.detail.target.querySelector('.error-partial .error-message');
                if (err) toast(err.textContent, 'error', 4600);
                var ok = evt.detail.target.querySelector('[data-toast-success]');
                if (ok) toast(ok.getAttribute('data-toast-success'), 'success');
            }
        });
    }

    // ==========================================
    // 4. SEARCH — autocomplete + phím tắt "/"
    // ==========================================
    function initSearchAutocomplete() {
        var searchInput = document.querySelector('.search-bar input[name="q"]');
        if (!searchInput) return;
        var suggestTimer = null;
        var suggestBox = null;
        var activeIndex = -1;
        var wrap = searchInput.closest('.search-bar');

        function hideSuggestions() {
            if (suggestBox) { suggestBox.remove(); suggestBox = null; }
            activeIndex = -1;
            searchInput.removeAttribute('aria-activedescendant');
        }

        function highlightActive() {
            if (!suggestBox) return;
            var opts = suggestBox.querySelectorAll('.search-suggest-item');
            opts.forEach(function(a, i) {
                var active = i === activeIndex;
                a.classList.toggle('active', active);
                if (active) {
                    a.id = 'suggest-opt-' + i;
                    searchInput.setAttribute('aria-activedescendant', 'suggest-opt-' + i);
                    if (a.scrollIntoView) a.scrollIntoView({ block: 'nearest' });
                }
            });
        }

        function showSuggestions(items) {
            hideSuggestions();
            if (!items.length) return;
            suggestBox = document.createElement('div');
            suggestBox.className = 'search-suggest';
            suggestBox.setAttribute('role', 'listbox');
            suggestBox.setAttribute('aria-label', 'Gợi ý tìm kiếm');
            items.forEach(function(it) {
                var a = document.createElement('a');
                a.className = 'search-suggest-item';
                a.href = it.url;
                a.setAttribute('role', 'option');
                a.textContent = it.title;
                suggestBox.appendChild(a);
            });
            wrap.appendChild(suggestBox);
        }

        // v2.9.0 — Focus vào input rỗng → hiện lịch sử tìm kiếm gần đây
        searchInput.addEventListener('focus', function() {
            if (searchInput.value.trim().length >= 2) return;
            var history = getSearchHistory ? getSearchHistory() : [];
            if (!history.length) return;
            hideSuggestions();
            suggestBox = document.createElement('div');
            suggestBox.className = 'search-suggest';
            suggestBox.setAttribute('role', 'listbox');
            suggestBox.setAttribute('aria-label', 'Tìm kiếm gần đây');
            var heading = document.createElement('div');
            heading.className = 'search-suggest-heading';
            heading.textContent = 'Tìm kiếm gần đây';
            suggestBox.appendChild(heading);
            history.forEach(function(q) {
                var a = document.createElement('a');
                a.className = 'search-suggest-item';
                a.href = '/search?q=' + encodeURIComponent(q);
                a.setAttribute('role', 'option');
                a.textContent = '🕘 ' + q;
                suggestBox.appendChild(a);
            });
            wrap.appendChild(suggestBox);
        });

        searchInput.addEventListener('input', function() {
            clearTimeout(suggestTimer);
            var v = this.value.trim();
            if (v.length < 2) { hideSuggestions(); return; }
            suggestTimer = setTimeout(function() {
                fetch('/api/suggest?q=' + encodeURIComponent(v))
                    .then(function(r) { return r.json(); })
                    .then(function(d) { showSuggestions(d.data || []); })
                    .catch(function() {});
            }, 250);
        });

        // Keyboard nav: ↑ ↓ di chuyển, Enter chọn, Esc đóng (WCAG 2.1.1)
        searchInput.addEventListener('keydown', function(e) {
            if (!suggestBox) return;
            var opts = suggestBox.querySelectorAll('.search-suggest-item');
            if (!opts.length) return;
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                activeIndex = (activeIndex + 1) % opts.length;
                highlightActive();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                activeIndex = (activeIndex - 1 + opts.length) % opts.length;
                highlightActive();
            } else if (e.key === 'Enter' && activeIndex >= 0) {
                e.preventDefault();
                opts[activeIndex].click();
            } else if (e.key === 'Escape') {
                hideSuggestions();
            }
        });

        document.addEventListener('click', function(e) {
            if (suggestBox && !wrap.contains(e.target)) hideSuggestions();
        });
        searchInput.closest('form').addEventListener('submit', hideSuggestions);
    }

    function initSearchShortcut() {
        document.addEventListener('keydown', function(e) {
            // "/" focus search — bỏ qua nếu đang gõ trong input/textarea
            var tag = (e.target.tagName || '').toLowerCase();
            var isTyping = tag === 'input' || tag === 'textarea' || tag === 'select' || e.target.isContentEditable;
            if (e.key === '/' && !isTyping) {
                e.preventDefault();
                var input = document.querySelector('.search-bar input[name="q"]');
                if (input) {
                    input.focus();
                    input.select();
                } else {
                    window.location.href = '/search';
                }
            }
        });
    }

    // ==========================================
    // 5. MEGA MENU — hamburger
    // ==========================================
    function initMenu() {
        var menuToggle = document.getElementById('menuToggle');
        var siteMenu = document.getElementById('site-menu');
        if (!menuToggle || !siteMenu) return;

        function closeMenu() {
            siteMenu.hidden = true;
            menuToggle.setAttribute('aria-expanded', 'false');
        }

        menuToggle.addEventListener('click', function(e) {
            e.stopPropagation();
            if (siteMenu.hidden) {
                siteMenu.hidden = false;
                menuToggle.setAttribute('aria-expanded', 'true');
            } else {
                closeMenu();
            }
        });

        document.addEventListener('click', function(e) {
            if (!siteMenu.hidden && !siteMenu.contains(e.target) && !menuToggle.contains(e.target)) {
                closeMenu();
            }
        });
        document.addEventListener('keydown', function(e) {
            if (e.key === 'Escape') closeMenu();
        });
        siteMenu.querySelectorAll('a').forEach(function(a) {
            a.addEventListener('click', closeMenu);
        });
        siteMenu.querySelectorAll('form').forEach(function(f) {
            f.addEventListener('submit', closeMenu);
        });
    }

    // ==========================================
    // 6. ANNOUNCEMENT BANNER
    // ==========================================
    function initAnnouncement() {
        var bannerSlot = document.getElementById('announcement-slot');
        if (!bannerSlot) return;
        fetch('/api/announcement').then(function(r) { return r.json(); }).then(function(d) {
            if (d && d.text) {
                var dismissed = sessionStorage.getItem('kg-ann-dismissed');
                if (dismissed === d.text) return;
                var div = document.createElement('div');
                div.className = 'container site-announcement';
                var banner = document.createElement('div');
                banner.className = 'announcement-banner ' + (d.kind || 'info');
                var icon = document.createElement('span');
                icon.setAttribute('aria-hidden', 'true');
                icon.textContent = '📢';
                var text = document.createElement('span');
                text.className = 'ann-text';
                text.textContent = d.text;
                var close = document.createElement('button');
                close.type = 'button';
                close.className = 'announcement-close';
                close.setAttribute('aria-label', 'Đóng thông báo');
                close.textContent = '×';
                close.addEventListener('click', function() {
                    sessionStorage.setItem('kg-ann-dismissed', d.text);
                    div.remove();
                });
                banner.appendChild(icon);
                banner.appendChild(text);
                banner.appendChild(close);
                div.appendChild(banner);
                bannerSlot.parentNode.insertBefore(div, bannerSlot.nextSibling);
            }
        }).catch(function() {});
    }

    // ==========================================
    // 7. DUPLICATE TITLE CHECK — game + news
    // ==========================================
    function initDuplicateCheck(input, endpoint, formatMsg) {
        if (!input) return;
        var timer = null;
        input.addEventListener('input', function() {
            clearTimeout(timer);
            var v = this.value.trim();
            var warn = document.getElementById('dup-warning');
            if (v.length < 3) { if (warn) warn.remove(); return; }
            timer = setTimeout(function() {
                fetch(endpoint + encodeURIComponent(v))
                    .then(function(r) { return r.json(); })
                    .then(function(d) {
                        var existing = document.getElementById('dup-warning');
                        if (existing) existing.remove();
                        var msg = formatMsg(d);
                        if (!msg) return;
                        var el = document.createElement('small');
                        el.id = 'dup-warning';
                        el.className = 'form-hint';
                        el.style.color = 'var(--warning)';
                        el.style.fontWeight = '550';
                        el.textContent = msg;
                        input.parentNode.appendChild(el);
                    }).catch(function() {});
            }, 500);
        });
    }

    // ==========================================
    // 8. UPLOAD — generic handler cho mọi .upload-zone
    // Markup: .upload-zone[data-upload-endpoint][data-max-size][data-url-target]
    // Tự tìm: input[type=file], img preview, .upload-status bên trong.
    // Thay thế 6 block JS copy-paste cũ (game/news new+edit, avatar, repo).
    // ==========================================
    function initUploads() {
        document.querySelectorAll('.upload-zone[data-upload-endpoint]').forEach(function(zone) {
            var endpoint = zone.getAttribute('data-upload-endpoint');
            var maxMB = parseInt(zone.getAttribute('data-max-size') || '10', 10);
            var fileInput = zone.querySelector('input[type="file"]');
            var urlField = zone.getAttribute('data-url-target') ?
                document.querySelector(zone.getAttribute('data-url-target')) : null;
            var preview = zone.querySelector('img');
            var status = zone.querySelector('.upload-status');
            if (!fileInput || !status) return;

            function setStatusText(text, cls) {
                status.textContent = text;
                status.className = 'upload-status ' + (cls || '');
            }

            // Nếu URL field có sẵn giá trị (edit form) → preview ngay
            if (urlField && urlField.value && preview) preview.src = urlField.value;

            fileInput.addEventListener('change', function() {
                var file = fileInput.files && fileInput.files[0];
                if (!file) return;
                if (file.size > maxMB * 1024 * 1024) {
                    setStatusText('Ảnh quá lớn (tối đa ' + maxMB + ' MB).', 'error');
                    return;
                }
                var allowed = ['image/jpeg', 'image/png', 'image/webp', 'image/gif'];
                if (allowed.indexOf(file.type) === -1) {
                    setStatusText('Định dạng không hỗ trợ. Dùng JPG/PNG/WebP/GIF.', 'error');
                    return;
                }
                setStatusText('Đang tải lên…', 'progress');
                var fd = new FormData();
                fd.append('file', file);
                fetch(endpoint, { method: 'POST', body: fd, credentials: 'same-origin' })
                    .then(function(r) { return r.json(); })
                    .then(function(data) {
                        if (data.error) {
                            setStatusText(data.error, 'error');
                            return;
                        }
                        if (urlField) urlField.value = data.url;
                        if (preview) preview.src = data.url;
                        setStatusText('Đã tải lên (' + Math.round(data.size / 1024) + ' KB)', 'success');
                    })
                    .catch(function(err) {
                        setStatusText('Lỗi mạng: ' + err.message, 'error');
                    });
            });
        });
    }

    // ==========================================
    // 9. SHARE — event delegation qua data-share-platform
    // ==========================================
    function copyShareLink(shareUrl) {
        if (navigator.clipboard && navigator.clipboard.writeText) {
            navigator.clipboard.writeText(shareUrl).then(function() {
                toast('Đã sao chép link vào clipboard!', 'success');
            }, function() {
                window.prompt('Copy link chia sẻ:', shareUrl);
            });
        } else {
            window.prompt('Copy link chia sẻ:', shareUrl);
        }
    }

    function initShare() {
        document.addEventListener('click', function(e) {
            var btn = e.target.closest('.share-buttons [data-share-platform]');
            if (!btn) return;
            var container = btn.closest('.share-buttons');
            if (!container) return;
            var shareUrl = container.dataset.shareUrl || window.location.href;
            var title = container.dataset.title || document.title;
            var platform = btn.dataset.sharePlatform;

            // v2.9.1 FIX — POST /games/{slug}/share từng là DEAD CODE: route +
            // handler + cột share_count tồn tại từ v0.x nhưng KHÔNG có chỗ
            // nào gọi → share_count mãi mãi = 0. Giờ fire-and-forget fetch
            // (không chờ response, không block clipboard/social share).
            // Endpoint cho phép khách (CurrentUser) → gọi luôn; 4xx im lặng bỏ qua.
            // `data-slug` có sẵn trên .share-buttons của trang game (trước đây
            // chỉ để cache-bust/ko dùng — giờ là khóa cho share analytics).
            var slug = container.dataset.slug;
            if (slug && platform) {
                try {
                    fetch('/games/' + encodeURIComponent(slug) + '/share', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
                        body: 'platform=' + encodeURIComponent(platform),
                        keepalive: true
                    }).catch(function() {});
                } catch (err) { /* ignore */ }
            }

            if (platform === 'copy') {
                e.preventDefault();
                copyShareLink(shareUrl);
            } else if (platform === 'native') {
                e.preventDefault();
                if (navigator.share) {
                    navigator.share({ url: shareUrl, title: title }).catch(function() {});
                } else {
                    copyShareLink(shareUrl);
                }
            }
            // facebook/twitter/telegram/whatsapp: mở tab mới qua href mặc định
        });

        // News share button (a11y + clipboard fallback)
        var newsShareBtn = document.getElementById('news-share-btn');
        if (newsShareBtn) {
            newsShareBtn.addEventListener('click', function() {
                var url = newsShareBtn.getAttribute('data-share-url');
                var title = newsShareBtn.getAttribute('data-share-title');
                if (navigator.share) {
                    navigator.share({ url: url, title: title }).catch(function() {});
                } else {
                    copyShareLink(url);
                }
            });
        }
    }

    // ==========================================
    // 10. FORMS UX — char counter, confirm, double-submit
    // ==========================================

    // Char counter: gắn vào mọi textarea[input] có maxlength + .char-counter sibling
    function initCharCounters() {
        // Counter theo cặp id cụ thể (news form)
        [['title', 'title-counter', 200],
         ['excerpt', 'excerpt-counter', 500],
         ['content', 'content-counter', 50000]].forEach(function(cfg) {
            var input = document.getElementById(cfg[0]);
            var counter = document.getElementById(cfg[1]);
            if (!input || !counter) return;
            function update() {
                var len = (input.value || '').length;
                counter.textContent = len + ' / ' + cfg[2];
                counter.classList.toggle('warn', len > cfg[2] * 0.8 && len <= cfg[2]);
                counter.classList.toggle('danger', len >= cfg[2]);
            }
            input.addEventListener('input', update);
            update();
        });

        // Comment form: counter <span>0</span>/N trong .comment-form-actions
        document.querySelectorAll('.comment-form textarea[maxlength]').forEach(function(ta) {
            var form = ta.closest('.comment-form');
            if (!form) return;
            var counterSpan = form.querySelector('.char-counter span');
            var max = parseInt(ta.getAttribute('maxlength'), 10);
            if (!counterSpan || !max) return;
            ta.addEventListener('input', function() {
                counterSpan.textContent = String(ta.value.length);
            });
        });
    }

    // data-confirm trên form — thay confirm() inline (SPA-friendly)
    function initConfirmForms() {
        document.addEventListener('submit', function(e) {
            var form = e.target.closest('form[data-confirm]');
            if (form && !form.dataset.confirmed) {
                e.preventDefault();
                if (window.confirm(form.getAttribute('data-confirm'))) {
                    form.dataset.confirmed = '1';
                    // Re-dispatch: requestSubmit giữ submitter + validation
                    if (form.requestSubmit) form.requestSubmit();
                    else form.submit();
                }
            }
        }, true);
    }

    // Chống double-submit form thường (không HTMX)
    function initDoubleSubmitGuard() {
        document.addEventListener('submit', function(e) {
            var form = e.target;
            if (form.hasAttribute('hx-post') || form.hasAttribute('hx-get')) return;
            if (form.dataset.submitted === '1') {
                e.preventDefault();
                return;
            }
            form.dataset.submitted = '1';
            setTimeout(function() { form.dataset.submitted = '0'; }, 4000);
        }, true);
    }

    // ==========================================
    // 11. MISC UX
    // ==========================================
    function initMisc() {
        // Theme toggle button
        var toggle = document.getElementById('themeToggle');
        if (toggle) toggle.addEventListener('click', toggleTheme);

        // Auto-resize textarea (form lớn)
        document.querySelectorAll('textarea.form-control-editor, textarea.editor').forEach(function(ta) {
            ta.addEventListener('input', function() {
                ta.style.height = 'auto';
                ta.style.height = Math.min(ta.scrollHeight, 600) + 'px';
            });
        });

        // Auto-mark notifications read khi click (keepalive sống sót navigation)
        document.querySelectorAll('.notification-item.unread .notification-link').forEach(function(link) {
            link.addEventListener('click', function() {
                var item = this.closest('.notification-item');
                if (item) {
                    var id = item.id.replace('notif-', '');
                    fetch('/notifications/' + id + '/read', { method: 'POST', keepalive: true });
                }
            });
        });

        // Smooth scroll cho anchor same-page
        document.querySelectorAll('a[href^="#"]').forEach(function(a) {
            a.addEventListener('click', function(e) {
                var id = this.getAttribute('href');
                if (id.length < 2) return;
                var target = document.querySelector(id);
                if (target) {
                    e.preventDefault();
                    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
                }
            });
        });

        // Image lazy-load fallback — ẩn ảnh bị hỏng, thay bằng fallback initials?
        // (ảnh hỏng để alt text hiển thị tự nhiên, tránh layout shift)
        document.querySelectorAll('img[loading="lazy"]').forEach(function(img) {
            img.addEventListener('error', function() {
                img.style.display = 'none';
            }, { once: true });
        });

        // Admin nav: highlight link active theo path hiện tại
        var path = window.location.pathname;
        document.querySelectorAll('.admin-nav-link').forEach(function(a) {
            var href = a.getAttribute('href') || '';
            if (href === path) {
                a.classList.add('active');
            } else if (href !== '/admin' && path.indexOf(href) === 0) {
                a.classList.add('active');
            }
        });
    }

    // News search autocomplete (ô tìm tin tức trong /news)
    function initNewsSearchAutocomplete() {
        var newsSearchInput = document.querySelector('.news-filters input[name="q"]');
        if (!newsSearchInput) return;
        var nsTimer = null;
        var nsBox = null;

        newsSearchInput.addEventListener('input', function() {
            var q = newsSearchInput.value.trim();
            clearTimeout(nsTimer);
            if (nsBox) { nsBox.remove(); nsBox = null; }
            if (q.length < 2) return;
            nsTimer = setTimeout(function() {
                fetch('/api/news-suggest?q=' + encodeURIComponent(q))
                    .then(function(r) { return r.json(); })
                    .then(function(data) {
                        if (nsBox) { nsBox.remove(); nsBox = null; }
                        if (!data[1] || data[1].length === 0) return;
                        nsBox = document.createElement('div');
                        nsBox.className = 'search-suggest news-search-suggest';
                        nsBox.setAttribute('role', 'listbox');
                        nsBox.setAttribute('aria-label', 'Gợi ý tìm tin tức');
                        for (var i = 0; i < data[1].length; i++) {
                            var a = document.createElement('a');
                            a.className = 'search-suggest-item';
                            a.href = data[3][i];
                            a.textContent = data[1][i];
                            a.setAttribute('role', 'option');
                            nsBox.appendChild(a);
                        }
                        newsSearchInput.parentNode.style.position = 'relative';
                        newsSearchInput.parentNode.appendChild(nsBox);
                    })
                    .catch(function() {});
            }, 250);
        });
        newsSearchInput.addEventListener('blur', function() {
            setTimeout(function() { if (nsBox) { nsBox.remove(); nsBox = null; } }, 200);
        });
    }

    // ==========================================
    // 12. v2.3.0 — CODE BLOCK COPY BUTTON + SERVICE WORKER
    // ==========================================
    // Copy-to-clipboard cho mọi .code-block-wrapper (được markdown engine
    // emit khi render code fence). Click button → textContent của <pre>
    // kề cận → navigator.clipboard.writeText.
    // Fallback cho browser không hỗ trợ Clipboard API (vd: không-HTTPS):
    // dùng execCommand + temporary <textarea>.
    function copyToClipboard(text) {
        if (navigator.clipboard && window.isSecureContext) {
            return navigator.clipboard.writeText(text).catch(function() {
                return legacyCopy(text);
            });
        }
        return Promise.resolve(legacyCopy(text));
    }

    function legacyCopy(text) {
        var ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.opacity = '0';
        ta.style.pointerEvents = 'none';
        document.body.appendChild(ta);
        ta.select();
        var ok = false;
        try { ok = document.execCommand('copy'); } catch (e) { ok = false; }
        document.body.removeChild(ta);
        return ok ? Promise.resolve() : Promise.reject(new Error('execCommand failed'));
    }

    function initCopyCodeButtons() {
        // Event delegation: 1 listener cho document → xử lý click bất kỳ
        // .code-copy-btn, kể cả cho button render sau HTMX swap.
        document.addEventListener('click', function(e) {
            var btn = e.target.closest && e.target.closest('.code-copy-btn');
            if (!btn) return;
            var wrapper = btn.closest('.code-block-wrapper');
            if (!wrapper) return;
            var pre = wrapper.querySelector('pre.code-block');
            if (!pre) return;
            // textContent strip HTML tags — syntect đã escape HTML nên
            // textContent trả về code gốc (không có HTML entities).
            var code = pre.textContent || '';
            copyToClipboard(code).then(
                function() {
                    btn.classList.add('code-copy-btn-copied');
                    var orig = btn.textContent;
                    btn.textContent = 'Đã chép';
                    setTimeout(function() {
                        btn.classList.remove('code-copy-btn-copied');
                        btn.textContent = orig || 'Sao chép';
                    }, 1500);
                },
                function() {
                    if (window.lsToast) {
                        window.lsToast('Không sao chép được — trình duyệt chặn clipboard', 'error');
                    }
                }
            );
        });
    }

    // Service Worker: cache-first cho /static/* (immutable), network-only
    // cho HTML + API + WebSocket. Lợi ích: visit sau → static assets
    // serve ngay từ cache (0 round-trip), FCP cực nhanh.
    // Chỉ đăng ký trên HTTPS (bảo mật). Skip trên dev localhost.
    function initServiceWorker() {
        if (!('serviceWorker' in navigator)) return;
        if (!window.isSecureContext) return;
        // Version query trong URL buộc browser download SW mới khi deploy
        // mới (URL khác → SW script khác → SW update). SW strategy:
        // skipWaiting → clients.claim để update apply ngay lập tức.
        // v3.5.1: lấy version từ chính URL của app.js (?v=CARGO_PKG_VERSION
        // do template render) — SW luôn đồng bộ version với app, không còn
        // hardcode "2.9.2" stale.
        var swVersion = '3.5.1';
        try {
            var scripts = document.querySelectorAll('script[src*="app.js?v="]');
            for (var i = 0; i < scripts.length; i++) {
                var m = (scripts[i].getAttribute('src') || '').match(/[?&]v=([0-9][0-9.]*)/);
                if (m) { swVersion = m[1]; break; }
            }
        } catch (e) { /* giữ default */ }
        window.addEventListener('load', function() {
            navigator.serviceWorker
                .register('/static/js/sw.js?v=' + swVersion, { scope: '/' })
                .then(function(reg) {
                    if (reg && typeof reg.update === 'function') {
                        // Trigger update check sau 60s nếu user keep tab mở
                        setTimeout(function() { reg.update && reg.update(); }, 60000);
                    }
                })
                .catch(function(err) {
                    // SW fail không break UI — chỉ log console
                    if (window.console && console.warn) {
                        console.warn('SW registration failed:', err);
                    }
                });
        });
    }

    // ==========================================
    // v2.9.0 — CONFETTI (điểm danh / huy hiệu)
    // ==========================================
    // window.lsConfetti(n) — n mảnh giấy rơi từ trên đầu viewport.
    // Dùng CSS animation (rẻ, không lib). Tự dọn sau 3.5s.
    window.lsConfetti = function(count) {
        try {
            if (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
            var n = count || 40;
            var colors = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6', '#a855f7', '#ec4899'];
            var frag = document.createDocumentFragment();
            for (var i = 0; i < n; i++) {
                var piece = document.createElement('div');
                piece.className = 'confetti-piece';
                piece.style.left = (Math.random() * 100) + 'vw';
                piece.style.background = colors[Math.floor(Math.random() * colors.length)];
                piece.style.animationDuration = (1.8 + Math.random() * 1.4) + 's';
                piece.style.animationDelay = (Math.random() * 0.4) + 's';
                if (Math.random() > 0.5) piece.style.borderRadius = '50%';
                frag.appendChild(piece);
            }
            document.body.appendChild(frag);
            setTimeout(function() {
                frag.querySelectorAll('.confetti-piece').forEach(function(el) { el.remove(); });
            }, 3600);
        } catch (e) { /* fail-safe */ }
    };

    // Confetti khi checkin partial có marker
    document.addEventListener('htmx:afterSwap', function(ev) {
        var marker = ev.detail && ev.detail.target && ev.detail.target.querySelector
            ? ev.detail.target.querySelector('[data-confetti-trigger]')
            : null;
        if (marker) {
            window.lsConfetti(45);
            marker.remove();
        }
    });

    // ==========================================
    // v2.9.0 — DRAFT AUTOSAVE (form game/news)
    // ==========================================
    // Form có data-draft-key="..." sẽ tự lưu nội dung vào localStorage
    // mỗi 5s (khi thay đổi) và gợi ý khôi phục khi quay lại sau refresh.
    // Submit thành công → tự xóa nháp.
    function initDraftAutosave() {
        var forms = document.querySelectorAll('form[data-draft-key]');
        if (!forms.length) return;
        forms.forEach(function(form) {
            var key = 'ls-draft-' + form.getAttribute('data-draft-key');
            var fields = form.querySelectorAll('input[type="text"], input[type="url"], input[type="date"], textarea, select');

            // Khôi phục nháp nếu có (chỉ khi form đang trống)
            try {
                var saved = localStorage.getItem(key);
                if (saved && fields.length) {
                    var data = JSON.parse(saved);
                    var isEmpty = !fields[0].value;
                    if (isEmpty && data && Object.keys(data).length) {
                        if (window.confirm('Có bản nháp chưa gửi từ lần trước. Khôi phục nội dung?')) {
                            fields.forEach(function(f) {
                                if (f.name && data[f.name] !== undefined) f.value = data[f.name];
                            });
                            window.lsToast('Đã khôi phục bản nháp', 'success');
                        } else {
                            localStorage.removeItem(key);
                        }
                    }
                }
            } catch (e) {}

            // Autosave throttle 5s
            var timer = null;
            var dirty = false;
            form.addEventListener('input', function() { dirty = true; });
            setInterval(function() {
                if (!dirty) return;
                dirty = false;
                try {
                    var data = {};
                    fields.forEach(function(f) {
                        if (f.name && f.value) data[f.name] = f.value;
                    });
                    localStorage.setItem(key, JSON.stringify(data));
                } catch (e) {}
            }, 5000);

            // Xóa nháp khi submit
            form.addEventListener('submit', function() {
                try { localStorage.removeItem(key); } catch (e) {}
            });
        });
    }

    // ==========================================
    // v2.9.0 — LỊCH SỬ TÌM KIẾM (localStorage, max 8)
    // ==========================================
    function getSearchHistory() {
        try {
            return JSON.parse(localStorage.getItem('ls-search-history') || '[]');
        } catch (e) { return []; }
    }
    function pushSearchHistory(q) {
        if (!q || q.length < 2) return;
        try {
            var list = getSearchHistory().filter(function(x) { return x !== q; });
            list.unshift(q);
            localStorage.setItem('ls-search-history', JSON.stringify(list.slice(0, 8)));
        } catch (e) {}
    }
    function initSearchHistory() {
        var form = document.querySelector('form.search-bar[action="/search"]');
        if (!form) return;
        var input = form.querySelector('input[name="q"]');
        if (!input) return;
        form.addEventListener('submit', function() {
            pushSearchHistory(input.value.trim());
        });
    }

    // ==========================================
    // v2.9.0 — HỘP THOẠI PHÍM TẮT (? để mở)
    // ==========================================
    function initKeyboardHelp() {
        document.addEventListener('keydown', function(e) {
            // Bỏ qua khi đang gõ trong input/textarea
            var tag = (e.target && e.target.tagName || '').toLowerCase();
            if (tag === 'input' || tag === 'textarea' || tag === 'select') return;
            if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
                e.preventDefault();
                toggleKbdHelp(true);
            } else if (e.key === 'Escape') {
                toggleKbdHelp(false);
            }
        });
    }
    function toggleKbdHelp(show) {
        var existing = document.getElementById('ls-kbd-help');
        if (!show) {
            if (existing) existing.remove();
            return;
        }
        if (existing) { existing.remove(); return; }
        var overlay = document.createElement('div');
        overlay.className = 'kbd-help-overlay';
        overlay.id = 'ls-kbd-help';
        overlay.innerHTML = '<div class="kbd-help-dialog" role="dialog" aria-label="Phím tắt">'
            + '<h3>Phím tắt</h3>'
            + '<div class="kbd-row"><span>Tìm kiếm</span><kbd class="kbd-key">/</kbd></div>'
            + '<div class="kbd-row"><span>Trợ giúp phím tắt</span><kbd class="kbd-key">?</kbd></div>'
            + '<div class="kbd-row"><span>Đóng hộp thoại này</span><kbd class="kbd-key">Esc</kbd></div>'
            + '<div class="kbd-row"><span>Trang chủ</span><kbd class="kbd-key">g</kbd> <kbd class="kbd-key">h</kbd></div>'
            + '<div class="kbd-row"><span>Bảng xếp hạng</span><kbd class="kbd-key">g</kbd> <kbd class="kbd-key">l</kbd></div>'
            + '<div class="kbd-row"><span>Game ngẫu nhiên</span><kbd class="kbd-key">g</kbd> <kbd class="kbd-key">r</kbd></div>'
            + '<p style="margin:14px 0 0;font-size:.8rem;color:var(--fg-muted)">Gõ 2 phím liên tiếp (g rồi h).</p>'
            + '</div>';
        overlay.addEventListener('click', function(ev) {
            if (ev.target === overlay) overlay.remove();
        });
        document.body.appendChild(overlay);
    }
    // Go-to sequences: g→h (home), g→l (leaderboard), g→r (random)
    function initGoToSequences() {
        var pendingG = false;
        var pendingTimer = null;
        document.addEventListener('keydown', function(e) {
            var tag = (e.target && e.target.tagName || '').toLowerCase();
            if (tag === 'input' || tag === 'textarea' || tag === 'select') return;
            if (e.ctrlKey || e.metaKey || e.altKey) return;
            if (e.key === 'g' && !pendingG) {
                pendingG = true;
                clearTimeout(pendingTimer);
                pendingTimer = setTimeout(function() { pendingG = false; }, 1200);
                return;
            }
            if (pendingG) {
                if (e.key === 'h') { window.location.href = '/'; }
                else if (e.key === 'l') { window.location.href = '/leaderboard'; }
                else if (e.key === 'r') { window.location.href = '/games/random'; }
                pendingG = false;
                clearTimeout(pendingTimer);
            }
        });
    }

    // ==========================================
    // v2.9.0 — NÚT CÀI PWA (beforeinstallprompt)
    // ==========================================
    var deferredInstallPrompt = null;
    function initPwaInstall() {
        window.addEventListener('beforeinstallprompt', function(e) {
            e.preventDefault();
            deferredInstallPrompt = e;
            // Hiện menu item "Cài đặt ứng dụng" nếu có slot
            var slot = document.getElementById('pwa-install-slot');
            if (slot) {
                var btn = document.createElement('button');
                btn.type = 'button';
                btn.className = 'menu-link';
                btn.innerHTML = '<span class="menu-icon"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg></span>Cài đặt ứng dụng';
                btn.addEventListener('click', function() {
                    if (!deferredInstallPrompt) return;
                    deferredInstallPrompt.prompt();
                    deferredInstallPrompt.userChoice.then(function() {
                        deferredInstallPrompt = null;
                        btn.remove();
                    });
                });
                slot.appendChild(btn);
            }
        });
    }

    // ==========================================
    // v2.9.0 — MARKDOWN PREVIEW (editor game/news)
    // ==========================================
    function initMarkdownPreview() {
        document.querySelectorAll('[data-preview-source]').forEach(function(toggleBtn) {
            toggleBtn.addEventListener('click', function() {
                var srcId = toggleBtn.getAttribute('data-preview-source');
                var src = document.getElementById(srcId);
                var target = document.getElementById(srcId + '-preview');
                if (!src || !target) return;
                if (target.hidden) {
                    // Render preview
                    var body = new URLSearchParams();
                    body.append('text', src.value.slice(0, 20000));
                    fetch('/api/preview', {
                        method: 'POST',
                        credentials: 'same-origin',
                        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
                        body: body.toString()
                    })
                        .then(function(r) { return r.ok ? r.json() : Promise.reject(); })
                        .then(function(d) {
                            target.innerHTML = d.html || '<p class="muted">(trống)</p>';
                            target.hidden = false;
                            src.style.display = 'none';
                            toggleBtn.textContent = '✏️ Sửa';
                        })
                        .catch(function() {
                            window.lsToast('Không tải được bản xem trước', 'error');
                        });
                } else {
                    target.hidden = true;
                    src.style.display = '';
                    toggleBtn.textContent = '👁 Xem trước';
                }
            });
        });
    }

    // ==========================================
    // BOOTSTRAP
    // ==========================================
    document.addEventListener('DOMContentLoaded', function() {
        initMenu();
        initSearchAutocomplete();
        initSearchShortcut();
        initAnnouncement();
        initUploads();
        initShare();
        initCharCounters();
        initConfirmForms();
        initDoubleSubmitGuard();
        initMisc();
        initNewsSearchAutocomplete();
        initCopyCodeButtons();
        initServiceWorker();
        // v2.9.0
        initDraftAutosave();
        initSearchHistory();
        initKeyboardHelp();
        initGoToSequences();
        initPwaInstall();
        initMarkdownPreview();

        // Duplicate check — game form (#f-title) + news form (#title)
        // v2.0 FIX: bản cũ chỉ check #title (id của form news) nên
        // game form không bao giờ được cảnh báo trùng.
        initDuplicateCheck(
            document.getElementById('f-title'),
            '/api/check-duplicate?title=',
            function(d) {
                return d.similar > 0 ? 'Có thể đã có ' + d.similar + ' game tên tương tự.' : null;
            }
        );
        initDuplicateCheck(
            document.querySelector('#news-form input[name="title"], #title'),
            '/api/news-check-duplicate?title=',
            function(d) {
                return d.exists ? 'Đã có ' + d.count + ' tin tương tự. Hãy đổi tiêu đề hoặc kiểm tra nội dung trùng.' : null;
            }
        );
    });

    // HTMX config sau khi htmx load (defer → chạy sau app.js)
    if (window.htmx) {
        initHtmx();
    } else {
        window.addEventListener('DOMContentLoaded', initHtmx);
    }
})();

/* ============================================================
   v3.0.0 — RETENTION enhancements
   ============================================================ */

/* XP Toast: partial HTMX trả về element có attribute data-xp-toast
   (vd data-xp-toast="+50 XP") → hiện toast nổi phía dưới màn hình.
   Giúp người chơi THẤY ngay phần thưởng — dopamine loop của retention. */
(function () {
  'use strict';
  function showXpToast(text) {
    var toast = document.createElement('div');
    toast.className = 'xp-toast';
    toast.textContent = text;
    document.body.appendChild(toast);
    // Self-remove sau 2.8s (khớp animation trong style.css)
    setTimeout(function () { toast.remove(); }, 2800);
  }
  document.body.addEventListener('htmx:afterSwap', function (evt) {
    var target = evt.detail && evt.detail.target;
    if (!target) return;
    // Tìm phần tử có data-xp-toast trong swapped content
    var el = target.querySelector
      ? target.querySelector('[data-xp-toast]')
      : null;
    if (!el && target.getAttribute && target.getAttribute('data-xp-toast')) {
      el = target;
    }
    if (el) {
      var text = el.getAttribute('data-xp-toast');
      if (text) showXpToast(text);
    }
  });
})();

/* v3.4.0 — GHI CHÚ: confirm qua data-confirm đã có handler chuẩn
   `initConfirmForms()` (app.js giữa file, dùng requestSubmit giữ HTML
   validation). Các form v3.4.0 (impersonate, thu hồi mật khẩu) dùng
   data-confirm thay vì onsubmit="confirm('{{ user_data }}')" — chống
   XSS inline-JS khi user data chứa quotes. KHÔNG đăng ký handler thứ 2
   ở đây (audit v2: 2 handler capture-phase = confirm dialog hiện 2 lần).
*/

/* v3.4.0 — Nút copy (data-copy-target): copy textContent của element
   chỉ định. Dùng textContent nên KHÔNG có vấn đề escape/inline-JS với
   ký tự đặc biệt trong mật khẩu ('"<>...). */
(function () {
  'use strict';
  document.addEventListener('click', function (e) {
    var btn = e.target.closest ? e.target.closest('[data-copy-target]') : null;
    if (!btn) return;
    var el = document.getElementById(btn.getAttribute('data-copy-target'));
    if (!el) return;
    var text = el.textContent || '';
    function done() {
      var old = btn.textContent;
      btn.textContent = '✓ Đã copy';
      setTimeout(function () { btn.textContent = old; }, 1600);
    }
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(done, done);
    } else {
      // Fallback textarea cho HTTP / browser cũ
      var ta = document.createElement('textarea');
      ta.value = text;
      ta.style.position = 'fixed';
      ta.style.opacity = '0';
      document.body.appendChild(ta);
      ta.select();
      try { document.execCommand('copy'); } catch (err) { /* ignore */ }
      ta.remove();
      done();
    }
  });
})();

/* v3.4.0 — Sinh mật khẩu ngẫu nhiên crypto-safe (crypto.getRandomValues
   thay Math.random — audit: Math.random không crypto-safe). Ký tự an toàn
   dễ đọc: không 0/O/1/l/I. */
(function () {
  'use strict';
  function genPassword(len) {
    var charset = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789';
    var out = '';
    // Lấy đúng len byte ngẫu nhiên (unbiased qua rejection sampling đơn giản:
    // lấy 256 giá trị vì 256 % 56 != 0 — lặp lại tới khi đủ)
    var buf = new Uint8Array(len * 2);
    crypto.getRandomValues(buf);
    for (var i = 0; i < buf.length && out.length < len; i++) {
      // 224 = 56 * 4 — vùng không bias (buf[i] < 224 an toàn)
      if (buf[i] < 224) out += charset[buf[i] % charset.length];
    }
    if (out.length < len) return genPassword(len); // cực hiếm — đệ quy bù
    return out;
  }
  document.addEventListener('click', function (e) {
    var btn = e.target.closest ? e.target.closest('[data-pwd-target]') : null;
    if (!btn) return;
    var input = document.getElementById(btn.getAttribute('data-pwd-target'));
    if (!input) return;
    input.value = genPassword(16);
    input.focus();
    input.select();
  });
})();

/* v3.6.2 — HERO FX "TĨNH MẶC ĐỊNH" cho hồ sơ AI Agent mặc định (GLM 5.3):
   CSS mới chỉ animate khi <html> mang class `fx-full`. Hàm này quyết định
   có bật animation hay không dựa trên sức máy THẬT TẾ:
     1) Trang có .ai-hero-fx (chỉ trang profile AI mặc định có);
     2) Không bật reduced-motion (tôn trọng setting OS);
     3) CPU >= 4 lõi, RAM >= 4GB (navigator.deviceMemory, nếu báo cáo);
     4) Probe FPS nhanh (~0.7s): nếu < 45fps → coi máy yếu, giữ TĨNH.
   Máy yếu → hero vẫn đẹp (gradient art tĩnh) nhưng KHÔNG tốn GPU → hết
   lag khi cuộn trang. Máy mạnh → full animation như cũ. */
(function () {
  'use strict';
  function initHeroFx() {
    if (!document.querySelector('.ai-hero-fx')) return;
    var reduce = false;
    try {
      reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    } catch (e) { /* ignore */ }
    if (reduce) return;
    var cores = navigator.hardwareConcurrency || 4;
    var mem = navigator.deviceMemory || 8; // Chrome báo GiB; Firefox/Safari không có → coi như đủ
    if (cores < 4 || mem < 4) return; // máy yếu → giữ tĩnh, không probe
    // Probe: đếm frame trong ~0.7s — nếu không đạt ~45fps thì bỏ qua
    var start = performance.now();
    var frames = 0;
    function tick(now) {
      frames++;
      var elapsed = now - start;
      if (elapsed < 700) { requestAnimationFrame(tick); return; }
      var fps = frames / (elapsed / 1000);
      if (fps >= 45) {
        document.documentElement.classList.add('fx-full');
      }
    }
    requestAnimationFrame(tick);
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initHeroFx);
  } else {
    initHeroFx();
  }
})();
