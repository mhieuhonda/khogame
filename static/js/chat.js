// ============================================
// Louis Space v2.9 — Live Chat frontend (typing indicator + online users)
// ============================================
// WebSocket realtime chat. Falls back gracefully:
//   - User not logged in → input disabled, "Đăng nhập" button shown
//   - WS connect fail → retry with exponential backoff (max 30s)
//   - WS disconnect → show "Đang kết nối lại…" + auto reconnect
//   - History fetch fail → show error message + retry button
//
// Security notes:
//   - All user content rendered via textContent (no innerHTML cho msg body)
//     → XSS-safe kể cả khi attacker gửi <script> tags
//   - URL/email trong message body KHÔNG auto-link (tránh phishing)
//   - Admin delete: ẩn khỏi UI, server giữ record
//
// v2.0 FIX: selector phát hiện user dùng `a.avatar-link[href^="/u/"]`
// (bản cũ `a.avatar-linkref^="/u/"` là CSS syntax error → querySelector
// throw → toàn bộ init() sập → chat không load history được).
// ============================================

(function() {
    'use strict';

    var MAX_MSG = 200;            // Giữ tối đa N message trong DOM
    var SCROLL_THRESHOLD = 80;    // px từ đáy — auto-scroll nếu user ở gần đáy

    var ws = null;
    var wsRetryDelay = 1000;       // ms, nhân đôi mỗi lần fail (cap 30s)
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

    // Avatar node — dùng DOM API an toàn thay vì innerHTML
    function avatarNode(msg) {
        var wrap = el('span', 'chat-msg-avatar');
        if (msg.avatar_url) {
            var img = document.createElement('img');
            img.src = msg.avatar_url;
            img.alt = '';
            img.className = 'avatar avatar-sm';
            img.loading = 'lazy';
            img.decoding = 'async';
            img.width = 32;
            img.height = 32;
            wrap.appendChild(img);
            return wrap;
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
        var fb = el('span', 'avatar avatar-sm avatar-fallback');
        fb.textContent = initials;
        wrap.appendChild(fb);
        return wrap;
    }

    function renderMessage(msg) {
        var node = el('div', 'chat-msg');
        node.setAttribute('data-id', msg.id);
        // v2.9.2 FIX: currentUser.id luôn null (init chỉ lấy được username từ
        // header) → so sánh id không bao giờ khớp → highlight "tin của mình"
        // không hoạt động. So sánh username (đáng tin từ cả 2 nguồn).
        if (msg.user_id && currentUser &&
            (msg.user_id === currentUser.id ||
             (currentUser.username && msg.username === currentUser.username))) {
            node.classList.add('chat-msg-own');
        }
        // v2.9.2 FIX: server trả role::text từ Postgres enum ('admin' /
        // 'moderator' — lowercase) → so sánh 'Admin'/'Moderator' hoa-hoa
        // không bao giờ khớp → badge staff không hiện. Normalize lower-case
        // để an toàn với cả 2 chuẩn dữ liệu.
        var roleLower = (msg.role || '').toLowerCase();
        var isStaff = roleLower === 'admin' || roleLower === 'moderator';

        node.appendChild(avatarNode(msg));

        var bubble = el('div', 'chat-msg-bubble');
        var header = el('div', 'chat-msg-header');
        var author = el('span', 'chat-msg-author');
        author.textContent = msg.display_name || msg.username;
        header.appendChild(author);
        if (isStaff) {
            var badge = el('span', 'chat-msg-badge');
            badge.textContent = roleLower === 'admin' ? 'Admin' : 'Mod';
            header.appendChild(badge);
        }
        var time = el('span', 'chat-msg-time');
        time.title = msg.created_at;
        time.textContent = timeAgo(msg.created_at);
        header.appendChild(time);
        bubble.appendChild(header);

        var body = el('div', 'chat-msg-content');
        body.textContent = msg.content;  // textContent = XSS-safe
        bubble.appendChild(body);

        node.appendChild(bubble);
        return node;
    }

    function renderDeletedMessage(msg) {
        var node = el('div', 'chat-msg chat-msg-deleted');
        node.setAttribute('data-id', msg.id);
        var bubble = el('div', 'chat-msg-bubble');
        var body = el('div', 'chat-msg-content chat-msg-deleted-text');
        body.textContent = 'Tin nhắn đã bị ẩn bởi quản trị viên';
        bubble.appendChild(body);
        node.appendChild(bubble);
        return node;
    }

    function markMessageDeleted(id) {
        var node = document.querySelector('.chat-msg[data-id="' + id + '"]');
        if (!node) return;
        node.classList.add('chat-msg-deleted');
        var header = node.querySelector('.chat-msg-header');
        if (header) header.remove();
        var body = node.querySelector('.chat-msg-content');
        if (body) {
            body.textContent = 'Tin nhắn đã bị ẩn bởi quản trị viên';
            body.classList.add('chat-msg-deleted-text');
        }
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
        removePlaceholders(box);

        var node = msg.is_deleted ? renderDeletedMessage(msg) : renderMessage(msg);
        box.appendChild(node);

        // Trim DOM — giữ N node cuối
        while (box.children.length > MAX_MSG) {
            box.removeChild(box.firstChild);
        }

        if (isNearBottom) {
            scrollToBottom(false);
        }
    }

    function removePlaceholders(box) {
        var loading = box.querySelector('.chat-loading');
        if (loading) loading.remove();
        var empty = box.querySelector('.chat-empty');
        if (empty) empty.remove();
    }

    function renderEmptyState() {
        var box = $('chat-messages');
        if (!box) return;
        box.innerHTML = '';
        var empty = el('div', 'chat-empty');
        var p = document.createElement('p');
        p.textContent = 'Chưa có tin nhắn nào. Hãy là người đầu tiên!';
        empty.appendChild(p);
        box.appendChild(empty);
    }

    function loadHistory() {
        fetch('/chat/history', { headers: { 'Accept': 'application/json' } })
            .then(function(r) {
                if (!r.ok) throw new Error('HTTP ' + r.status);
                return r.json();
            })
            .then(function(data) {
                if (data.online != null) {
                    var count = $('chat-online-count');
                    if (count) count.textContent = data.online;
                }
                if (data.today_count != null) {
                    var today = $('chat-today-count');
                    if (today) today.textContent = data.today_count + ' tin hôm nay';
                }
                var box = $('chat-messages');
                if (!box) return;
                box.innerHTML = '';
                if (!data.messages || data.messages.length === 0) {
                    renderEmptyState();
                    return;
                }
                // data.messages old→new (server đã đảo). Render tuần tự.
                data.messages.forEach(function(msg) {
                    var node = msg.is_deleted ? renderDeletedMessage(msg) : renderMessage(msg);
                    box.appendChild(node);
                });
                isNearBottom = true;
                scrollToBottom(false);
            })
            .catch(function() {
                var box = $('chat-messages');
                if (!box) return;
                box.innerHTML = '';
                var errNode = el('div', 'chat-empty chat-empty-error');
                var p = document.createElement('p');
                p.textContent = 'Không tải được tin nhắn.';
                errNode.appendChild(p);
                var btn = el('button', 'btn btn-outline btn-sm');
                btn.id = 'chat-retry';
                btn.type = 'button';
                btn.textContent = 'Thử lại';
                errNode.appendChild(btn);
                box.appendChild(errNode);
                btn.addEventListener('click', loadHistory);
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
            setStatus('Đã kết nối · realtime');
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
                // v2.9.0 — nhận message → xóa typing indicator của người gửi
                clearTypingFor(event.message.user_id);
            } else if (event.type === 'delete') {
                markMessageDeleted(event.id);
            } else if (event.type === 'presence') {
                var count = $('chat-online-count');
                if (count) count.textContent = event.online;
                // v2.9.0 — refresh danh sách online users
                fetchOnlineUsers();
            } else if (event.type === 'typing') {
                // v2.9.0 — typing indicator (bỏ qua chính mình)
                if (currentUser && event.user_id === currentUser.id) return;
                showTyping(event.display_name, event.user_id);
            }
        };
        ws.onclose = function(ev) {
            ws = null;
            if (ev.code === 1008 || ev.code === 4001) {
                // 1008: policy violation (auth fail). 4001: custom auth-fail code.
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

    // ============================================
    // v2.9.0 — TYPING INDICATOR + ONLINE USERS
    // ============================================
    var lastTypingSent = 0;
    var typingUsers = {};   // user_id → {name, timeout}
    var typingTimers = {};

    function showTyping(displayName, userId) {
        var indicator = $('chat-typing-indicator');
        if (!indicator) return;
        typingUsers[userId] = displayName;
        renderTypingIndicator();
        // Tự xóa sau 4s nếu không có activity mới
        clearTimeout(typingTimers[userId]);
        typingTimers[userId] = setTimeout(function() {
            clearTypingFor(userId);
        }, 4000);
    }

    function clearTypingFor(userId) {
        if (!(userId in typingUsers)) return;
        delete typingUsers[userId];
        clearTimeout(typingTimers[userId]);
        renderTypingIndicator();
    }

    function renderTypingIndicator() {
        var indicator = $('chat-typing-indicator');
        if (!indicator) return;
        var names = Object.keys(typingUsers).map(function(k) { return typingUsers[k]; });
        if (names.length === 0) {
            indicator.textContent = '';
            indicator.classList.remove('typing-dots');
            return;
        }
        var text;
        if (names.length === 1) text = names[0] + ' đang gõ';
        else if (names.length === 2) text = names[0] + ' và ' + names[1] + ' đang gõ';
        else text = names.length + ' người đang gõ';
        indicator.textContent = text;
        indicator.classList.add('typing-dots');
    }

    function fetchOnlineUsers() {
        fetch('/chat/online-users', { credentials: 'same-origin' })
            .then(function(r) { return r.ok ? r.json() : null; })
            .then(function(data) {
                if (!data || !data.users) return;
                var panel = $('chat-online-panel');
                var list = $('chat-online-list');
                var count = $('chat-online-count');
                var panelCount = $('chat-online-panel-count');
                if (count) count.textContent = data.online;
                if (panelCount) panelCount.textContent = data.online;
                if (!panel || !list) return;
                list.textContent = '';
                data.users.slice(0, 30).forEach(function(u) {
                    var row = el('div', 'online-user-row');
                    var dot = el('span', 'online-dot');
                    row.appendChild(dot);
                    if (u.avatar_url) {
                        var img = document.createElement('img');
                        img.className = 'online-avatar';
                        img.src = u.avatar_url;
                        img.alt = '';
                        img.loading = 'lazy';
                        row.appendChild(img);
                    } else {
                        var fb = el('span', 'online-avatar-fallback');
                        fb.textContent = (u.display_name || '?').slice(0, 1).toUpperCase();
                        row.appendChild(fb);
                    }
                    var name = el('span');
                    name.textContent = u.display_name;
                    if (u.role === 'Admin' || u.role === 'admin') name.style.color = '#a855f7';
                    else if (u.role === 'Moderator' || u.role === 'moderator') name.style.color = '#60a5fa';
                    row.appendChild(name);
                    list.appendChild(row);
                });
                panel.hidden = false;
            })
            .catch(function() { /* panel ẩn nếu fail */ });
    }

    function sendMessage(text) {
        if (!ws || ws.readyState !== WebSocket.OPEN) {
            setStatus('Chưa kết nối tới máy chủ. Thử lại sau 2 giây.', true);
            return false;
        }
        if (!text || !text.trim()) return false;
        var truncated = text.length > 500 ? text.slice(0, 500) : text;
        // Plain text frame — backend treat non-JSON là chat message
        ws.send(truncated);
        return true;
    }

    function init() {
        // Detect current user từ header (avatar-link nếu đã login)
        try {
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
        } catch (e) {
            // Selector fail-safe — không làm sập init
            currentUser = null;
        }

        // Scroll tracking — ngừng auto-scroll nếu user scroll lên đọc history
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
                    // Refocus để gõ tiếp nhanh
                    setTimeout(function() { input.focus(); }, 0);
                }
            });
        }

        // Enter để gửi (input 1 dòng — không cần shift)
        var input = $('chat-input');
        if (input) {
            input.addEventListener('keydown', function(e) {
                if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    if (form) form.dispatchEvent(new Event('submit', { cancelable: true }));
                }
            });
        }

        // v2.9.0 — TYPING INDICATOR: gửi "đang gõ" throttle 3s khi input
        if (input) {
            input.addEventListener('input', function() {
                if (!input.value.trim()) return;
                var now = Date.now();
                if (now - lastTypingSent > 3000) {
                    lastTypingSent = now;
                    try {
                        fetch('/chat/typing', { method: 'POST', credentials: 'same-origin' }).catch(function() {});
                    } catch (e) {}
                }
            });
        }

        // v2.9.0 — Panel người online: poll 20s + ngay khi init
        fetchOnlineUsers();
        setInterval(fetchOnlineUsers, 20000);

        // Load history trước, rồi connect WS nhận live updates
        loadHistory();
        connectWs();

        // Tab visibility — reconnect nếu ẩn tab >5 phút (WS có thể đã drop)
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
