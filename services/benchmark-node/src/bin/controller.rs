use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use benchmark_node::{
    MAX_NDJSON_BYTES,
    academy::{AcademyClient, ApiError},
    config::ControllerConfig,
    fleet::FleetManager,
    gateway::{GatewayHandle, GatewayMetrics},
    ndjson,
};
use benchmark_protocol::{
    ArcFramesRequest, BENCHMARK_VERSION, ExecutorEvent, ExecutorRequest, HarnessClaim,
    HarnessLeaseRequest, HarnessProgressRequest, HarnessResultRequest, HarnessStageRequest,
    KaggleClaim, KaggleResultRequest,
};
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::BufReader,
    net::{UnixStream, unix::OwnedWriteHalf},
    sync::{mpsc, watch},
};

#[derive(Default)]
struct ControllerMetrics {
    healthy: AtomicBool,
    running: AtomicBool,
    last_activity_epoch: AtomicU64,
    claims: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    academy_errors: AtomicU64,
    executor_errors: AtomicU64,
    fleet_errors: AtomicU64,
    frames_dropped: AtomicU64,
    model: Mutex<ModelTotals>,
}

#[derive(Default)]
struct ModelTotals {
    active: Option<Arc<GatewayMetrics>>,
    requests: u64,
    errors: u64,
    input_tokens: u64,
    output_tokens: u64,
    latency_ms: u64,
}

#[derive(Default)]
struct ModelView {
    requests: u64,
    errors: u64,
    input_tokens: u64,
    output_tokens: u64,
    latency_ms: u64,
    completed_last_30_seconds: usize,
}

impl ControllerMetrics {
    fn start_gateway(&self, gateway: Arc<GatewayMetrics>) {
        let mut model = self.model.lock().unwrap();
        debug_assert!(model.active.is_none());
        model.active = Some(gateway);
    }

    fn finish_gateway(&self) {
        let mut model = self.model.lock().unwrap();
        if let Some(gateway) = model.active.take() {
            let snapshot = gateway.snapshot();
            model.requests += snapshot.requests;
            model.errors += snapshot.errors;
            model.input_tokens += snapshot.input_tokens;
            model.output_tokens += snapshot.output_tokens;
            model.latency_ms += snapshot.latency_ms;
        }
    }

    fn model_view(&self) -> ModelView {
        let model = self.model.lock().unwrap();
        let active = model.active.as_ref().map(|gateway| gateway.snapshot());
        ModelView {
            requests: model.requests + active.as_ref().map_or(0, |value| value.requests),
            errors: model.errors + active.as_ref().map_or(0, |value| value.errors),
            input_tokens: model.input_tokens
                + active.as_ref().map_or(0, |value| value.input_tokens),
            output_tokens: model.output_tokens
                + active.as_ref().map_or(0, |value| value.output_tokens),
            latency_ms: model.latency_ms + active.as_ref().map_or(0, |value| value.latency_ms),
            completed_last_30_seconds: active
                .as_ref()
                .map_or(0, |value| value.completed_last_30_seconds),
        }
    }
}

enum RunEnd {
    Result(HarnessResultRequest),
    Stale,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = ControllerConfig::load().await?;
    let academy = AcademyClient::new(config.academy_base_url.clone(), config.worker_token.clone())
        .context("build Academy client")?;
    let fleet = match config.fleet.as_ref() {
        Some(fleet) => Some(FleetManager::new(fleet, &config.aws_region).await),
        None => None,
    };
    let metrics = Arc::new(ControllerMetrics::default());
    metrics.healthy.store(true, Ordering::Relaxed);
    touch(&metrics);
    let listener = tokio::net::TcpListener::bind(config.metrics_bind)
        .await
        .context("bind loopback health/metrics listener")?;
    let monitoring = Router::new()
        .route("/healthz", get(health))
        .route("/metrics", get(prometheus))
        .with_state(metrics.clone());
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, monitoring).await {
            eprintln!("monitoring listener stopped: {error}");
        }
    });
    if let Some(fleet) = fleet.clone() {
        tokio::spawn(report_capacity(academy.clone(), fleet, metrics.clone()));
    }
    run(&config, &academy, &metrics, fleet.as_ref()).await
}

