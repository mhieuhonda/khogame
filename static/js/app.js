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

    function initHtmx() {
        if (!window.htmx) return;

        // Progress bar mỏng dưới header khi có request đang chạy
        document.body.addEventListener('htmx:beforeRequest', function() {
            if (!progressBar) {
                progressBar = document.getElementById('htmx-progress');
            }
            if (progressBar) {
                progressBar.classList.remove('done');
                progressBar.classList.add('active');
            }
        });

        function finishProgress() {
            if (!progressBar) progressBar = document.getElementById('htmx-progress');
            if (progressBar) {
                progressBar.classList.add('done');
                setTimeout(function() {
                    progressBar.classList.remove('active', 'done');
                }, 250);
            }
        }

        document.body.addEventListener('htmx:afterRequest', finishProgress);

        // HTMX error → toast thân thiện (không only console)
        document.body.addEventListener('htmx:responseError', function(evt) {
            var xhr = evt.detail.xhr;
            var status = xhr ? xhr.status : 0;
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

        // Swap thành công chứa .error-partial → toast lỗi
        document.body.addEventListener('htmx:afterSwap', function(evt) {
            var errBox = evt.detail && evt.detail.target ?
                evt.detail.target.querySelector : null;
            if (typeof errBox === 'function') {
                var err = evt.detail.target.querySelector('.error-partial .error-message');
                if (err) toast(err.textContent, 'error', 4200);
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
        window.addEventListener('load', function() {
            navigator.serviceWorker
                .register('/static/js/sw.js?v=2.5.0', { scope: '/' })
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
