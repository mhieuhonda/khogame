// ============================================
// Louis Space - Live Chat frontend
// ============================================
// WebSocket realtime chat. Falls back gracefully:
//   - User not logged in → input disabled, "Đăng nhập" button shown
//   - WS connect fail → retry with exponential backoff (max 30s)
//   - WS disconnect → show "Đang kết nối lại…" + auto reconnect
//   - History fetch fail → show error message + retry button
//
// Security notes:
//   - All user content rendered via textContent (no innerHTML for msg body)
//     → XSS-safe even if attacker sends <script> tags
//   - URL/email/etc in message body NOT auto-linked (avoid phishing)
//   - Admin delete: hidden from UI, server keeps record
// ============================================

(function() {
    'use strict';

    var MAX_MSG = 200;            // Keep at most N messages in DOM
    var SCROLL_THRESHOLD = 80;    // px from bottom — auto-scroll if user near bottom

    var ws = null;
    var wsRetryDelay = 1000;       // ms, doubled each failure (cap 30s)
    var historyLoaded = false;
    var currentUser = null;
    var isNearBottom = true;

    function $(id) { return document.getElementById(id); }
    function el(tag, cls) {
        var e = document.createElement(tag);
        if (cls) e.className = cls;
        return e;
    }

    function setStatus(text, isErr) {
        var s = $('chat-conn-status');
        if (!s) return;
        s.textContent = text;
        s.classList.toggle('chat-error', !!isErr);
    }

    function escapeHtml(s) {
        var div = document.createElement('div');
        div.textContent = s == null ? '' : String(s);
        return div.innerHTML;
    }

    function timeAgo(dateStr) {
        try {
            var d = new Date(dateStr);
            var now = Date.now();
            var diff = Math.max(0, now - d.getTime());
            var sec = Math.floor(diff / 1000);
            if (sec < 30) return 'vừa xong';
            if (sec < 60) return sec + ' giây trước';
            var min = Math.floor(sec / 60);
            if (min < 60) return min + ' phút trước';
            var hr = Math.floor(min / 60);
            if (hr < 24) return hr + ' giờ trước';
            var day = Math.floor(hr / 24);
            if (day < 7) return day + ' ngày trước';
            return d.toLocaleDateString('vi-VN');
        } catch (e) {
            return '';
        }
    }

    function avatarHtml(msg) {
        if (msg.avatar_url) {
            return '<img src="' + escapeHtml(msg.avatar_url) + '" alt="" class="avatar avatar-sm chat-avatar" loading="lazy" decoding="async">';
        }
        var initials = '?';
        var name = msg.display_name || msg.username || '';
        if (name) {
            var parts = name.trim().split(/\s+/);
            if (parts.length >= 2) {
                initials = (parts[0][0] || '') + (parts[parts.length - 1][0] || '');
            } else if (parts.length === 1 && parts[0].length > 0) {
                initials = parts[0][0];
            }
            initials = initials.toUpperCase();
        }
        return '<div class="avatar avatar-sm avatar-fallback chat-avatar">' + escapeHtml(initials) + '</div>';
    }

    function renderMessage(msg) {
        var node = el('div', 'chat-msg');
        node.setAttribute('data-id', msg.id);
        if (msg.user_id && currentUser && msg.user_id === currentUser.id) {
            node.classList.add('chat-msg-self');
        }
        var isStaff = msg.role === 'Admin' || msg.role === 'Moderator';

        var header = el('div', 'chat-msg-header');
        header.innerHTML = avatarHtml(msg) +
            '<span class="chat-msg-author' + (isStaff ? ' chat-msg-author-staff' : '') + '">' +
            escapeHtml(msg.display_name || msg.username) +
            (isStaff ? '<span class="chat-msg-badge">' + escapeHtml(msg.role === 'Admin' ? 'Admin' : 'Mod') + '</span>' : '') +
            '</span>' +
            '<span class="chat-msg-time" title="' + escapeHtml(msg.created_at) + '">' +
            escapeHtml(timeAgo(msg.created_at)) + '</span>';
        node.appendChild(header);

        var body = el('div', 'chat-msg-body');
        body.textContent = msg.content;  // textContent = XSS-safe
        node.appendChild(body);

        return node;
    }

    function renderDeletedMessage(msg) {
        var node = el('div', 'chat-msg chat-msg-deleted');
        node.setAttribute('data-id', msg.id);
        var body = el('div', 'chat-msg-body');
        body.textContent = '⊘ Tin nhắn đã bị ẩn bởi quản trị viên';
        body.classList.add('chat-msg-deleted-text');
        node.appendChild(body);
        return node;
    }

    function markMessageDeleted(id) {
        var node = document.querySelector('.chat-msg[data-id="' + id + '"]');
        if (!node) return;
        node.classList.add('chat-msg-deleted');
        var body = node.querySelector('.chat-msg-body');
        if (body) {
            body.textContent = '⊘ Tin nhắn đã bị ẩn bởi quản trị viên';
            body.classList.add('chat-msg-deleted-text');
        }
        // Hide avatar/header for cleanliness
        var header = node.querySelector('.chat-msg-header');
        if (header) header.style.display = 'none';
    }

    function shouldAutoScroll() {
        var box = $('chat-messages');
        if (!box) return false;
        var distFromBottom = box.scrollHeight - box.scrollTop - box.clientHeight;
        return distFromBottom < SCROLL_THRESHOLD;
    }

    function scrollToBottom(smooth) {
        var box = $('chat-messages');
        if (!box) return;
        box.scrollTo({
            top: box.scrollHeight,
            behavior: smooth ? 'smooth' : 'auto'
        });
    }

    function appendMessage(msg) {
        var box = $('chat-messages');
        if (!box) return;
        // Remove loading placeholder if present
        var loading = box.querySelector('.chat-loading');
        if (loading) loading.remove();
        // Remove "no messages" placeholder
        var empty = box.querySelector('.chat-empty');
        if (empty) empty.remove();

        var node = msg.is_deleted ? renderDeletedMessage(msg) : renderMessage(msg);
        box.appendChild(node);

        // Trim DOM — keep last MAX_MSG nodes
        while (box.children.length > MAX_MSG) {
            box.removeChild(box.firstChild);
        }

        if (isNearBottom) {
            scrollToBottom(false);
        }
    }

    function prependMessage(msg) {
        var box = $('chat-messages');
        if (!box) return;
        var loading = box.querySelector('.chat-loading');
        if (loading) loading.remove();
        var empty = box.querySelector('.chat-empty');
        if (empty) empty.remove();

        var node = msg.is_deleted ? renderDeletedMessage(msg) : renderMessage(msg);
        box.insertBefore(node, box.firstChild);

        while (box.children.length > MAX_MSG) {
            box.removeChild(box.lastChild);
        }
    }

    function renderEmptyState() {
        var box = $('chat-messages');
        if (!box) return;
        box.innerHTML = '';
        var empty = el('div', 'chat-empty');
        empty.innerHTML = '<div class="chat-empty-icon">💬</div>' +
            '<p>Chưa có tin nhắn nào. Hãy là người đầu tiên!</p>';
        box.appendChild(empty);
    }

    function loadHistory() {
        fetch('/chat/history', { headers: { 'Accept': 'application/json' } })
            .then(function(r) {
                if (!r.ok) throw new Error('HTTP ' + r.status);
                return r.json();
            })
            .then(function(data) {
                historyLoaded = true;
                if (data.online != null) {
                    var count = $('chat-online-count');
                    if (count) count.textContent = data.online;
                }
                if (data.today_count != null) {
                    var today = $('chat-today-count');
                    if (today) today.textContent = '💬 ' + data.today_count + ' tin hôm nay';
                }
                var box = $('chat-messages');
                if (!box) return;
                box.innerHTML = '';
                if (!data.messages || data.messages.length === 0) {
                    renderEmptyState();
                    return;
                }
                // data.messages is old→new (server reversed). Render sequentially.
                data.messages.forEach(function(msg) {
                    var node = msg.is_deleted ? renderDeletedMessage(msg) : renderMessage(msg);
                    box.appendChild(node);
                });
                isNearBottom = true;
                scrollToBottom(false);
            })
            .catch(function(err) {
                console.error('loadHistory error:', err);
                var box = $('chat-messages');
                if (box) {
                    box.innerHTML = '';
                    var errNode = el('div', 'chat-empty chat-empty-error');
                    errNode.innerHTML = '<div class="chat-empty-icon">⚠️</div>' +
                        '<p>Không tải được tin nhắn.</p>' +
                        '<button class="btn btn-outline btn-sm" id="chat-retry">Thử lại</button>';
                    box.appendChild(errNode);
                    var btn = $('chat-retry');
                    if (btn) btn.addEventListener('click', loadHistory);
                }
            });
    }

    function connectWs() {
        if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
            return;
        }
        var proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        var url = proto + '//' + window.location.host + '/chat/ws';
        try {
            ws = new WebSocket(url);
        } catch (e) {
            setStatus('Trình duyệt không hỗ trợ WebSocket. Tải lại trang.', true);
            return;
        }
        ws.onopen = function() {
            wsRetryDelay = 1000;  // reset backoff
            setStatus('Đã kết nối • realtime');
        };
        ws.onmessage = function(ev) {
            var event;
            try {
                event = JSON.parse(ev.data);
            } catch (e) {
                return;
            }
            if (event.type === 'message') {
                appendMessage(event.message);
            } else if (event.type === 'delete') {
                markMessageDeleted(event.id);
            } else if (event.type === 'presence') {
                var count = $('chat-online-count');
                if (count) count.textContent = event.online;
            }
        };
        ws.onclose = function(ev) {
            ws = null;
            if (ev.code === 1008 || ev.code === 4001) {
                // 1008: policy violation (auth failure). 4001: our custom auth-fail code.
                setStatus('Cần đăng nhập để chat', true);
                return;
            }
            setStatus('Mất kết nối — đang thử lại…', true);
            setTimeout(function() {
                if (!ws) connectWs();
            }, wsRetryDelay);
            // Exponential backoff, cap 30s
            wsRetryDelay = Math.min(wsRetryDelay * 2, 30000);
        };
        ws.onerror = function() {
            // onclose sẽ follow
        };
    }

    function sendMessage(text) {
        if (!ws || ws.readyState !== WebSocket.OPEN) {
            setStatus('Chưa kết nối tới máy chủ. Thử lại sau 2 giây.', true);
            return false;
        }
        if (!text || !text.trim()) return false;
        var truncated = text.length > 500 ? text.slice(0, 500) : text;
        // Plain text frame — backend treats non-JSON as chat message
        ws.send(truncated);
        return true;
    }

    function init() {
        // Detect current user from page (avatar-link if logged in)
        var avatarLink = document.querySelector('a.avatar-link[href^="/u/"]');
        if (avatarLink) {
            var username = (avatarLink.getAttribute('href') || '').replace('/u/', '');
            var img = avatarLink.querySelector('img.avatar-sm');
            currentUser = {
                username: username,
                id: null,
                avatar_url: img ? img.getAttribute('src') : null,
                display_name: img ? img.getAttribute('alt') : username
            };
        }

        // Scroll tracking — stop auto-scroll if user scrolled up to read history
        var box = $('chat-messages');
        if (box) {
            box.addEventListener('scroll', function() {
                var distFromBottom = box.scrollHeight - box.scrollTop - box.clientHeight;
                isNearBottom = distFromBottom < SCROLL_THRESHOLD;
            });
        }

        // Form submit
        var form = $('chat-form');
        if (form) {
            form.addEventListener('submit', function(e) {
                e.preventDefault();
                var input = $('chat-input');
                if (!input) return;
                var content = input.value;
                if (!content.trim()) return;
                if (sendMessage(content)) {
                    input.value = '';
                    // Refocus for rapid typing
                    setTimeout(function() { input.focus(); }, 0);
                }
            });
        }

        // Enter to send (no shift — single line input)
        var input = $('chat-input');
        if (input) {
            input.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    if (form) form.dispatchEvent(new Event('submit', { cancelable: true }));
                }
            });
        }

        // Load history first, then connect WS for live updates
        loadHistory();
        connectWs();

        // Tab visibility — reconnect if was hidden for >5min (WS may have dropped)
        var lastHidden = null;
        document.addEventListener('visibilitychange', function() {
            if (document.hidden) {
                lastHidden = Date.now();
            } else if (lastHidden) {
                var elapsed = Date.now() - lastHidden;
                if (elapsed > 5 * 60 * 1000) {
                    if (ws) { try { ws.close(); } catch (e) {} ws = null; }
                    connectWs();
                }
                lastHidden = null;
            }
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
