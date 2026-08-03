use anyhow::{Context, Result, bail};
use benchmark_node::{MAX_NDJSON_BYTES, config::ExecutorConfig, ndjson, random_token};
use benchmark_protocol::{
    BenchmarkKind, ExecutorEvent, ExecutorRequest, KaggleResultRequest, ModelProvider,
    is_builtin_harness,
};
use chrono::Utc;
use serde_json::json;
use std::{
    collections::HashSet, os::unix::fs::PermissionsExt, path::Path, process::Stdio, sync::Arc,
    time::Duration,
};
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
    let slot = Arc::new(Semaphore::new(2));
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
    // Harbor creates detached Compose containers without our run label. Only Terminal-Bench can
    // own those containers, and the controller admits one Terminal-Bench run at a time; ARC cleanup
    // therefore remains label-scoped and cannot remove the concurrent Terminal-Bench environment.
    let baseline_containers = if matches!(
        &request,
        ExecutorRequest::Run {
            benchmark_kind: BenchmarkKind::Frontier | BenchmarkKind::Bundled,
            ..
        }
    ) {
        list_container_ids(config, Some("label=com.docker.compose.project")).await
    } else {
        None
    };

    let mode = match &request {
        ExecutorRequest::Run { .. } => "--executor-ndjson",
        ExecutorRequest::Kaggle { .. } => "--executor-kaggle-ndjson",
        _ => unreachable!(),
    };
    // The adapter runs with a deliberately scrubbed environment so it cannot inherit
    // controller credentials. Keep only the non-secret runtime settings that Podman and
    // the benchmark mode require.
    let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "PROD".into());
    let podman_rootful = std::env::var("HARNESS_PODMAN_ROOTFUL").unwrap_or_default();
    let containers_conf = std::env::var("CONTAINERS_CONF").ok();
    let containers_storage_conf = std::env::var("CONTAINERS_STORAGE_CONF").ok();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let harbor_user =
        std::env::var("HARNESS_HARBOR_USER").unwrap_or_else(|_| "exposure-executor".into());
    let mut child = Command::new(&config.python);
    child
        .arg(&config.adapter)
        .arg(mode)
        .current_dir(&work)
        .env_clear()
        .env(
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        )
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("HOME", config.state_directory.join("executor"))
        .env("ENVIRONMENT", environment)
        .env("HARNESS_HARBOR_USER", harbor_user)
        .env("HARNESS_PODMAN_ROOTFUL", podman_rootful)
        .env("PYTHONUNBUFFERED", "1")
        .env("HARNESS_ENV", "executor")
        .env(
            "HARNESS_CACHE_DIRECTORY",
            config.state_directory.join("cache"),
        )
        .env(
            "HARNESS_RAM_LOCK",
            config.state_directory.join("executor/ram.lock"),
        )
        .env("HARNESS_IMAGE", &config.sandbox_image)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true);
    if let Some(xdg_runtime_dir) = xdg_runtime_dir {
        child.env("XDG_RUNTIME_DIR", xdg_runtime_dir);
    }
    if let Some(containers_conf) = containers_conf {
        child.env("CONTAINERS_CONF", containers_conf);
    }
    if let Some(containers_storage_conf) = containers_storage_conf {
        child.env("CONTAINERS_STORAGE_CONF", containers_storage_conf);
    }
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
                        if let Err(error) = ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await {
                            stop_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
                            return Err(error);
                        }
                        if terminal {
                            finish_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
                            return Ok(());
                        }
                    }
                    Ok(None) => {
                        let status = child.wait().await.context("wait for adapter")?;
                        cleanup_run_containers(run_id, config, baseline_containers.as_ref()).await;
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
                        stop_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
                        let event = crash_event(&request, &format!("invalid adapter message: {error}"));
                        ndjson::write(&mut output, &event, MAX_NDJSON_BYTES).await?;
                        return Ok(());
                    }
                }
            }
            control_message = ndjson::read::<ExecutorRequest, _>(&mut control, 1024) => {
                match control_message {
                    Ok(Some(ExecutorRequest::Cancel)) | Ok(None) => {
                        stop_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
                        return Ok(());
                    }
                    _ => {
                        stop_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
                        bail!("controller sent an invalid in-flight command");
                    }
                }
            }
            _ = &mut deadline => {
                stop_child(&mut child, run_id, config, baseline_containers.as_ref()).await;
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
            provider,
            benchmark_kind,
            ..
        } => {
            valid_submission_source(repo_url)?;
            if *deadline_at <= Utc::now()
                || gateway_token.len() != 64
                || !valid_submission_model(*provider, *benchmark_kind, repo_url, model_profile)
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

fn valid_submission_model(
    provider: ModelProvider,
    benchmark_kind: BenchmarkKind,
    source: &str,
    model_id: &str,
) -> bool {
    provider.supports_model(model_id)
        && (!is_builtin_harness(source)
            || matches!(benchmark_kind, BenchmarkKind::Frontier)
            || provider.supports_images(model_id))
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
        ExecutorRequest::Run { benchmark_kind, .. } => ExecutorEvent::Result {
            status: "infra_failed".into(),
            benchmark_state: failed_benchmark_state(*benchmark_kind, &safe),
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

fn failed_benchmark_state(
    kind: benchmark_protocol::BenchmarkKind,
    message: &str,
) -> serde_json::Value {
    let skipped = json!({"status":"skipped"});
    let failed = || json!({"status":"infra_failed","error":message});
    json!({
        "arc": if matches!(kind, benchmark_protocol::BenchmarkKind::Frontier) {
            skipped.clone()
        } else {
            failed()
        },
        "frontier": if matches!(kind, benchmark_protocol::BenchmarkKind::Arc) {
            skipped
        } else {
            failed()
        },
        "ram": failed()
    })
}

async fn finish_child(
    child: &mut Child,
    run_id: uuid::Uuid,
    config: &ExecutorConfig,
    baseline_containers: Option<&HashSet<String>>,
) {
    let group = child.id();
    let leader_finished = tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .is_ok();
    if let Some(group) = group
        && process_group_exists(group).await
    {
        terminate_process_group(group, Duration::from_secs(3)).await;
    }
    if !leader_finished {
        wait_for_child(child).await;
    }
    cleanup_run_containers(run_id, config, baseline_containers).await;
}

async fn stop_child(
    child: &mut Child,
    run_id: uuid::Uuid,
    config: &ExecutorConfig,
    baseline_containers: Option<&HashSet<String>>,
) {
    if let Some(group) = child.id() {
        terminate_process_group(group, Duration::from_secs(3)).await;
    }
    wait_for_child(child).await;
    cleanup_run_containers(run_id, config, baseline_containers).await;
}

async fn wait_for_child(child: &mut Child) {
    if tokio::time::timeout(Duration::from_secs(1), child.wait())
        .await
        .is_err()
    {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}

async fn terminate_process_group(group: u32, grace: Duration) {
    signal_process_group(group, "-TERM").await;
    let deadline = tokio::time::Instant::now() + grace;
    while process_group_exists(group).await && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if process_group_exists(group).await {
        signal_process_group(group, "-KILL").await;
    }
}

async fn process_group_exists(group: u32) -> bool {
    Command::new("pkill")
        .args(["-0", "-g", &group.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

async fn signal_process_group(group: u32, signal: &str) {
    let _ = Command::new("pkill")
        .args([signal, "-g", &group.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

async fn list_container_ids(
    config: &ExecutorConfig,
    filter: Option<&str>,
) -> Option<HashSet<String>> {
    let home = config.state_directory.join("executor");
    let mut command = Command::new("podman");
    command.args(["ps", "-aq"]).env("HOME", home);
    if let Some(filter) = filter {
        command.args(["--filter", filter]);
    }
    let output = command.output().await.ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        std::str::from_utf8(&output.stdout)
            .unwrap_or_default()
            .lines()
            .filter(|line| !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .map(str::to_owned)
            .collect(),
    )
}

fn containers_to_remove(
    baseline: Option<&HashSet<String>>,
    current: Option<HashSet<String>>,
    labelled: Option<HashSet<String>>,
) -> Vec<String> {
    let mut remove = labelled.unwrap_or_default();
    if let (Some(baseline), Some(current)) = (baseline, current) {
        remove.extend(current.difference(baseline).cloned());
    }
    let mut ids: Vec<String> = remove.into_iter().collect();
    ids.sort();
    ids
}

async fn cleanup_run_containers(
    run_id: uuid::Uuid,
    config: &ExecutorConfig,
    baseline_containers: Option<&HashSet<String>>,
) {
    let label = format!("label=academy.harness.run={run_id}");
    let current = list_container_ids(config, Some("label=com.docker.compose.project")).await;
    let labelled = list_container_ids(config, Some(&label)).await;
    let ids = containers_to_remove(baseline_containers, current, labelled);
    if !ids.is_empty() {
        let _ = Command::new("podman")
            .args(["rm", "-f"])
            .args(&ids)
            .env("HOME", config.state_directory.join("executor"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
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
    let expected = if std::env::var("HARNESS_PODMAN_ROOTFUL").as_deref() == Ok("1") {
        "false v2"
    } else {
        "true v2"
    };
    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != expected {
        bail!("executor requires Podman {expected}");
    }
    for command in ["bwrap", "socat", "docker", "pkill"] {
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
            ModelProvider::Bedrock,
            BenchmarkKind::Arc,
            "builtin://forge",
            "google.gemma-4-31b"
        ));
        assert!(!valid_submission_model(
            ModelProvider::Bedrock,
            BenchmarkKind::Arc,
            "builtin://forge",
            "openai.gpt-oss-120b"
        ));
        assert!(valid_submission_model(
            ModelProvider::Bedrock,
            BenchmarkKind::Arc,
            "https://github.com/example/agent",
            "openai.gpt-oss-120b"
        ));
        assert!(valid_submission_model(
            ModelProvider::Cerebras,
            BenchmarkKind::Arc,
            "builtin://forge",
            "gemma-4-31b"
        ));
        assert!(valid_submission_model(
            ModelProvider::Cerebras,
            BenchmarkKind::Frontier,
            "builtin://forge",
            "zai-glm-4.7"
        ));
    }

    #[tokio::test]
    async fn process_group_escalation_kills_term_ignoring_descendants() {
        let mut child = Command::new("sh")
            .args(["-c", "sh -c 'trap \"\" TERM; sleep 30' & wait"])
            .process_group(0)
            .spawn()
            .unwrap();
        let group = child.id().unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        terminate_process_group(group, Duration::from_millis(100)).await;
        let status = tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .expect("process group should stop")
            .unwrap();
        assert!(!status.success());
        assert!(!process_group_exists(group).await);
    }

    #[test]
    fn cleanup_preserves_baseline_and_removes_new_compose_containers() {
        let baseline = HashSet::from(["existing".to_string(), "old-run".to_string()]);
        let current = HashSet::from([
            "existing".to_string(),
            "old-run".to_string(),
            "terminal-main".to_string(),
            "terminal-sidecar".to_string(),
        ]);
        let labelled = HashSet::from(["old-run".to_string(), "arc".to_string()]);
        assert_eq!(
            containers_to_remove(Some(&baseline), Some(current), Some(labelled)),
            ["arc", "old-run", "terminal-main", "terminal-sidecar"]
        );
    }
}