async fn run(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    fleet: Option<&FleetManager>,
) -> Result<()> {
    if drain_if_terminating(fleet, metrics).await? {
        return park_for_termination().await;
    }
    if let Some(fleet) = fleet {
        fleet
            .set_protected(false)
            .await
            .context("mark initialized benchmark node idle")?;
    }
    let mut delay = Duration::from_secs(2);
    loop {
        touch(metrics);
        if drain_if_terminating(fleet, metrics).await? {
            return park_for_termination().await;
        }
        match academy.claim().await {
            Ok(Some(claim)) => {
                delay = Duration::from_secs(2);
                metrics.claims.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = protect_claimed_work(fleet).await {
                    metrics.fleet_errors.fetch_add(1, Ordering::Relaxed);
                    post_run_result(
                        academy,
                        metrics,
                        failure_result(
                            &claim,
                            &format!("could not protect worker capacity: {error}"),
                        ),
                    )
                    .await;
                    if release_work(fleet, metrics).await? {
                        return park_for_termination().await;
                    }
                    continue;
                }
                metrics.running.store(true, Ordering::Relaxed);
                process_run(config, academy, metrics, claim).await;
                metrics.running.store(false, Ordering::Relaxed);
                if release_work(fleet, metrics).await? {
                    return park_for_termination().await;
                }
                continue;
            }
            Ok(None) => {}
            Err(ApiError::Unauthorized) => bail!("Academy rejected X-Worker-Token"),
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("claim failed: {error}");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(30));
                continue;
            }
        }
        match academy.kaggle_claim().await {
            Ok(Some(claim)) => {
                delay = Duration::from_secs(2);
                metrics.claims.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = protect_claimed_work(fleet).await {
                    metrics.fleet_errors.fetch_add(1, Ordering::Relaxed);
                    fail_kaggle_claim(
                        academy,
                        metrics,
                        &claim,
                        &format!("could not protect worker capacity: {error}"),
                    )
                    .await;
                    if release_work(fleet, metrics).await? {
                        return park_for_termination().await;
                    }
                    continue;
                }
                metrics.running.store(true, Ordering::Relaxed);
                process_kaggle(config, academy, metrics, claim).await;
                metrics.running.store(false, Ordering::Relaxed);
                if release_work(fleet, metrics).await? {
                    return park_for_termination().await;
                }
                continue;
            }
            Ok(None) => {
                delay = Duration::from_secs(2);
            }
            Err(ApiError::Unauthorized) => bail!("Academy rejected X-Worker-Token"),
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("Kaggle claim failed: {error}");
            }
        }
        tokio::time::sleep(delay).await;
    }
}

async fn report_capacity(
    academy: AcademyClient,
    fleet: FleetManager,
    metrics: Arc<ControllerMetrics>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        match academy.capacity().await {
            Ok(capacity) => {
                if let Err(error) = fleet.publish_capacity(&capacity).await {
                    metrics.fleet_errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("capacity metric publish failed: {error}");
                }
            }
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("capacity snapshot failed: {error}");
            }
        }
    }
}

async fn protect_claimed_work(fleet: Option<&FleetManager>) -> Result<()> {
    let Some(fleet) = fleet else {
        return Ok(());
    };
    if let Err(protection_error) = fleet.set_protected(true).await {
        // A scale-in can win the few milliseconds between claim and protection. The
        // termination hook still gives the claimed run its full deadline to drain.
        if fleet.termination_waiting().await.unwrap_or(false) {
            eprintln!("node entered termination while claiming; draining under lifecycle hook");
            return Ok(());
        }
        return Err(protection_error);
    }
    Ok(())
}

async fn release_work(fleet: Option<&FleetManager>, metrics: &ControllerMetrics) -> Result<bool> {
    let Some(fleet) = fleet else {
        return Ok(false);
    };
    if drain_if_terminating(Some(fleet), metrics).await? {
        return Ok(true);
    }
    fleet
        .set_protected(false)
        .await
        .context("release benchmark node scale-in protection")?;
    Ok(false)
}

