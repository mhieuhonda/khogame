//! Dev tool: dump rendered HTML của docs/markdown_guide.md ra stdout
//! (dùng cho e2e test UI bằng browser — không chạy ở CI).
fn main() {
    let md = include_str!("../docs/markdown_guide.md");
    print!("{}", khogame::services::markdown::render(md));
}
