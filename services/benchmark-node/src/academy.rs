use benchmark_protocol::{
    ArcFramesRequest, HarnessClaim, HarnessLeaseRequest, HarnessProgressRequest,
    HarnessResultRequest, HarnessStageRequest, KaggleClaim, KaggleResultRequest,
};
use reqwest::StatusCode;
use serde::{Serialize, de::DeserializeOwned};
use std::{error::Error, fmt, time::Duration};

#[derive(Clone)]
pub struct AcademyClient {
    base_url: String,
    worker_token: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    StaleLease,
    Unauthorized,
    Temporary(String),
    Rejected { status: u16, detail: String },
    InvalidResponse(String),
}

impl ApiError {
    pub fn is_temporary(&self) -> bool {
        matches!(self, Self::Temporary(_))
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleLease => write!(formatter, "Academy rejected a stale lease"),
            Self::Unauthorized => write!(formatter, "Academy rejected the worker credential"),
            Self::Temporary(detail) => write!(formatter, "temporary Academy failure: {detail}"),
            Self::Rejected { status, detail } => {
                write!(formatter, "Academy returned HTTP {status}: {detail}")
            }
            Self::InvalidResponse(detail) => {
                write!(formatter, "invalid Academy response: {detail}")
            }
        }
    }
}

impl Error for ApiError {}

impl AcademyClient {
    pub fn new(base_url: String, worker_token: String) -> Result<Self, reqwest::Error> {
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            worker_token,
            http: reqwest::Client::builder()
                .user_agent("exposure-benchmark-node/1")
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(15))
                .build()?,
        })
    }

    pub async fn claim(&self) -> Result<Option<HarnessClaim>, ApiError> {
        self.post_response("/api/worker/harness/claim", &serde_json::json!({}))
            .await
    }

    pub async fn heartbeat(&self, request: &HarnessLeaseRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/heartbeat", request)
            .await
    }

    pub async fn stage(&self, request: &HarnessStageRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/stage", request).await
    }

    pub async fn progress(&self, request: &HarnessProgressRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/progress", request)
            .await
    }

    pub async fn frames(&self, request: &ArcFramesRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/arc/frames", request)
            .await
    }

    pub async fn result(&self, request: &HarnessResultRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/result", request).await
    }

    pub async fn kaggle_claim(&self) -> Result<Option<KaggleClaim>, ApiError> {
        self.post_response("/api/worker/harness/kaggle/claim", &serde_json::json!({}))
            .await
    }

    pub async fn kaggle_result(&self, request: &KaggleResultRequest) -> Result<(), ApiError> {
        self.post_empty("/api/worker/harness/kaggle/result", request)
            .await
    }

    async fn post_empty<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<(), ApiError> {
        let response = self.send(path, body).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(response_error(response).await)
        }
    }

    async fn post_response<B, R>(&self, path: &str, body: &B) -> Result<R, ApiError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let response = self.send(path, body).await?;
        if !response.status().is_success() {
            return Err(response_error(response).await);
        }
        response
            .json()
            .await
            .map_err(|error| ApiError::InvalidResponse(error.to_string()))
    }

    async fn send<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<reqwest::Response, ApiError> {
        self.request(path, body)
            .send()
            .await
            .map_err(|error| ApiError::Temporary(error.to_string()))
    }

    fn request<B: Serialize + ?Sized>(&self, path: &str, body: &B) -> reqwest::RequestBuilder {
        self.http
            .post(format!("{}{path}", self.base_url))
            .header("X-Worker-Token", &self.worker_token)
            .json(body)
    }
}

async fn response_error(response: reqwest::Response) -> ApiError {
    let status = response.status();
    if matches!(status, StatusCode::CONFLICT | StatusCode::UNAUTHORIZED) {
        return classify_error(status, String::new());
    }
    let detail = response
        .text()
        .await
        .unwrap_or_default()
        .chars()
        .take(1000)
        .collect::<String>();
    classify_error(status, detail)
}

fn classify_error(status: StatusCode, detail: String) -> ApiError {
    if status == StatusCode::CONFLICT {
        return ApiError::StaleLease;
    }
    if status == StatusCode::UNAUTHORIZED {
        return ApiError::Unauthorized;
    }
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        ApiError::Temporary(format!("HTTP {} {detail}", status.as_u16()))
    } else {
        ApiError::Rejected {
            status: status.as_u16(),
            detail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_request_sends_worker_header_and_null_decodes() {
        let client =
            AcademyClient::new("https://academy.example".into(), "worker-secret".into()).unwrap();
        let request = client
            .request("/api/worker/harness/claim", &serde_json::json!({}))
            .build()
            .unwrap();
        assert_eq!(
            request.url().as_str(),
            "https://academy.example/api/worker/harness/claim"
        );
        assert_eq!(request.headers()["x-worker-token"], "worker-secret");
        assert_eq!(
            serde_json::from_str::<Option<HarnessClaim>>("null").unwrap(),
            None
        );
    }

    #[test]
    fn conflict_maps_to_stale_lease() {
        assert_eq!(
            classify_error(StatusCode::CONFLICT, String::new()),
            ApiError::StaleLease
        );
    }

    #[test]
    fn outage_is_retryable_but_auth_rejection_is_not() {
        assert!(
            classify_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "database unavailable".into()
            )
            .is_temporary()
        );
        assert_eq!(
            classify_error(StatusCode::UNAUTHORIZED, String::new()),
            ApiError::Unauthorized
        );
    }
}
