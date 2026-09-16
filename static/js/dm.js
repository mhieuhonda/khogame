// dm.js — Chat riêng / nhóm chat (v3.14.0).
// - Upload ảnh đính kèm qua POST /uploads/chat/image (JSON {url}).
// - Đếm ký tự textarea.
// - Auto-scroll hộp chat xuống cuối khi load + sau mỗi lần HTMX swap.
(function () {
    "use strict";

    function scrollBox() {
        var box = document.getElementById("dm-box");
        if (box) box.scrollTop = box.scrollHeight;
    }

    function bindCounter() {
        var ta = document.getElementById("dm-content");
        var count = document.getElementById("dm-count");
        if (!ta || !count) return;
        // v3.16.0 FIX (LOW-9): đếm theo code point cho khớp server
        // (chars()) — .length đếm UTF-16 units, emoji bị tính gấp đôi.
        var update = function () { count.textContent = String(Array.from(ta.value).length); };
        ta.addEventListener("input", update);
        update();
    }

    function bindUpload() {
        var file = document.getElementById("dm-file");
        var hidden = document.getElementById("dm-image-url");
        var preview = document.getElementById("dm-preview");
        if (!file || !hidden || !preview) return;
        file.addEventListener("change", function () {
            if (!file.files || !file.files[0]) return;
            var fd = new FormData();
            fd.append("file", file.files[0]);
            preview.textContent = "Đang tải ảnh…";
            fetch("/uploads/chat/image", { method: "POST", body: fd, credentials: "same-origin" })
                .then(function (r) { return r.json().then(function (j) { return { ok: r.ok, body: j }; }); })
                .then(function (res) {
                    if (res.ok && res.body.url) {
                        hidden.value = res.body.url;
                        preview.innerHTML = "";
                        var img = document.createElement("img");
                        img.src = res.body.url;
                        img.alt = "Ảnh đính kèm";
                        img.className = "dm-image dm-preview-img";
                        preview.appendChild(img);
                    } else {
                        preview.textContent = (res.body && res.body.error) || "Tải ảnh thất bại.";
                    }
                    file.value = "";
                })
                .catch(function () {
                    preview.textContent = "Tải ảnh thất bại — thử lại.";
                    file.value = "";
                });
        });
    }

    document.addEventListener("DOMContentLoaded", function () {
        scrollBox();
        bindCounter();
        bindUpload();
    });
    // Poll/push swap nội dung hộp chat → cuộn xuống cuối để thấy tin mới.
    document.body.addEventListener("htmx:afterSwap", function (e) {
        if (e.target && e.target.id === "dm-box") scrollBox();
    });
    // Form gửi xong (2xx) → clear ô nhập + preview. Poll của #dm-box có
    // target khác nên không bao giờ xóa nhầm chữ đang gõ. Gửi lỗi thì
    // giữ nguyên chữ để user sửa rồi gửi lại.
    // (afterRequest detail có `xhr` nhưng KHÔNG có `successful` — check
    // status code trực tiếp.)
    document.body.addEventListener("htmx:afterRequest", function (e) {
        if (!(e.target && e.target.id === "dm-form")) return;
        var s = e.detail && e.detail.xhr ? e.detail.xhr.status : 0;
        if (s >= 200 && s < 300) clearForm();
    });
    // HTMX swap thành công trên #dm-box (gửi tin OK) → cuộn xuống cuối,
    // clear ô nhập + preview ảnh. Chỉ chạy khi swap THÀNH CÔNG nên gửi
    // lỗi thì chữ vẫn giữ nguyên để user sửa rồi gửi lại.
    // (Không dùng hx-on::after-request: htmx self-hosted trigger
    // "htmx:afterRequest" camelCase còn attribute đăng ký
    // "htmx:after-request" kebab — DOM phân biệt hoa/thường nên handler
    // trong attribute không bao giờ chạy.)
    function clearForm() {
        var f = document.getElementById("dm-form");
        if (f) f.reset();
        var preview = document.getElementById("dm-preview");
        if (preview) preview.innerHTML = "";
        bindCounterRefresh();
    }

    function bindCounterRefresh() {
        var count = document.getElementById("dm-count");
        if (count) count.textContent = "0";
    }
})();
