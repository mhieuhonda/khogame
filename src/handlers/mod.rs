pub mod admin;
pub mod ai_agent;
pub mod api;
pub mod arcade;
pub mod auth;
pub mod chat;
pub mod collections;
pub mod comments;
pub mod feedback;
pub mod games;
pub mod gamification;
pub mod interactions;
pub mod news;
pub mod notifications;
pub mod pages;
pub mod prefs;
pub mod profile;
pub mod quests;
pub mod referral;
pub mod repos;
pub mod reviews;
pub mod rps;
pub mod shop;
pub mod uploads;
pub mod word_chain;

/// v3.4.0 — Arcade (Oẳn tù tì + Nối từ) tạm dừng chờ Hieu Louis xem xét.
///
/// Khi `true`:
/// - GET /rps + /word-chain render trang "tính năng đang được xem xét"
///   (thay vì UI chơi).
/// - POST /rps/play, /word-chain/match, /word-chain/move trả lỗi thân thiện
///   (chặn chơi trực tiếp qua HTMX/curl).
///
/// Muốn bật lại game: đổi `false` + deploy (1 dòng, không migration).
pub const ARCADE_UNDER_REVIEW: bool = true;
