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
    /// Đếm số lượt chơi hôm nay (dùng được với cả pool và transaction —
    /// executor generic để gọi trong tx chống race vượt cap).
    /// # Errors
    /// Trả lỗi khi DB fail.
    pub async fn plays_today_count<'e, E>(executor: E, user_id: Uuid) -> AppResult<i64>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let sql = format!(
            "SELECT COUNT(*) FROM word_chain_plays
             WHERE user_id = $1 AND created_at >= {}",
            crate::utils::SQL_TODAY_START_VN
        );
        let c: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
            .bind(user_id)
            .fetch_one(executor)
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
    fn test_last_char_str() {
        // v3.3.0 — dùng cho luân phiên chữ nối PvP
        assert_eq!(last_char_str("xinh"), "h");
        assert_eq!(last_char_str("yeu"), "u");
        assert_eq!(last_char_str("di"), "i");
        assert_eq!(last_char_str(""), ""); // từ rỗng → chuỗi rỗng (validate chặn trước)
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

// ============================================================
// v3.3.0 — PvP MATCHMAKING: Nối từ đấu với NGƯỜI DÙNG NGẪU NHIÊN.
//
// Luộc luật chơi chuẩn Nối từ:
// - 2 người lần lượt nối từ: từ mới PHẢI bắt đầu bằng chữ cái cuối của
//   từ vừa nối, KHÔNG được tái sử dụng từ trong trận (words_used chặn
//   vòng lặp "anh"↔"hoa" vô hạn).
// - Nước đầu: đánh từ bất kỳ trong từ điển.
// - Hết 90s không đánh → THUA (thực thi server-side khi poll status —
//   client không tự quyết kết quả).
// - Từ không hợp lệ → KHÔNG thua ngay (chống gõ nháy mất trận), được
//   đánh lại trong thời gian còn lại; từ invalid vẫn ghi play row.
// - Không ghép được ai sau 120s → tự ghép GLM 5.3 (AI Agent mặc định).
//   GLM đánh NGAY trong request của bạn (không cần poll thêm).
// - Người thắng trận nhận +4 XP (reward match). Mỗi từ hợp lệ vẫn +3 XP
//   như bot mode. Daily cap 20 từ/ngày giữ nguyên.
// ============================================================

use chrono::{DateTime, Utc};

/// Thời gian chờ ghép người (giây) trước khi fallback AI.
pub const WORD_CHAIN_PVP_WAIT_SECS: i64 = 120;
/// Thời gian 1 nước đánh (giây) — quá = thua.
pub const WORD_CHAIN_MOVE_SECS: i64 = 90;
/// XP thưởng cho NGƯỜI THẮNG trận.
pub const WORD_CHAIN_PVP_XP_WIN: i32 = 4;
/// Số từ hiển thị trong "chuỗi nối" trên UI.
const WORDS_TAIL: usize = 10;

/// Hàng `word_chain_matches`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WordChainMatchRow {
    pub id: i64,
    pub player1_id: Uuid,
    pub player2_id: Option<Uuid>,
    pub status: String,
    pub winner_id: Option<Uuid>,
    pub turn_user_id: Option<Uuid>,
    pub current_letter: Option<String>,
    pub words_used: Vec<String>,
    pub move_deadline: Option<DateTime<Utc>>,
    pub is_ai_fallback: bool,
}

/// Đối thủ (cho UI).
#[derive(Debug, Clone)]
pub struct WcOpponent {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub is_ai: bool,
}

/// Trạng thái trận Nối từ cho handler render partial.
#[derive(Debug)]
pub enum WordChainPvpStatus {
    Waiting {
        match_id: i64,
        wait_secs: i64,
    },
    Active {
        match_id: i64,
        my_turn: bool,
        letter: Option<char>,
        words: Vec<String>,
        opponent: WcOpponent,
        is_ai: bool,
        deadline_secs: Option<i64>,
        plays_today: i64,
        valid_lifetime: i64,
        total_xp: i64,
        level: LevelInfo,
        notice: Option<String>,
    },
    Finished {
        match_id: i64,
        winner_is_me: bool,
        reason: String,
        words: Vec<String>,
        opponent: WcOpponent,
        is_ai: bool,
        total_xp: i64,
        level: LevelInfo,
        plays_today: i64,
        valid_lifetime: i64,
    },
    Cancelled,
}

