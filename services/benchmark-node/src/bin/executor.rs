use anyhow::{Context, Result, bail};
use benchmark_node::{MAX_NDJSON_BYTES, config::ExecutorConfig, ndjson, random_token};
use benchmark_protocol::{
    ExecutorEvent, ExecutorRequest, KaggleResultRequest, bedrock_model_supports_images,
    is_bedrock_model, is_builtin_harness,
};
use chrono::Utc;
use serde_json::json;
use std::{os::unix::fs::PermissionsExt, path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, BufReader},
    net::{UnixListener, UnixStream},
    process::{Child, Command},
    sync::Semaphore,
};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(ExecutorConfig::load()?);
    verify_host(&config).await?;
    let parent = config
        .socket
        .parent()
        .context("BENCHMARK_EXECUTOR_SOCKET needs a parent")?;
    tokio::fs::create_dir_all(parent)
        .await
        .context("create executor socket directory")?;
    // The executor owns this directory; the controller's shared group may traverse and
    // connect, but cannot replace the socket. setgid gives the socket that shared group.
    tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o2750))
        .await
        .context("protect executor socket directory")?;
    if tokio::fs::try_exists(&config.socket).await.unwrap_or(false) {
        tokio::fs::remove_file(&config.socket)
            .await
            .context("remove stale executor socket")?;
    }
    let listener = UnixListener::bind(&config.socket).context("bind executor Unix socket")?;
    tokio::fs::set_permissions(&config.socket, std::fs::Permissions::from_mode(0o660))
        .await
        .context("protect executor socket")?;
    let slot = Arc::new(Semaphore::new(1));
    loop {
        let (stream, _) = listener.accept().await.context("accept controller")?;
        let config = config.clone();
        let slot = slot.clone();
        tokio::spawn(async move {
            let Ok(permit) = slot.try_acquire_owned() else {
                let (_, mut writer) = stream.into_split();
                let _ = ndjson::write(
                    &mut writer,
                    &ExecutorEvent::Log {
                        level: "error".into(),
                        message: "executor is already running a job".into(),
                    },
                    MAX_NDJSON_BYTES,
                )
                .await;
                return;
            };
            if let Err(error) = handle(stream, &config).await {
                eprintln!("executor connection failed: {error}");
            }
            drop(permit);
        });
    }
}

async fn handle(stream: UnixStream, config: &ExecutorConfig) -> Result<()> {
    if let Some(expected) = config.controller_uid {
        let actual = stream
            .peer_cred()
            .context("read controller peer credentials")?
            .uid();
        if actual != expected {
            bail!("rejected controller uid {actual}");
        }
    }
    let (read, mut write) = stream.into_split();
    let mut control = BufReader::new(read);
    let Some(request) = ndjson::read::<ExecutorRequest, _>(&mut control, MAX_NDJSON_BYTES).await?
    else {
        return Ok(());
    };
    match request {
        ExecutorRequest::Ping => {
            ndjson::write(&mut write, &ExecutorEvent::Pong, MAX_NDJSON_BYTES).await
        }
        ExecutorRequest::Cancel => Ok(()),
        request @ (ExecutorRequest::Run { .. } | ExecutorRequest::Kaggle { .. }) => {
            run_adapter(request, control, write, config).await
        }
    }
}

