//! Storage service — lưu file user upload lên disk (persistent volume).
//!
//! Lưu ý: đây là storage LOCAL trên VPS, mount qua Docker volume
//! `khogame-storage:/app/storage` (xem deploy/compose.prod.yml).
//! Coolify quản lý volume → file tồn tại qua container restart/redeploy.
//!
//! # Security
//!
//! - Filename do server sinh (UUID v4) — không bao giờ dùng tên file
//!   từ client để tránh path traversal (`../../etc/passwd`) và đụng độ tên.
//! - Extension whitelist: jpg/jpeg/png/webp/gif — block SVG (có thể
//!   chứa `<script>` JS) và mọi định dạng khác.
//! - Magic-byte check: 4 byte đầu phải khớp signature của extension khai
//!   báo — chặn upload file .exe đổi tên thành .jpg.
//! - Size limit: 5MB cho avatar (square), 10MB cho cover image.
//! - MIME type sniff qua magic bytes, KHÔNG tin Content-Type header
//!   (client có thể fake `Content-Type: image/jpeg` cho file zip).

use crate::error::{AppError, AppResult};
use std::path::PathBuf;
use tokio::{fs, io};
use uuid::Uuid;

/// Kích thước tối đa cho từng loại upload (bytes).
pub const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024; // 5 MB
pub const MAX_COVER_BYTES: usize = 10 * 1024 * 1024; // 10 MB

/// Loại upload — quyết định sub-directory và size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadKind {
    /// Ảnh đại diện — sub-dir `avatars`, max 5MB.
    Avatar,
    /// Ảnh bìa game — sub-dir `games`, max 10MB.
    GameCover,
    /// Ảnh bìa tin tức — sub-dir `news`, max 10MB.
    NewsCover,
    /// Ảnh thumbnail repo GitHub (custom, không phải từ GitHub) — sub-dir `repos`, max 5MB.
    RepoImage,
}

impl UploadKind {
    #[must_use]
    pub const fn subdir(self) -> &'static str {
        match self {
            Self::Avatar => "avatars",
            Self::GameCover => "games",
            Self::NewsCover => "news",
            Self::RepoImage => "repos",
        }
    }

    #[must_use]
    pub const fn max_bytes(self) -> usize {
        match self {
            Self::Avatar | Self::RepoImage => MAX_AVATAR_BYTES,
            Self::GameCover | Self::NewsCover => MAX_COVER_BYTES,
        }
    }
}

/// Extension hợp lệ + magic bytes (prefix bytes đầu file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageExt {
    Jpeg,
    Png,
    Webp,
    Gif,
}

impl ImageExt {
    /// Khớp extension (case-insensitive) — trả về `None` nếu không hợp lệ.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }

    /// MIME type chuẩn cho response Content-Type khi serve.
    #[must_use]
    pub const fn mime_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }

    /// Extension file (lowercase, không có dấu chấm).
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    /// Đọc magic bytes đầu file (4-12 byte tuỳ định dạng) để verify
    /// nội dung file thật sự là image, không phải file giả mạo đổi tên.
    ///
    /// Trả về `true` nếu `bytes` bắt đầu bằng signature của format này.
    #[must_use]
    pub fn matches_magic(self, bytes: &[u8]) -> bool {
        match self {
            // JPEG: bắt đầu bằng FF D8 FF (SOI marker).
            Self::Jpeg => {
                bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
            }
            // PNG: 8-byte signature "\x89PNG\r\n\x1a\n".
            Self::Png => {
                bytes.len() >= 8 && bytes[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            }
            // WebP: bắt đầu bằng "RIFF....WEBP" (12 byte).
            Self::Webp => bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
            // GIF: bắt đầu bằng "GIF87a" hoặc "GIF89a".
            Self::Gif => {
                bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a")
            }
        }
    }
}

