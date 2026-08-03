use crate::{constant_time_eq, random_token};
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use benchmark_protocol::ModelProvider;
use serde_json::{Map, Value, json};
use std::{
    collections::{HashSet, VecDeque},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_MESSAGES: usize = 12;
const MAX_TOOL_RESULT_BYTES: usize = 8 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_IMAGE_DATA_CHARS: usize = 384 * 1024;
const MAX_IMAGE_PARTS: usize = 16;
const MAX_OUTPUT_TOKENS: u64 = 2048;
const CEREBRAS_API_URL: &str = "https://api.cerebras.ai/v1/chat/completions";
const CEREBRAS_KEY_COUNT: usize = 4;
const CEREBRAS_CONCURRENCY_PER_KEY: usize = 8;
const NO_PREFERRED_KEY: usize = usize::MAX;

#[derive(Clone)]
enum GatewayUpstream {
    Bedrock {
        url: Arc<str>,
        key: Arc<str>,
        slots: Arc<Semaphore>,
    },
    Cerebras {
        pool: Arc<CerebrasKeyPool>,
        preferred_key: Arc<AtomicUsize>,
    },
}

#[derive(Clone)]
struct GatewayState {
    token: Arc<str>,
    model_id: Arc<str>,
    profile_name: Arc<str>,
    reasoning_effort: Arc<str>,
    provider: ModelProvider,
    upstream: GatewayUpstream,
    http: reqwest::Client,
    metrics: Arc<GatewayMetrics>,
}

struct CerebrasKeyState {
    credential: Arc<str>,
    slots: Arc<Semaphore>,
    in_flight: AtomicUsize,
    cooldown_until: Mutex<Option<Instant>>,
}

pub struct CerebrasKeyPool {
    keys: Vec<Arc<CerebrasKeyState>>,
    cursor: AtomicUsize,
    http: reqwest::Client,
    upstream_url: Arc<str>,
    default_cooldown: Duration,
}

struct CerebrasLease {
    key: Arc<CerebrasKeyState>,
    index: usize,
    _permit: OwnedSemaphorePermit,
}

impl Drop for CerebrasLease {
    fn drop(&mut self) {
        self.key.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

impl CerebrasKeyPool {
    pub fn new(keys: Vec<String>) -> Result<Self> {
        Self::with_url(keys, CEREBRAS_API_URL, Duration::from_secs(1))
    }

    fn with_url(keys: Vec<String>, upstream_url: &str, default_cooldown: Duration) -> Result<Self> {
        if keys.len() != CEREBRAS_KEY_COUNT
            || keys.iter().any(|key| key.len() < 20)
            || keys.iter().collect::<HashSet<_>>().len() != keys.len()
        {
            bail!("Cerebras key pool requires four distinct API keys");
        }
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(CEREBRAS_KEY_COUNT * CEREBRAS_CONCURRENCY_PER_KEY)
            .build()
            .context("build Cerebras HTTP client")?;
        Ok(Self {
            keys: keys
                .into_iter()
                .map(|credential| {
                    Arc::new(CerebrasKeyState {
                        credential: credential.into(),
                        slots: Arc::new(Semaphore::new(CEREBRAS_CONCURRENCY_PER_KEY)),
                        in_flight: AtomicUsize::new(0),
                        cooldown_until: Mutex::new(None),
                    })
                })
                .collect(),
            cursor: AtomicUsize::new(0),
            http,
            upstream_url: upstream_url.to_owned().into(),
            default_cooldown,
        })
    }

    fn try_acquire(&self, preferred: usize, excluded: u64) -> Option<CerebrasLease> {
        let now = Instant::now();
        let mut candidates = Vec::with_capacity(self.keys.len());
        for (index, key) in self.keys.iter().enumerate() {
            if excluded & (1 << index) != 0 {
                continue;
            }
            let healthy = {
                let mut cooldown = key.cooldown_until.lock().unwrap();
                if cooldown.is_some_and(|until| until <= now) {
                    *cooldown = None;
                }
                cooldown.is_none()
            };
            let load = key.in_flight.load(Ordering::Relaxed);
            if healthy && load < CEREBRAS_CONCURRENCY_PER_KEY {
                candidates.push((index, load));
            }
        }
        let minimum = candidates.iter().map(|(_, load)| *load).min()?;
        let selected = if candidates
            .iter()
            .any(|(index, load)| *index == preferred && *load == minimum)
        {
            preferred
        } else {
            let start = self.cursor.fetch_add(1, Ordering::Relaxed) % self.keys.len();
            (0..self.keys.len())
                .map(|offset| (start + offset) % self.keys.len())
                .find(|index| {
                    candidates
                        .iter()
                        .any(|(candidate, load)| candidate == index && *load == minimum)
                })?
        };
        let key = self.keys[selected].clone();
        let permit = key.slots.clone().try_acquire_owned().ok()?;
        key.in_flight.fetch_add(1, Ordering::Relaxed);
        Some(CerebrasLease {
            key,
            index: selected,
            _permit: permit,
        })
    }

    fn cool_down(&self, index: usize, duration: Duration) {
        *self.keys[index].cooldown_until.lock().unwrap() = Some(Instant::now() + duration);
    }
}

#[derive(Default)]
pub struct GatewayMetrics {
    requests: AtomicU64,
    upstream_attempts: AtomicU64,
    rate_limits: AtomicU64,
    errors: AtomicU64,
    input_tokens: AtomicU64,
    output_tokens: AtomicU64,
    latency_ms: AtomicU64,
    completions: Mutex<VecDeque<Instant>>,
    key_requests: Mutex<Vec<u64>>,
}

#[derive(Clone)]
pub struct GatewaySnapshot {
    pub requests: u64,
    pub upstream_attempts: u64,
    pub rate_limits: u64,
    pub errors: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub latency_ms: u64,
    pub completed_last_30_seconds: usize,
    pub key_requests: Vec<u64>,
}

impl GatewayMetrics {
    fn record_attempt(&self, key_index: Option<usize>) {
        self.upstream_attempts.fetch_add(1, Ordering::Relaxed);
        if let Some(index) = key_index {
            let mut requests = self.key_requests.lock().unwrap();
            if requests.len() <= index {
                requests.resize(index + 1, 0);
            }
            requests[index] += 1;
        }
    }

    fn record_rate_limit(&self) {
        self.rate_limits.fetch_add(1, Ordering::Relaxed);
    }

    fn record(&self, response: Option<&Value>, latency: Duration, error: bool) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.latency_ms
            .fetch_add(latency.as_millis() as u64, Ordering::Relaxed);
        if error {
            self.errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let usage = response
            .and_then(|value| value.get("usage"))
            .and_then(Value::as_object);
        self.input_tokens.fetch_add(
            usage
                .and_then(|value| {
                    value
                        .get("prompt_tokens")
                        .or_else(|| value.get("input_tokens"))
                })
                .and_then(Value::as_u64)
                .unwrap_or(0),
            Ordering::Relaxed,
        );
        self.output_tokens.fetch_add(
            usage
                .and_then(|value| {
                    value
                        .get("completion_tokens")
                        .or_else(|| value.get("output_tokens"))
                })
                .and_then(Value::as_u64)
                .unwrap_or(0),
            Ordering::Relaxed,
        );
        let mut completions = self.completions.lock().unwrap();
        completions.push_back(Instant::now());
        while completions.len() > 10_000 {
            completions.pop_front();
        }
    }

    pub fn snapshot(&self) -> GatewaySnapshot {
        let now = Instant::now();
        let mut completions = self.completions.lock().unwrap();
        while completions
            .front()
            .is_some_and(|time| now.duration_since(*time) > Duration::from_secs(120))
        {
            completions.pop_front();
        }
        GatewaySnapshot {
            requests: self.requests.load(Ordering::Relaxed),
            upstream_attempts: self.upstream_attempts.load(Ordering::Relaxed),
            rate_limits: self.rate_limits.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            input_tokens: self.input_tokens.load(Ordering::Relaxed),
            output_tokens: self.output_tokens.load(Ordering::Relaxed),
            latency_ms: self.latency_ms.load(Ordering::Relaxed),
            completed_last_30_seconds: completions
                .iter()
                .filter(|time| now.duration_since(**time) <= Duration::from_secs(30))
                .count(),
            key_requests: self.key_requests.lock().unwrap().clone(),
        }
    }
}

pub struct GatewayHandle {
    pub socket_path: PathBuf,
    pub token: String,
    pub metrics: Arc<GatewayMetrics>,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl GatewayHandle {
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        directory: &Path,
        run_id: uuid::Uuid,
        provider: ModelProvider,
        region: &str,
        model_id: &str,
        profile_name: &str,
        reasoning_effort: &str,
        bedrock_key: &str,
        cerebras_pool: Arc<CerebrasKeyPool>,
        maximum_concurrency: usize,
    ) -> Result<Self> {
        let run_directory = directory.join(run_id.to_string());
        tokio::fs::create_dir_all(&run_directory)
            .await
            .context("create gateway directory")?;
        // The executor's shared group may traverse and connect, but cannot replace gateway
        // files. setgid gives the socket that shared group instead of the private controller
        // group.
        tokio::fs::set_permissions(&run_directory, std::fs::Permissions::from_mode(0o2750))
            .await
            .context("protect gateway directory")?;
        let socket_path = run_directory.join("bedrock.sock");
        if tokio::fs::try_exists(&socket_path).await.unwrap_or(false) {
            tokio::fs::remove_file(&socket_path)
                .await
                .context("remove stale gateway socket")?;
        }
        let listener = tokio::net::UnixListener::bind(&socket_path)
            .context("bind Bedrock gateway Unix socket")?;
        tokio::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))
            .await
            .context("protect gateway socket")?;

        let token = random_token();
        let metrics = Arc::new(GatewayMetrics::default());
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(20))
            .pool_max_idle_per_host(maximum_concurrency)
            .build()
            .context("build Bedrock HTTP client")?;
        let upstream = match provider {
            ModelProvider::Bedrock => GatewayUpstream::Bedrock {
                url: format!("https://bedrock-mantle.{region}.api.aws/openai/v1/chat/completions")
                    .into(),
                key: bedrock_key.to_owned().into(),
                slots: Arc::new(Semaphore::new(maximum_concurrency)),
            },
            ModelProvider::Cerebras => GatewayUpstream::Cerebras {
                pool: cerebras_pool,
                preferred_key: Arc::new(AtomicUsize::new(NO_PREFERRED_KEY)),
            },
        };
        let state = GatewayState {
            token: token.clone().into(),
            model_id: model_id.to_owned().into(),
            profile_name: profile_name.to_owned().into(),
            reasoning_effort: reasoning_effort.to_owned().into(),
            provider,
            upstream,
            http,
            metrics: metrics.clone(),
        };
        let router = Router::new()
            .route("/health", get(health))
            .route("/metrics", get(metrics_handler))
            .route("/v1/models", get(models))
            .route("/v1/chat/completions", post(chat_completions))
            .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
            .with_state(state);
        let (shutdown, signal) = oneshot::channel();
        let task = tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = signal.await;
                })
                .await;
            if let Err(error) = result {
                eprintln!("Bedrock gateway stopped unexpectedly: {error}");
            }
        });
        Ok(Self {
            socket_path,
            token,
            metrics,
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
        }
        let _ = tokio::fs::remove_file(&self.socket_path).await;
        if let Some(directory) = self.socket_path.parent() {
            let _ = tokio::fs::remove_dir(directory).await;
        }
    }
}

