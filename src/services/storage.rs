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

/// Tổng số pixel tối đa cho ảnh raster (chống decompression bomb: file vài KB
/// nén có thể nở ra hàng trăm MP khi decode → OOM/OOM-kill ở bước resize/hiển
/// thị. Reject ngay từ header, không cần decode full hay thêm crate mới).
pub const MAX_IMAGE_PIXELS: u64 = 25_000_000; // 25 MP

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

/// Đọc (width, height) ảnh raster từ ~30 byte header đầu, KHÔNG decode full
/// (không cần thêm crate). Trả `None` khi không parse được (file hỏng/cắt cụt).
fn image_dimensions(ext: ImageExt, bytes: &[u8]) -> Option<(u32, u32)> {
    match ext {
        // PNG: 8-byte signature + length(4) + "IHDR"(4) + width(4 BE) + height(4 BE).
        ImageExt::Png => {
            if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
                return None;
            }
            let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            Some((w, h))
        }
        // GIF: "GIF87a"/"GIF89a" (6) + width(2 LE) + height(2 LE) (logical screen descriptor).
        ImageExt::Gif => {
            if bytes.len() < 10 {
                return None;
            }
            let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
            let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
            Some((w, h))
        }
        // JPEG: quét SOF markers (C0-C3, C5-C7, C9-CB, CD-CF). Mỗi segment:
        // FF | marker | len(2 BE, tính cả 2 byte len) | payload. SOF payload:
        // precision(1) + height(2 BE) + width(2 BE). Marker không có len
        // (SOI D8, EOI D9, RST D0-D7, TEM 01) thì bỏ qua 1 byte.
        ImageExt::Jpeg => {
            let mut i = 2; // bỏ qua SOI (FF D8).
            while i + 3 < bytes.len() {
                // Tìm byte FF đầu segment.
                if bytes[i] != 0xFF {
                    i += 1;
                    continue;
                }
                // Bỏ qua padding FF FF FF...
                let mut j = i + 1;
                while j < bytes.len() && bytes[j] == 0xFF {
                    j += 1;
                }
                if j >= bytes.len() {
                    return None;
                }
                let marker = bytes[j];
                // Marker standalone (không có length) — bước tiếp.
                if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
                    i = j + 1;
                    continue;
                }
                if j + 2 >= bytes.len() {
                    return None;
                }
                let seg_len = u16::from_be_bytes([bytes[j + 1], bytes[j + 2]]) as usize;
                if seg_len < 2 || j + 1 + seg_len > bytes.len() {
                    return None;
                }
                // SOFn (trừ DHT C4, JPG C8, DAC CC) chứa dimensions.
                if matches!(
                    marker,
                    0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD
                        | 0xCE | 0xCF
                ) {
                    // SOF payload tối thiểu 7 byte: precision(1)+h(2)+w(2)+components(1)+...
                    if seg_len < 9 {
                        return None;
                    }
                    let h = u16::from_be_bytes([bytes[j + 4], bytes[j + 5]]) as u32;
                    let w = u16::from_be_bytes([bytes[j + 6], bytes[j + 7]]) as u32;
                    return Some((w, h));
                }
                i = j + 1 + seg_len;
            }
            None
        }
        // WebP: "RIFF"(4) + size(4) + "WEBP"(4) + chunk FourCC(4) + size(4) + data.
        ImageExt::Webp => {
            if bytes.len() < 20 {
                return None;
            }
            // So sánh FourCC bằng `==` (như `matches_magic` đang dùng) thay vì
            // `match` slice với byte-string pattern (không compile: &[u8] vs &[u8; 4]).
            let fourcc = &bytes[12..16];
            if fourcc == b"VP8 " {
                // VP8 lossy: frame tag(3) + start code 9D 01 2A(3) + w(2 LE, 14 bit) + h(2 LE, 14 bit).
                if bytes.len() < 30 || bytes[23] != 0x9D || bytes[24] != 0x01 || bytes[25] != 0x2A
                {
                    return None;
                }
                let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as u32;
                let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as u32;
                Some((w, h))
            } else if fourcc == b"VP8L" {
                // VP8L lossless: signature 0x2F(1) + 4 byte gói width-1 (14 bit) + height-1 (14 bit).
                if bytes.len() < 25 || bytes[20] != 0x2F {
                    return None;
                }
                let b1 = bytes[21] as u32;
                let b2 = bytes[22] as u32;
                let b3 = bytes[23] as u32;
                let b4 = bytes[24] as u32;
                let w = (b1 | ((b2 & 0x3F) << 8)) + 1;
                let h = (((b2 >> 6) & 0x03) | (b3 << 2) | ((b4 & 0x0F) << 10)) + 1;
                Some((w, h))
            } else if fourcc == b"VP8X" {
                // VP8X extended: data 10 byte, width-1 ở byte 4-6 (24 bit LE),
                // height-1 ở byte 7-9 (24 bit LE) tính từ đầu data (offset 20).
                if bytes.len() < 30 {
                    return None;
                }
                let w = (bytes[24] as u32
                    | ((bytes[25] as u32) << 8)
                    | ((bytes[26] as u32) << 16))
                    + 1;
                let h = (bytes[27] as u32
                    | ((bytes[28] as u32) << 8)
                    | ((bytes[29] as u32) << 16))
                    + 1;
                Some((w, h))
            } else {
                None
            }
        }
    }
}

/// Validate pixel dimensions sau magic-byte check: reject dimension 0,
/// không parse được, hoặc tổng pixel vượt `MAX_IMAGE_PIXELS`.
fn validate_pixel_dimensions(ext: ImageExt, bytes: &[u8]) -> AppResult<()> {
    let (w, h) = image_dimensions(ext, bytes).ok_or_else(|| {
        AppError::BadRequest(
            "Không đọc được kích thước ảnh (header hỏng hoặc file cắt cụt).".into(),
        )
    })?;
    if w == 0 || h == 0 {
        return Err(AppError::BadRequest(
            "Kích thước ảnh không hợp lệ (width/height = 0).".into(),
        ));
    }
    if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
        return Err(AppError::BadRequest(format!(
            "Ảnh quá lớn ({}×{} px). Tối đa {} MP để chống decompression bomb.",
            w,
            h,
            MAX_IMAGE_PIXELS / 1_000_000
        )));
    }
    Ok(())
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

    // 3b) Giới hạn pixel dimensions (chống decompression bomb + EXIF/GPS leak
    // ý thức: không decode full, chỉ đọc ~30 byte header để lấy width/height.
    // Reject khi dimension = 0, không parse được, hoặc width*height > 25MP.
    // Định dạng khác ngoài 4 loại raster hiện hỗ trợ thì cho qua (giữ nguyên)).
    validate_pixel_dimensions(ext, bytes)?;

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