impl WordChainRepo {
    /// POST /word-chain/match — join match chờ của người khác, hoặc tạo
    /// hàng chờ mới; nếu đang có trận active → trả lại trận (resume).
    ///
    /// # Errors
    ///
    /// Trả lỗi khi DB fail.
    pub async fn pvp_join_or_create(pool: &PgPool, user_id: Uuid) -> AppResult<WordChainPvpStatus> {
        // 1) Đang có trận active → resume.
        if let Some(m) = Self::my_active_match(pool, user_id).await? {
            return Self::build_active(pool, user_id, m, None).await;
        }

        // 2) Huỷ hàng chờ cũ của tôi.
        sqlx::query(
            "UPDATE word_chain_matches SET status = 'cancelled', updated_at = NOW()
             WHERE player1_id = $1 AND status = 'waiting'",
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        // 3) Join match waiting của người khác (FIFO, SKIP LOCKED).
        for _ in 0..3 {
            let candidate: Option<i64> = sqlx::query_scalar(
                r#"SELECT id FROM word_chain_matches
                   WHERE status = 'waiting'
                     AND player1_id <> $1
                     AND created_at > NOW() - make_interval(secs => $2)
                   ORDER BY created_at ASC
                   LIMIT 1
                   FOR UPDATE SKIP LOCKED"#,
            )
            .bind(user_id)
            .bind(WORD_CHAIN_PVP_WAIT_SECS)
            .fetch_optional(pool)
            .await?;
            let Some(match_id) = candidate else { break };
            let updated = sqlx::query(
                r#"UPDATE word_chain_matches SET player2_id = $1, status = 'active',
                       turn_user_id = player1_id, current_letter = NULL,
                       move_deadline = NOW() + make_interval(secs => $2), updated_at = NOW()
                   WHERE id = $3 AND status = 'waiting'"#,
            )
            .bind(user_id)
            .bind(WORD_CHAIN_MOVE_SECS)
            .bind(match_id)
            .execute(pool)
            .await?;
            if updated.rows_affected() == 0 {
                continue;
            }
            let m = Self::load_match(pool, match_id)
                .await?
                .ok_or_else(|| AppError::NotFound("Match biến mất sau khi join".into()))?;
            return Self::build_active(
                pool,
                user_id,
                m,
                Some("🎯 Ghép được đối thủ! Đối thủ đánh trước — bạn chờ nước đầu.".into()),
            )
            .await;
        }

        // 4) Tạo hàng chờ mới.
        let match_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO word_chain_matches (player1_id, status)
               VALUES ($1, 'waiting') RETURNING id"#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(WordChainPvpStatus::Waiting {
            match_id,
            wait_secs: WORD_CHAIN_PVP_WAIT_SECS,
        })
    }

    /// POST /word-chain/move — đánh 1 từ trong trận active.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi không có trận / chưa đến lượt / quá daily cap / DB fail.
    pub async fn pvp_move(
        pool: &PgPool,
        user_id: Uuid,
        raw_word: &str,
    ) -> AppResult<WordChainPvpStatus> {
        // v3.4.2 FIX (audit "FOR UPDATE ngoài tx"): trước đây SELECT FOR
        // UPDATE chạy autocommit → khoá row nhả ngay khi statement kết thúc,
        // 2 request move đồng thời cùng qua check turn → cùng UPDATE
        // words_used (1 từ bị mất) + cùng INSERT play +4 XP cho 1 lượt.
        // Giờ TOÀN BỘ đọc-kiểm-tra-ghi chạy trong 1 transaction, row lock
        // giữ đến COMMIT — move thứ 2 chờ move thứ 1 xong, thấy turn đã
        // đổi → trả "chưa đến lượt".
        let mut tx = pool.begin().await?;
        let Some(m) = Self::my_active_match_tx(&mut tx, user_id).await? else {
            tx.rollback().await?;
            return Err(AppError::BadRequest(
                "Bạn chưa có trận nào đang chạy — bấm \"Tìm đối thủ\" trước!".into(),
            ));
        };
        if m.turn_user_id != Some(user_id) {
            tx.rollback().await?;
            return Err(AppError::BadRequest("Chưa đến lượt của bạn!".into()));
        }

        // Daily cap (chống farm XP bằng bot gõ hộ) — đếm trong tx.
        let plays_today = Self::plays_today_count(&mut *tx, user_id).await?;
        if plays_today >= WORD_CHAIN_DAILY_CAP {
            tx.rollback().await?;
            return Err(AppError::BadRequest(format!(
                "Bạn đã nối {WORD_CHAIN_DAILY_CAP} lượt hôm nay — quay lại vào ngày mai!"
            )));
        }

        let opponent_id = if m.player1_id == user_id {
            m.player2_id
        } else {
            Some(m.player1_id)
        };
        let Some(opponent_id) = opponent_id else {
            tx.rollback().await?;
            return Err(AppError::BadRequest("Trận chưa đủ 2 người chơi".into()));
        };

        // Timeout: quá hạn mà vẫn nhấn (deadline đã qua khi request tới)
        // → thua. (Poll status thường xử lý trước khi tới đây.)
        if let Some(dl) = m.move_deadline {
            if Utc::now() > dl {
                Self::finish_by_timeout_tx(&mut tx, &m, user_id).await?;
                let match_id = m.id;
                tx.commit().await?;
                return Self::build_finished(
                    pool,
                    user_id,
                    match_id,
                    false,
                    format!("⏰ Hết {WORD_CHAIN_MOVE_SECS} giây không đánh được — đối thủ thắng!"),
                )
                .await;
            }
        }

        // Validate từ.
        let word = normalize_word(raw_word);
        let mut notice: Option<String> = None;
        let base_invalid = validate_word(&word);
        if let Some(reason) = base_invalid {
            notice = Some(reason);
        } else if let Some(letter) = &m.current_letter {
            if !word.starts_with(letter.as_str()) {
                notice = Some(format!(
                    "Từ phải bắt đầu bằng chữ \"{letter}\" (luật nối từ)"
                ));
            }
        }
        if notice.is_none() && m.words_used.iter().any(|w| w == &word) {
            notice = Some("Từ này đã dùng trong trận — chọn từ khác!".into());
        }

        if let Some(reason) = notice {
            // Ghi play invalid (giống bot mode — có lịch sử) KHÔNG trừ/thua.
            sqlx::query(
                r#"INSERT INTO word_chain_plays (user_id, word, is_valid, bot_word, xp_awarded)
                   VALUES ($1, $2, FALSE, NULL, 0)"#,
            )
            .bind(user_id)
            .bind(&word)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Self::build_active(pool, user_id, m, Some(reason)).await;
        }

        // === Nước đi HỢP LỆ ===
        let mut new_words = m.words_used.clone();
        new_words.push(word.clone());
        let new_letter = last_char_str(&word);
        // Guard điều kiện: chỉ thành công khi trận còn active + VẪN đến
        // lượt mình (turn là tôi ở thời điểm UPDATE chạy) — chống ghi đè
        // words_used khi đối thủ/AI đã đi nước khác giữa lúc đọc và ghi.
        let updated = sqlx::query(
            r#"UPDATE word_chain_matches SET words_used = $1, current_letter = $2,
                   turn_user_id = $3, move_deadline = NOW() + make_interval(secs => $4),
                   updated_at = NOW()
               WHERE id = $5 AND status = 'active' AND turn_user_id = $6"#,
        )
        .bind(&new_words)
        .bind(&new_letter)
        .bind(opponent_id)
        .bind(WORD_CHAIN_MOVE_SECS)
        .bind(m.id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            // Trạng thái trận đã đổi (timeout/finish/đối thủ đi trước do
            // race cũ) — hủy nước đi, đọc lại trận và báo người chơi.
            tx.rollback().await?;
            let fresh = Self::load_match(pool, m.id).await?.unwrap_or(m);
            return Self::build_active(
                pool,
                user_id,
                fresh,
                Some("Trạng thái trận vừa thay đổi — xem lại và đánh tiếp!".into()),
            )
            .await;
        }

        // XP cho từ hợp lệ + play row (cùng tx với UPDATE ở trên).
        sqlx::query(
            r#"INSERT INTO word_chain_plays (user_id, word, is_valid, bot_word, xp_awarded)
               VALUES ($1, $2, TRUE, NULL, $3)"#,
        )
        .bind(user_id)
        .bind(&word)
        .bind(WORD_CHAIN_XP_PER_VALID)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'word_chain', $2)",
        )
        .bind(user_id)
        .bind(WORD_CHAIN_XP_PER_VALID)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"INSERT INTO user_xp_totals (user_id, total_xp)
               VALUES ($1, $2)
               ON CONFLICT (user_id)
               DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
        )
        .bind(user_id)
        .bind(i64::from(WORD_CHAIN_XP_PER_VALID))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        // Đối thủ là AI? → AI đánh NGAY trong request này.
        let opp_is_ai = Self::is_ai_agent(pool, opponent_id).await?;
        if opp_is_ai {
            return Self::ai_move(pool, user_id, opponent_id, m.id, new_words).await;
        }

        let m2 = Self::load_match(pool, m.id).await?.unwrap_or(m);
        Self::build_active(
            pool,
            user_id,
            m2,
            Some(format!("✅ Đã nối \"{word}\" — chờ đối thủ...")),
        )
        .await
    }

    /// GET /word-chain/match/{id}/status — poll + thực thi timeout/fallback.
    ///
    /// # Errors
    ///
    /// Trả lỗi khi không thuộc match / DB fail.
    pub async fn pvp_status(
        pool: &PgPool,
        user_id: Uuid,
        match_id: i64,
    ) -> AppResult<WordChainPvpStatus> {
        let Some(m) = Self::load_match(pool, match_id).await? else {
            return Err(AppError::NotFound("Không tìm thấy match".into()));
        };
        if m.player1_id != user_id && m.player2_id != Some(user_id) {
            return Err(AppError::Forbidden("Bạn không thuộc match này".into()));
        }
        match m.status.as_str() {
            "finished" => {
                let winner_is_me = m.winner_id == Some(user_id);
                let reason = if winner_is_me {
                    "🎉 Bạn thắng!".into()
                } else {
                    "Bạn thua trận này — thử lại nhé!".into()
                };
                Self::build_finished(pool, user_id, m.id, winner_is_me, reason).await
            }
            "cancelled" => Ok(WordChainPvpStatus::Cancelled),
            "waiting" => {
                // Chỉ P1 poll được match waiting (P2 chưa tồn tại).
                if m.player1_id == user_id {
                    let age_secs: f64 = sqlx::query_scalar(
                        "SELECT EXTRACT(EPOCH FROM (NOW() - created_at))::float8
                         FROM word_chain_matches WHERE id = $1",
                    )
                    .bind(match_id)
                    .fetch_one(pool)
                    .await?;
                    if age_secs >= WORD_CHAIN_PVP_WAIT_SECS as f64 {
                        // Fallback AI: active với GLM 5.3, tôi đi trước.
                        // v3.4.2 FIX: guard `AND status = 'waiting'` — hai tab
                        // poll đồng thời đều chạy fallback, hoặc người chơi
                        // thật JOIN giữa lúc đọc và ghi → fallback ghi đè
                        // player2_id, đá người thật khỏi trận. Giờ chỉ 1
                        // request chuyển được status; request thua đọc lại.
                        let ai_id =
                            crate::repositories::AiAgentRepo::default_agent_user_id(pool).await?;
                        let updated = sqlx::query(
                            r#"UPDATE word_chain_matches SET player2_id = $1, status = 'active',
                                   turn_user_id = $2, current_letter = NULL,
                                   move_deadline = NOW() + make_interval(secs => $3),
                                   is_ai_fallback = TRUE, updated_at = NOW()
                               WHERE id = $4 AND status = 'waiting'"#,
                        )
                        .bind(ai_id)
                        .bind(user_id)
                        .bind(WORD_CHAIN_MOVE_SECS)
                        .bind(match_id)
                        .execute(pool)
                        .await?;
                        if updated.rows_affected() == 0 {
                            // Trận không còn waiting (đã có người join / tab
                            // khác đã fallback) — đọc lại trạng thái thật.
                            let fresh = Self::load_match(pool, match_id).await?.unwrap_or(m);
                            return match fresh.status.as_str() {
                                "active" => {
                                    Self::build_active(pool, user_id, fresh, None).await
                                }
                                "finished" => Self::build_finished(
                                    pool,
                                    user_id,
                                    fresh.id,
                                    fresh.winner_id == Some(user_id),
                                    "Trận đã kết thúc.".into(),
                                )
                                .await,
                                _ => Ok(WordChainPvpStatus::Cancelled),
                            };
                        }
                        let m2 = Self::load_match(pool, match_id).await?.unwrap_or(m);
                        return Self::build_active(
                            pool,
                            user_id,
                            m2,
                            Some(
                                "🤖 Không tìm được người chơi — GLM 5.3 vào sân! Bạn đánh trước."
                                    .into(),
                            ),
                        )
                        .await;
                    }
                    return Ok(WordChainPvpStatus::Waiting {
                        match_id,
                        wait_secs: WORD_CHAIN_PVP_WAIT_SECS - age_secs as i64,
                    });
                }
                Ok(WordChainPvpStatus::Cancelled)
            }
            _ => {
                // active — thực thi timeout nếu quá hạn (server-side).
                if let Some(turn) = m.turn_user_id {
                    if let Some(dl) = m.move_deadline {
                        if Utc::now() > dl {
                            let loser_is_me = turn == user_id;
                            let winner = if loser_is_me {
                                if m.player1_id == user_id {
                                    m.player2_id
                                } else {
                                    Some(m.player1_id)
                                }
                            } else {
                                Some(turn)
                            };
                            // v3.4.2 FIX (audit HIGH "timeout double-XP"):
                            // guard `AND status = 'active'` + check
                            // rows_affected — cả 2 người chơi poll mỗi 3s,
                            // khi deadline trễ cả 2 poll đều thấy "quá hạn"
                            // và cùng finish + cùng +4 XP cho winner. Giờ chỉ
                            // request nào chuyển được status mới được thưởng.
                            let mut tx = pool.begin().await?;
                            let finished = sqlx::query(
                                "UPDATE word_chain_matches SET status = 'finished', winner_id = $1, updated_at = NOW() WHERE id = $2 AND status = 'active'",
                            )
                            .bind(winner)
                            .bind(m.id)
                            .execute(&mut *tx)
                            .await?;
                            if finished.rows_affected() > 0 {
                                // +4 XP cho người thắng (nếu là người thật)
                                // và chỉ khi chưa chạm ngưỡng thắng/ngày
                                // (v3.4.2: timeout-win không ghi play row →
                                // cap theo play không ăn được — 2 tài khoản
                                // hợp tác để nhau timeout farm XP vô hạn.
                                // Giờ đếm trận THẮNG trong ngày, vượt cap →
                                // kết thúc trận nhưng không cộng XP).
                                if let Some(w) = winner {
                                    let ai = Self::is_ai_agent(pool, w).await?;
                                    if !ai {
                                        let wins_today: i64 = sqlx::query_scalar(
                                            r#"SELECT COUNT(*) FROM word_chain_matches
                                               WHERE winner_id = $1 AND status = 'finished'
                                                 AND updated_at >= date_trunc('day', NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh') AT TIME ZONE 'Asia/Ho_Chi_Minh'"#,
                                        )
                                        .bind(w)
                                        .fetch_one(&mut *tx)
                                        .await
                                        .unwrap_or(0);
                                        if wins_today <= WORD_CHAIN_DAILY_CAP {
                                            sqlx::query(
                                                "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'word_chain_win', $2)",
                                            )
                                            .bind(w)
                                            .bind(WORD_CHAIN_PVP_XP_WIN)
                                            .execute(&mut *tx)
                                            .await?;
                                            sqlx::query(
                                                r#"INSERT INTO user_xp_totals (user_id, total_xp)
                                                   VALUES ($1, $2)
                                                   ON CONFLICT (user_id)
                                                   DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
                                            )
                                            .bind(w)
                                            .bind(i64::from(WORD_CHAIN_PVP_XP_WIN))
                                            .execute(&mut *tx)
                                            .await?;
                                        }
                                    }
                                }
                            }
                            tx.commit().await?;
                            let reason = if loser_is_me {
                                format!("⏰ Hết {WORD_CHAIN_MOVE_SECS} giây không đánh — bạn thua!")
                            } else {
                                "⏰ Đối thủ hết giờ không đánh — BẠN THẮNG!".into()
                            };
                            return Self::build_finished(pool, user_id, m.id, !loser_is_me, reason)
                                .await;
                        }
                    }
                }
                Self::build_active(pool, user_id, m, None).await
            }
        }
    }

    /// Nước đi của AI (GLM 5.3) ngay trong request của người chơi.
    async fn ai_move(
        pool: &PgPool,
        me: Uuid,
        ai_id: Uuid,
        match_id: i64,
        mut words: Vec<String>,
    ) -> AppResult<WordChainPvpStatus> {
        let m = Self::load_match(pool, match_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Match không tồn tại".into()))?;
        let Some(letter) = m.current_letter.clone() else {
            return Self::build_active(pool, me, m, None).await;
        };
        let candidates: Vec<&str> = VI_VOCAB
            .iter()
            .copied()
            .filter(|w| w.starts_with(letter.as_str()) && !words.contains(&(*w).to_string()))
            .collect();
        if candidates.is_empty() {
            // AI hết từ → người chơi thắng.
            // v3.4.2: guard status='active' — chống double-finish + double XP
            // nếu 2 request chạy ai_move đè nhau.
            let mut tx = pool.begin().await?;
            let finished = sqlx::query(
                "UPDATE word_chain_matches SET status = 'finished', winner_id = $1, updated_at = NOW() WHERE id = $2 AND status = 'active'",
            )
            .bind(me)
            .bind(match_id)
            .execute(&mut *tx)
            .await?;
            if finished.rows_affected() > 0 {
                sqlx::query(
                    "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'word_chain_win', $2)",
                )
                .bind(me)
                .bind(WORD_CHAIN_PVP_XP_WIN)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    r#"INSERT INTO user_xp_totals (user_id, total_xp)
                       VALUES ($1, $2)
                       ON CONFLICT (user_id)
                       DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
                )
                .bind(me)
                .bind(i64::from(WORD_CHAIN_PVP_XP_WIN))
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            return Self::build_finished(
                pool,
                me,
                match_id,
                true,
                format!("🏆 GLM 5.3 không tìm được từ bắt đầu bằng \"{letter}\" — BẠN THẮNG!"),
            )
            .await;
        }
        let rand_val: i32 = {
            use rand::RngExt;
            rand::rng().random_range(0..1000)
        };
        let ai_word = candidates
            [usize::try_from(rand_val.rem_euclid(candidates.len() as i32)).unwrap_or(0)]
        .to_string();
        words.push(ai_word.clone());
        let new_letter = last_char_str(&ai_word);
        sqlx::query(
            r#"UPDATE word_chain_matches SET words_used = $1, current_letter = $2,
                   turn_user_id = $3, move_deadline = NOW() + make_interval(secs => $4),
                   updated_at = NOW()
               WHERE id = $5"#,
        )
        .bind(&words)
        .bind(&new_letter)
        .bind(me)
        .bind(WORD_CHAIN_MOVE_SECS)
        .bind(match_id)
        .execute(pool)
        .await?;
        // AI ghi play row hợp lệ (KHÔNG cộng XP — AI không cần XP, giữ
        // bảng xếp hạng sạch; AI cũng bị loại khỏi leaderboard sẵn).
        sqlx::query(
            r#"INSERT INTO word_chain_plays (user_id, word, is_valid, bot_word, xp_awarded)
               VALUES ($1, $2, TRUE, NULL, 0)"#,
        )
        .bind(ai_id)
        .bind(&ai_word)
        .execute(pool)
        .await?;

        let m2 = Self::load_match(pool, match_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Match không tồn tại".into()))?;
        Self::build_active(
            pool,
            me,
            m2,
            Some(format!("🤖 GLM 5.3 nối \"{ai_word}\" — tới lượt bạn!")),
        )
        .await
    }

    /// Kết thúc trận do người thua timeout tại request move của chính họ.
    /// v3.4.2: chạy trong transaction của pvp_move (row đang FOR UPDATE),
    /// guard `AND status = 'active'` + cap thắng/ngày (xem pvp_status).
    async fn finish_by_timeout_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        m: &WordChainMatchRow,
        loser: Uuid,
    ) -> AppResult<()> {
        let winner = if m.player1_id == loser {
            m.player2_id
        } else {
            Some(m.player1_id)
        };
        let finished = sqlx::query(
            "UPDATE word_chain_matches SET status = 'finished', winner_id = $1, updated_at = NOW() WHERE id = $2 AND status = 'active'",
        )
        .bind(winner)
        .bind(m.id)
        .execute(&mut **tx)
        .await?;
        if finished.rows_affected() == 0 {
            // Trận đã bị finish ở request khác — không thưởng lần nữa.
            return Ok(());
        }
        if let Some(w) = winner {
            let ai = Self::is_ai_agent(&mut **tx, w).await?;
            if !ai {
                // Cap thắng/ngày — chống farm timeout (đồng thuận với
                // pvp_status; cùng 1 luật kinh tế).
                let wins_today: i64 = sqlx::query_scalar(
                    r#"SELECT COUNT(*) FROM word_chain_matches
                       WHERE winner_id = $1 AND status = 'finished'
                         AND updated_at >= date_trunc('day', NOW() AT TIME ZONE 'Asia/Ho_Chi_Minh') AT TIME ZONE 'Asia/Ho_Chi_Minh'"#,
                )
                .bind(w)
                .fetch_one(&mut **tx)
                .await
                .unwrap_or(0);
                if wins_today <= WORD_CHAIN_DAILY_CAP {
                    sqlx::query(
                        "INSERT INTO xp_events (user_id, reason, amount) VALUES ($1, 'word_chain_win', $2)",
                    )
                    .bind(w)
                    .bind(WORD_CHAIN_PVP_XP_WIN)
                    .execute(&mut **tx)
                    .await?;
                    sqlx::query(
                        r#"INSERT INTO user_xp_totals (user_id, total_xp)
                           VALUES ($1, $2)
                           ON CONFLICT (user_id)
                           DO UPDATE SET total_xp = user_xp_totals.total_xp + $2, updated_at = NOW()"#,
                    )
                    .bind(w)
                    .bind(i64::from(WORD_CHAIN_PVP_XP_WIN))
                    .execute(&mut **tx)
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// Trận active của tôi — FOR UPDATE BÊN TRONG transaction của pvp_move
    /// (row lock giữ đến COMMIT; bản cũ chạy autocommit = khoá vô nghĩa).
    async fn my_active_match_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user_id: Uuid,
    ) -> AppResult<Option<WordChainMatchRow>> {
        sqlx::query_as::<_, WordChainMatchRow>(
            r#"SELECT id, player1_id, player2_id, status, winner_id, turn_user_id,
                      current_letter, words_used, move_deadline, is_ai_fallback
               FROM word_chain_matches
               WHERE status = 'active' AND (player1_id = $1 OR player2_id = $1)
               ORDER BY updated_at DESC
               LIMIT 1
               FOR UPDATE"#,
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(Into::into)
    }

    /// Trận active của tôi (đọc thường — cho pvp_join_or_create, đường
    /// join đã có UPDATE guard status nên không cần khoá ở bước đọc).
    async fn my_active_match(pool: &PgPool, user_id: Uuid) -> AppResult<Option<WordChainMatchRow>> {
        sqlx::query_as::<_, WordChainMatchRow>(
            r#"SELECT id, player1_id, player2_id, status, winner_id, turn_user_id,
                      current_letter, words_used, move_deadline, is_ai_fallback
               FROM word_chain_matches
               WHERE status = 'active' AND (player1_id = $1 OR player2_id = $1)
               ORDER BY updated_at DESC
               LIMIT 1"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
    }

    async fn load_match(pool: &PgPool, id: i64) -> AppResult<Option<WordChainMatchRow>> {
        sqlx::query_as::<_, WordChainMatchRow>(
            r#"SELECT id, player1_id, player2_id, status, winner_id, turn_user_id,
                      current_letter, words_used, move_deadline, is_ai_fallback
               FROM word_chain_matches WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(Into::into)
    }

    async fn is_ai_agent<'e, E>(executor: E, user_id: Uuid) -> AppResult<bool>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        let role: String = sqlx::query_scalar("SELECT role::text FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(executor)
            .await?
            .unwrap_or_else(|| "user".into());
        Ok(role == "ai_agent")
    }

    /// Build trạng thái Active (kèm stats + notice cho UI).
    #[allow(clippy::too_many_arguments)]
    async fn build_active(
        pool: &PgPool,
        user_id: Uuid,
        m: WordChainMatchRow,
        notice: Option<String>,
    ) -> AppResult<WordChainPvpStatus> {
        let Some(opp_id) = (if m.player1_id == user_id {
            m.player2_id
        } else {
            Some(m.player1_id)
        }) else {
            return Ok(WordChainPvpStatus::Waiting {
                match_id: m.id,
                wait_secs: WORD_CHAIN_PVP_WAIT_SECS,
            });
        };
        let opp_is_ai = Self::is_ai_agent(pool, opp_id).await?;
        let (username, display_name): (String, String) =
            sqlx::query_as("SELECT username, display_name FROM users WHERE id = $1")
                .bind(opp_id)
                .fetch_one(pool)
                .await?;
        let words: Vec<String> = m
            .words_used
            .iter()
            .rev()
            .take(WORDS_TAIL)
            .rev()
            .cloned()
            .collect();
        let deadline_secs = m.move_deadline.map(|dl| {
            (dl - Utc::now())
                .num_seconds()
                .clamp(0, WORD_CHAIN_MOVE_SECS)
        });
        let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
            .await
            .unwrap_or(0);
        let plays_today = Self::plays_today_count(pool, user_id).await.unwrap_or(0);
        let valid_lifetime = Self::valid_lifetime_count(pool, user_id).await.unwrap_or(0);
        Ok(WordChainPvpStatus::Active {
            match_id: m.id,
            my_turn: m.turn_user_id == Some(user_id),
            letter: m.current_letter.and_then(|l| l.chars().next()),
            words,
            opponent: WcOpponent {
                user_id: opp_id,
                username,
                display_name,
                is_ai: opp_is_ai,
            },
            is_ai: opp_is_ai,
            deadline_secs,
            plays_today,
            valid_lifetime,
            total_xp,
            level: crate::models::gamification::level_from_xp(total_xp),
            notice,
        })
    }

    async fn build_finished(
        pool: &PgPool,
        user_id: Uuid,
        match_id: i64,
        winner_is_me: bool,
        reason: String,
    ) -> AppResult<WordChainPvpStatus> {
        let m = Self::load_match(pool, match_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Match không tồn tại".into()))?;
        let opp_id = (if m.player1_id == user_id {
            m.player2_id
        } else {
            Some(m.player1_id)
        })
        .ok_or_else(|| AppError::NotFound("Match thiếu đối thủ".into()))?;
        let opp_is_ai = Self::is_ai_agent(pool, opp_id).await?;
        let (username, display_name): (String, String) =
            sqlx::query_as("SELECT username, display_name FROM users WHERE id = $1")
                .bind(opp_id)
                .fetch_one(pool)
                .await?;
        let words: Vec<String> = m
            .words_used
            .iter()
            .rev()
            .take(WORDS_TAIL)
            .rev()
            .cloned()
            .collect();
        let total_xp = crate::repositories::GamificationRepo::total_xp(pool, user_id)
            .await
            .unwrap_or(0);
        let plays_today = Self::plays_today_count(pool, user_id).await.unwrap_or(0);
        let valid_lifetime = Self::valid_lifetime_count(pool, user_id).await.unwrap_or(0);
        Ok(WordChainPvpStatus::Finished {
            match_id,
            winner_is_me,
            reason,
            words,
            opponent: WcOpponent {
                user_id: opp_id,
                username,
                display_name,
                is_ai: opp_is_ai,
            },
            is_ai: opp_is_ai,
            total_xp,
            level: crate::models::gamification::level_from_xp(total_xp),
            plays_today,
            valid_lifetime,
        })
    }
}

/// Ký tự cuối của từ (lowercase ASCII sau normalize) → String 1 ký tự.
#[must_use]
pub fn last_char_str(word: &str) -> String {
    word.chars()
        .last()
        .map(|c| c.to_string())
        .unwrap_or_default()
}
