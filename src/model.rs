use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Clone)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub is_admin: bool,
}

#[derive(FromRow)]
pub struct Video {
    pub id: Uuid,
    pub youtube_id: String,
    pub title: String,
    pub level: String,
}

#[derive(FromRow)]
pub struct VideoWithProgress {
    pub id: Uuid,
    pub youtube_id: String,
    pub title: String,
    pub level: String,
    pub max_position: f32,
    pub duration: f32,
}

#[derive(FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub level: String,
}

#[derive(FromRow)]
pub struct SubmissionView {
    pub id: Uuid,
    pub task_id: Uuid,
    pub repo_url: String,
    pub status: String,
    pub feedback: Option<String>,
    pub demo_video_url: Option<String>,
    pub display_name: String,
    pub task_title: String,
}

/// One student's standing. Points: 20 per completed video, 50 per passed project.
#[derive(FromRow)]
pub struct LeaderRow {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
    pub videos: i64,
    pub projects: i64,
}

pub const PTS_VIDEO: i64 = 20;
pub const PTS_PROJECT: i64 = 50;

impl LeaderRow {
    pub fn points(&self) -> i64 {
        self.videos * PTS_VIDEO + self.projects * PTS_PROJECT
    }
}

#[derive(FromRow)]
pub struct StatRow {
    pub display_name: String,
    pub video_title: String,
    pub seconds_watched: f32,
    pub max_position: f32,
    pub duration: f32,
    pub updated_at: DateTime<Utc>,
}
