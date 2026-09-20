use std::path::Path;

use axum::{
	body::Body,
	extract::State,
	http::{header, HeaderMap, HeaderValue, Request},
	response::IntoResponse,
	response::Response,
	routing::get,
	Router,
};
use tower::ServiceBuilder;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::{
	config::state::AppState,
	errors::{APIError, APIResult},
};

pub const FAVICON: &str = "/favicon.ico";
const SW: &str = "/sw.js";
/// Web app manifest emitted by vite-plugin-pwa at the dist root. Without an explicit
/// route it fell through to the SPA fallback and came back as index.html, so iOS
/// could not read the app name/icons when adding the site to the home screen.
const MANIFEST: &str = "/manifest.webmanifest";
const INDEX: &str = "/";
const INDEX_HTML: &str = "/index.html";
const ASSETS: &str = "/assets";
const DIST: &str = "/dist";

/// NoirPanther: the web app shipped without a single security header. Book descriptions are
/// rendered as HTML (they come from files downloaded off the internet), the app could be framed
/// by any site, and responses were open to content-type sniffing. These headers close the parts
/// the markup sanitizer cannot: no framing, no off-site form posts, no third-party images or
/// frames, no plugins. `unsafe-inline` stays because index.html boots with an inline script and
/// the styles are injected at runtime — script injection is already prevented in the renderer.
fn security_headers() -> tower::layer::util::Stack<
	SetResponseHeaderLayer<HeaderValue>,
	tower::layer::util::Stack<
		SetResponseHeaderLayer<HeaderValue>,
		tower::layer::util::Stack<
			SetResponseHeaderLayer<HeaderValue>,
			tower::layer::util::Identity,
		>,
	>,
> {
	const CSP: &str = "default-src 'self'; \
		script-src 'self' 'unsafe-inline' 'unsafe-eval' blob:; \
		style-src 'self' 'unsafe-inline'; \
		img-src 'self' data: blob:; \
		font-src 'self' data:; \
		media-src 'self' data: blob:; \
		connect-src 'self' ws: wss:; \
		worker-src 'self' blob:; \
		frame-src 'none'; object-src 'none'; base-uri 'self'; form-action 'self'; \
		frame-ancestors 'none'";

	ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::CONTENT_SECURITY_POLICY,
			HeaderValue::from_static(CSP),
		))
		.layer(SetResponseHeaderLayer::if_not_present(
			header::X_CONTENT_TYPE_OPTIONS,
			HeaderValue::from_static("nosniff"),
		))
		.layer(SetResponseHeaderLayer::if_not_present(
			header::REFERRER_POLICY,
			HeaderValue::from_static("strict-origin-when-cross-origin"),
		))
		.into_inner()
}

pub(crate) fn mount(app_state: AppState) -> Router<AppState> {
	let dist_path = Path::new(&app_state.config.client_dir);
	let static_assets = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::VARY,
			HeaderValue::from_static("Accept-Encoding"),
		))
		.layer(SetResponseHeaderLayer::overriding(
			header::CACHE_CONTROL,
			HeaderValue::from_static("public, max-age=31536000, immutable, no-transform"),
		))
		.service(
			ServeDir::new(dist_path.join("assets"))
				.precompressed_br()
				.precompressed_gzip(),
		);

	let dist_files = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::VARY,
			HeaderValue::from_static("Accept-Encoding"),
		))
		.layer(SetResponseHeaderLayer::if_not_present(
			header::CACHE_CONTROL,
			HeaderValue::from_static("no-cache"),
		))
		.service(
			ServeDir::new(dist_path)
				.precompressed_br()
				.precompressed_gzip(),
		);

	let spa_fallback = ServiceBuilder::new()
		.layer(SetResponseHeaderLayer::if_not_present(
			header::CACHE_CONTROL,
			HeaderValue::from_static("no-cache"),
		))
		.service(ServeFile::new(dist_path.join("index.html")));

	Router::new()
		.route(INDEX, get(index_html))
		.route(INDEX_HTML, get(index_html))
		.route(FAVICON, get(favicon))
		.route(SW, get(serve_sw))
		.route(MANIFEST, get(serve_manifest))
		.nest_service(ASSETS, static_assets)
		.nest_service(DIST, dist_files)
		.fallback_service(spa_fallback)
		// applied last so it covers every route above, including the SPA fallback
		.layer(security_headers())
}

pub(crate) fn relative_favicon_path() -> String {
	format!("{ASSETS}{FAVICON}")
}

// https://github.com/tokio-rs/axum/discussions/608#discussioncomment-7772294
async fn favicon(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	let mut response = serve_dist_file(ctx, headers, "favicon.ico").await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("public, max-age=86400"),
	);

	Ok(response)
}

async fn index_html(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	serve_with_no_cache(ctx, headers, "index.html").await
}

async fn serve_sw(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	serve_with_no_cache(ctx, headers, "sw.js").await
}

async fn serve_manifest(
	State(ctx): State<AppState>,
	headers: HeaderMap,
) -> APIResult<impl IntoResponse> {
	let mut response = serve_with_no_cache(ctx, headers, "manifest.webmanifest").await?;
	response.headers_mut().insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("application/manifest+json"),
	);
	Ok(response)
}

async fn serve_with_no_cache(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	let mut response = serve_dist_file(ctx, headers, path).await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("no-cache, no-store, must-revalidate"),
	);

	Ok(response)
}

async fn serve_with_no_store(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	// Manifest and bootstrap must bypass caches.
	let mut response = serve_dist_file(ctx, headers, path).await?;
	response.headers_mut().insert(
		header::CACHE_CONTROL,
		HeaderValue::from_static("no-cache, no-store, must-revalidate"),
	);

	Ok(response)
}

async fn serve_dist_file(
	ctx: AppState,
	headers: HeaderMap,
	path: &str,
) -> APIResult<Response> {
	let mut req = Request::new(Body::empty());
	*req.headers_mut() = headers;

	match ServeFile::new(Path::new(&ctx.config.client_dir).join(path))
		.try_call(req)
		.await
	{
		Ok(res) => Ok(res.into_response()),
		Err(e) => {
			tracing::error!(error = ?e, path, "Error serving dist file");
			Err(APIError::InternalServerError(e.to_string()))
		},
	}
}