async fn run_adapter(
    request: ExecutorRequest,
    mut control: BufReader<tokio::net::unix::OwnedReadHalf>,
    mut output: tokio::net::unix::OwnedWriteHalf,
    config: &ExecutorConfig,
) -> Result<()> {
    validate_request(&request)?;
    let run_id = match &request {
        ExecutorRequest::Run { run_id, .. } => *run_id,
        ExecutorRequest::Kaggle { claim } => claim.run_id,
        _ => unreachable!(),
    };
    let runs = config.state_directory.join("runs");
    tokio::fs::create_dir_all(&runs)
        .await
        .context("create runs directory")?;
    let work = runs.join(format!("{}-{}", run_id, &random_token()[..12]));
    tokio::fs::create_dir(&work)
        .await
        .context("create run work directory")?;
    tokio::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700))
        .await
        .context("protect run work directory")?;

    let mode = match &request {
        ExecutorRequest::Run { .. } => "--executor-ndjson",
        ExecutorRequest::Kaggle { .. } => "--executor-kaggle-ndjson",
        _ => unreachable!(),
    };
    let mut child = Command::new(&config.python);
    child
        .arg(&config.adapter)
        .arg(mode)
        .current_dir(&work)
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("HOME", config.state_directory.join("executor"))
        .env("PYTHONUNBUFFERED", "1")
        .env("HARNESS_ENV", "executor")
        .env(
            "HARNESS_CACHE_DIRECTORY",
            config.state_directory.join("cache"),
        )
        .env("HARNESS_IMAGE", &config.sandbox_image)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = child.spawn().context("start Python benchmark adapter")?;
    let mut child_input = child.stdin.take().context("adapter stdin")?;
    ndjson::write(&mut child_input, &request, MAX_NDJSON_BYTES).await?;
    drop(child_input);
    let child_output = child.stdout.take().context("adapter stdout")?;
    let child_error = child.stderr.take().context("adapter stderr")?;
    let mut messages = BufReader::new(child_output);
    let error_task = tokio::spawn(capture_stderr(child_error));
    let timeout = request_timeout(&request);
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            message = ndjson::read::<ExecutorEvent, _>(&mut messages, MAX_NDJSON_BYTES) => {
                match message {
                    Ok(Some(event)) => {
                        let terminal = matches!(event, ExecutorEvent::Result { .. } | ExecutorEvent::KaggleResult { .. });
                        ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await?;
                        if terminal {
                            finish_child(&mut child).await;
                            return Ok(());
                        }
                    }
                    Ok(None) => {
                        let status = child.wait().await.context("wait for adapter")?;
                        let tail = error_task.await.unwrap_or_default();
                        let message = format!(
                            "Python benchmark adapter exited with {status}: {}",
                            redact(&tail, &request)
                        );
                        let event = crash_event(&request, &message);
                        ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await?;
                        return Ok(());
                    }
                    Err(error) => {
                        let _ = child.kill().await;
                        let event = crash_event(&request, &format!("invalid adapter message: {error}"));
                        ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await?;
                        return Ok(());
                    }
                }
            }
            control_message = ndjson::read::<ExecutorRequest, _>(&mut control, 1024) => {
                match control_message {
                    Ok(Some(ExecutorRequest::Cancel)) | Ok(None) => {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        return Ok(());
                    }
                    _ => {
                        let _ = child.kill().await;
                        bail!("controller sent an invalid in-flight command");
                    }
                }
            }
            _ = &mut deadline => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let event = crash_event(&request, "restricted executor deadline expired");
                ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await?;
                return Ok(());
            }
        }
    }
}

fn validate_request(request: &ExecutorRequest) -> Result<()> {
    match request {
        ExecutorRequest::Run {
            repo_url,
            deadline_at,
            gateway_socket,
            gateway_token,
            model_profile,
            ..
        } => {
            valid_submission_source(repo_url)?;
            if *deadline_at <= Utc::now()
                || gateway_token.len() != 64
                || !valid_submission_model(repo_url, model_profile)
            {
                bail!("invalid local-run capability or deadline");
            }
            let socket = Path::new(gateway_socket);
            if !socket.is_absolute()
                || socket.file_name().and_then(|name| name.to_str()) != Some("bedrock.sock")
            {
                bail!("invalid model gateway socket path");
            }
        }
        ExecutorRequest::Kaggle { claim } => {
            valid_repo_url(&claim.repo_url)?;
            if claim.benchmark_version != benchmark_protocol::BENCHMARK_VERSION
                || claim.competition != "arc-prize-2026-arc-agi-3"
                || !matches!(claim.phase.as_str(), "submit" | "poll")
            {
                bail!("invalid official-submission request");
            }
        }
        _ => bail!("invalid executor job"),
    }
    Ok(())
}

fn valid_repo_url(raw: &str) -> Result<()> {
    let url = reqwest::Url::parse(raw).context("invalid GitHub repository URL")?;
    let segments = url
        .path_segments()
        .map(|parts| parts.filter(|part| !part.is_empty()).count())
        .unwrap_or(0);
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || segments != 2
    {
        bail!("repository must be an uncredentialed https://github.com/owner/repo URL");
    }
    Ok(())
}

fn valid_submission_source(raw: &str) -> Result<()> {
    if is_builtin_harness(raw) {
        return Ok(());
    }
    valid_repo_url(raw)
}

