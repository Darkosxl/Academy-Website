use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const BENCHMARK_VERSION: &str = "harness-2026-sprint-v2";
pub const ARC_GAMES: [&str; 13] = [
    "ls20", "vc33", "ar25", "cn04", "s5i5", "sp80", "bp35", "ft09", "m0r0", "re86", "cd82", "sb26",
    "r11l",
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
            lease_token,
            deadline_at: "2026-08-01T12:00:00Z".parse().unwrap(),
            benchmark_version: BENCHMARK_VERSION.into(),
        };
        assert_eq!(
            serde_json::to_value(claim).unwrap(),
            serde_json::json!({
                "id": id,
                "repo_url": "https://github.com/example/agent",
                "lease_token": lease_token,
                "deadline_at": "2026-08-01T12:00:00Z",
                "benchmark_version": "harness-2026-sprint-v2"
            })
        );
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
            state: serde_json::json!({"status":"running","done":3,"total":13}),
        };
        let line = serde_json::to_string(&event).unwrap();
        assert!(line.starts_with("{\"type\":\"progress\""));
        assert_eq!(serde_json::from_str::<ExecutorEvent>(&line).unwrap(), event);
    }
}
