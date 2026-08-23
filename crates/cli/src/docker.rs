use anyhow::{Context, Result};
use bollard::{Docker, query_parameters::CreateImageOptionsBuilder};
use futures_util::StreamExt;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullProgress {
    pub image: String,
    pub current: u64,
    pub total: u64,
    pub status: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LayerProgress {
    current: u64,
    total: u64,
}

fn aggregate_layers(layers: &BTreeMap<String, LayerProgress>) -> (u64, u64) {
    layers.values().fold((0, 0), |(current, total), layer| {
        if layer.total == 0 {
            (current, total)
        } else {
            (
                current.saturating_add(layer.current.min(layer.total)),
                total.saturating_add(layer.total),
            )
        }
    })
}

async fn pull_image_with_client<F>(docker: &Docker, image: &str, mut report: F) -> Result<()>
where
    F: FnMut(PullProgress),
{
    let options = CreateImageOptionsBuilder::default()
        .from_image(image)
        .build();
    let mut stream = docker.create_image(Some(options), None, None);
    let mut layers = BTreeMap::<String, LayerProgress>::new();
    let mut last_status = String::from("Preparing");

    while let Some(event) = stream.next().await {
        let event = event.with_context(|| format!("pull Docker image {image}"))?;
        if let Some(error) = event.error.filter(|error| !error.trim().is_empty()) {
            anyhow::bail!("Docker failed to pull {image}: {error}");
        }

        if let Some(status) = event.status.filter(|status| !status.trim().is_empty()) {
            last_status = status;
        }

        if let (Some(layer), Some(detail)) = (event.id, event.progress_detail) {
            let current = detail.current.unwrap_or_default().max(0) as u64;
            let total = detail.total.unwrap_or_default().max(0) as u64;
            if current > 0 || total > 0 {
                layers.insert(layer, LayerProgress { current, total });
            }
        }

        let (current, total) = aggregate_layers(&layers);
        report(PullProgress {
            image: image.to_string(),
            current,
            total,
            status: last_status.clone(),
        });
    }

    report(PullProgress {
        image: image.to_string(),
        current: 1,
        total: 1,
        status: String::from("Complete"),
    });
    Ok(())
}

pub async fn pull_image<F>(image: &str, report: F) -> Result<()>
where
    F: FnMut(PullProgress),
{
    let docker = Docker::connect_with_local_defaults().context("connect to local Docker daemon")?;
    pull_image_with_client(&docker, image, report).await
}

pub async fn pull_images<F>(images: &[String], mut report: F) -> Result<()>
where
    F: FnMut(PullProgress),
{
    let docker = Docker::connect_with_local_defaults().context("connect to local Docker daemon")?;
    let mut seen = BTreeSet::new();
    for image in images {
        if !seen.insert(image.clone()) {
            continue;
        }
        pull_image_with_client(&docker, image, &mut report).await?;
    }
    Ok(())
}

pub fn pull_images_blocking<F>(images: &[String], report: F) -> Result<()>
where
    F: FnMut(PullProgress),
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create Docker pull runtime")?;
    runtime.block_on(pull_images(images, report))
}

pub fn short_image_name(image: &str) -> &str {
    image.rsplit('/').next().unwrap_or(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_progress_is_summed_and_clamped() {
        let layers = BTreeMap::from([
            (
                "a".to_string(),
                LayerProgress {
                    current: 50,
                    total: 100,
                },
            ),
            (
                "b".to_string(),
                LayerProgress {
                    current: 90,
                    total: 80,
                },
            ),
            (
                "unknown".to_string(),
                LayerProgress {
                    current: 10,
                    total: 0,
                },
            ),
        ]);
        assert_eq!(aggregate_layers(&layers), (130, 180));
    }

    #[test]
    fn short_image_name_keeps_repository_and_tag() {
        assert_eq!(
            short_image_name("ghcr.io/noah-bozkurt/argus-control-api:abc123"),
            "argus-control-api:abc123"
        );
    }
}
