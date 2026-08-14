//! Frontend embutido no binário via `rust-embed`.
//!
//! Os assets do `ui/dist/` são compilados dentro do binário em build-time.
//! Em desenvolvimento (sem `ui/dist/`), o fallback retorna 404 para que
//! o Vite dev server (porta 5173) seja usado no lugar.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../ui/dist"]
struct UiAssets;

/// Serve um asset estático do bundle ou faz fallback para `index.html` (SPA).
///
/// Esta handler deve ser registrada como fallback do router Axum, para que:
/// - Arquivos concretos (JS, CSS, imagens) sejam servidos com MIME correto
/// - Qualquer rota desconhecida retorne `index.html` para o router do React
/// - Rotas da API (`/api/*`, `/healthz`, `/readyz`, `/metrics`) tenham prioridade
///   porque são registradas antes no router
pub async fn serve_ui(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Se o path exato existir no bundle, servir com MIME type correto
    if let Some(content) = UiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // Fallback SPA: tentar com `.html`
    let html_path = format!("{path}.html");
    if let Some(content) = UiAssets::get(&html_path) {
        let mime = mime_guess::from_path(&html_path).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.to_vec()))
            .unwrap();
    }

    // Fallback final: servir index.html para rotas do React Router
    match UiAssets::get("index.html") {
        Some(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(
                header::CACHE_CONTROL,
                "no-cache, no-store, must-revalidate",
            )
            .body(Body::from(content.data.to_vec()))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(
                "Frontend nao disponivel. Execute `cd ui && npm run build`.",
            ))
            .unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_assets_struct_exists() {
        // Verifica que o embed compila
        let _ = std::mem::size_of::<UiAssets>();
    }

    #[tokio::test]
    async fn test_serve_ui_returns_response() {
        let resp = serve_ui(Uri::from_static("/any/path")).await;
        let status = resp.into_response().status();
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::OK,
            "Expected 200 or 404, got {}",
            status
        );
    }
}
