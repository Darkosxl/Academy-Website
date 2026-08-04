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
    gateway::{GatewayHandle, GatewayMetrics, OpenAiKeyPool, ProviderCredentials},
    ndjson,
};
use benchmark_protocol::{
    ArcFramesRequest, BENCHMARK_VERSION, BenchmarkKind, ExecutorEvent, ExecutorRequest,
    HarnessClaim, HarnessLeaseRequest, HarnessProgressRequest, HarnessResultRequest,
    HarnessStageRequest, KaggleClaim, KaggleResultRequest, RUN_DEADLINE_SECONDS,
    is_builtin_harness,
};
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::BufReader,
    net::{UnixStream, unix::OwnedWriteHalf},
    sync::{Mutex as AsyncMutex, Semaphore, mpsc, watch},
};
use uuid::Uuid;

#[derive(Default)]
struct ControllerMetrics {
    healthy: AtomicBool,
    running: AtomicU64,
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
    active: HashMap<Uuid, Arc<GatewayMetrics>>,
    requests: u64,
    upstream_attempts: u64,
    rate_limits: u64,
    errors: u64,
    input_tokens: u64,
    output_tokens: u64,
    latency_ms: u64,
}

#[derive(Default)]
struct ModelView {
    requests: u64,
    upstream_attempts: u64,
    rate_limits: u64,
    errors: u64,
    input_tokens: u64,
    output_tokens: u64,
    latency_ms: u64,
    completed_last_30_seconds: usize,
}

impl ControllerMetrics {
    fn start_gateway(&self, run_id: Uuid, gateway: Arc<GatewayMetrics>) {
        let mut model = self.model.lock().unwrap();
        debug_assert!(model.active.insert(run_id, gateway).is_none());
    }

    fn finish_gateway(&self, run_id: Uuid) {
        let mut model = self.model.lock().unwrap();
        if let Some(gateway) = model.active.remove(&run_id) {
            let snapshot = gateway.snapshot();
            model.requests += snapshot.requests;
            model.upstream_attempts += snapshot.upstream_attempts;
            model.rate_limits += snapshot.rate_limits;
            model.errors += snapshot.errors;
            model.input_tokens += snapshot.input_tokens;
            model.output_tokens += snapshot.output_tokens;
            model.latency_ms += snapshot.latency_ms;
        }
    }

    fn model_view(&self) -> ModelView {
        let model = self.model.lock().unwrap();
        let mut view = ModelView {
            requests: model.requests,
            upstream_attempts: model.upstream_attempts,
            rate_limits: model.rate_limits,
            errors: model.errors,
            input_tokens: model.input_tokens,
            output_tokens: model.output_tokens,
            latency_ms: model.latency_ms,
            completed_last_30_seconds: 0,
        };
        for gateway in model.active.values() {
            let active = gateway.snapshot();
            view.requests += active.requests;
            view.upstream_attempts += active.upstream_attempts;
            view.rate_limits += active.rate_limits;
            view.errors += active.errors;
            view.input_tokens += active.input_tokens;
            view.output_tokens += active.output_tokens;
            view.latency_ms += active.latency_ms;
            view.completed_last_30_seconds += active.completed_last_30_seconds;
        }
        view
    }
}

enum RunEnd {
    Result(HarnessResultRequest),
    Stale,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = ControllerConfig::load().await?;
    let credentials = ProviderCredentials {
        bedrock_api_key: config.bedrock_api_key.clone().into(),
        cerebras: Arc::new(OpenAiKeyPool::cerebras(config.cerebras_api_keys.clone())?),
        deepinfra: Arc::new(OpenAiKeyPool::deepinfra(vec![
            config.deepinfra_api_key.clone(),
        ])?),
    };
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
    run(&config, &academy, &metrics, fleet.as_ref(), &credentials).await
}

async fn run(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    fleet: Option<&FleetManager>,
    credentials: &ProviderCredentials,
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
    let arc_lane = Arc::new(Semaphore::new(1));
    let terminal_lane = Arc::new(Semaphore::new(1));
    let fleet_transition = Arc::new(AsyncMutex::new(()));
    tokio::try_join!(
        run_kind_lane(
            config,
            academy,
            metrics,
            fleet,
            credentials,
            BenchmarkKind::Arc,
            arc_lane.clone(),
            fleet_transition.clone(),
        ),
        run_kind_lane(
            config,
            academy,
            metrics,
            fleet,
            credentials,
            BenchmarkKind::Frontier,
            terminal_lane.clone(),
            fleet_transition.clone(),
        ),
        run_maintenance_lane(
            config,
            academy,
            metrics,
            fleet,
            credentials,
            arc_lane,
            terminal_lane,
            fleet_transition,
        ),
    )?;
    park_for_termination().await
}

