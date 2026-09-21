use std::sync::Arc;

use tokio::net::TcpStream;

use crate::{sse, write_simple, AppState};

// 静态资源（嵌入二进制）
const INDEX_HTML: &str = include_str!("../static/index.html");
const STYLE_CSS: &str = include_str!("../static/style.css");
const APP_JS: &str = include_str!("../static/app.js");

pub async fn dispatch(
    stream: &mut TcpStream,
    path: &str,
    state: Arc<AppState>,
) -> std::io::Result<()> {
    let path = path.split('?').next().unwrap_or("/");
    match path {
        "/" | "/index.html" => {
            write_simple(stream, 200, "text/html; charset=utf-8", INDEX_HTML).await
        },
        "/style.css" => write_simple(stream, 200, "text/css; charset=utf-8", STYLE_CSS).await,
        "/app.js" => {
            write_simple(stream, 200, "application/javascript; charset=utf-8", APP_JS).await
        },
        "/api/health" => {
            write_simple(
                stream,
                200,
                "application/json; charset=utf-8",
                r#"{"ok":true,"service":"aicore-dashboard-web"}"#,
            )
            .await
        },
        "/api/snapshot" => {
            let snap = state.client.fetch().await;
            let json = serde_json::to_string(&snap)
                .unwrap_or_else(|_| r#"{"errors":["serialize failed"]}"#.to_string());
            write_simple(stream, 200, "application/json; charset=utf-8", &json).await
        },
        "/api/events" => sse::stream(stream, state).await,
        _ => write_simple(stream, 404, "text/plain", "not found").await,
    }
}
