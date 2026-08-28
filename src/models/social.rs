//! Mạng xã hội trên hồ sơ người dùng (v2.7.0).
//!
//! Hỗ trợ 10 nền tảng: GitHub, Facebook, Zalo, Discord, YouTube, TikTok,
//! Instagram, Twitter (X), Telegram và Website cá nhân.
//!
//! ## Thiết kế lưu trữ
//! Bảng `user_social_links` (migration 019) — 1 row/user với cột JSONB
//! `links` dạng `{"github": "https://github.com/user", ...}`. Key là
//! `Platform::id` do server kiểm soát; value là URL đã validate qua
//! allowlist hostname từng nền tảng (chống XSS `javascript:`/`data:`
//! và chống nhúng link bừaplatform khác).
//!
//! ## Validation
//! Mỗi platform có allowlist hostname riêng (ví dụ `github` chỉ nhận
//! `github.com`). `website` là ngoại lệ duy nhất: nhận MỌI URL
//! `http(s)://` vì bản chất là "trang web cá nhân" — host nào cũng có
//! thể hợp lệ. Tất cả đều chặn control byte (CR/LF — header injection)
//! và giới hạn 300 ký tự.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Độ dài tối đa cho 1 URL mạng xã hội (URL dài hơn chắc chắn không phải
/// profile cá nhân — chặn payload rác vào DB).
pub const MAX_SOCIAL_URL_LEN: usize = 300;

/// Một nền tảng mạng xã hội được hỗ trợ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocialPlatform {
    /// ID lưu trong DB (key của JSON) + name của input form
    /// (`social_<id>`). Không bao giờ tới tay user tự ý đặt.
    pub id: &'static str,
    /// Nhãn hiển thị tiếng Việt trong form edit + label aria.
    pub label: &'static str,
    /// Các host được phép (lowercase, so khớp host chính xác hoặc con
    /// của `.` + host). Ví dụ "facebook.com" khớp cả "www.facebook.com".
    pub allowed_hosts: &'static [&'static str],
}

impl SocialPlatform {
    /// True nếu `host` (đã lowercase) thuộc allowlist của platform.
    /// "www.github.com" được chấp nhận cho "github.com" (subdomain www
    /// phổ biến khi user copy link). Subdomain KHÁC (vd
    /// `gist.github.com`, `evil.github.com` cùng-origin spoof) KHÔNG
    /// được phép — chặn link nội dung/địa chỉ phụ thay vì link profile.
    #[must_use]
    pub fn host_allowed(&self, host: &str) -> bool {
        let host = host.strip_prefix("www.").unwrap_or(host);
        self.allowed_hosts.contains(&host)
    }
}

/// Danh sách 10 nền tảng, thứ tự hiển thị cố định (order phía server
/// để mọi client hiển thị nhất quán). 4 nền tảng đầu theo yêu cầu
/// (GitHub, Facebook, Zalo, Discord), 6 nền tảng còn lại phổ biến
/// tiếp theo.
pub const PLATFORMS: &[SocialPlatform] = &[
    SocialPlatform {
        id: "github",
        label: "GitHub",
        allowed_hosts: &["github.com"],
    },
    SocialPlatform {
        id: "facebook",
        label: "Facebook",
        allowed_hosts: &["facebook.com", "fb.com", "fb.me", "m.facebook.com"],
    },
    SocialPlatform {
        id: "zalo",
        label: "Zalo",
        allowed_hosts: &["zalo.me", "id.zalo.me"],
    },
    SocialPlatform {
        id: "discord",
        label: "Discord",
        allowed_hosts: &[
            "discord.com",
            "discord.gg",
            "ptb.discord.com",
            "canary.discord.com",
        ],
    },
    SocialPlatform {
        id: "youtube",
        label: "YouTube",
        allowed_hosts: &["youtube.com", "youtu.be", "m.youtube.com"],
    },
    SocialPlatform {
        id: "tiktok",
        label: "TikTok",
        allowed_hosts: &["tiktok.com", "vt.tiktok.com", "vm.tiktok.com"],
    },
    SocialPlatform {
        id: "instagram",
        label: "Instagram",
        allowed_hosts: &["instagram.com", "instagr.am"],
    },
    SocialPlatform {
        id: "twitter",
        label: "Twitter (X)",
        allowed_hosts: &["twitter.com", "x.com", "mobile.twitter.com"],
    },
    SocialPlatform {
        id: "telegram",
        label: "Telegram",
        allowed_hosts: &["t.me", "telegram.me"],
    },
    SocialPlatform {
        id: "website",
        label: "Website cá nhân",
        allowed_hosts: &[],
    },
];

