## Mô tả thay đổi

<!-- Tóm tắt NGẮN GỌN thay đổi này làm gì và tại sao. -->

## Loại thay đổi

- [ ] 🐛 Bug fix (không đổi behavior ngoài phần bị lỗi)
- [ ] ✨ Tính năng mới
- [ ] ♻️ Refactor (không đổi behavior)
- [ ] ⚡ Hiệu năng
- [ ] 📚 Tài liệu
- [ ] 🔧 Hạ tầng/CI

## Checklist tự kiểm tra

- [ ] `cargo fmt --all` đã chạy
- [ ] `cargo clippy --all-targets -- -D warnings` sạch
- [ ] `cargo test --locked` pass (thêm test mới nếu thêm logic)
- [ ] Thay đổi DB (nếu có) có migration tương ứng + đã review rollback
- [ ] Không commit secret/credentials

## Ảnh chụp (nếu là UI)

<!-- Kèm ảnh before/after cho thay đổi giao diện. -->