async fn drain_if_terminating(
    fleet: Option<&FleetManager>,
    metrics: &ControllerMetrics,
) -> Result<bool> {
    let Some(fleet) = fleet else {
        return Ok(false);
    };
    if !fleet.termination_waiting().await? {
        return Ok(false);
    }
    fleet.complete_termination().await?;
    metrics.healthy.store(false, Ordering::Relaxed);
    Ok(true)
}

async fn park_for_termination() -> Result<()> {
    std::future::pending::<()>().await;
    Ok(())
}

async fn process_run(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    claim: HarnessClaim,
) {
    if claim.benchmark_version != BENCHMARK_VERSION {
        post_run_result(
            academy,
            metrics,
            failure_result(&claim, "worker and claim benchmark versions differ"),
        )
        .await;
        return;
    }
    let gateway = match GatewayHandle::start(
        &config.gateway_directory,
        claim.id,
        &config.aws_region,
        &config.model_id,
        &config.profile_name,
        &config.reasoning_effort,
        &config.bedrock_api_key,
        config.maximum_model_concurrency,
    )
    .await
    {
        Ok(gateway) => gateway,
        Err(error) => {
            post_run_result(
                academy,
                metrics,
                failure_result(&claim, &format!("model gateway failed to start: {error}")),
            )
            .await;
            return;
        }
    };
    metrics.start_gateway(gateway.metrics.clone());
    let result = execute_run(config, academy, metrics, &claim, &gateway).await;
    match result {
        RunEnd::Result(result) => post_run_result(academy, metrics, result).await,
        RunEnd::Stale => eprintln!("dropping reclaimed run {}", claim.id),
    }
    metrics.finish_gateway();
    gateway.stop().await;
}