/// Tìm platform theo id (dùng khi validate form input).
#[must_use]
pub fn platform_by_id(id: &str) -> Option<&'static SocialPlatform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

/// Struct mạng xã hội của 1 user — deserialize từ JSONB `links`.
///
/// `BTreeMap` thay `HashMap` để thứ tự key ổn định (test + serialize
/// về JSON không bị đảo thứ tự giữa các lần chạy).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialLinks {
    #[serde(flatten)]
    links: BTreeMap<String, String>,
}

impl SocialLinks {
    /// Tạo struct rỗng (user chưa đặt link nào).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse từ `serde_json::Value` (cột JSONB). Object rỗng/null/ sai
    /// kiểu → trả struct rỗng thay vì lỗi — user chưa đặt link nào vẫn
    /// phải load được hồ sơ (fail-open an toàn vì field chỉ là link công
    /// khai, không ảnh hưởng quyền).
    #[must_use]
    pub fn from_json_value(v: &serde_json::Value) -> Self {
        let mut out = BTreeMap::new();
        if let serde_json::Value::Object(map) = v {
            for (k, val) in map {
                // Chỉ giữ platform hợp lệ + value là string. Key lạ
                // (không thuộc PLATFORMS) bỏ qua — dọn data cũ nếu có.
                if platform_by_id(k).is_some() {
                    if let serde_json::Value::String(url) = val {
                        if !url.is_empty() {
                            out.insert(k.clone(), url.clone());
                        }
                    }
                }
            }
        }
        Self { links: out }
    }