async fn run_kind_lane(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    fleet: Option<&FleetManager>,
    credentials: &ProviderCredentials,
    kind: BenchmarkKind,
    lane: Arc<Semaphore>,
    fleet_transition: Arc<AsyncMutex<()>>,
) -> Result<()> {
    let mut delay = Duration::from_secs(2);
    loop {
        let permit = lane.acquire().await.context("benchmark lane closed")?;
        touch(metrics);
        if stop_claiming_if_terminating(fleet, metrics, &fleet_transition).await? {
            return Ok(());
        }
        match academy.claim(kind).await {
            Ok(Some(claim)) => {
                delay = Duration::from_secs(2);
                metrics.claims.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = start_claimed_work(fleet, metrics, &fleet_transition).await {
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
                    if release_failed_claim(fleet, metrics, &fleet_transition).await? {
                        return Ok(());
                    }
                    continue;
                }
                process_run(config, academy, metrics, claim, credentials).await;
                if finish_claimed_work(fleet, metrics, &fleet_transition).await? {
                    return Ok(());
                }
                continue;
            }
            Ok(None) => delay = Duration::from_secs(2),
            Err(ApiError::Unauthorized) => bail!("Academy rejected X-Worker-Token"),
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("{kind} claim failed: {error}");
                delay = (delay * 2).min(Duration::from_secs(30));
            }
        }
        drop(permit);
        tokio::time::sleep(delay).await;
    }
}