impl Drop for GatewayHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn health() -> Json<Value> {
    Json(json!({"ok": true}))
}

async fn metrics_handler(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let snapshot = state.metrics.snapshot();
    Json(json!({
        "requests": snapshot.requests,
        "upstream_attempts": snapshot.upstream_attempts,
        "rate_limits": snapshot.rate_limits,
        "errors": snapshot.errors,
        "input_tokens": snapshot.input_tokens,
        "output_tokens": snapshot.output_tokens,
        "latency_ms": snapshot.latency_ms,
        "completed_last_30s": snapshot.completed_last_30_seconds,
        "key_requests": snapshot.key_requests,
    }))
    .into_response()
}

async fn models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if !authorized(&state, &headers) {
        return error_response(StatusCode::UNAUTHORIZED, "invalid gateway capability");
    }
    Json(json!({
        "object": "list",
        "data": [{
            "id": state.profile_name.as_ref(),
            "object": "model",
            "created": 0,
            "owned_by": state.provider.as_str()
        }]
    }))
    .into_response()
}

async fn chat_completions(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&state, &headers) {
        return error_response(StatusCode::UNAUTHORIZED, "invalid gateway capability");
    }
    let request = match sanitize_chat(body, &state.model_id, &state.reasoning_effort) {
        Ok(request) => request,
        Err(error) => return error_response(StatusCode::BAD_REQUEST, &error.to_string()),
    };
    let started = Instant::now();
    let value = match &state.upstream {
        GatewayUpstream::Bedrock { url, key, slots } => {
            send_bedrock(&state, url, key, slots, &request).await
        }
        GatewayUpstream::Cerebras {
            pool,
            preferred_key,
        } => send_cerebras(&state, pool, preferred_key, &request).await,
    };
    let mut value = match value {
        Ok(value) => value,
        Err((status, message)) => {
            state.metrics.record(None, started.elapsed(), true);
            return error_response(status, message);
        }
    };
    state.metrics.record(Some(&value), started.elapsed(), false);
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "model".into(),
            Value::String(state.profile_name.to_string()),
        );
    }
    let snapshot = state.metrics.snapshot();
    let mut response = Json(value).into_response();
    let completed = HeaderValue::from(snapshot.completed_last_30_seconds);
    response
        .headers_mut()
        .insert("x-bedrock-completed-last-30s", completed.clone());
    response
        .headers_mut()
        .insert("x-model-completed-last-30s", completed);
    response
}

