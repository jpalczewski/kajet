#[macro_use]
extern crate rust_i18n;

i18n!("../../locales", fallback = "en");

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use axum_embed::ServeEmbed;
use kajet_core::types::AppState;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(RustEmbed, Clone)]
#[folder = "../../frontend/"]
struct Assets;

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub async fn serve(state: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let assets = ServeEmbed::<Assets>::new();

    let app = Router::new()
        .route("/api/search", get(api_search))
        .route("/api/i18n", get(api_i18n))
        .route("/ws", get(ws_handler))
        .fallback_service(assets)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// i18n API — returns dashboard translations as JSON
// ---------------------------------------------------------------------------

const DASHBOARD_KEYS: &[&str] = &[
    "title",
    "subtitle",
    "ws_connecting",
    "ws_connected",
    "ws_disconnected",
    "panel_query_log",
    "panel_stats",
    "panel_search",
    "waiting_for_queries",
    "stats_message",
    "search_placeholder",
    "searching",
    "dashboard_no_results",
    "results_suffix",
];

async fn api_i18n() -> Json<HashMap<String, String>> {
    let translations: HashMap<String, String> = DASHBOARD_KEYS
        .iter()
        .map(|&key| (key.to_string(), t!(key).to_string()))
        .collect();
    Json(translations)
}

// ---------------------------------------------------------------------------
// Search API (for the dashboard playground)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

async fn api_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    match state.engine.search(&params.q, params.limit).await {
        Ok(results) => {
            let html: String = results
                .iter()
                .map(|r| {
                    format!(
                        r#"<div class="chunk">
                            <div class="breadcrumb">{}</div>
                            <div class="content">{}</div>
                            <span class="score">{:.4}</span>
                        </div>"#,
                        html_escape(&r.breadcrumb),
                        html_escape(&r.content),
                        r.score,
                    )
                })
                .collect();

            Html(if html.is_empty() {
                format!(r#"<div class="empty">{}</div>"#, t!("dashboard_no_results"))
            } else {
                html
            })
        }
        Err(e) => Html(format!(r#"<div class="error">{}</div>"#, e)),
    }
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// WebSocket — live query event stream
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.events.subscribe();

    while let Ok(event) = rx.recv().await {
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_ampersand() {
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn html_escape_angle_brackets() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn html_escape_quotes() {
        assert_eq!(html_escape(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn html_escape_all_special_chars() {
        assert_eq!(
            html_escape(r#"<a href="x">&</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn html_escape_plain_text_unchanged() {
        assert_eq!(html_escape("hello world"), "hello world");
    }
}
