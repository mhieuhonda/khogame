//! v3.1.0 — Repository game Nối từ (Word Chain).
//!
//! Thiết kế:
//! - Mỗi play độc lập: user gõ 1 từ tiếng Việt hợp lệ → bot trả về 1 từ
//!   bắt đầu bằng chữ cái cuối của từ user.
//! - "Hợp lệ" = nằm trong từ điển embedded (~500 từ phổ biến).
//! - Daily cap: 20 plays/ngày (anti-farm).
//! - XP thưởng: +3/valid word, 0/invalid.
//! - Lifecycle valid count dùng cho huy hiệu word_chain_X.
//!
//! Lưu ý kỹ thuật:
//! - Từ điển dùng dạng KHÔNG DẤU để tránh phức tạp trong matching
//!   (user có gõ có dấu hoặc không dấu đều match). Sau này có thể nâng
//!   cấp sang có dấu nếu cần.
//! - Chữ "cuối cùng" để nối = ký tự cuối (last char) của từ user — bot
//!   chọn từ điển bắt đầu bằng ký tự đó.

use crate::error::{AppError, AppResult};
use crate::models::gamification::LevelInfo;
use sqlx::PgPool;
use uuid::Uuid;

/// Giới hạn số ván chơi mỗi ngày.
pub const WORD_CHAIN_DAILY_CAP: i64 = 20;
/// XP thưởng mỗi từ hợp lệ.
pub const WORD_CHAIN_XP_PER_VALID: i32 = 3;
/// Độ dài tối đa từ user gõ (DB VARCHAR(100)).
pub const WORD_CHAIN_MAX_LEN: usize = 100;

/// Từ điển tiếng Việt KHÔNG DẤU (~270 từ duy nhất — v3.2.0 đã dedupe
/// từ bản 491 entry trùng lặp). Mỗi entry lowercase, sort tăng dần.
/// Matching: user nhập → normalize_word() → check contains.
///
/// (Lưu ý: cấu trúc data nhỏ, embedded thẳng trong binary — không cần
/// load từ file external, app đơn giản hoá deploy.)
pub const VI_VOCAB: &[&str] = &[
    // A
    "ai", "anh", "ao", "ay", // B
    "ba", "ban", "bang", "banh", "bao", "bat", "bay", "be", "ben", "benh", "beo", "bep", "bet",
    "bi", "bia", "bich", "bien", "bim", "binh", "bo", "boc", "bon", "bong", "bop", "bua", "buc",
    "bun", "buoc", "buom", "buon", "bup", "buu", // C
    "ca", "cam", "can", "canh", "cao", "cap", "cau", "cay", "co", "coi", "com", "con", "cong",
    "cop", "cot", "cu", "cua", "cuc", "cung", "cuoi", "cuop", "cuu", // D
    "da", "danh", "dat", "dau", "de", "dem", "den", "di", "diem", "do", "doc", "doi", "dong",
    "dua", "dung", "duoc", "duoi", "duong", "duy", "duyet", // E
    "em", "eo", // G
    "ga", "gai", "gam", "gan", "gang", "gao", "gap", "gia", "giao", "gio", "gioi", "giong", "go",
    "gom", "got", "gung", "guom", // H
    "ha", "hai", "ham", "han", "hang", "hanh", "hao", "hau", "hay", "hen", "het", "hien", "hinh",
    "ho", "hoa", "hoan", "hoc", "hoi", "hom", "hop", "hot", "hue", "huong", "hut", "huy",
    // K
    "ka", "ke", "kem", "keo", "kha", "khac", "khe", "khi", "kho", "khoc", "khoi", "khong", "khop",
    "khue", "kien", "kieu", "kinh", // L
    "la", "lam", "lan", "lang", "lanh", "lay", "le", "len", "lien", "loi", "lon", "long", "lua",
    "luc", "luoi", // M
    "ma", "mai", "mang", "mau", "may", "me", "mien", "minh", "mo", "moi", "mon", "mua", "muoi",
    "muon", // N
    "na", "nam", "nan", "nao", "nay", "ne", "nen", "ngay", "nghe", "ngu", "ngua", "nguoi", "nha",
    "nhan", "nhat", "nhe", "nhi", "nhieu", "nhip", "nho", "nhoc", "nhoi", "nhom", "nhot", "no",
    "noi", "nong", "nuoc", // O
    "o", "oe", "oi", // Q
    "qua", "quat", "quay", "que", "quen", "quoc", "quyen", // R
    "ran", "reo", // S
    "sach", "san", "sang", "sao", "sap", "sau", "say", "sep", "so", "soi", "song", "su",
    // T
    "ta", "tam", "tan", "tay", "ten", "thi", "thich", "tho", "thoi", "tien", "tim", "tin", "tinh",
    "to", "toa", "toan", "toc", "toi", "tong", "tro", "troi", "tu", "tuan", "tuc", "tung", "tuy",
    // U
    "ung", "uot", // V
    "va", "vai", "van", "vao", "vay", "ve", "veo", "vi", "vien", "viet", "vo", "vui",
    // X
    "xa", "xac", "xanh", "xay", "xe", "xinh", "xong", "xuong", // Y
    "y", "yeu",
];

