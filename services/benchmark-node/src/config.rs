use anyhow::{Context, Result, bail};
use aws_config::BehaviorVersion;
use aws_sdk_secretsmanager::Client as SecretsClient;
use serde::Deserialize;
use std::{collections::HashSet, env, net::SocketAddr, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Environment {
    Dev,
    Prod,
}

#[derive(Deserialize)]
struct SecretDocument {
    #[serde(alias = "WORKER_TOKEN")]
    worker_token: String,
    #[serde(alias = "BEDROCK_API_KEY")]
    bedrock_api_key: String,
    #[serde(alias = "CEREBRAS_API_KEYS")]
    cerebras_api_keys: Vec<String>,
}

pub struct ControllerConfig {
    pub academy_base_url: String,
    pub worker_token: String,
    pub bedrock_api_key: String,
    pub cerebras_api_keys: Vec<String>,
    pub aws_region: String,
    pub reasoning_effort: String,
    pub maximum_model_concurrency: usize,
    pub executor_socket: PathBuf,
    pub gateway_directory: PathBuf,
    pub metrics_bind: SocketAddr,
    pub fleet: Option<FleetConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FleetConfig {
    pub auto_scaling_group: String,
    pub instance_id: String,
    pub termination_hook: String,
    pub metric_namespace: String,
}

impl ControllerConfig {
    pub async fn load() -> Result<Self> {
        // Local binaries run from the repository root. Production systemd units use an
        // explicit EnvironmentFile, so this is a no-op there.
        dotenvy::dotenv().ok();
        let environment = deployment_environment(
            env::var("ENVIRONMENT").ok().as_deref(),
            env::var("HARNESS_ENV").ok().as_deref(),
        )?;
        let aws_region = value("AWS_REGION", "us-east-1");
        let secret_id = env::var("BENCHMARK_SECRET_ID").unwrap_or_default();
        let secrets = if secret_id.trim().is_empty() {
            SecretDocument {
                worker_token: env::var("WORKER_TOKEN").unwrap_or_default(),
                bedrock_api_key: env::var("BEDROCK_API_KEY").unwrap_or_default(),
                cerebras_api_keys: local_cerebras_keys(),
            }
        } else {
            load_secret(secret_id.trim(), &aws_region).await?
        };
        if secrets.worker_token.len() < 32 || secrets.bedrock_api_key.len() < 20 {
            bail!("worker and model credentials are missing or too short");
        }
        validate_cerebras_keys(&secrets.cerebras_api_keys)?;
        let academy_base_url = required("ACADEMY_BASE_URL")?
            .trim_end_matches('/')
            .to_owned();
        let parsed = reqwest::Url::parse(&academy_base_url).context("ACADEMY_BASE_URL")?;
        if parsed.scheme() != "https" && environment != Environment::Dev {
            bail!("ACADEMY_BASE_URL must use HTTPS when ENVIRONMENT=PROD");
        }
        let reasoning_effort = value("BEDROCK_REASONING_EFFORT", "none");
        if !matches!(
            reasoning_effort.as_str(),
            "none" | "low" | "medium" | "high"
        ) {
            bail!("BEDROCK_REASONING_EFFORT must be none, low, medium, or high");
        }
        let maximum_model_concurrency = value("BEDROCK_MAX_CONCURRENCY", "32")
            .parse::<usize>()
            .context("BEDROCK_MAX_CONCURRENCY")?;
        if !(1..=128).contains(&maximum_model_concurrency) {
            bail!("BEDROCK_MAX_CONCURRENCY must be between 1 and 128");
        }
        let metrics_bind: SocketAddr = value("BENCHMARK_METRICS_BIND", "127.0.0.1:9108")
            .parse()
            .context("BENCHMARK_METRICS_BIND")?;
        if !metrics_bind.ip().is_loopback() {
            bail!("BENCHMARK_METRICS_BIND must be loopback-only");
        }
        let fleet = fleet_config(
            env::var("BENCHMARK_ASG_NAME").ok(),
            env::var("BENCHMARK_INSTANCE_ID").ok(),
            env::var("BENCHMARK_TERMINATION_HOOK").ok(),
            value("BENCHMARK_METRIC_NAMESPACE", "Exposure/Benchmark"),
        )?;
        Ok(Self {
            academy_base_url,
            worker_token: secrets.worker_token,
            bedrock_api_key: secrets.bedrock_api_key,
            cerebras_api_keys: secrets.cerebras_api_keys,
            aws_region,
            reasoning_effort,
            maximum_model_concurrency,
            executor_socket: value(
                "BENCHMARK_EXECUTOR_SOCKET",
                "/run/exposure-benchmark/executor/executor.sock",
            )
            .into(),
            gateway_directory: value(
                "BENCHMARK_GATEWAY_DIRECTORY",
                "/run/exposure-benchmark/gateways",
            )
            .into(),
            metrics_bind,
            fleet,
        })
    }
}

pub struct ExecutorConfig {
    pub socket: PathBuf,
    pub state_directory: PathBuf,
    pub adapter: PathBuf,
    pub python: PathBuf,
    pub sandbox_image: String,
    pub controller_uid: Option<u32>,
}

impl ExecutorConfig {
    pub fn load() -> Result<Self> {
        let controller_uid = env::var("BENCHMARK_CONTROLLER_UID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.parse::<u32>().context("BENCHMARK_CONTROLLER_UID"))
            .transpose()?;
        Ok(Self {
            socket: value(
                "BENCHMARK_EXECUTOR_SOCKET",
                "/run/exposure-benchmark/executor/executor.sock",
            )
            .into(),
            state_directory: value("BENCHMARK_STATE_DIRECTORY", "/var/lib/exposure-benchmark")
                .into(),
            adapter: value(
                "BENCHMARK_ADAPTER",
                "/opt/exposure-benchmark/adapters/runner.py",
            )
            .into(),
            python: value(
                "BENCHMARK_PYTHON",
                "/var/lib/exposure-benchmark/venv/bin/python",
            )
            .into(),
            sandbox_image: value(
                "BENCHMARK_SANDBOX_IMAGE",
                "localhost/exposure-harness-arc:0.9.9",
            ),
            controller_uid,
        })
    }
}

async fn load_secret(secret_id: &str, region: &str) -> Result<SecretDocument> {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_owned()))
        .load()
        .await;
    let response = SecretsClient::new(&config)
        .get_secret_value()
        .secret_id(secret_id)
        .send()
        .await
        .context("read BENCHMARK_SECRET_ID from AWS Secrets Manager")?;
    let value = response
        .secret_string()
        .context("benchmark secret must be a JSON SecretString")?;
    serde_json::from_str(value).context("decode benchmark secret JSON")
}

fn value(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn split_api_keys(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(String::from)
        .collect()
}

fn local_cerebras_keys() -> Vec<String> {
    let combined = env::var("CEREBRAS_API_KEYS").unwrap_or_default();
    if !combined.trim().is_empty() {
        return split_api_keys(&combined);
    }
    [
        "CEM_CEREBRAS_API_KEY",
        "VARRO_CEREBRAS_API_KEY",
        "EXPOSURE_CEREBRAS_API_KEY",
        "TERMINUSEYE_CEREBRAS_API_KEY",
    ]
    .iter()
    .filter_map(|name| env::var(name).ok())
    .filter(|key| !key.trim().is_empty())
    .collect()
}

fn validate_cerebras_keys(keys: &[String]) -> Result<()> {
    if keys.len() != 4
        || keys.iter().any(|key| key.len() < 20)
        || keys.iter().collect::<HashSet<_>>().len() != keys.len()
    {
        bail!("CEREBRAS_API_KEYS must contain four distinct API keys")
    }
    Ok(())
}

fn required(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("{name} is required"))
}

fn deployment_environment(current: Option<&str>, legacy: Option<&str>) -> Result<Environment> {
    let raw = current.or(legacy).unwrap_or("PROD").trim();
    if raw.eq_ignore_ascii_case("DEV") || raw.eq_ignore_ascii_case("local") {
        Ok(Environment::Dev)
    } else if raw.eq_ignore_ascii_case("PROD") || raw.eq_ignore_ascii_case("production") {
        Ok(Environment::Prod)
    } else {
        bail!("ENVIRONMENT must be DEV or PROD")
    }
}

fn fleet_config(
    auto_scaling_group: Option<String>,
    instance_id: Option<String>,
    termination_hook: Option<String>,
    metric_namespace: String,
) -> Result<Option<FleetConfig>> {
    let values = [
        auto_scaling_group.as_deref().unwrap_or_default().trim(),
        instance_id.as_deref().unwrap_or_default().trim(),
        termination_hook.as_deref().unwrap_or_default().trim(),
    ];
    if values.iter().all(|value| value.is_empty()) {
        return Ok(None);
    }
    if values.iter().any(|value| value.is_empty()) {
        bail!(
            "BENCHMARK_ASG_NAME, BENCHMARK_INSTANCE_ID, and BENCHMARK_TERMINATION_HOOK must be set together"
        );
    }
    if metric_namespace.trim().is_empty() {
        bail!("BENCHMARK_METRIC_NAMESPACE must not be empty");
    }
    Ok(Some(FleetConfig {
        auto_scaling_group: values[0].to_owned(),
        instance_id: values[1].to_owned(),
        termination_hook: values[2].to_owned(),
        metric_namespace: metric_namespace.trim().to_owned(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_defaults_to_production_and_keeps_legacy_names_compatible() {
        assert_eq!(
            deployment_environment(None, None).unwrap(),
            Environment::Prod
        );
        assert_eq!(
            deployment_environment(Some("DEV"), None).unwrap(),
            Environment::Dev
        );
        assert_eq!(
            deployment_environment(Some("prod"), None).unwrap(),
            Environment::Prod
        );
        assert_eq!(
            deployment_environment(None, Some("local")).unwrap(),
            Environment::Dev
        );
        assert_eq!(
            deployment_environment(None, Some("production")).unwrap(),
            Environment::Prod
        );
        assert!(deployment_environment(Some("staging"), None).is_err());
    }

    #[test]
    fn fleet_configuration_is_optional_but_never_partial() {
        assert_eq!(
            fleet_config(None, None, None, "Exposure/Benchmark".into()).unwrap(),
            None
        );
        assert!(
            fleet_config(
                Some("workers".into()),
                None,
                Some("drain".into()),
                "Exposure/Benchmark".into()
            )
            .is_err()
        );
        assert_eq!(
            fleet_config(
                Some("workers".into()),
                Some("i-123".into()),
                Some("drain".into()),
                "Exposure/Benchmark".into()
            )
            .unwrap()
            .unwrap(),
            FleetConfig {
                auto_scaling_group: "workers".into(),
                instance_id: "i-123".into(),
                termination_hook: "drain".into(),
                metric_namespace: "Exposure/Benchmark".into(),
            }
        );
    }

    #[test]
    fn cerebras_key_list_is_exact_and_distinct() {
        let keys = split_api_keys(
            "csk-11111111111111111111, csk-22222222222222222222,csk-33333333333333333333,csk-44444444444444444444",
        );
        assert_eq!(keys.len(), 4);
        assert!(validate_cerebras_keys(&keys).is_ok());
        assert!(validate_cerebras_keys(&keys[..3]).is_err());
        assert!(validate_cerebras_keys(&vec![keys[0].clone(); 4]).is_err());
    }
}
