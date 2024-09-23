use crate::builds::Build;
use anyhow::Context as _;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_tracing_opentelemetry::middleware::OtelAxumLayer;
use http::StatusCode;
use octocrab::Octocrab;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tower_http::catch_panic::CatchPanicLayer;
use tracing::info;

mod builds;
mod config;
mod init_tracing;

#[derive(Clone)]
struct AppState {
    config: config::Config,
    octocrab: Octocrab,
}

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

async fn handle_download(
    config: &config::Config,
    octocrab: &Octocrab,
    build: Build,
    artifact_name: &str,
) -> anyhow::Result<Response<axum::body::Body>> {
    let artifact = build
        .artifacts
        .iter()
        .find(|a| a.name == artifact_name)
        .ok_or_else(|| anyhow::anyhow!("Artifact not found"))?;

    let version_name = build.generate_version_name();
    let mut filename = artifact_name.replace(
        &config.builds.artifact_prefix,
        &format!(
            "{}preview_{}_",
            &config.builds.artifact_prefix, version_name
        ),
    );
    filename.push_str(".zip");

    info!("filename = {:?}", filename);

    let stream =
        builds::stream_build_artifact(&config.builds, &octocrab, artifact.artifact_id).await?;

    let body = axum::body::Body::from_stream(stream);

    let response = axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/zip")
        .header(
            http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        // the size reported here is not accurate to the size of the zip
        // .header(http::header::CONTENT_LENGTH, artifact.size)
        // cache indefinitely
        .header(http::header::CACHE_CONTROL, "public, max-age=31536000")
        .body(body)
        .unwrap();

    Ok(response)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing::init_tracing().context("Setting up the opentelemetry exporter")?;

    let environment = std::env::var("ENVIRONMENT").context(
        "Please set ENVIRONMENT env var (probably you want to use either 'prod' or 'dev')",
    )?;

    let config = config::Config::load(&environment).context("Loading config has failed")?;

    info!("config: {:?}", config);

    let listen_port = config.http.port;

    let octocrab = Octocrab::builder()
        .personal_token(config.github.token.clone())
        .build()
        .context("Building octocrab")?;

    let app = axum::Router::new()
        .route(
            "/api/builds",
            axum::routing::get(|State(AppState { config, octocrab })| async move {
                let builds = builds::fetch_builds(&config.builds, &octocrab).await?;

                Ok::<_, AppError>(Json(builds))
            }),
        )
        .route(
            "/api/builds/latest",
            axum::routing::get(|State(AppState { config, octocrab })| async move {
                let build = builds::fetch_latest_build(&config.builds, &octocrab).await?;

                info!("Latest build is {:?}", build);

                Ok::<_, AppError>(Json(build))
            }),
        )
        .route(
            "/api/download/:run_id/:artifact_name",
            axum::routing::get(
                |State(AppState { config, octocrab }),
                 Path((run_id, artifact_name)): Path<(octocrab::models::RunId, String)>| async move {
                    info!("Downloading artifact: {:?}/{}", run_id, artifact_name);

                    let build = builds::fetch_build(&config.builds, &octocrab, run_id).await?;

                    Ok::<_, AppError>(handle_download(&config, &octocrab, build, &artifact_name).await?)
                },
            ),
        )
        .route(
            "/api/download_latest/:artifact_name",
            axum::routing::get(
                |State(AppState { config, octocrab }),
                 Path(artifact_name): Path<String>| async move {
                    info!("Downloading latest artifact: {}", artifact_name);

                    let build = builds::fetch_latest_build(&config.builds, &octocrab).await?;

                    info!("Latest build is {:?}", build);

                    Ok::<_, AppError>(handle_download(&config, &octocrab, build, &artifact_name).await?)
                },
            ),
        )
        .layer(CatchPanicLayer::new())
        .layer(OtelAxumLayer::default())
        .with_state(AppState { config, octocrab });

    let listener = tokio::net::TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        listen_port,
    ))
    .await
    .context("Binding to the listen port")?;

    info!("Listening on http://0.0.0.0:{}", listen_port);
    axum::serve(listener, app)
        .await
        .context("Running the server")?;

    Ok(())
}