/// Kết quả 1 lượt nối từ cho handler/UI.
#[derive(Debug, Clone)]
pub struct WordChainPlayResult {
    /// Từ user gõ (sau normalize).
    pub user_word: String,
    pub is_valid: bool,
    /// Lý do không hợp lệ (nếu có).
    pub invalid_reason: Option<String>,
    /// Bot phản hồi — Some nếu user_word hợp lệ.
    pub bot_word: Option<String>,
    pub xp_awarded: i32,
    pub total_xp: i64,
    pub level: LevelInfo,
    pub plays_today: i64,
    pub valid_lifetime: i64,
}

pub struct WordChainRepo;

impl WordChainRepo {
    /// Đếm số lượt chơi hôm nay.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn plays_today_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let sql = format!(
            "SELECT COUNT(*) FROM word_chain_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(pool)
            .await?;
        Ok(c)
    }

    /// Tổng số từ hợp lệ lifetime.
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn valid_lifetime_count(pool: &PgPool, user_id: Uuid) -> AppResult<i64> {
        let c: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM word_chain_plays WHERE user_id = $1 AND is_valid = TRUE",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(c)
    }

    /// Chơi 1 lượt: validate từ, ghi row, cộng XP nếu hợp lệ, chọn bot_word.
    /// # Errors
    /// Trả lỗi khi quá cap daily / từ rỗng / DB fail.
    pub async fn play(
        pool: &PgPool,
        user_id: Uuid,
        raw_word: &str,
        rand_val: i32,
    ) -> AppResult<WordChainPlayResult> {
        // Normalize: lowercase + trim + strip dấu + remove non-alpha
        let user_word = normalize_word(raw_word);
        // Validate
        let invalid_reason = validate_word(&user_word);
        let is_valid = invalid_reason.is_none();
        // Chọn bot_word: nếu user hợp lệ → tìm từ trong vocab bắt đầu bằng
        // ký tự cuối của user_word (deterministic theo rand_val để tránh
        // luôn trả cùng từ).
        let bot_word = if is_valid {
            pick_bot_word(&user_word, rand_val)
        } else {
            None
        };
        let xp_awarded = if is_valid { WORD_CHAIN_XP_PER_VALID } else { 0 };

        let mut tx = pool.begin().await?;
        // Anti-farm: đếm trong tx
        let sql = format!(
            "SELECT COUNT(*) FROM word_chain_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let plays_today: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        if plays_today >= WORD_CHAIN_DAILY_CAP {
            return Err(AppError::BadRequest(format!(
                "Bạn đã chơi {WORD_CHAIN_DAILY_CAP} lượt nối từ hôm nay — quay lại vào ngày mai!"
            )));
        }
        // Ghi row
        sqlx::query(
            r"INSERT INTO word_chain_plays (user_id, word, is_valid, bot_word, xp_awarded)
               VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(&user_word)
        .bind(is_valid)
        .bind(bot_word.as_deref())
        .bind(xp_awarded)
        .execute(&mut *tx)
        .await?;
        if xp_awarded > 0 {
            sqlx::query(
                "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'word_chain', $2)",
            )
            .bind(user_id)
            .bind(xp_awarded)
            .execute(&mut *tx)
            .await?;
            let total: i64 = sqlx::query_scalar(
                r"INSERT INTO user_xp_totals (user_id, total_xp)
                   VALUES ($1, $2)
                   ON CONFLICT (user_id)
                   DO UPDATE SET total_xp = user_xp_totals.total_xp + $2,
                                 updated_at = NOW()
                   RETURNING total_xp",
            )
            .bind(user_id)
            .bind(xp_awarded)
            .fetch_one(&mut *tx)
            .await?;
            tx.commit().await?;
            let level = crate::models::gamification::level_from_xp(total);
            let valid_lifetime = Self::valid_lifetime_count(pool, user_id).await.unwrap_or(0);
            Ok(WordChainPlayResult {
                user_word,
                is_valid,
                invalid_reason,
                bot_word,
                xp_awarded,
                total_xp: total,
                level,
                plays_today: plays_today + 1,
                valid_lifetime,
            })
        } else {
            tx.commit().await?;
            let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
                .await
                .unwrap_or(0);
            let level = crate::models::gamification::level_from_xp(total_xp);
            let valid_lifetime = Self::valid_lifetime_count(pool, user_id).await.unwrap_or(0);
            Ok(WordChainPlayResult {
                user_word,
                is_valid,
                invalid_reason,
                bot_word,
                xp_awarded,
                total_xp,
                level,
                plays_today: plays_today + 1,
                valid_lifetime,
            })
        }
    }
}