async fn send_bedrock(
    state: &GatewayState,
    url: &str,
    key: &str,
    slots: &Arc<Semaphore>,
    request: &Value,
) -> std::result::Result<Value, (StatusCode, &'static str)> {
    let permit = match tokio::time::timeout(Duration::from_secs(2), slots.acquire()).await {
        Ok(Ok(permit)) => permit,
        _ => return Err((StatusCode::TOO_MANY_REQUESTS, "gateway concurrency is full")),
    };
    state.metrics.record_attempt(None);
    let response = state
        .http
        .post(url)
        .bearer_auth(key)
        .json(request)
        .send()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "Bedrock request failed"))?;
    drop(permit);
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        state.metrics.record_rate_limit();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Bedrock rate limited the request",
        ));
    }
    if !response.status().is_success() {
        return Err((StatusCode::BAD_GATEWAY, "Bedrock rejected the request"));
    }
    response
        .json::<Value>()
        .await
        .map_err(|_| (StatusCode::BAD_GATEWAY, "Bedrock returned invalid JSON"))
}

async fn send_cerebras(
    state: &GatewayState,
    pool: &CerebrasKeyPool,
    preferred_key: &AtomicUsize,
    request: &Value,
) -> std::result::Result<Value, (StatusCode, &'static str)> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut excluded = 0u64;
    loop {
        let preferred = preferred_key.load(Ordering::Relaxed);
        let lease = loop {
            if let Some(lease) = pool.try_acquire(preferred, excluded) {
                break lease;
            }
            if Instant::now() >= deadline {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "no Cerebras key is currently available",
                ));
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        };
        state.metrics.record_attempt(Some(lease.index));
        let response = pool
            .http
            .post(pool.upstream_url.as_ref())
            .bearer_auth(lease.key.credential.as_ref())
            .json(request)
            .send()
            .await
            .map_err(|_| (StatusCode::BAD_GATEWAY, "Cerebras request failed"))?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            state.metrics.record_rate_limit();
            let cooldown = rate_limit_cooldown(response.headers(), pool.default_cooldown);
            pool.cool_down(lease.index, cooldown);
            excluded |= 1 << lease.index;
            drop(lease);
            if excluded.count_ones() as usize == pool.keys.len() {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "all Cerebras keys are rate limited",
                ));
            }
            continue;
        }
        if !response.status().is_success() {
            return Err((StatusCode::BAD_GATEWAY, "Cerebras rejected the request"));
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|_| (StatusCode::BAD_GATEWAY, "Cerebras returned invalid JSON"))?;
        preferred_key.store(lease.index, Ordering::Relaxed);
        return Ok(value);
    }
}

