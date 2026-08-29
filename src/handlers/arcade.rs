//! v3.0.0 — Handlers arcade: vòng quay may mắn (/spin) + câu đố
//! hằng ngày (/trivia).

use crate::error::{AppError, AppResult};
use crate::middleware::{AuthUser, CurrentUser};
use crate::repositories::{SpinRepo, TriviaRepo};
use crate::state::AppState;
use crate::templates::{SpinTemplate, TriviaTemplate};
use axum::extract::State;
use serde::Deserialize;
use std::sync::Arc;

// ============================================================
// LUCKY SPIN
// ============================================================

/// GET /spin — trang vòng quay (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn spin_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<SpinTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let today_prize = SpinRepo::today_spin(&state.db, user.id).await?;
    let level = crate::repositories::GamificationRepo::level_of(&state.db, user.id)
        .await
        .unwrap_or_else(|_| crate::models::gamification::level_from_xp(0));
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(SpinTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        today_prize,
        level,
    })
}

/// POST /spin — quay 1 lượt (HTMX). Trả partial kết quả.
/// # Errors
/// Trả lỗi khi đã quay / DB fail.
pub async fn do_spin(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
) -> AppResult<axum::response::Html<String>> {
    // Trọng số 0..1000 — rand thống nhất với bảng SpinPrize
    use rand::RngExt;
    let rand_val: i32 = rand::rng().random_range(0..1000);
    let (xp, total, level) = SpinRepo::spin(&state.db, user.id, rand_val).await?;
    let tier = crate::models::retention::SpinPrize::pick(rand_val).tier;
    let jackpot = tier == "legendary";
    // Huy hiệu level — best-effort
    let db = state.db.clone();
    let uid = user.id;
    tokio::spawn(async move {
        crate::services::gamification::check_achievements(&db, uid).await;
    });
    let celebrate = if jackpot {
        "spin-result jackpot celebrate"
    } else {
        "spin-result"
    };
    Ok(axum::response::Html(format!(
        "<div class='{celebrate}' data-xp-toast=\"+{xp} XP\">\
           <div class='spin-prize-xp'>+{xp} XP</div>\
           <p class='spin-prize-total'>Tổng XP: <strong>{total}</strong> · Cấp {} — {}</p>\
         </div>",
        level.level, level.title
    )))
}

// ============================================================
// DAILY TRIVIA
// ============================================================

#[derive(Debug, Deserialize)]
pub struct TriviaAnswerForm {
    pub question_id: i32,
    pub answer_index: i32,
}

/// GET /trivia — trang câu đố hằng ngày (yêu cầu đăng nhập).
/// # Errors
/// Trả lỗi khi DB fail.
pub async fn trivia_page(
    State(state): State<Arc<AppState>>,
    CurrentUser(current_user): CurrentUser,
) -> AppResult<TriviaTemplate> {
    let Some(user) = current_user else {
        return Err(AppError::Unauthorized);
    };
    let questions = TriviaRepo::today_questions(&state.db, user.id).await?;
    let correct_today = TriviaRepo::correct_today_count(&state.db, user.id).await?;
    let unread = crate::handlers::auth::unread_count(&state, user.id).await;
    Ok(TriviaTemplate {
        current_user: Some(user),
        unread_notifications: unread,
        questions,
        correct_today,
    })
}

/// POST /trivia/answer — trả lời 1 câu (HTMX). Trả partial đúng/sai +
/// mở khóa đáp án. Trả partial có data-correct để client đếm.
/// # Errors
/// Trả lỗi khi trả lời không hợp lệ / DB fail.
pub async fn answer_trivia(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    axum::extract::Form(form): axum::extract::Form<TriviaAnswerForm>,
) -> AppResult<axum::response::Html<String>> {
    let result =
        TriviaRepo::answer(&state.db, user.id, form.question_id, form.answer_index).await?;
    // Bonus cả 3 câu đúng — thử mỗi lần (idempotent trong repo)
    let bonus = TriviaRepo::maybe_award_all_bonus(&state.db, user.id).await?;
    let status = if result.is_correct {
        "correct"
    } else {
        "wrong"
    };
    let xp_note = if result.xp_awarded > 0 {
        format!(" +{} XP", result.xp_awarded)
    } else {
        String::new()
    };
    let bonus_note = if bonus > 0 {
        format!("<div class='trivia-bonus'>🎉 Hoàn thành cả 3 câu! Thưởng thêm +{bonus} XP</div>")
    } else {
        String::new()
    };
    // Render lại các lựa chọn: đúng tô xanh, lựa chọn của user tô đỏ nếu sai
    let mut choices = String::new();
    if let Some(arr) = result.options.as_array() {
        for (i, opt) in arr.iter().enumerate() {
            let text = opt.as_str().unwrap_or_default();
            let cls = if i as i32 == result.correct_index {
                "trivia-choice correct"
            } else if i as i32 == form.answer_index {
                "trivia-choice wrong"
            } else {
                "trivia-choice dim"
            };
            let mark = if i as i32 == result.correct_index {
                " ✓"
            } else if i as i32 == form.answer_index {
                " ✗"
            } else {
                ""
            };
            choices.push_str(&format!(
                "<div class='{cls}'>{}. {}{mark}</div>",
                i + 1,
                crate::utils::html_escape(text)
            ));
        }
    }
    Ok(axum::response::Html(format!(
        "<div class='trivia-feedback {status}' data-correct='{}' data-xp-toast=\"{xp_note}\">\
           <strong>{}</strong>{}\
           <div class='trivia-choices-review'>{choices}</div>\
           <p class='trivia-explanation'>{}</p>\
         </div>{bonus_note}",
        result.is_correct,
        if result.is_correct {
            "✅ Chính xác!"
        } else {
            "❌ Chưa đúng"
        },
        xp_note,
        crate::utils::html_escape(&result.explanation)
    )))
}
