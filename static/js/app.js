// ============================================
// Kho Game - Frontend JavaScript
// ============================================

(function() {
    'use strict';

    // ===== Theme toggle =====
    function getStoredTheme() {
        return localStorage.getItem('kg-theme');
    }

    function applyTheme(theme) {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('kg-theme', theme);
    }

    // Đồng bộ theme giữa các tab đang mở của cùng site: user đổi theme
    // ở tab A → tab B (đang mở trang game) cũng đổi theo ngay lập tức
    // thay vì đến khi reload mới thấy chớp nhác theme cũ.
    window.addEventListener('storage', function(e) {
        if (e.key === 'kg-theme' && (e.newValue === 'dark' || e.newValue === 'light')) {
            document.documentElement.setAttribute('data-theme', e.newValue);
        }
    });

    // Lần đầu (chưa chọn theme): theo hệ điều hành của người dùng.
    // Sau khi user chủ động bấm toggle thì luôn tôn trọng lựa chọn đó.
    function initialTheme() {
        var stored = getStoredTheme();
        if (stored === 'dark' || stored === 'light') return stored;
        if (window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches) {
            return 'light';
        }
        return 'dark';
    }

    function toggleTheme() {
        const current = document.documentElement.getAttribute('data-theme') || 'dark';
        const next = current === 'dark' ? 'light' : 'dark';
        applyTheme(next);
        // Đồng bộ theme lên server (nếu đã đăng nhập)
        fetch('/api/preferences/theme', {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: 'theme=' + encodeURIComponent(next)
        }).catch(function() {});
    }

    // Apply stored/system theme on load
    applyTheme(initialTheme());

    document.addEventListener('DOMContentLoaded', function() {
        const toggle = document.getElementById('themeToggle');
        if (toggle) {
            toggle.addEventListener('click', toggleTheme);
        }

        // ===== Search autocomplete (gợi ý game khi gõ) =====
        const searchInput = document.querySelector('.search-bar input[name="q"]');
        if (searchInput) {
            let suggestTimer = null;
            let suggestBox = null;
            let activeIndex = -1; // index option đang highlight (keyboard nav)
            const wrap = searchInput.closest('.search-bar');

            function hideSuggestions() {
                if (suggestBox) { suggestBox.remove(); suggestBox = null; }
                activeIndex = -1;
                searchInput.removeAttribute('aria-activedescendant');
            }

            function highlightActive() {
                if (!suggestBox) return;
                const opts = suggestBox.querySelectorAll('.search-suggest-item');
                opts.forEach(function(a, i) {
                    const active = i === activeIndex;
                    a.classList.toggle('active', active);
                    if (active) {
                        a.id = 'suggest-opt-' + i;
                        searchInput.setAttribute('aria-activedescendant', 'suggest-opt-' + i);
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
                    const a = document.createElement('a');
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
                const v = this.value.trim();
                if (v.length < 2) { hideSuggestions(); return; }
                suggestTimer = setTimeout(function() {
                    fetch('/api/suggest?q=' + encodeURIComponent(v))
                        .then(function(r) { return r.json(); })
                        .then(function(d) { showSuggestions(d.data || []); })
                        .catch(function() {});
                }, 250);
            });

            // Keyboard navigation: ↑ ↓ di chuyển, Enter chọn, Esc đóng —
            // ARIA listbox KHÔNG có arrow-key nav là lỗi WCAG 2.1.1
            // (bàn phím không thể dùng được widget).
            searchInput.addEventListener('keydown', function(e) {
                if (!suggestBox) return;
                const opts = suggestBox.querySelectorAll('.search-suggest-item');
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

            // Đóng khi click ra ngoài / Escape
            document.addEventListener('click', function(e) {
                if (suggestBox && !wrap.contains(e.target)) hideSuggestions();
            });
            // Ẩn gợi ý khi submit form để không che kết quả
            searchInput.closest('form').addEventListener('submit', hideSuggestions);
        }

        // ===== Hamburger menu =====
        const menuToggle = document.getElementById('menuToggle');
        const siteMenu = document.getElementById('site-menu');
        function closeMenu() {
            if (!siteMenu || !menuToggle) return;
            siteMenu.hidden = true;
            menuToggle.classList.remove('open');
            menuToggle.setAttribute('aria-expanded', 'false');
        }
        if (menuToggle && siteMenu) {
            menuToggle.addEventListener('click', function(e) {
                e.stopPropagation();
                if (siteMenu.hidden) {
                    siteMenu.hidden = false;
                    menuToggle.classList.add('open');
                    menuToggle.setAttribute('aria-expanded', 'true');
                } else {
                    closeMenu();
                }
            });
            // Đóng khi bấm ra ngoài
            document.addEventListener('click', function(e) {
                if (!siteMenu.hidden && !siteMenu.contains(e.target) && !menuToggle.contains(e.target)) {
                    closeMenu();
                }
            });
            // Đóng bằng Escape
            document.addEventListener('keydown', function(e) {
                if (e.key === 'Escape') closeMenu();
            });
            // Đóng sau khi chọn link (trước khi điều hướng)
            siteMenu.querySelectorAll('a').forEach(function(a) {
                a.addEventListener('click', closeMenu);
            });
            // Đóng menu sau submit form trong menu (vd đăng xuất)
            siteMenu.querySelectorAll('form').forEach(function(f) {
                f.addEventListener('submit', closeMenu);
            });
        }

        // ===== Announcement banner toàn site =====
        const bannerSlot = document.getElementById('announcement-slot');
        if (bannerSlot) {
            fetch('/api/announcement').then(function(r) { return r.json(); }).then(function(d) {
                if (d && d.text) {
                    const div = document.createElement('div');
                    div.className = 'container site-announcement';
                    const dismissed = sessionStorage.getItem('kg-ann-dismissed');
                    if (dismissed === d.text) return;
                    div.innerHTML = '<div class="announcement-banner ' + (d.kind || 'info') + '">' +
                        '<span>📢</span><span class="ann-text"></span>' +
                        '<button class="announcement-close" aria-label="Đóng">×</button></div>';
                    div.querySelector('.ann-text').textContent = d.text;
                    div.querySelector('.announcement-close').addEventListener('click', function() {
                        sessionStorage.setItem('kg-ann-dismissed', d.text);
                        div.remove();
                    });
                    bannerSlot.parentNode.insertBefore(div, bannerSlot.nextSibling);
                }
            }).catch(function() {});
        }

        // ===== Cảnh báo trùng tiêu đề khi đăng game =====
        const titleInput = document.getElementById('title');
        if (titleInput) {
            let debounce = null;
            titleInput.addEventListener('input', function() {
                clearTimeout(debounce);
                const v = this.value.trim();
                const warn = document.getElementById('dup-warning');
                if (v.length < 3) { if (warn) warn.remove(); return; }
                debounce = setTimeout(function() {
                    fetch('/api/check-duplicate?title=' + encodeURIComponent(v))
                        .then(function(r) { return r.json(); })
                        .then(function(d) {
                            const existing = document.getElementById('dup-warning');
                            if (existing) existing.remove();
                            if (d.similar > 0) {
                                const el = document.createElement('small');
                                el.id = 'dup-warning';
                                el.style.color = '#fcd34d';
                                el.textContent = '⚠️ Có thể đã có ' + d.similar + ' game tên tương tự.';
                                titleInput.parentNode.appendChild(el);
                            }
                        }).catch(function() {});
                }, 500);
            });
        }

        // Auto-mark notifications as read when viewed
        const notifItems = document.querySelectorAll('.notification-item.unread .notification-link');
        notifItems.forEach(function(link) {
            link.addEventListener('click', function() {
                const item = this.closest('.notification-item');
                if (item) {
                    const id = item.id.replace('notif-', '');
                    fetch('/notifications/' + id + '/read', { method: 'POST' });
                }
            });
        });
    });

    // ===== Share functions =====
    // Sử dụng event delegation: dữ liệu lấy từ data-* attributes trên .share-buttons,
    // tránh XSS từ title có dấu nháy đơn khi truyền vào onclick='...'.
    document.addEventListener('click', function(e) {
        const btn = e.target.closest('.share-buttons [data-share-platform]');
        if (!btn) return;
        const container = btn.closest('.share-buttons');
        if (!container) return;
        const slug = container.dataset.slug || '';
        const title = container.dataset.title || '';
        const shareUrl = container.dataset.shareUrl || (window.location.origin + '/games/' + slug);
        const platform = btn.dataset.sharePlatform;
        if (platform === 'copy') {
            copyShareLink(slug, shareUrl);
        } else if (platform === 'native') {
            nativeShare(slug, title, shareUrl);
        } else {
            recordShare(slug, platform);
        }
    });

    window.recordShare = function(slug, platform) {
        fetch('/games/' + encodeURIComponent(slug) + '/share', {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: 'platform=' + encodeURIComponent(platform)
        }).catch(function() {});
    };

    window.copyShareLink = function(slug, optUrl) {
        const url = optUrl || (window.location.origin + '/games/' + slug);
        if (navigator.clipboard) {
            navigator.clipboard.writeText(url).then(function() {
                showToast('Đã sao chép link!', 'success');
            });
        } else {
            const textarea = document.createElement('textarea');
            textarea.value = url;
            document.body.appendChild(textarea);
            textarea.select();
            try {
                document.execCommand('copy');
                showToast('Đã sao chép link!', 'success');
            } catch (e) {
                showToast('Không thể sao chép link', 'error');
            }
            document.body.removeChild(textarea);
        }
        recordShare(slug, 'copy');
    };

    window.nativeShare = function(slug, title, optUrl) {
        const url = optUrl || (window.location.origin + '/games/' + slug);
        if (navigator.share) {
            navigator.share({ title: title, url: url }).catch(function() {});
            recordShare(slug, 'native');
        } else {
            copyShareLink(slug, url);
        }
    };

    // ===== Toast notifications =====
    window.showToast = function(message, type) {
        type = type || 'info';
        const toast = document.createElement('div');
        toast.className = 'toast toast-' + type;
        toast.textContent = message;
        toast.style.cssText = 'position:fixed;bottom:24px;right:24px;padding:12px 20px;background:var(--bg-card);border:1px solid var(--border);border-radius:8px;color:var(--text-primary);box-shadow:0 4px 12px rgba(0,0,0,0.3);z-index:2000;animation:slideUp 0.3s ease;max-width:320px;';
        if (type === 'success') toast.style.borderColor = '#10b981';
        if (type === 'error') toast.style.borderColor = '#ef4444';
        document.body.appendChild(toast);
        setTimeout(function() {
            toast.style.opacity = '0';
            toast.style.transition = 'opacity 0.3s';
            setTimeout(function() { toast.remove(); }, 300);
        }, 3000);
    };

    // ===== Char counter for textareas =====
    // Dùng Array.from (code points Unicode) thay vì .length (UTF-16 code
    // units) để khớp cách đếm .chars().count() phía Rust server — emoji
    // 😀 là 1 ký tự Rust nhưng 2 units trong JS, đếm lệch làm user tưởng
    // còn chỗ nhưng submit bị chặn.
    document.addEventListener('input', function(e) {
        if (e.target.tagName === 'TEXTAREA' && e.target.maxLength > 0) {
            const counter = e.target.parentElement.querySelector('.char-counter span');
            if (counter) {
                counter.textContent = Array.from(e.target.value).length;
            }
        }
    });

    // Form reset (sau HTMX submit thành công) không fire event 'input' —
    // counter sẽ giữ số cũ hiển thị sai. Đồng bộ thủ công khi reset.
    document.addEventListener('reset', function(e) {
        const ta = e.target.querySelector('textarea[maxlength]');
        if (ta) {
            const counter = ta.parentElement.querySelector('.char-counter span');
            if (counter) {
                // reset() chạy sau event — đợi microtask kế tiếp
                setTimeout(function() { counter.textContent = Array.from(ta.value).length; }, 0);
            }
        }
    }, true);

    // ===== HTMX events =====
    document.addEventListener('htmx:afterRequest', function(evt) {
        if (evt.detail.failed) {
            console.error('HTMX request failed', evt.detail);
            if (evt.detail.xhr.status === 401) {
                window.location.href = '/login';
            }
        }
    });

    document.addEventListener('htmx:responseError', function(evt) {
        console.error('HTMX response error', evt.detail);
        const status = evt.detail.xhr.status;
        if (status === 429) {
            // Server rate-limit (10 bình luận/phút, 20 tải/phút...) —
            // đọc Retry-After header (giây) để đếm ngược chính xác thay
            // vì 'khoảng 1 phút' đoán mò.
            const retryAfter = parseInt(evt.detail.xhr.getResponseHeader('Retry-After'), 10);
            if (retryAfter > 0 && isFinite(retryAfter)) {
                showToast('Bạn thao tác quá nhanh. Thử lại sau ' + retryAfter + ' giây.', 'error');
            } else {
                showToast('Bạn thao tác quá nhanh. Vui lòng chờ khoảng 1 phút rồi thử lại.', 'error');
            }
        } else if (status === 503) {
            showToast('Hệ thống đang bảo trì. Vui lòng thử lại sau ít phút.', 'error');
        } else {
            showToast('Có lỗi xảy ra. Vui lòng thử lại.', 'error');
        }
    });

    // ===== Image lazy loading fallback =====
    document.addEventListener('DOMContentLoaded', function() {
        const images = document.querySelectorAll('img[loading="lazy"]');
        if (!('loading' in HTMLImageElement.prototype)) {
            images.forEach(function(img) {
                img.src = img.dataset.src || img.src;
            });
        }
    });

    // ===== Confirm dialog for delete actions =====
    document.addEventListener('submit', function(e) {
        const form = e.target;
        if (form.method.toLowerCase() === 'post' && form.action.includes('/delete')) {
            if (!confirm('Bạn có chắc chắn muốn thực hiện hành động này?')) {
                e.preventDefault();
            }
        }
    });

    // ===== Chống double-submit form thường (không HTMX) =====
    // 2 click nhanh nút submit (hoặc Enter + click) gửi 2 request:
    // tạo game trùng, bình luận đôi, follow/unfollow lệch trạng thái.
    // Disable nút ngay khi submit bắt đầu — form vẫn submit vì nút
    // đã nằm trong event pipeline (disable trong submit handler không
    // chặn submit). HTMX form đã có hx-disabled-elt riêng.
    document.addEventListener('submit', function(e) {
        const form = e.target;
        if (form.hasAttribute('hx-post') || form.hasAttribute('hx-get')) return;
        // Chỉ disable khi form hợp lệ (submit thật sự diễn ra)
        setTimeout(function() {
            const btn = form.querySelector('button[type="submit"]:not([disabled])');
            if (btn && form.checkValidity()) {
                btn.setAttribute('disabled', '');
                btn.style.opacity = '0.6';
                // Khôi phục sau 10s phòng server lỗi (user cần submit lại)
                setTimeout(function() {
                    btn.removeAttribute('disabled');
                    btn.style.opacity = '';
                }, 10000);
            }
        }, 0);
    }, true);

    // ===== Smooth scroll for anchor links =====
    document.addEventListener('click', function(e) {
        const link = e.target.closest('a[href^="#"]');
        if (link) {
            const href = link.getAttribute('href');
            // href="#" hoặc "#!" (placeholder link) → không scroll, chỉ chặn
            // nhảy lên đầu trang. querySelector('#') sẽ throw DOMException.
            if (href === '#' || href.length < 2) {
                e.preventDefault();
                return;
            }
            try {
                const target = document.querySelector(href);
                if (target) {
                    e.preventDefault();
                    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
                }
            } catch (err) {
                // selector không hợp lệ (vd href="#a b") — bỏ qua, để browser
                // xử lý default thay vì crash listener này cho các click sau.
                console.warn('Bỏ qua anchor không hợp lệ:', href);
            }
        }
    });

    // ===== Search shortcut (press / to focus search) =====
    document.addEventListener('keydown', function(e) {
        if (e.key === '/' && document.activeElement.tagName !== 'INPUT' && document.activeElement.tagName !== 'TEXTAREA') {
            e.preventDefault();
            const search = document.querySelector('.search-bar input');
            if (search) search.focus();
        }
    });

    // ===== Auto-resize textareas =====
    document.addEventListener('input', function(e) {
        if (e.target.tagName === 'TEXTAREA' && e.target.classList.contains('auto-resize')) {
            e.target.style.height = 'auto';
            e.target.style.height = e.target.scrollHeight + 'px';
        }
    });

    // Không console.log ở prod: noise trong devtools người dùng cuối,
    // một số công ty quét console.log khi audit vendor JS.
})();