fn rate_limit_cooldown(headers: &reqwest::header::HeaderMap, fallback: Duration) -> Duration {
    ["retry-after", "x-ratelimit-reset-tokens-minute"]
        .iter()
        .find_map(|name| {
            headers
                .get(*name)?
                .to_str()
                .ok()?
                .parse::<f64>()
                .ok()
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
                .map(|seconds| Duration::from_secs_f64(seconds.clamp(0.1, 60.0)))
        })
        .unwrap_or(fallback)
}

fn authorized(state: &GatewayState, headers: &HeaderMap) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|value| constant_time_eq(value.as_bytes(), state.token.as_bytes()))
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let payload = json!({"error":{"message":message,"type":"invalid_request_error"}});
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

fn sanitize_message_content(content: &Value, role: &str) -> Result<Value> {
    if let Some(text) = content.as_str() {
        return Ok(Value::String(clip(
            text,
            if role == "tool" {
                MAX_TOOL_RESULT_BYTES
            } else {
                MAX_TEXT_BYTES
            },
        )));
    }
    let parts = content
        .as_array()
        .filter(|_| role == "user")
        .context("message content must be text or user image parts")?;
    if parts.is_empty() || parts.len() > MAX_IMAGE_PARTS + 1 {
        bail!("multimodal messages contain too many parts");
    }
    let mut images = 0usize;
    let mut clean = Vec::with_capacity(parts.len());
    for part in parts {
        let part = part.as_object().context("message parts must be objects")?;
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                let text = part
                    .get("text")
                    .and_then(Value::as_str)
                    .context("text parts require text")?;
                clean.push(json!({"type":"text","text":clip(text, MAX_TEXT_BYTES)}));
            }
            Some("image_url") => {
                images += 1;
                if images > MAX_IMAGE_PARTS {
                    bail!("multimodal messages contain too many images");
                }
                let url = part
                    .get("image_url")
                    .and_then(Value::as_object)
                    .and_then(|image| image.get("url"))
                    .and_then(Value::as_str)
                    .context("image parts require a URL")?;
                let encoded = ["data:image/png;base64,", "data:image/jpeg;base64,"]
                    .iter()
                    .find_map(|prefix| url.strip_prefix(prefix))
                    .context("only inline PNG or JPEG images are allowed")?;
                if encoded.is_empty()
                    || encoded.len() > MAX_IMAGE_DATA_CHARS
                    || !encoded.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
                    })
                {
                    bail!("image data is invalid or too large");
                }
                clean.push(json!({"type":"image_url","image_url":{"url":url}}));
            }
            _ => bail!("unsupported message part"),
        }
    }
    Ok(Value::Array(clean))
}