async fn execute_run(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    claim: &HarnessClaim,
    gateway: &GatewayHandle,
) -> RunEnd {
    let stream = match UnixStream::connect(&config.executor_socket).await {
        Ok(stream) => stream,
        Err(error) => {
            metrics.executor_errors.fetch_add(1, Ordering::Relaxed);
            return RunEnd::Result(failure_result(
                claim,
                &format!("restricted executor unavailable: {error}"),
            ));
        }
    };
    let (read, mut write) = stream.into_split();
    let request = ExecutorRequest::Run {
        run_id: claim.id,
        repo_url: claim.repo_url.clone(),
        deadline_at: claim.deadline_at,
        gateway_socket: gateway.socket_path.to_string_lossy().into_owned(),
        gateway_token: gateway.token.clone(),
        model_profile: config.profile_name.clone(),
    };
    if let Err(error) = ndjson::write(&mut write, &request, MAX_NDJSON_BYTES).await {
        metrics.executor_errors.fetch_add(1, Ordering::Relaxed);
        return RunEnd::Result(failure_result(
            claim,
            &format!("could not start restricted executor: {error}"),
        ));
    }
    let mut reader = BufReader::new(read);
    let (frames, frame_input) = mpsc::channel(4);
    let (stale_sender, mut stale_receiver) = watch::channel(false);
    let frame_task = tokio::spawn(send_frames(
        academy.clone(),
        claim.id,
        claim.lease_token,
        frame_input,
        stale_sender,
        metrics.clone(),
    ));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let remaining = (claim.deadline_at - Utc::now())
        .to_std()
        .unwrap_or(Duration::ZERO);
    let deadline = tokio::time::sleep(remaining);
    tokio::pin!(deadline);
    let mut heartbeat_failures = 0usize;
    let mut stage_reported = false;

    let end = loop {
        touch(metrics);
        tokio::select! {
            message = ndjson::read::<ExecutorEvent, _>(&mut reader, MAX_NDJSON_BYTES) => {
                match message {
                    Ok(Some(ExecutorEvent::Ready { commit_sha })) => {
                        if stage_reported || !valid_sha(&commit_sha) {
                            break RunEnd::Result(failure_result(claim, "executor returned an invalid commit SHA"));
                        }
                        let fingerprint = format!("{:x}", Sha256::digest(config.model_id.as_bytes()));
                        let request = HarnessStageRequest {
                            id: claim.id,
                            lease_token: claim.lease_token,
                            status: "running".into(),
                            commit_sha,
                            bedrock_profile: config.profile_name.clone(),
                            bedrock_profile_fingerprint: fingerprint,
                        };
                        match academy.stage(&request).await {
                            Ok(()) => stage_reported = true,
                            Err(ApiError::StaleLease) => break RunEnd::Stale,
                            Err(error) => {
                                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                                break RunEnd::Result(failure_result(claim, &format!("could not record run stage: {error}")));
                            }
                        }
                    }
                    Ok(Some(ExecutorEvent::Progress { benchmark, state })) => {
                        if !stage_reported {
                            break RunEnd::Result(failure_result(claim, "executor sent progress before checkout was recorded"));
                        }
                        let request = HarnessProgressRequest {
                            id: claim.id,
                            lease_token: claim.lease_token,
                            benchmark,
                            state,
                        };
                        match academy.progress(&request).await {
                            Ok(()) => {}
                            Err(ApiError::StaleLease) => break RunEnd::Stale,
                            Err(error) => {
                                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                                eprintln!("progress post failed for {}: {error}", claim.id);
                            }
                        }
                    }
                    Ok(Some(ExecutorEvent::Frames { frames: batch })) => {
                        if stage_reported {
                            let count = batch.len() as u64;
                            if frames.try_send(batch).is_err() {
                                metrics.frames_dropped.fetch_add(count, Ordering::Relaxed);
                            }
                        }
                    }
                    Ok(Some(ExecutorEvent::Result {
                        status,
                        benchmark_state,
                        score_arc,
                        score_frontier,
                        ram_1session_mb,
                        ram_10session_mb,
                        error_log,
                    })) => {
                        if !matches!(status.as_str(), "done" | "partial" | "failed" | "infra_failed") {
                            break RunEnd::Result(failure_result(claim, "executor returned an invalid terminal status"));
                        }
                        break RunEnd::Result(HarnessResultRequest {
                            id: claim.id,
                            lease_token: claim.lease_token,
                            status,
                            benchmark_state,
                            score_arc,
                            score_frontier,
                            ram_1session_mb,
                            ram_10session_mb,
                            error_log,
                        });
                    }
                    Ok(Some(ExecutorEvent::Log { level, message })) => {
                        eprintln!("executor {level}: {}", message.chars().take(2000).collect::<String>());
                    }
                    Ok(Some(_)) => break RunEnd::Result(failure_result(claim, "executor returned an unexpected message")),
                    Ok(None) => {
                        metrics.executor_errors.fetch_add(1, Ordering::Relaxed);
                        break RunEnd::Result(failure_result(claim, "restricted executor exited without a result"));
                    }
                    Err(error) => {
                        metrics.executor_errors.fetch_add(1, Ordering::Relaxed);
                        break RunEnd::Result(failure_result(claim, &format!("invalid executor message: {error}")));
                    }
                }
            }
            _ = heartbeat.tick() => {
                let heartbeat_request = HarnessLeaseRequest { id: claim.id, lease_token: claim.lease_token };
                match academy.heartbeat(&heartbeat_request).await {
                    Ok(()) => heartbeat_failures = 0,
                    Err(ApiError::StaleLease) => break RunEnd::Stale,
                    Err(error) => {
                        metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                        heartbeat_failures += 1;
                        eprintln!("heartbeat failed for {} ({heartbeat_failures}/3): {error}", claim.id);
                        if heartbeat_failures >= 3 {
                            break RunEnd::Result(failure_result(claim, "Academy was unavailable for three heartbeats"));
                        }
                    }
                }
            }
            changed = stale_receiver.changed() => {
                if changed.is_ok() && *stale_receiver.borrow() {
                    break RunEnd::Stale;
                }
            }
            _ = &mut deadline => {
                break RunEnd::Result(failure_result(claim, "the 600-second benchmark deadline expired"));
            }
        }
    };
    let _ = cancel_executor(&mut write).await;
    drop(frames);
    frame_task.abort();
    end
}