fn valid_submission_model(source: &str, model_id: &str) -> bool {
    is_bedrock_model(model_id)
        && (!is_builtin_harness(source) || bedrock_model_supports_images(model_id))
}

fn request_timeout(request: &ExecutorRequest) -> Duration {
    match request {
        ExecutorRequest::Run { deadline_at, .. } => (*deadline_at - Utc::now())
            .to_std()
            .unwrap_or(Duration::ZERO),
        ExecutorRequest::Kaggle { .. } => Duration::from_secs(290),
        _ => Duration::ZERO,
    }
}

async fn capture_stderr(mut input: tokio::process::ChildStderr) -> String {
    let mut tail = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match input.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(length) => {
                tail.extend_from_slice(&buffer[..length]);
                if tail.len() > 8000 {
                    tail.drain(..tail.len() - 8000);
                }
            }
        }
    }
    String::from_utf8_lossy(&tail).into_owned()
}

fn redact(value: &str, request: &ExecutorRequest) -> String {
    let mut value = value.to_owned();
    match request {
        ExecutorRequest::Run { gateway_token, .. } => {
            value = value.replace(gateway_token, "[redacted capability]");
        }
        ExecutorRequest::Kaggle { claim } => {
            value = value.replace(&claim.token, "[redacted Kaggle token]");
        }
        _ => {}
    }
    value.chars().take(2000).collect()
}

fn crash_event(request: &ExecutorRequest, message: &str) -> ExecutorEvent {
    let safe: String = message.chars().take(2000).collect();
    match request {
        ExecutorRequest::Run { .. } => ExecutorEvent::Result {
            status: "infra_failed".into(),
            benchmark_state: json!({
                "arc":{"status":"infra_failed","error":safe.clone()},
                "frontier":{"status":"infra_failed","error":safe.clone()},
                "ram":{"status":"infra_failed","error":safe.clone()}
            }),
            score_arc: None,
            score_frontier: None,
            ram_1session_mb: None,
            ram_10session_mb: None,
            error_log: Some(safe),
        },
        ExecutorRequest::Kaggle { claim } => ExecutorEvent::KaggleResult {
            result: KaggleResultRequest {
                id: claim.id,
                lease_token: claim.lease_token,
                status: "failed".into(),
                kernel_slug: claim.kernel_slug.clone(),
                kernel_version: claim.kernel_version,
                submission_ref: claim.submission_ref.clone(),
                public_score: None,
                private_score: None,
                status_message: Some(safe),
            },
        },
        _ => unreachable!(),
    }
}

async fn finish_child(child: &mut Child) {
    if tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

async fn verify_host(config: &ExecutorConfig) -> Result<()> {
    for path in [&config.python, &config.adapter] {
        if !tokio::fs::try_exists(path).await.unwrap_or(false) {
            bail!("required executor artifact is missing: {}", path.display());
        }
    }
    tokio::fs::create_dir_all(config.state_directory.join("executor"))
        .await
        .context("create executor home")?;
    let output = Command::new("podman")
        .args([
            "info",
            "--format",
            "{{.Host.Security.Rootless}} {{.Host.CgroupsVersion}}",
        ])
        .output()
        .await
        .context("run podman info")?;
    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "true v2" {
        bail!("executor requires rootless Podman with cgroup v2");
    }
    for command in ["bwrap", "socat", "docker"] {
        let status = Command::new("sh")
            .args(["-c", &format!("command -v {command}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;
        if !status.success() {
            bail!("executor requires {command}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_sources_allow_only_github_or_pinned_builtins() {
        assert!(valid_submission_source("https://github.com/example/agent").is_ok());
        assert!(valid_submission_source("builtin://forge").is_ok());
        assert!(valid_submission_source("builtin://reki").is_ok());
        assert!(valid_submission_source("builtin://unknown").is_err());
        assert!(valid_submission_source("file:///etc/passwd").is_err());
        assert!(valid_submission_model(
            "builtin://forge",
            "google.gemma-4-31b"
        ));
        assert!(!valid_submission_model(
            "builtin://forge",
            "openai.gpt-oss-120b"
        ));
        assert!(valid_submission_model(
            "https://github.com/example/agent",
            "openai.gpt-oss-120b"
        ));
    }
}