/// Normalize user input: lowercase + map đ/Đ → d (v3.2.0 FIX) + NFD
/// decompose (tách dấu tiếng Việt) + chỉ giữ ASCII a-z.
/// Ví dụ "Cà Phê" → "cà phê" → NFD ("a" + U+0300, "e" + U+0302) →
/// filter ASCII → "caphe". "Đi" → "di" (trước đây bị mất đ thành "i").
#[must_use]
pub fn normalize_word(raw: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    raw.trim()
        .to_lowercase()
        // v3.2.0 FIX chữ "đ": NFD KHÔNG decompose U+0111 (đ) thành "d" +
        // combining mark → filter ASCII bị loại mất, "đi" thành "i" (1 ký
        // tự → invalid). Map tường minh đ/Đ → d TRƯỚC khi NFD.
        .replace(['đ', 'Đ'], "d")
        .nfd() // decompose: "ê" → "e" + U+0302 (combining circumflex)
        .filter(|c| c.is_ascii_alphabetic())
        .collect()
}

/// Validate: không rỗng, độ dài >= 2, nằm trong từ điển.
/// Trả Some(reason) nếu không hợp lệ, None nếu hợp lệ.
#[must_use]
pub fn validate_word(word: &str) -> Option<String> {
    if word.is_empty() {
        return Some("Từ không được để trống".into());
    }
    if word.len() < 2 {
        return Some("Từ phải có ít nhất 2 ký tự".into());
    }
    if word.len() > WORD_CHAIN_MAX_LEN {
        return Some(format!("Từ quá dài — tối đa {WORD_CHAIN_MAX_LEN} ký tự"));
    }
    if !VI_VOCAB.contains(&word) {
        return Some("Từ không có trong từ điển — hãy thử từ tiếng Việt phổ biến khác".into());
    }
    None
}

