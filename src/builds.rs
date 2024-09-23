use anyhow::{bail, Context as _};
use futures_util::TryStreamExt;
use http::Uri;
use http_body_util::BodyStream;
use octocrab::Octocrab;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Build {
    pub run_id: octocrab::models::RunId,
    pub run_number: i64,
    pub commit: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub artifacts: Vec<BuildArtifact>,
}

impl Build {
    pub fn generate_version_name(&self) -> String {
        format!(
            "r{}-{}-{}",
            self.run_number,
            &self.commit[..7],
            self.date.format("%Y%m%d%H%M%S")
        )
    }
}

#[derive(Debug, Serialize)]
pub struct BuildArtifact {
    pub name: String,
    pub size: u64,
    pub artifact_id: octocrab::models::ArtifactId,
}

#[tracing::instrument(skip_all, fields(run_id = %run.id))]
async fn fetch_build_details(
    config: &crate::config::Builds,
    octocrab: &Octocrab,
    run: octocrab::models::workflows::Run,
) -> anyhow::Result<Build> {
    let artifacts = octocrab
        .actions()
        .list_workflow_run_artifacts(&config.owner, &config.repo, run.id)
        .send()
        .await?
        .value
        .unwrap()
        .into_stream(&octocrab)
        .try_collect::<Vec<_>>()
        .await
        .context("Getting artifacts for a workflow run")?;

    let artifacts = artifacts
        .into_iter()
        .filter(|a| a.name.starts_with(&config.artifact_prefix))
        .map(|a| BuildArtifact {
            name: a.name,
            size: a.size_in_bytes as u64,
            artifact_id: a.id,
        })
        .collect::<Vec<_>>();

    Ok(Build {
        run_id: run.id,
        run_number: run.run_number,
        commit: run.head_commit.id,
        date: run.created_at,
        artifacts,
    })
}

#[tracing::instrument(skip_all, fields(run_id))]
pub async fn fetch_build(
    config: &crate::config::Builds,
    octocrab: &Octocrab,
    run_id: octocrab::models::RunId,
) -> anyhow::Result<Build> {
    let run = octocrab
        .workflows(&config.owner, &config.repo)
        .get(run_id)
        .await
        .context("Getting a workflow run")?;

    fetch_build_details(config, octocrab, run).await
}

#[tracing::instrument(skip_all)]
pub async fn fetch_builds(
    config: &crate::config::Builds,
    octocrab: &Octocrab,
) -> anyhow::Result<Vec<Build>> {
    let runs = octocrab
        .workflows(&config.owner, &config.repo)
        .list_all_runs()
        .branch(&config.branch)
        .status("success")
        // .per_page(100)
        .send()
        .await
        .context("Listing repo workflow runs")?;

    let mut builds = Vec::new();

    for run in runs.items {
        // TODO: cache responses, github even provides an etag for us!
        // although the response probably shouldn't change at all, since we only look at complete workflow runs
        let build = fetch_build_details(config, octocrab, run).await?;

        builds.push(build);
    }

    Ok(builds)
}

#[tracing::instrument(skip_all)]
pub async fn fetch_latest_build(
    config: &crate::config::Builds,
    octocrab: &Octocrab,
) -> anyhow::Result<Build> {
    let runs = octocrab
        .workflows(&config.owner, &config.repo)
        .list_all_runs()
        .branch(&config.branch)
        .status("success")
        .per_page(1)
        .send()
        .await
        .context("Listing repo workflow runs")?;

    let Some(run) = runs.items.into_iter().next() else {
        bail!("No successful workflow runs found")
    };

    // TODO: cache responses, github even provides an etag for us!
    // although the response probably shouldn't change at all, since we only look at complete workflow runs
    let build = fetch_build_details(config, octocrab, run).await?;

    return Ok(build);
}

#[tracing::instrument(skip_all)]
pub async fn stream_build_artifact(
    config: &crate::config::Builds,
    octocrab: &Octocrab,
    artifact_id: octocrab::models::ArtifactId,
) -> anyhow::Result<impl futures_core::Stream<Item = octocrab::Result<bytes::Bytes>>> {
    let route = format!(
        "/repos/{owner}/{repo}/actions/artifacts/{artifact_id}/{archive_format}",
        owner = &config.owner,
        repo = &config.repo,
        artifact_id = artifact_id,
        archive_format = octocrab::params::actions::ArchiveFormat::Zip,
    );

    let uri = Uri::builder().path_and_query(route).build()?;

    let response = octocrab
        .follow_location_to_data(octocrab._get(uri).await?)
        .await?;

    Ok(BodyStream::new(response.into_body())
        .try_filter_map(|frame| futures_util::future::ok(frame.into_data().ok())))
}
