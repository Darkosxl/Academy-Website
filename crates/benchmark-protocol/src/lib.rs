use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use uuid::Uuid;

pub const BENCHMARK_VERSION: &str = "harness-2026-sprint-v3";
pub const ARC_CONCURRENCY: usize = 5;
pub const RUN_DEADLINE_SECONDS: i64 = 9 * 60 * 60;
pub const DEFAULT_BEDROCK_MODEL: &str = "xai.grok-4.3";
pub const DEFAULT_CEREBRAS_MODEL: &str = "gemma-4-31b";
pub const CEREBRAS_MODEL_IDS: &[&str] = &["gemma-4-31b", "zai-glm-4.7"];
pub const CEREBRAS_IMAGE_MODEL_IDS: &[&str] = &["gemma-4-31b"];
pub const BEDROCK_MODEL_IDS: &[&str] = &[
    "deepseek.v3.1",
    "deepseek.v3.2",
    "google.gemma-3-12b-it",
    "google.gemma-3-27b-it",
    "google.gemma-3-4b-it",
    "google.gemma-4-26b-a4b",
    "google.gemma-4-31b",
    "google.gemma-4-e2b",
    "minimax.minimax-m2",
    "minimax.minimax-m2.1",
    "minimax.minimax-m2.5",
    "mistral.devstral-2-123b",
    "mistral.magistral-small-2509",
    "mistral.ministral-3-14b-instruct",
    "mistral.ministral-3-3b-instruct",
    "mistral.ministral-3-8b-instruct",
    "mistral.mistral-large-3-675b-instruct",
    "mistral.voxtral-mini-3b-2507",
    "mistral.voxtral-small-24b-2507",
    "moonshotai.kimi-k2-thinking",
    "moonshotai.kimi-k2.5",
    "nvidia.nemotron-nano-12b-v2",
    "nvidia.nemotron-nano-3-30b",
    "nvidia.nemotron-nano-9b-v2",
    "nvidia.nemotron-super-3-120b",
    "openai.gpt-oss-120b",
    "openai.gpt-oss-20b",
    "openai.gpt-oss-safeguard-120b",
    "openai.gpt-oss-safeguard-20b",
    "qwen.qwen3-235b-a22b-2507",
    "qwen.qwen3-32b",
    "qwen.qwen3-coder-30b-a3b-instruct",
    "qwen.qwen3-coder-480b-a35b-instruct",
    "qwen.qwen3-coder-next",
    "qwen.qwen3-next-80b-a3b-instruct",
    "qwen.qwen3-vl-235b-a22b-instruct",
    "writer.palmyra-vision-7b",
    "xai.grok-4.3",
    "zai.glm-4.6",
    "zai.glm-4.7",
    "zai.glm-4.7-flash",
    "zai.glm-5",
];

pub fn is_bedrock_model(model_id: &str) -> bool {
    BEDROCK_MODEL_IDS.binary_search(&model_id).is_ok()
}

pub const BEDROCK_IMAGE_MODEL_IDS: &[&str] = &[
    "google.gemma-3-12b-it",
    "google.gemma-3-27b-it",
    "google.gemma-3-4b-it",
    "google.gemma-4-26b-a4b",
    "google.gemma-4-31b",
    "google.gemma-4-e2b",
    "qwen.qwen3-vl-235b-a22b-instruct",
    "writer.palmyra-vision-7b",
    "xai.grok-4.3",
];

pub fn bedrock_model_supports_images(model_id: &str) -> bool {
    BEDROCK_IMAGE_MODEL_IDS.binary_search(&model_id).is_ok()
}

pub fn is_cerebras_model(model_id: &str) -> bool {
    CEREBRAS_MODEL_IDS.binary_search(&model_id).is_ok()
}

pub fn cerebras_model_supports_images(model_id: &str) -> bool {
    CEREBRAS_IMAGE_MODEL_IDS.binary_search(&model_id).is_ok()
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    #[default]
    Bedrock,
    Cerebras,
}

impl ModelProvider {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bedrock => "bedrock",
            Self::Cerebras => "cerebras",
        }
    }

    pub fn supports_model(self, model_id: &str) -> bool {
        match self {
            Self::Bedrock => is_bedrock_model(model_id),
            Self::Cerebras => is_cerebras_model(model_id),
        }
    }

    pub fn supports_images(self, model_id: &str) -> bool {
        match self {
            Self::Bedrock => bedrock_model_supports_images(model_id),
            Self::Cerebras => cerebras_model_supports_images(model_id),
        }
    }
}