fn sanitize_chat(body: Value, model_id: &str, reasoning_effort: &str) -> Result<Value> {
    let object = body.as_object().context("request body must be an object")?;
    if object.get("stream").and_then(Value::as_bool) == Some(true) {
        bail!("streaming is not supported");
    }
    let source = object
        .get("messages")
        .and_then(Value::as_array)
        .context("messages must be an array")?;
    let mut instructions = Vec::new();
    let mut messages = Vec::new();
    for message in source {
        let source = message
            .as_object()
            .context("each message must be an object")?;
        let role = source
            .get("role")
            .and_then(Value::as_str)
            .context("each message requires a role")?;
        if matches!(role, "system" | "developer") {
            if let Some(text) = source.get("content").and_then(Value::as_str) {
                instructions.push(clip(text, MAX_TEXT_BYTES));
            }
            continue;
        }
        if !matches!(role, "user" | "assistant" | "tool") {
            bail!("unsupported message role: {role}");
        }
        let mut clean = Map::new();
        clean.insert("role".into(), Value::String(role.into()));
        match source.get("content") {
            Some(Value::Null) | None if role == "assistant" => {
                clean.insert("content".into(), Value::Null);
            }
            Some(content) => {
                clean.insert("content".into(), sanitize_message_content(content, role)?);
            }
            _ => bail!("message content is required"),
        }
        if role == "tool" {
            let call_id = source
                .get("tool_call_id")
                .and_then(Value::as_str)
                .context("tool messages require tool_call_id")?;
            clean.insert("tool_call_id".into(), Value::String(clip(call_id, 200)));
        }
        if role == "assistant"
            && let Some(calls) = source.get("tool_calls")
        {
            if !calls.is_array() {
                bail!("tool_calls must be an array");
            }
            clean.insert("tool_calls".into(), calls.clone());
        }
        messages.push(Value::Object(clean));
    }
    if messages.len() > MAX_MESSAGES {
        // Terminus puts the task description in its first user turn. Keep that
        // anchor as well as the most recent terminal context so longer runs do
        // not lose what they are trying to solve.
        let initial_task = messages
            .first()
            .filter(|message| message.get("role").and_then(Value::as_str) == Some("user"))
            .cloned();
        messages = messages.split_off(messages.len() - (MAX_MESSAGES - 1));
        if let Some(initial_task) = initial_task {
            messages.insert(0, initial_task);
        }
    }
    while messages
        .first()
        .and_then(|message| message.get("role"))
        .and_then(Value::as_str)
        != Some("user")
    {
        if messages.is_empty() {
            bail!("messages require a user turn");
        }
        messages.remove(0);
    }
    if !instructions.is_empty() {
        messages.insert(
            0,
            json!({"role":"system","content":clip(&instructions.join("\n\n"), MAX_TEXT_BYTES)}),
        );
    }
    let mut result = Map::new();
    result.insert("model".into(), Value::String(model_id.into()));
    result.insert("messages".into(), Value::Array(messages));
    let requested = object
        .get("max_completion_tokens")
        .or_else(|| object.get("max_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(MAX_OUTPUT_TOKENS)
        .clamp(1, MAX_OUTPUT_TOKENS);
    result.insert("max_completion_tokens".into(), Value::from(requested));
    if let Some(value) = object.get("temperature") {
        let temperature = value.as_f64().context("temperature must be a number")?;
        if !(0.0..=2.0).contains(&temperature) {
            bail!("temperature must be between 0 and 2");
        }
        result.insert("temperature".into(), Value::from(temperature));
    }
    if model_supports_reasoning_effort(model_id) {
        result.insert(
            "reasoning_effort".into(),
            Value::String(reasoning_effort.into()),
        );
    }
    if let Some(tools) = object.get("tools") {
        let tools = tools.as_array().context("tools must be an array")?;
        if tools.len() > 64 {
            bail!("at most 64 tools are allowed");
        }
        for tool in tools {
            let function = tool
                .get("function")
                .and_then(Value::as_object)
                .context("only function tools are supported")?;
            let name = function
                .get("name")
                .and_then(Value::as_str)
                .context("tool name is required")?;
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                bail!("invalid tool name");
            }
        }
        result.insert("tools".into(), Value::Array(tools.clone()));
        if let Some(choice) = object.get("tool_choice") {
            result.insert("tool_choice".into(), choice.clone());
        }
    }
    if let Some(response_format) = object.get("response_format") {
        let format = response_format
            .as_object()
            .context("response_format must be an object")?;
        let kind = format
            .get("type")
            .and_then(Value::as_str)
            .context("response_format requires a type")?;
        if !matches!(kind, "json_object" | "json_schema") {
            bail!("unsupported response_format type: {kind}");
        }
        if serde_json::to_vec(response_format)?.len() > MAX_TEXT_BYTES {
            bail!("response_format exceeds {MAX_TEXT_BYTES} bytes");
        }
        result.insert("response_format".into(), response_format.clone());
    }
    Ok(Value::Object(result))
}

fn model_supports_reasoning_effort(model_id: &str) -> bool {
    model_id.starts_with("xai.")
        || model_id.starts_with("openai.")
        || model_id.starts_with("google.gemma-4-")
}

fn clip(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let mut end = maximum;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn concurrent_cerebras_requests_use_four_keys_and_fail_over_after_429() {
        #[derive(Clone)]
        struct MockState {
            keys: Arc<Vec<String>>,
            attempts: Arc<Mutex<Vec<u64>>>,
            rate_limited: Arc<std::sync::atomic::AtomicBool>,
        }

        async fn mock_completion(State(state): State<MockState>, headers: HeaderMap) -> Response {
            let credential = headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .unwrap();
            let index = state.keys.iter().position(|key| key == credential).unwrap();
            state.attempts.lock().unwrap()[index] += 1;
            if index == 0 && !state.rate_limited.swap(true, Ordering::Relaxed) {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(header::RETRY_AFTER, "60")],
                    Json(json!({"error":"mock limit"})),
                )
                    .into_response();
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
            Json(json!({
                "id":"mock",
                "choices":[{"message":{"role":"assistant","content":"ok"}}],
                "usage":{"prompt_tokens":1,"completion_tokens":1}
            }))
            .into_response()
        }

        let keys = Arc::new(
            (0..CEREBRAS_KEY_COUNT)
                .map(|index| format!("test-cerebras-api-key-{index:02}"))
                .collect::<Vec<_>>(),
        );
        let attempts = Arc::new(Mutex::new(vec![0; CEREBRAS_KEY_COUNT]));
        let rate_limited = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mock = Router::new()
            .route("/", post(mock_completion))
            .with_state(MockState {
                keys: keys.clone(),
                attempts: attempts.clone(),
                rate_limited,
            });
        let server = tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });

        let pool = Arc::new(
            CerebrasKeyPool::with_url(
                keys.as_ref().clone(),
                &format!("http://{address}/"),
                Duration::from_secs(60),
            )
            .unwrap(),
        );
        let state = Arc::new(GatewayState {
            token: "test-gateway-token".into(),
            model_id: "gemma-4-31b".into(),
            profile_name: "gemma-4-31b".into(),
            reasoning_effort: "none".into(),
            provider: ModelProvider::Cerebras,
            upstream: GatewayUpstream::Cerebras {
                pool: pool.clone(),
                preferred_key: Arc::new(AtomicUsize::new(NO_PREFERRED_KEY)),
            },
            http: reqwest::Client::new(),
            metrics: Arc::new(GatewayMetrics::default()),
        });
        let request = Arc::new(json!({
            "model":"gemma-4-31b",
            "messages":[{"role":"user","content":"ready"}]
        }));
        let mut calls = Vec::new();
        for _ in 0..8 {
            let state = state.clone();
            let request = request.clone();
            calls.push(tokio::spawn(async move {
                let GatewayUpstream::Cerebras {
                    pool,
                    preferred_key,
                } = &state.upstream
                else {
                    unreachable!()
                };
                send_cerebras(&state, pool, preferred_key, &request).await
            }));
        }
        for call in calls {
            assert!(call.await.unwrap().is_ok());
        }

        let snapshot = state.metrics.snapshot();
        assert_eq!(snapshot.rate_limits, 1);
        assert_eq!(snapshot.upstream_attempts, 9);
        assert_eq!(snapshot.key_requests.len(), CEREBRAS_KEY_COUNT);
        assert!(snapshot.key_requests.iter().all(|attempts| *attempts > 0));

        let before = attempts.lock().unwrap().clone();
        for _ in 0..2 {
            let GatewayUpstream::Cerebras {
                pool,
                preferred_key,
            } = &state.upstream
            else {
                unreachable!()
            };
            assert!(
                send_cerebras(&state, pool, preferred_key, &request)
                    .await
                    .is_ok()
            );
        }
        let after = attempts.lock().unwrap().clone();
        let deltas = after
            .iter()
            .zip(before)
            .map(|(after, before)| after - before)
            .collect::<Vec<_>>();
        assert_eq!(deltas.iter().filter(|delta| **delta > 0).count(), 1);
        assert!(deltas.contains(&2));

        server.abort();
    }

    #[test]
    fn sanitization_pins_model_keeps_bounded_sampling_and_task_context() {
        let mut messages = vec![json!({"role":"system","content":"rules"})];
        for index in 0..20 {
            messages.push(json!({"role":"user","content":format!("question {index}")}));
            messages.push(json!({"role":"assistant","content":format!("answer {index}")}));
        }
        let result = sanitize_chat(
            json!({
                "model":"attacker-choice",
                "messages": messages,
                "temperature": 2,
                "stream": false,
                "max_tokens": 9000
            }),
            "xai.grok-4.3",
            "none",
        )
        .unwrap();
        assert_eq!(result["model"], "xai.grok-4.3");
        assert_eq!(result["reasoning_effort"], "none");
        assert_eq!(result["max_completion_tokens"], MAX_OUTPUT_TOKENS);
        assert_eq!(result["temperature"].as_f64(), Some(2.0));
        assert!(result["messages"].as_array().unwrap().len() <= MAX_MESSAGES + 1);
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][1]["role"], "user");
        assert_eq!(result["messages"][1]["content"], "question 0");
    }

    #[test]
    fn sanitization_caps_tool_output_and_rejects_streaming() {
        let result = sanitize_chat(
            json!({"messages":[
                {"role":"user","content":"go"},
                {"role":"assistant","content":null,"tool_calls":[]},
                {"role":"tool","tool_call_id":"call_1","content":"x".repeat(9000)},
                {"role":"user","content":"continue"}
            ]}),
            "xai.grok-4.3",
            "none",
        )
        .unwrap();
        assert_eq!(
            result["messages"][2]["content"].as_str().unwrap().len(),
            8192
        );
        assert!(
            sanitize_chat(
                json!({"messages":[{"role":"user","content":"go"}],"stream":true}),
                "xai.grok-4.3",
                "none"
            )
            .is_err()
        );
    }

    #[test]
    fn sanitization_preserves_structured_output() {
        let response_format = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "terminus_action",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {"analysis": {"type": "string"}},
                    "required": ["analysis"],
                    "additionalProperties": false
                }
            }
        });
        let result = sanitize_chat(
            json!({
                "messages": [{"role": "user", "content": "choose an action"}],
                "response_format": response_format,
            }),
            "gemma-4-31b",
            "none",
        )
        .unwrap();
        assert_eq!(result["response_format"], response_format);
    }

    #[test]
    fn sanitization_keeps_bounded_inline_images_only() {
        let result = sanitize_chat(
            json!({"messages":[{"role":"user","content":[
                {"type":"image_url","image_url":{"url":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="}},
                {"type":"text","text":"choose an action"}
            ]}]}),
            "google.gemma-4-31b",
            "none",
        )
        .unwrap();
        assert_eq!(
            result["messages"][0]["content"][0]["image_url"]["url"],
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        );
        assert!(
            sanitize_chat(
                json!({"messages":[{"role":"user","content":[
                    {"type":"image_url","image_url":{"url":"https://example.com/board.png"}}
                ]}]}),
                "google.gemma-4-31b",
                "none",
            )
            .is_err()
        );
        let mistral = sanitize_chat(
            json!({"messages":[{"role":"user","content":"go"}]}),
            "mistral.mistral-large-3-675b-instruct",
            "none",
        )
        .unwrap();
        assert!(mistral.get("reasoning_effort").is_none());
    }
}