/// Lấy root dir cho storage từ env `STORAGE_DIR` (default: `/app/storage`
/// trong Docker, `./storage` khi chạy dev ngoài container).
fn storage_root() -> PathBuf {
    std::env::var("STORAGE_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map_or_else(|| PathBuf::from("storage"), PathBuf::from)
}

/// Lưu ảnh upload vào disk. Trả về URL path công khai (relative root),
/// ví dụ `/uploads/avatars/abc-123.jpg` — router sẽ serve directory
/// `STORAGE_DIR/avatars/abc-123.jpg` cho URL này.
///
/// # Errors
///
/// Trả về lỗi khi:
/// - Extension không hợp lệ (không trong whitelist).
/// - Magic bytes không khớp extension (file giả mạo).
/// - File quá lớn (vuợt `kind.max_bytes()`).
/// - I/O lỗi khi tạo directory hoặc ghi file.
pub async fn save_upload(
    kind: UploadKind,
    original_filename: Option<&str>,
    content_type: Option<&str>,
    bytes: &[u8],
) -> AppResult<String> {
    // 1) Lấy extension từ filename, fallback qua content-type.
    let ext = detect_extension(original_filename, content_type).ok_or_else(|| {
        AppError::BadRequest("Định dạng ảnh không hợp lệ. Hỗ trợ: JPG, PNG, WebP, GIF.".into())
    })?;

    // 2) Validate size.
    let max = kind.max_bytes();
    if bytes.len() > max {
        return Err(AppError::BadRequest(format!(
            "Ảnh quá lớn ({} KB). Tối đa {} MB cho {}.",
            bytes.len() / 1024,
            max / 1024 / 1024,
            match kind {
                UploadKind::Avatar => "ảnh đại diện",
                UploadKind::GameCover => "ảnh bìa game",
                UploadKind::NewsCover => "ảnh bìa tin tức",
                UploadKind::RepoImage => "ảnh repo",
            }
        )));
    }

    // 3) Magic byte check — chặn file giả mạo (vd .exe đổi tên .jpg).
    if !ext.matches_magic(bytes) {
        return Err(AppError::BadRequest(
            "Nội dung file không khớp định dạng khai báo (magic bytes sai). Có thể file bị hỏng hoặc giả mạo.".into(),
        ));
    }

    // 4) Sinh filename UUID — không bao giờ dùng tên file client gửi.
    let filename = format!("{}.{}", Uuid::new_v4(), ext.extension());
    let subdir = kind.subdir();
    let root = storage_root();
    let dir = root.join(subdir);

    // 5) Tạo dir recursively (idempotent — `create_dir_all` skip nếu tồn tại).
    fs::create_dir_all(&dir).await.map_err(io_to_app_error)?;

    // 6) Ghi file (atomic-ish: ghi thẳng. Nếu cần atomic, ghi .tmp rồi rename.
    // Hiện không cần vì filename là UUID — không race condition giữa 2 upload).
    let file_path = dir.join(&filename);
    fs::write(&file_path, bytes)
        .await
        .map_err(io_to_app_error)?;

    // 7) Trả về URL công khai (relative root, sẽ được router serve).
    let url = format!("/uploads/{subdir}/{filename}");
    tracing::info!(
        "Upload saved: kind={:?} bytes={} url={}",
        kind,
        bytes.len(),
        url
    );
    Ok(url)
}

/// Dò extension từ filename (ưu tiên) hoặc Content-Type (fallback).
/// Trả về `None` nếu cả 2 đều không hợp lệ.
fn detect_extension(filename: Option<&str>, content_type: Option<&str>) -> Option<ImageExt> {
    // Ưu tiên extension từ filename — client có thể gửi Content-Type sai.
    if let Some(fname) = filename {
        if let Some(dot_idx) = fname.rfind('.') {
            let ext_str = &fname[dot_idx + 1..];
            if let Some(ext) = ImageExt::from_extension(ext_str) {
                return Some(ext);
            }
        }
    }
    // Fallback: parse Content-Type (vd "image/png" → "png").
    if let Some(ct) = content_type {
        // Lấy substring sau "image/"
        let lower = ct.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("image/") {
            // Strip "; charset=..." portion nếu có.
            let ext_str = rest.split(';').next().unwrap_or(rest).trim();
            return ImageExt::from_extension(ext_str);
        }
    }
    None
}

/// Convert `std::io::Error` → `AppError::Internal` với context rõ ràng.
/// Dùng cho các thao tác fs (create_dir_all, write) — không leak path
/// absolute ra user message.
fn io_to_app_error(e: io::Error) -> AppError {
    tracing::error!("Storage I/O error: {e:?}");
    AppError::Internal(anyhow::anyhow!(
        "Lỗi khi ghi file upload — kiểm tra disk space và quyền ghi thư mục storage"
    ))
}

/// Kiểm tra đường dẫn URL có phải do hệ thống upload sinh ra không
/// (điểm bằng `/uploads/`). Dùng trong validation form (avatar_url,
/// cover_image) để phân biệt URL upload vs URL remote http(s) — cả 2
/// đều hợp lệ nhưng có policy bảo mật khác nhau.
#[must_use]
pub fn is_upload_url(url: &str) -> bool {
    url.starts_with("/uploads/")
        && !url.contains("..")
        && !url.contains('\n')
        && !url.contains('\r')
}

/// Resolve URL `/uploads/...` thành đường dẫn file trên disk — dùng
/// cho ServeDir (đã handle qua tower_http) hoặc cho handler serve tay
/// khi cần thêm header (vd Cache-Control immutable).
///
/// Trả về `None` nếu URL không hợp lệ (không bắt đầu `/uploads/`,
/// chứa `..`, hoặc escape root dir). Đây là guard path traversal —
/// NÊN dùng hàm này mỗi khi convert URL → disk path.
#[must_use]
pub fn resolve_upload_path(url: &str) -> Option<PathBuf> {
    if !is_upload_url(url) {
        return None;
    }
    // Strip leading "/uploads/" → relative path "avatars/abc.jpg".
    let rel = &url["/uploads/".len()..];
    // Path traversal check: không chứa `..` segment.
    if rel.split('/').any(|seg| seg == "..") {
        return None;
    }
    let path = storage_root().join(rel);
    // Verify canonical path vẫn nằm trong storage root (chống symlink escape).
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return None,
    };
    let root_canonical = storage_root()
        .canonicalize()
        .unwrap_or_else(|_| storage_root());
    if !canonical.starts_with(&root_canonical) {
        return None;
    }
    Some(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_ext_from_extension() {
        assert_eq!(ImageExt::from_extension("jpg"), Some(ImageExt::Jpeg));
        assert_eq!(ImageExt::from_extension("JPEG"), Some(ImageExt::Jpeg));
        assert_eq!(ImageExt::from_extension("png"), Some(ImageExt::Png));
        assert_eq!(ImageExt::from_extension("webp"), Some(ImageExt::Webp));
        assert_eq!(ImageExt::from_extension("gif"), Some(ImageExt::Gif));
        // Disallowed extensions.
        assert_eq!(ImageExt::from_extension("svg"), None);
        assert_eq!(ImageExt::from_extension("exe"), None);
        assert_eq!(ImageExt::from_extension("pdf"), None);
        assert_eq!(ImageExt::from_extension(""), None);
    }

    #[test]
    fn test_magic_byte_check() {
        // JPEG SOI.
        assert!(ImageExt::Jpeg.matches_magic(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!ImageExt::Jpeg.matches_magic(&[0x89, 0x50])); // PNG signature

        // PNG signature (8 bytes).
        assert!(ImageExt::Png.matches_magic(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]));
        assert!(!ImageExt::Png.matches_magic(&[0xFF, 0xD8, 0xFF]));

        // WebP: "RIFF" + 4 size bytes + "WEBP".
        assert!(ImageExt::Webp.matches_magic(b"RIFF\x00\x00\x00\x00WEBP"));
        assert!(!ImageExt::Webp.matches_magic(b"RIFFxxxxMP4 "));

        // GIF87a / GIF89a.
        assert!(ImageExt::Gif.matches_magic(b"GIF89a..."));
        assert!(ImageExt::Gif.matches_magic(b"GIF87a..."));
        assert!(!ImageExt::Gif.matches_magic(b"GIF88a..."));
    }

    #[test]
    fn test_detect_extension_priority_filename_over_content_type() {
        // Filename .png + Content-Type image/jpeg → ưu tiên PNG.
        let ext = detect_extension(Some("photo.png"), Some("image/jpeg"));
        assert_eq!(ext, Some(ImageExt::Png));
    }

    #[test]
    fn test_detect_extension_fallback_to_content_type() {
        // No filename (Some empty), Content-Type image/webp → WebP.
        let ext = detect_extension(None, Some("image/webp"));
        assert_eq!(ext, Some(ImageExt::Webp));
    }

    #[test]
    fn test_detect_extension_strips_charset() {
        let ext = detect_extension(Some("x.jpg"), Some("image/jpeg; charset=utf-8"));
        assert_eq!(ext, Some(ImageExt::Jpeg));
    }

    #[test]
    fn test_detect_extension_rejects_svg() {
        assert_eq!(detect_extension(Some("evil.svg"), None), None);
    }

    #[test]
    fn test_is_upload_url() {
        assert!(is_upload_url("/uploads/avatars/abc.jpg"));
        assert!(is_upload_url("/uploads/news/x.webp"));
        // Path traversal.
        assert!(!is_upload_url("/uploads/../etc/passwd"));
        assert!(!is_upload_url("/uploads/avatars/../../../etc/passwd"));
        // CRLF injection.
        assert!(!is_upload_url("/uploads/x\nSet-Cookie: bad=1"));
        // Wrong prefix.
        assert!(!is_upload_url("https://example.com/uploads/x.jpg"));
        assert!(!is_upload_url("javascript:alert(1)"));
    }

    #[test]
    fn test_upload_kind_max_bytes() {
        assert_eq!(UploadKind::Avatar.max_bytes(), MAX_AVATAR_BYTES);
        assert_eq!(UploadKind::RepoImage.max_bytes(), MAX_AVATAR_BYTES);
        assert_eq!(UploadKind::GameCover.max_bytes(), MAX_COVER_BYTES);
        assert_eq!(UploadKind::NewsCover.max_bytes(), MAX_COVER_BYTES);
    }
}