async fn send_frames(
    academy: AcademyClient,
    run_id: uuid::Uuid,
    lease_token: uuid::Uuid,
    mut input: mpsc::Receiver<Vec<benchmark_protocol::ArcFrame>>,
    stale: watch::Sender<bool>,
    metrics: Arc<ControllerMetrics>,
) {
    while let Some(frames) = input.recv().await {
        if frames.len() > 64 || !frames.iter().all(benchmark_protocol::ArcFrame::is_valid) {
            metrics
                .frames_dropped
                .fetch_add(frames.len() as u64, Ordering::Relaxed);
            continue;
        }
        let request = ArcFramesRequest {
            run_id,
            lease_token,
            frames,
        };
        match academy.frames(&request).await {
            Ok(()) => {}
            Err(ApiError::StaleLease) => {
                let _ = stale.send(true);
                return;
            }
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                metrics
                    .frames_dropped
                    .fetch_add(request.frames.len() as u64, Ordering::Relaxed);
                eprintln!("best-effort frame batch dropped: {error}");
            }
        }
    }
}

async fn post_run_result(
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    result: HarnessResultRequest,
) {
    for attempt in 0..4 {
        match academy.result(&result).await {
            Ok(()) => {
                if result.status == "done" {
                    metrics.completed.fetch_add(1, Ordering::Relaxed);
                } else {
                    metrics.failed.fetch_add(1, Ordering::Relaxed);
                }
                return;
            }
            Err(ApiError::StaleLease) => return,
            Err(error) if error.is_temporary() && attempt < 3 => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
            }
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("terminal result rejected for {}: {error}", result.id);
                return;
            }
        }
    }
}

async fn process_kaggle(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    claim: KaggleClaim,
) {
    let result = execute_kaggle(config, metrics, &claim)
        .await
        .unwrap_or_else(|error| KaggleResultRequest {
            id: claim.id,
            lease_token: claim.lease_token,
            status: "failed".into(),
            kernel_slug: claim.kernel_slug.clone(),
            kernel_version: claim.kernel_version,
            submission_ref: claim.submission_ref.clone(),
            public_score: None,
            private_score: None,
            status_message: Some(error.to_string().chars().take(2000).collect()),
        });
    post_kaggle_result(academy, metrics, &result).await;
}

async fn fail_kaggle_claim(
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    claim: &KaggleClaim,
    message: &str,
) {
    let result = KaggleResultRequest {
        id: claim.id,
        lease_token: claim.lease_token,
        status: "failed".into(),
        kernel_slug: claim.kernel_slug.clone(),
        kernel_version: claim.kernel_version,
        submission_ref: claim.submission_ref.clone(),
        public_score: None,
        private_score: None,
        status_message: Some(message.chars().take(2000).collect()),
    };
    post_kaggle_result(academy, metrics, &result).await;
}

async fn post_kaggle_result(
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    result: &KaggleResultRequest,
) {
    for attempt in 0..4 {
        match academy.kaggle_result(result).await {
            Ok(()) | Err(ApiError::StaleLease) => return,
            Err(error) if error.is_temporary() && attempt < 3 => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
            }
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("Kaggle result rejected for {}: {error}", result.id);
                return;
            }
        }
    }
}