    /// Serialize về `serde_json::Value` object để bind vào JSONB.
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.links
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect(),
        )
    }

    /// URL của 1 platform (None nếu user không đặt).
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&str> {
        self.links.get(id).map(String::as_str)
    }

    /// Giá trị hiển thị trong input form edit — "" nếu chưa đặt.
    /// Helper template-friendly (Askama render &str đơn giản hơn Option).
    #[must_use]
    pub fn display_value(&self, id: &str) -> &str {
        self.links.get(id).map(String::as_str).unwrap_or_default()
    }

    /// Đặt URL cho platform (không validate — dùng sau khi đã qua
    /// `validate_url`).
    pub fn set(&mut self, id: &str, url: Option<String>) {
        match url {
            Some(u) if !u.is_empty() => {
                self.links.insert(id.to_string(), u);
            }
            _ => {
                self.links.remove(id);
            }
        }
    }

    /// True nếu user chưa đặt link nào.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Validate URL cho 1 platform theo allowlist hostname.
    ///
    /// Trả về:
    /// * `Ok(None)` — URL rỗng (user muốn xóa link) → xóa khỏi DB.
    /// * `Ok(Some(normalized))` — URL hợp lệ (đã trim).
    /// * `Err(msg)` — lỗi validation (msg là thông báo tiếng Việt hiển
    ///   thẳng cho user qua AppError::BadRequest).
    ///
    /// Normalize: thêm `https://` nếu user gõ `github.com/user` không có
    /// scheme (trải nghiệm người dùng tốt hơn — form cũ nhiều user quên
    /// scheme). Sau đó bắt buộc host khớp allowlist.
    pub fn validate_url(platform: &SocialPlatform, raw: &str) -> Result<Option<String>, String> {
        // Chặn control byte (CR/LF/TAB/NUL) TRƯỚC khi trim — trim() ăn
        // mất tab/khoảng trắng cuối → URL với tab cuối lọt qua check.
        // Input chứa control byte là garbage/host-injection — chặn sớm.
        if raw.bytes().any(|b| b.is_ascii_control()) {
            return Err(format!(
                "URL {} chứa ký tự điều khiển không hợp lệ",
                platform.label
            ));
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if trimmed.chars().count() > MAX_SOCIAL_URL_LEN {
            return Err(format!(
                "URL {} tối đa {MAX_SOCIAL_URL_LEN} ký tự",
                platform.label
            ));
        }
        // Scheme: chỉ chấp nhận http(s):// (dùng nguyên bản) HOẶC không có
        // scheme nào (auto thêm https://). Mọi scheme khác (javascript:,
        // ftp:, data:, file:) → Err ngay — không auto-prefix (trước đây
        // "ftp://x" bị ghép thành "https://ftp://x" rồi parse thành host
        // "ftp" — lọt scheme độc vào DB).
        let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else if trimmed.contains("://") {
            return Err(format!(
                "URL {} phải bắt đầu bằng https:// (hoặc http://)",
                platform.label
            ));
        } else {
            format!("https://{trimmed}")
        };
        // Parse URL chuẩn — bắt mọi chuỗi rác như "https://" hoặc "https://a b"
        let parsed = url::Url::parse(&url)
            .map_err(|_| format!("URL {} không đúng định dạng", platform.label))?;
        // Defense-in-depth: scheme sau parse vẫn phải http/https.
        if parsed.scheme() != "https" && parsed.scheme() != "http" {
            return Err(format!(
                "URL {} phải bắt đầu bằng https:// (hoặc http://)",
                platform.label
            ));
        }
        let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
        if host.is_empty() {
            return Err(format!("URL {} thiếu tên miền", platform.label));
        }
        // Website cá nhân: nhận mọi host (đã qua check scheme + control byte
        // ở trên). Các platform khác: bắt buộc khớp allowlist hostname.
        if platform.id == "website" || platform.host_allowed(&host) {
            Ok(Some(url))
        } else {
            let examples: Vec<String> = platform
                .allowed_hosts
                .iter()
                .map(|h| format!("https://{h}/..."))
                .collect();
            Err(format!(
                "URL {} phải là link trên {}",
                platform.label,
                examples.join(", ")
            ))
        }
    }

    /// Validate toàn bộ form input (10 field `social_<id>`).
    ///
    /// `raw: &[(&str, Option<&str>)]` — cặp (platform_id, giá trị form).
    /// Trả về struct mới hoàn toàn (không merge với DB cũ): field vắng
    /// mặt trong form = xóa link — hành vi chuẩn HTML form (input để
    /// trống nghĩa là user chủ động xóa).
    pub fn validate_form(raw: &[(&str, Option<&str>)]) -> Result<Self, String> {
        let mut out = Self::new();
        for (id, val) in raw {
            let platform =
                platform_by_id(id).ok_or_else(|| format!("Nền tảng không hỗ trợ: {id}"))?;
            if let Some(url) = Self::validate_url(platform, val.unwrap_or_default())? {
                out.set(id, Some(url));
            }
        }
        Ok(out)
    }

    /// Danh sách link để render template theo thứ tự PLATFORMS.
    /// Chỉ trả link user đã đặt (không render placeholder rỗng).
    #[must_use]
    pub fn ordered(&self) -> Vec<SocialLinkView> {
        PLATFORMS
            .iter()
            .filter_map(|p| {
                self.links.get(p.id).map(|url| SocialLinkView {
                    id: p.id,
                    label: p.label,
                    url: url.clone(),
                })
            })
            .collect()
    }
}

