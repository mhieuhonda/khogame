// ============================================
// Kho Game - Frontend JavaScript
// ============================================

(function() {
    'use strict';

    // ===== Theme toggle =====
    function getStoredTheme() {
        return localStorage.getItem('kg-theme') || 'dark';
    }

    function applyTheme(theme) {
        document.documentElement.setAttribute('data-theme', theme);
        localStorage.setItem('kg-theme', theme);
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

    // Apply stored theme on load
    applyTheme(getStoredTheme());

    document.addEventListener('DOMContentLoaded', function() {
        const toggle = document.getElementById('themeToggle');
        if (toggle) {
            toggle.addEventListener('click', toggleTheme);
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
    document.addEventListener('input', function(e) {
        if (e.target.tagName === 'TEXTAREA' && e.target.maxLength > 0) {
            const counter = e.target.parentElement.querySelector('.char-counter span');
            if (counter) {
                counter.textContent = e.target.value.length;
            }
        }
    });

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
            // Server rate-limit (10 bình luận/phút, 20 tải/phút...)
            // — thông báo rõ thay vì lỗi chung chung.
            showToast('Bạn thao tác quá nhanh. Vui lòng chờ khoảng 1 phút rồi thử lại.', 'error');
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

    // ===== Smooth scroll for anchor links =====
    document.addEventListener('click', function(e) {
        const link = e.target.closest('a[href^="#"]');
        if (link) {
            const target = document.querySelector(link.getAttribute('href'));
            if (target) {
                e.preventDefault();
                target.scrollIntoView({ behavior: 'smooth', block: 'start' });
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

    // ===== Infinite scroll for game grids =====
    const observer = new IntersectionObserver(function(entries) {
        entries.forEach(function(entry) {
            if (entry.isIntersecting) {
                const trigger = entry.target;
                const url = trigger.getAttribute('data-load-more');
                if (url && !trigger.dataset.loading) {
                    trigger.dataset.loading = 'true';
                    fetch(url).then(r => r.text()).then(function(html) {
                        const grid = trigger.previousElementSibling;
                        if (grid) {
                            grid.insertAdjacentHTML('beforeend', html);
                        }
                        trigger.remove();
                    }).catch(function() {
                        delete trigger.dataset.loading;
                    });
                }
            }
        });
    }, { rootMargin: '200px' });

    document.addEventListener('DOMContentLoaded', function() {
        document.querySelectorAll('[data-load-more]').forEach(function(el) {
            observer.observe(el);
        });
    });

    // ===== Re-observe new load-more triggers after HTMX swaps =====
    document.addEventListener('htmx:afterSwap', function() {
        document.querySelectorAll('[data-load-more]').forEach(function(el) {
            observer.observe(el);
        });
    });

    console.log('🎮 Kho Game loaded successfully!');
})();