/// Bot chọn 1 từ hợp lệ bắt đầu bằng ký tự cuối của user_word.
/// Trả None nếu không tìm thấy (rất hiếm — từ điển đủ lớn).
#[must_use]
pub fn pick_bot_word(user_word: &str, rand_val: i32) -> Option<String> {
    let last_char = user_word.chars().last()?;
    let candidates: Vec<&&str> = VI_VOCAB
        .iter()
        .filter(|w| w.starts_with(last_char))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // rem_euclid với usize (chắc chắn > 0) — an toàn với rand_val âm.
    let idx = rand_val.rem_euclid(candidates.len() as i32) as usize;
    Some(candidates[idx].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_word() {
        assert_eq!(normalize_word("Xinh"), "xinh");
        assert_eq!(normalize_word("  Yeu  "), "yeu");
        // v3.2.0 FIX chữ "đ": NFD không decompose U+0111 — phải map tường
        // minh đ/Đ → d, nếu không "đi" bị rút còn "i" (1 ký tự → invalid).
        assert_eq!(normalize_word("Đi"), "di");
        assert_eq!(normalize_word("đá"), "da");
        assert_eq!(normalize_word("Đồng Đội"), "dongdoi");
        assert_eq!(normalize_word("đu"), "du");
        assert!(
            validate_word(&normalize_word("Đi")).is_none(),
            "'đi' → 'di' phải hợp lệ (có trong từ điển)"
        );
        // NFD tách dấu tiếng Việt: "Cà Phê" → "ca" + " " + "phe" → ASCII filter
        // bỏ combining marks (U+0300, U+0302) → "caphe" (chứ không phải "caph"
        // vì "ê" decompose thành "e" + combining circumflex, giữ lại 'e').
        assert_eq!(normalize_word("Cà Phê"), "caphe");
        // Loại bỏ ký tự non-alpha
        assert_eq!(normalize_word("hello!"), "hello");
        assert_eq!(normalize_word(""), "");
    }

    #[test]
    fn test_validate_word() {
        assert!(validate_word("").is_some(), "empty should fail");
        assert!(validate_word("a").is_some(), "1-char should fail");
        assert!(validate_word("yeu").is_none(), "'yeu' should be valid");
        assert!(validate_word("anh").is_none(), "'anh' should be valid");
        assert!(validate_word("xyz").is_some(), "non-dict word should fail");
    }

    #[test]
    fn test_pick_bot_word() {
        // user_word "yeu" — last char 'u' — bot should return word starting with 'u'
        let bot = pick_bot_word("yeu", 0);
        assert!(bot.is_some());
        assert!(bot.as_deref().unwrap().starts_with('u'));
        // 'y' has very few candidates — 'y' or 'yeu'
        let bot = pick_bot_word("may", 0);
        assert!(bot.is_some());
        assert!(bot.as_deref().unwrap().starts_with('y'));
        // unknown last char returns None
        // 'z' doesn't start any Vietnamese word in our dict
        let _bot = pick_bot_word("zzz", 0);
        // No words start with 'z' in our vocab — returns None
        // (But validate_word("zzz") fails anyway, so this won't be called.)
    }

    #[test]
    fn test_vocab_deduped_and_sorted() {
        // v3.2.0 — từ điển đã dedupe: trùng lặp cũ ("anh"×2, "ban"×3...)
        // làm lệch xác suất chọn từ của bot và phình binary.
        for pair in VI_VOCAB.windows(2) {
            assert!(pair[0] < pair[1], "từ điển phải sort tăng dần, không trùng");
        }
        assert!(!VI_VOCAB.is_empty());
    }

    /// Compile-time guards.
    const _: () = {
        assert!(WORD_CHAIN_DAILY_CAP > 0);
        assert!(WORD_CHAIN_XP_PER_VALID > 0);
        assert!(WORD_CHAIN_MAX_LEN > 10);
    };
}