/// 1 link hiển thị trong template hồ sơ (đã qua thứ tự PLATFORMS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialLinkView {
    pub id: &'static str,
    pub label: &'static str,
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_count_and_ids() {
        // 10 nền tảng: 4 yêu cầu (github, facebook, zalo, discord) + 6 khác
        assert_eq!(PLATFORMS.len(), 10);
        for (i, want) in [
            "github",
            "facebook",
            "zalo",
            "discord",
            "youtube",
            "tiktok",
            "instagram",
            "twitter",
            "telegram",
            "website",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(PLATFORMS[i].id, *want, "Platform thứ {i} sai");
        }
        // Mỗi platform phải unique id
        let ids: std::collections::HashSet<&str> = PLATFORMS.iter().map(|p| p.id).collect();
        assert_eq!(ids.len(), 10, "Platform id bị trùng");
        // Mỗi platform phải có label
        assert!(PLATFORMS.iter().all(|p| !p.label.is_empty()));
    }

    #[test]
    fn test_validate_github_ok() {
        let p = platform_by_id("github").unwrap();
        // URL chuẩn
        assert_eq!(
            SocialLinks::validate_url(p, "https://github.com/mhieuhonda").unwrap(),
            Some("https://github.com/mhieuhonda".to_string())
        );
        // Không có scheme → tự thêm https
        assert_eq!(
            SocialLinks::validate_url(p, "github.com/mhieuhonda").unwrap(),
            Some("https://github.com/mhieuhonda".to_string())
        );
        // www subdomain được chấp nhận
        assert_eq!(
            SocialLinks::validate_url(p, "https://www.github.com/user").unwrap(),
            Some("https://www.github.com/user".to_string())
        );
    }

    #[test]
    fn test_validate_github_wrong_host() {
        let p = platform_by_id("github").unwrap();
        // Host khác → lỗi
        assert!(SocialLinks::validate_url(p, "https://gitlab.com/user").is_err());
        // gist.github.com không thuộc allowlist (chặn link nội dung)
        assert!(SocialLinks::validate_url(p, "https://gist.github.com/x").is_err());
        // javascript: scheme → lỗi
        assert!(SocialLinks::validate_url(p, "javascript:alert(1)").is_err());
    }

    #[test]
    fn test_validate_zalo_discord_facebook() {
        let zalo = platform_by_id("zalo").unwrap();
        assert_eq!(
            SocialLinks::validate_url(zalo, "https://zalo.me/0123456789").unwrap(),
            Some("https://zalo.me/0123456789".to_string())
        );
        assert!(SocialLinks::validate_url(zalo, "https://facebook.com/x").is_err());

        let discord = platform_by_id("discord").unwrap();
        // Discord profile URL chuẩn
        assert!(SocialLinks::validate_url(discord, "https://discord.com/users/123456789").is_ok());
        // Invite server cũng hợp lệ (nhiều user chỉ có invite)
        assert!(SocialLinks::validate_url(discord, "https://discord.gg/abc").is_ok());

        let fb = platform_by_id("facebook").unwrap();
        assert!(SocialLinks::validate_url(fb, "https://fb.com/mhieuhonda").is_ok());
        assert!(SocialLinks::validate_url(fb, "https://m.facebook.com/user").is_ok());
    }

    #[test]
    fn test_validate_website_any_host() {
        let w = platform_by_id("website").unwrap();
        // Website cá nhân nhận mọi http(s) host
        assert!(SocialLinks::validate_url(w, "https://blog.example.com/post/1").is_ok());
        assert!(SocialLinks::validate_url(w, "vangioitutien.com").is_ok());
        // Nhưng vẫn chặn scheme nguy hiểm + control byte
        assert!(SocialLinks::validate_url(w, "javascript:alert(1)").is_err());
        assert!(SocialLinks::validate_url(w, "ftp://example.com").is_err());
    }

    #[test]
    fn test_validate_empty_means_remove() {
        let p = platform_by_id("github").unwrap();
        // Chuỗi rỗng/khoảng trắng = xóa link (Ok(None))
        assert_eq!(SocialLinks::validate_url(p, "").unwrap(), None);
        assert_eq!(SocialLinks::validate_url(p, "   ").unwrap(), None);
    }

    #[test]
    fn test_validate_url_length_limit() {
        let p = platform_by_id("github").unwrap();
        let long = format!("https://github.com/{}", "a".repeat(MAX_SOCIAL_URL_LEN));
        assert!(SocialLinks::validate_url(p, &long).is_err());
    }

    #[test]
    fn test_validate_control_bytes_blocked() {
        let p = platform_by_id("github").unwrap();
        // CR/LF trong URL — header injection
        assert!(SocialLinks::validate_url(p, "https://github.com/a\r\nSet-Cookie: x").is_err());
        assert!(SocialLinks::validate_url(p, "https://github.com/\t").is_err());
    }

    #[test]
    fn test_validate_form_roundtrip() {
        let raw = vec![
            ("github", Some("github.com/user")),
            ("facebook", Some("https://facebook.com/user")),
            ("zalo", Some("")), // rỗng → xóa
            ("discord", Some("https://discord.gg/xyz")),
            ("youtube", Some("https://youtube.com/@channel")),
            ("tiktok", Some("https://tiktok.com/@user")),
            ("instagram", Some("https://instagram.com/user")),
            ("twitter", Some("https://x.com/user")),
            ("telegram", Some("https://t.me/user")),
            ("website", Some("https://blog.example.com")),
        ];
        let links = SocialLinks::validate_form(&raw).unwrap();
        assert_eq!(links.get("github"), Some("https://github.com/user"));
        assert_eq!(links.get("zalo"), None, "rỗng phải bị xóa");
        assert_eq!(links.get("twitter"), Some("https://x.com/user"));
        assert_eq!(links.ordered().len(), 9);

        // Platform id lạ → lỗi
        let bad = vec![("hacker_platform", Some("https://evil.com"))];
        assert!(SocialLinks::validate_form(&bad).is_err());
        // URL sai host trong form → lỗi
        let bad2 = vec![("github", Some("https://evil.com"))];
        assert!(SocialLinks::validate_form(&bad2).is_err());
    }

    #[test]
    fn test_json_roundtrip_and_garbage() {
        let mut links = SocialLinks::new();
        links.set("github", Some("https://github.com/u".into()));
        links.set("telegram", Some("https://t.me/u".into()));
        let v = links.to_json_value();
        let back = SocialLinks::from_json_value(&v);
        assert_eq!(back, links);

        // JSONB rỗng/null/sai kiểu → struct rỗng (không panic)
        assert!(SocialLinks::from_json_value(&serde_json::json!({})).is_empty());
        assert!(SocialLinks::from_json_value(&serde_json::Value::Null).is_empty());
        assert!(SocialLinks::from_json_value(&serde_json::json!("not an object")).is_empty());
        // Object chứa key lạ + value sai kiểu → lọc sạch
        let dirty =
            serde_json::json!({"github": 42, "unknown_key": "x", "zalo": "https://zalo.me/1"});
        let cleaned = SocialLinks::from_json_value(&dirty);
        assert_eq!(cleaned.ordered().len(), 1);
        assert_eq!(cleaned.get("zalo"), Some("https://zalo.me/1"));
    }

    #[test]
    fn test_set_none_removes() {
        let mut links = SocialLinks::new();
        links.set("github", Some("https://github.com/u".into()));
        assert!(!links.is_empty());
        links.set("github", None);
        assert!(links.is_empty());
        // set chuỗi rỗng cũng xóa
        links.set("github", Some("https://github.com/u".into()));
        links.set("github", Some(String::new()));
        assert!(links.is_empty());
    }

    #[test]
    fn test_ordered_reserves_platform_order() {
        // Lưu ngược thứ tự → ordered() vẫn đúng PLATFORMS order
        let mut links = SocialLinks::new();
        links.set("website", Some("https://a.com".into()));
        links.set("github", Some("https://github.com/u".into()));
        links.set("zalo", Some("https://zalo.me/1".into()));
        let ord = links.ordered();
        assert_eq!(ord[0].id, "github");
        assert_eq!(ord[1].id, "zalo");
        assert_eq!(ord[2].id, "website");
        // Label tiếng Việt theo platform
        assert_eq!(ord[0].label, "GitHub");
    }
}
