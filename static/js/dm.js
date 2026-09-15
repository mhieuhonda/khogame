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
        var update = function () { count.textContent = String(ta.value.length); };
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
    // HTMX poll append/replace → cuộn xuống cuối để thấy tin mới.
    document.body.addEventListener("htmx:afterSwap", function (e) {
        if (e.target && e.target.id === "dm-box") scrollBox();
    });
})();