async fn run_maintenance_lane(
    config: &ControllerConfig,
    academy: &AcademyClient,
    metrics: &Arc<ControllerMetrics>,
    fleet: Option<&FleetManager>,
    credentials: &ProviderCredentials,
    arc_lane: Arc<Semaphore>,
    terminal_lane: Arc<Semaphore>,
    fleet_transition: Arc<AsyncMutex<()>>,
) -> Result<()> {
    let mut delay = Duration::from_secs(2);
    loop {
        let arc_permit = arc_lane.acquire().await.context("ARC lane closed")?;
        let Ok(terminal_permit) = terminal_lane.try_acquire() else {
            drop(arc_permit);
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        };
        touch(metrics);
        if stop_claiming_if_terminating(fleet, metrics, &fleet_transition).await? {
            return Ok(());
        }
        match academy.claim(BenchmarkKind::Bundled).await {
            Ok(Some(claim)) => {
                delay = Duration::from_secs(2);
                metrics.claims.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = start_claimed_work(fleet, metrics, &fleet_transition).await {
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
                    if release_failed_claim(fleet, metrics, &fleet_transition).await? {
                        return Ok(());
                    }
                    continue;
                }
                process_run(config, academy, metrics, claim, credentials).await;
                if finish_claimed_work(fleet, metrics, &fleet_transition).await? {
                    return Ok(());
                }
                continue;
            }
            Ok(None) => {}
            Err(ApiError::Unauthorized) => bail!("Academy rejected X-Worker-Token"),
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("bundled claim failed: {error}");
                delay = (delay * 2).min(Duration::from_secs(30));
                drop(terminal_permit);
                drop(arc_permit);
                tokio::time::sleep(delay).await;
                continue;
            }
        }
        match academy.kaggle_claim().await {
            Ok(Some(claim)) => {
                delay = Duration::from_secs(2);
                metrics.claims.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = start_claimed_work(fleet, metrics, &fleet_transition).await {
                    metrics.fleet_errors.fetch_add(1, Ordering::Relaxed);
                    fail_kaggle_claim(
                        academy,
                        metrics,
                        &claim,
                        &format!("could not protect worker capacity: {error}"),
                    )
                    .await;
                    if release_failed_claim(fleet, metrics, &fleet_transition).await? {
                        return Ok(());
                    }
                    continue;
                }
                process_kaggle(config, academy, metrics, claim).await;
                if finish_claimed_work(fleet, metrics, &fleet_transition).await? {
                    return Ok(());
                }
                continue;
            }
            Ok(None) => delay = Duration::from_secs(2),
            Err(ApiError::Unauthorized) => bail!("Academy rejected X-Worker-Token"),
            Err(error) => {
                metrics.academy_errors.fetch_add(1, Ordering::Relaxed);
                eprintln!("Kaggle claim failed: {error}");
                delay = (delay * 2).min(Duration::from_secs(30));
            }
        }
        drop(terminal_permit);
        drop(arc_permit);
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

async fn start_claimed_work(
    fleet: Option<&FleetManager>,
    metrics: &ControllerMetrics,
    fleet_transition: &AsyncMutex<()>,
) -> Result<()> {
    let _transition = fleet_transition.lock().await;
    protect_claimed_work(fleet).await?;
    metrics.running.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

async fn finish_claimed_work(
    fleet: Option<&FleetManager>,
    metrics: &ControllerMetrics,
    fleet_transition: &AsyncMutex<()>,
) -> Result<bool> {
    let _transition = fleet_transition.lock().await;
    let previous = metrics.running.fetch_sub(1, Ordering::Relaxed);
    if previous == 0 {
        metrics.running.store(0, Ordering::Relaxed);
        bail!("benchmark worker active-run counter underflowed");
    }
    if previous > 1 {
        return Ok(false);
    }
    release_work(fleet, metrics).await
}

async fn release_failed_claim(
    fleet: Option<&FleetManager>,
    metrics: &ControllerMetrics,
    fleet_transition: &AsyncMutex<()>,
) -> Result<bool> {
    let _transition = fleet_transition.lock().await;
    if metrics.running.load(Ordering::Relaxed) != 0 {
        return Ok(false);
    }
    release_work(fleet, metrics).await
}

async fn stop_claiming_if_terminating(
    fleet: Option<&FleetManager>,
    metrics: &ControllerMetrics,
    fleet_transition: &AsyncMutex<()>,
) -> Result<bool> {
    if !metrics.healthy.load(Ordering::Relaxed) {
        return Ok(true);
    }
    let Some(fleet) = fleet else {
        return Ok(false);
    };
    let _transition = fleet_transition.lock().await;
    if !fleet.termination_waiting().await? {
        return Ok(false);
    }
    metrics.healthy.store(false, Ordering::Relaxed);
    if metrics.running.load(Ordering::Relaxed) == 0 {
        fleet.complete_termination().await?;
    }
    Ok(true)
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
    credentials: &ProviderCredentials,
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
    if !claim.provider.supports_model(&claim.model_id) {
        post_run_result(
            academy,
            metrics,
            failure_result(&claim, "claim selected an unavailable model"),
        )
        .await;
        return;
    }
    if is_builtin_harness(&claim.repo_url)
        && !matches!(claim.benchmark_kind, BenchmarkKind::Frontier)
        && !claim.provider.supports_images(&claim.model_id)
    {
        post_run_result(
            academy,
            metrics,
            failure_result(
                &claim,
                "built-in visual agent requires an image-capable model",
            ),
        )
        .await;
        return;
    }
    let gateway = match GatewayHandle::start(
        &config.gateway_directory,
        claim.id,
        claim.provider,
        &config.aws_region,
        &claim.model_id,
        &claim.model_id,
        &config.reasoning_effort,
        credentials,
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
    metrics.start_gateway(claim.id, gateway.metrics.clone());
    let result = execute_run(config, academy, metrics, &claim, &gateway).await;
    match result {
        RunEnd::Result(result) => post_run_result(academy, metrics, result).await,
        RunEnd::Stale => eprintln!("dropping reclaimed run {}", claim.id),
    }
    metrics.finish_gateway(claim.id);
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
        provider: claim.provider,
        benchmark_kind: claim.benchmark_kind,
        deadline_at: claim.deadline_at,
        gateway_socket: gateway.socket_path.to_string_lossy().into_owned(),
        gateway_token: gateway.token.clone(),
        model_profile: claim.model_id.clone(),
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
    // A stopped run loses its lease in Academy. Poll often enough that the Stop button
    // tears down its executor and sandboxes promptly even before another event arrives.
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
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
                        let fingerprint = format!("{:x}", Sha256::digest(claim.model_id.as_bytes()));
                        let request = HarnessStageRequest {
                            id: claim.id,
                            lease_token: claim.lease_token,
                            status: "running".into(),
                            commit_sha,
                            bedrock_profile: claim.model_id.clone(),
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
                break RunEnd::Result(failure_result(
                    claim,
                    &format!("the {RUN_DEADLINE_SECONDS}-second benchmark deadline expired"),
                ));
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
    let skipped = json!({"status":"skipped"});
    let failed = || json!({"status":"infra_failed","error":message.clone()});
    HarnessResultRequest {
        id: claim.id,
        lease_token: claim.lease_token,
        status: "infra_failed".into(),
        benchmark_state: json!({
            "arc": if matches!(claim.benchmark_kind, BenchmarkKind::Frontier) {
                skipped.clone()
            } else {
                failed()
            },
            "frontier": if matches!(claim.benchmark_kind, BenchmarkKind::Arc) {
                skipped
            } else {
                failed()
            },
            "ram": failed()
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
            "exposure_benchmark_model_upstream_attempts_total {}\n",
            "exposure_benchmark_model_rate_limits_total {}\n",
            "exposure_benchmark_model_errors_total {}\n",
            "exposure_benchmark_model_input_tokens_total {}\n",
            "exposure_benchmark_model_output_tokens_total {}\n",
            "exposure_benchmark_model_latency_ms_total {}\n",
            "exposure_benchmark_model_completions_last_30_seconds {}\n"
        ),
        metrics.running.load(Ordering::Relaxed),
        metrics.claims.load(Ordering::Relaxed),
        metrics.completed.load(Ordering::Relaxed),
        metrics.failed.load(Ordering::Relaxed),
        metrics.academy_errors.load(Ordering::Relaxed),
        metrics.executor_errors.load(Ordering::Relaxed),
        metrics.fleet_errors.load(Ordering::Relaxed),
        metrics.frames_dropped.load(Ordering::Relaxed),
        model.requests,
        model.upstream_attempts,
        model.rate_limits,
        model.errors,
        model.input_tokens,
        model.output_tokens,
        model.latency_ms,
        model.completed_last_30_seconds,
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response()
}