impl fmt::Display for ModelProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ModelProvider {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bedrock" => Ok(Self::Bedrock),
            "cerebras" => Ok(Self::Cerebras),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkKind {
    Arc,
    Frontier,
    #[default]
    Bundled,
}

impl BenchmarkKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Arc => "arc",
            Self::Frontier => "frontier",
            Self::Bundled => "bundled",
        }
    }
}

impl fmt::Display for BenchmarkKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BenchmarkKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "arc" => Ok(Self::Arc),
            "frontier" => Ok(Self::Frontier),
            "bundled" => Ok(Self::Bundled),
            _ => Err(()),
        }
    }
}

pub const BUILTIN_HARNESSES: [(&str, &str, &str); 2] = [
    ("forge", "builtin://forge", "Forge"),
    ("reki", "builtin://reki", "Reki"),
];

pub fn builtin_harness_uri(id: &str) -> Option<&'static str> {
    BUILTIN_HARNESSES
        .iter()
        .find(|(candidate, _, _)| *candidate == id)
        .map(|(_, uri, _)| *uri)
}

pub fn builtin_harness_label(uri: &str) -> Option<&'static str> {
    BUILTIN_HARNESSES
        .iter()
        .find(|(_, candidate, _)| *candidate == uri)
        .map(|(_, _, label)| *label)
}

pub fn is_builtin_harness(uri: &str) -> bool {
    builtin_harness_label(uri).is_some()
}

fn default_bedrock_model() -> String {
    DEFAULT_BEDROCK_MODEL.into()
}

pub const ARC_GAMES: [&str; 25] = [
    "bp35", "m0r0", "ft09", "ar25", "s5i5", "sp80", "g50t", "lp85", "r11l", "sc25", "tn36", "sk48",
    "re86", "wa30", "cn04", "tu93", "tr87", "sb26", "su15", "ls20", "ka59", "cd82", "dc22", "vc33",
    "lf52",
];
pub const FRONTIER_TASKS: [&str; 5] = [
    "html-js-filter",
    "vllm-deepseek-streaming",
    "session-window-debug",
    "mvcc-lsm-compaction",
    "embedding-drift-monitor",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessClaim {
    pub id: Uuid,
    pub repo_url: String,
    #[serde(default)]
    pub provider: ModelProvider,
    #[serde(default)]
    pub benchmark_kind: BenchmarkKind,
    #[serde(default = "default_bedrock_model")]
    pub model_id: String,
    pub lease_token: Uuid,
    pub deadline_at: DateTime<Utc>,
    pub benchmark_version: String,
}

/// Global demand for benchmark worker slots. Academy is the source of truth; workers
/// publish this snapshot to CloudWatch so the private fleet can scale without exposing
/// either the queue or an EC2 endpoint publicly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessCapacity {
    pub queued: u64,
    pub active: u64,
    pub oldest_queued_seconds: u64,
}