async fn execute_kaggle(
    config: &ControllerConfig,
    metrics: &Arc<ControllerMetrics>,
    claim: &KaggleClaim,
) -> Result<KaggleResultRequest> {
    let stream = UnixStream::connect(&config.executor_socket)
        .await
        .context("restricted executor unavailable")?;
    let (read, mut write) = stream.into_split();
    ndjson::write(
        &mut write,
        &ExecutorRequest::Kaggle {
            claim: claim.clone(),
        },
        MAX_NDJSON_BYTES,
    )
    .await?;
    let mut reader = BufReader::new(read);
    let result = tokio::time::timeout(Duration::from_secs(290), async {
        loop {
            let event = ndjson::read::<ExecutorEvent, _>(&mut reader, MAX_NDJSON_BYTES)
                .await?
                .context("Kaggle executor exited without a result")?;
            match event {
                ExecutorEvent::KaggleResult { result }
                    if result.id == claim.id && result.lease_token == claim.lease_token =>
                {
                    break Ok(result);
                }
                ExecutorEvent::Log { level, message } => {
                    eprintln!(
                        "Kaggle executor {level}: {}",
                        message.chars().take(2000).collect::<String>()
                    );
                }
                _ => {
                    metrics.executor_errors.fetch_add(1, Ordering::Relaxed);
                    bail!("Kaggle executor returned an invalid result");
                }
            }
        }
    })
    .await
    .context("Kaggle executor deadline expired")??;
    let _ = cancel_executor(&mut write).await;
    Ok(result)
}

async fn cancel_executor(write: &mut OwnedWriteHalf) -> Result<()> {
    ndjson::write(write, &ExecutorRequest::Cancel, MAX_NDJSON_BYTES).await
}

fn failure_result(claim: &HarnessClaim, message: &str) -> HarnessResultRequest {
    let message: String = message.chars().take(8000).collect();
    HarnessResultRequest {
        id: claim.id,
        lease_token: claim.lease_token,
        status: "infra_failed".into(),
        benchmark_state: json!({
            "arc":{"status":"infra_failed","error":message.clone()},
            "frontier":{"status":"infra_failed","error":message.clone()},
            "ram":{"status":"infra_failed","error":message.clone()}
        }),
        score_arc: None,
        score_frontier: None,
        ram_1session_mb: None,
        ram_10session_mb: None,
        error_log: Some(message),
    }
}

fn valid_sha(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn touch(metrics: &ControllerMetrics) {
    metrics.last_activity_epoch.store(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        Ordering::Relaxed,
    );
}

async fn health(State(metrics): State<Arc<ControllerMetrics>>) -> impl IntoResponse {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let recent = now.saturating_sub(metrics.last_activity_epoch.load(Ordering::Relaxed)) < 65;
    let ok = metrics.healthy.load(Ordering::Relaxed) && recent;
    (
        if ok {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        axum::Json(json!({"ok":ok,"running":metrics.running.load(Ordering::Relaxed)})),
    )
}

async fn prometheus(State(metrics): State<Arc<ControllerMetrics>>) -> Response {
    let model = metrics.model_view();
    let body = format!(
        concat!(
            "exposure_benchmark_running {}\n",
            "exposure_benchmark_claims_total {}\n",
            "exposure_benchmark_completed_total {}\n",
            "exposure_benchmark_failed_total {}\n",
            "exposure_benchmark_academy_errors_total {}\n",
            "exposure_benchmark_executor_errors_total {}\n",
            "exposure_benchmark_fleet_errors_total {}\n",
            "exposure_benchmark_frames_dropped_total {}\n",
            "exposure_benchmark_model_requests_total {}\n",
            "exposure_benchmark_model_errors_total {}\n",
            "exposure_benchmark_model_input_tokens_total {}\n",
            "exposure_benchmark_model_output_tokens_total {}\n",
            "exposure_benchmark_model_latency_ms_total {}\n",
            "exposure_benchmark_model_completions_last_30_seconds {}\n"
        ),
        u8::from(metrics.running.load(Ordering::Relaxed)),
        metrics.claims.load(Ordering::Relaxed),
        metrics.completed.load(Ordering::Relaxed),
        metrics.failed.load(Ordering::Relaxed),
        metrics.academy_errors.load(Ordering::Relaxed),
        metrics.executor_errors.load(Ordering::Relaxed),
        metrics.fleet_errors.load(Ordering::Relaxed),
        metrics.frames_dropped.load(Ordering::Relaxed),
        model.requests,
        model.errors,
        model.input_tokens,
        model.output_tokens,
        model.latency_ms,
        model.completed_last_30_seconds,
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response()
}