impl HarnessCapacity {
    pub fn demand(&self) -> u64 {
        self.queued.saturating_add(self.active)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessLeaseRequest {
    pub id: Uuid,
    pub lease_token: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarnessStageRequest {
    pub id: Uuid,
    pub lease_token: Uuid,
    pub status: String,
    pub commit_sha: String,
    pub bedrock_profile: String,
    pub bedrock_profile_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessProgressRequest {
    pub id: Uuid,
    pub lease_token: Uuid,
    pub benchmark: String,
    pub state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessResultRequest {
    pub id: Uuid,
    pub lease_token: Uuid,
    pub status: String,
    pub benchmark_state: Value,
    pub score_arc: Option<f32>,
    pub score_frontier: Option<f32>,
    pub ram_1session_mb: Option<f32>,
    pub ram_10session_mb: Option<f32>,
    pub error_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArcFrame {
    pub game: String,
    pub seq: i32,
    pub grids: String,
    pub state: String,
    pub levels_completed: i32,
    pub baseline: Option<Vec<i32>>,
    pub action: Option<String>,
    pub action_x: Option<i32>,
    pub action_y: Option<i32>,
}

impl ArcFrame {
    pub fn is_valid(&self) -> bool {
        self.seq >= 0
            && self.game.len() == 4
            && self
                .game
                .bytes()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9'))
            && self.grids.len() <= 16 * 4097
            && self
                .grids
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f' | b'\n'))
            && (1..=16).contains(&self.grids.split('\n').count())
            && self.grids.split('\n').all(|grid| grid.len() == 4096)
            && matches!(
                self.state.as_str(),
                "NOT_PLAYED" | "NOT_FINISHED" | "WIN" | "GAME_OVER"
            )
            && (0..=254).contains(&self.levels_completed)
            && [self.action_x, self.action_y]
                .into_iter()
                .flatten()
                .all(|coordinate| (0..=63).contains(&coordinate))
            && self.action.as_ref().is_none_or(|action| {
                (1..=16).contains(&action.len())
                    && action
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
            && self
                .baseline
                .as_ref()
                .is_none_or(|values| values.len() <= 64 && values.iter().all(|value| *value >= 0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArcFramesRequest {
    pub run_id: Uuid,
    pub lease_token: Uuid,
    pub frames: Vec<ArcFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KaggleClaim {
    pub id: Uuid,
    pub run_id: Uuid,
    pub phase: String,
    pub repo_url: String,
    pub commit_sha: String,
    pub username: String,
    pub token: String,
    pub lease_token: Uuid,
    pub benchmark_version: String,
    pub competition: String,
    pub kernel_slug: Option<String>,
    pub kernel_version: Option<i32>,
    pub submission_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KaggleResultRequest {
    pub id: Uuid,
    pub lease_token: Uuid,
    pub status: String,
    pub kernel_slug: Option<String>,
    pub kernel_version: Option<i32>,
    pub submission_ref: Option<String>,
    pub public_score: Option<f32>,
    pub private_score: Option<f32>,
    pub status_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutorRequest {
    Run {
        run_id: Uuid,
        repo_url: String,
        #[serde(default)]
        provider: ModelProvider,
        #[serde(default)]
        benchmark_kind: BenchmarkKind,
        deadline_at: DateTime<Utc>,
        gateway_socket: String,
        gateway_token: String,
        model_profile: String,
    },
    Kaggle {
        claim: KaggleClaim,
    },
    Ping,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutorEvent {
    Pong,
    Ready {
        commit_sha: String,
    },
    Progress {
        benchmark: String,
        state: Value,
    },
    Frames {
        frames: Vec<ArcFrame>,
    },
    Result {
        status: String,
        benchmark_state: Value,
        score_arc: Option<f32>,
        score_frontier: Option<f32>,
        ram_1session_mb: Option<f32>,
        ram_10session_mb: Option<f32>,
        error_log: Option<String>,
    },
    KaggleResult {
        result: KaggleResultRequest,
    },
    Log {
        level: String,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (Uuid, Uuid) {
        (
            Uuid::parse_str("018f0f65-9abc-7def-8123-456789abcdef").unwrap(),
            Uuid::parse_str("118f0f65-9abc-7def-8123-456789abcdef").unwrap(),
        )
    }

    #[test]
    fn claim_wire_shape_stays_compatible() {
        let (id, lease_token) = ids();
        let claim = HarnessClaim {
            id,
            repo_url: "https://github.com/example/agent".into(),
            provider: ModelProvider::Cerebras,
            benchmark_kind: BenchmarkKind::Arc,
            model_id: DEFAULT_CEREBRAS_MODEL.into(),
            lease_token,
            deadline_at: "2026-08-01T12:00:00Z".parse().unwrap(),
            benchmark_version: BENCHMARK_VERSION.into(),
        };
        assert_eq!(
            serde_json::to_value(claim).unwrap(),
            serde_json::json!({
                "id": id,
                "repo_url": "https://github.com/example/agent",
                "provider": "cerebras",
                "benchmark_kind": "arc",
                "model_id": "gemma-4-31b",
                "lease_token": lease_token,
                "deadline_at": "2026-08-01T12:00:00Z",
                "benchmark_version": "harness-2026-sprint-v3"
            })
        );
    }

    #[test]
    fn model_allowlist_is_safe_and_stable() {
        assert!(is_bedrock_model(DEFAULT_BEDROCK_MODEL));
        assert!(BEDROCK_MODEL_IDS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(BEDROCK_MODEL_IDS.iter().all(|model| {
            model.len() <= 120
                && model
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        }));
        assert!(
            BEDROCK_IMAGE_MODEL_IDS
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        assert!(
            BEDROCK_IMAGE_MODEL_IDS
                .iter()
                .all(|model| is_bedrock_model(model))
        );
        assert!(bedrock_model_supports_images("google.gemma-4-31b"));
        assert!(!bedrock_model_supports_images("openai.gpt-oss-120b"));
        assert_eq!(CEREBRAS_MODEL_IDS, ["gemma-4-31b", "zai-glm-4.7"]);
        assert!(ModelProvider::Cerebras.supports_model(DEFAULT_CEREBRAS_MODEL));
        assert!(ModelProvider::Cerebras.supports_images(DEFAULT_CEREBRAS_MODEL));
        assert!(!ModelProvider::Cerebras.supports_images("zai-glm-4.7"));
    }

    #[test]
    fn builtin_harness_ids_and_uris_are_exact() {
        assert_eq!(builtin_harness_uri("forge"), Some("builtin://forge"));
        assert_eq!(builtin_harness_uri("reki"), Some("builtin://reki"));
        assert_eq!(builtin_harness_uri("unknown"), None);
        assert!(is_builtin_harness("builtin://forge"));
        assert!(!is_builtin_harness("builtin://unknown"));
    }

    #[test]
    fn arc_v3_runs_every_public_game_five_at_a_time() {
        assert_eq!(ARC_GAMES.len(), 25);
        assert_eq!(ARC_CONCURRENCY, 5);
        assert!(
            ARC_GAMES
                .iter()
                .enumerate()
                .all(|(index, game)| { !ARC_GAMES[..index].contains(game) })
        );
    }

    #[test]
    fn old_claims_default_to_the_selected_model() {
        let (id, lease_token) = ids();
        let claim: HarnessClaim = serde_json::from_value(serde_json::json!({
            "id": id,
            "repo_url": "https://github.com/example/agent",
            "lease_token": lease_token,
            "deadline_at": "2026-08-01T12:00:00Z",
            "benchmark_version": "harness-2026-sprint-v2"
        }))
        .unwrap();
        assert_eq!(claim.model_id, DEFAULT_BEDROCK_MODEL);
        assert_eq!(claim.provider, ModelProvider::Bedrock);
        assert_eq!(claim.benchmark_kind, BenchmarkKind::Bundled);
    }

    #[test]
    fn provider_and_benchmark_kind_parse_exact_wire_values() {
        assert_eq!("cerebras".parse(), Ok(ModelProvider::Cerebras));
        assert_eq!("frontier".parse(), Ok(BenchmarkKind::Frontier));
        assert!("Cerebras".parse::<ModelProvider>().is_err());
        assert!("all".parse::<BenchmarkKind>().is_err());
    }

    #[test]
    fn old_executor_requests_default_to_bundled_bedrock() {
        let (run_id, _) = ids();
        let request: ExecutorRequest = serde_json::from_value(serde_json::json!({
            "type": "run",
            "run_id": run_id,
            "repo_url": "https://github.com/example/agent",
            "deadline_at": "2026-08-01T12:00:00Z",
            "gateway_socket": "/run/exposure-benchmark/gateways/legacy/bedrock.sock",
            "gateway_token": "a".repeat(64),
            "model_profile": "xai.grok-4.3"
        }))
        .unwrap();
        let ExecutorRequest::Run {
            provider,
            benchmark_kind,
            ..
        } = request
        else {
            panic!("expected run request")
        };
        assert_eq!(provider, ModelProvider::Bedrock);
        assert_eq!(benchmark_kind, BenchmarkKind::Bundled);
    }

    #[test]
    fn capacity_wire_shape_and_demand_are_stable() {
        let capacity = HarnessCapacity {
            queued: 4,
            active: 1,
            oldest_queued_seconds: 17,
        };
        assert_eq!(capacity.demand(), 5);
        assert_eq!(
            serde_json::to_value(capacity).unwrap(),
            serde_json::json!({
                "queued": 4,
                "active": 1,
                "oldest_queued_seconds": 17
            })
        );
    }

    #[test]
    fn frame_request_requires_lease_on_the_wire() {
        let (run_id, lease_token) = ids();
        let value = serde_json::to_value(ArcFramesRequest {
            run_id,
            lease_token,
            frames: Vec::new(),
        })
        .unwrap();
        assert_eq!(value["lease_token"], lease_token.to_string());
        assert!(
            serde_json::from_value::<ArcFramesRequest>(serde_json::json!({
                "run_id": run_id,
                "frames": []
            }))
            .is_err()
        );
    }

    #[test]
    fn frame_validation_rejects_malformed_grids() {
        let frame = ArcFrame {
            game: "ls20".into(),
            seq: 0,
            grids: "0".repeat(4096),
            state: "NOT_PLAYED".into(),
            levels_completed: 0,
            baseline: Some(vec![10]),
            action: None,
            action_x: None,
            action_y: None,
        };
        assert!(frame.is_valid());
        assert!(
            !ArcFrame {
                grids: "0".repeat(4095),
                ..frame
            }
            .is_valid()
        );
    }

    #[test]
    fn ndjson_messages_are_tagged_and_round_trip() {
        let event = ExecutorEvent::Progress {
            benchmark: "arc".into(),
            state: serde_json::json!({"status":"running","done":3,"total":25}),
        };
        let line = serde_json::to_string(&event).unwrap();
        assert!(line.starts_with("{\"type\":\"progress\""));
        assert_eq!(serde_json::from_str::<ExecutorEvent>(&line).unwrap(), event);
    }
}
