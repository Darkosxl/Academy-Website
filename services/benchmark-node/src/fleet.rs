use crate::config::FleetConfig;
use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_autoscaling::Client as AutoScalingClient;
use aws_sdk_cloudwatch::{
    Client as CloudWatchClient,
    types::{Dimension, MetricDatum, StandardUnit},
};
use benchmark_protocol::HarnessCapacity;

#[derive(Clone)]
pub struct FleetManager {
    auto_scaling: AutoScalingClient,
    cloudwatch: CloudWatchClient,
    auto_scaling_group: String,
    instance_id: String,
    termination_hook: String,
    metric_namespace: String,
}

impl FleetManager {
    pub async fn new(config: &FleetConfig, region: &str) -> Self {
        let sdk = aws_config::defaults(BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_owned()))
            .load()
            .await;
        Self {
            auto_scaling: AutoScalingClient::new(&sdk),
            cloudwatch: CloudWatchClient::new(&sdk),
            auto_scaling_group: config.auto_scaling_group.clone(),
            instance_id: config.instance_id.clone(),
            termination_hook: config.termination_hook.clone(),
            metric_namespace: config.metric_namespace.clone(),
        }
    }

    pub async fn set_protected(&self, protected: bool) -> Result<()> {
        self.auto_scaling
            .set_instance_protection()
            .auto_scaling_group_name(&self.auto_scaling_group)
            .instance_ids(&self.instance_id)
            .protected_from_scale_in(protected)
            .send()
            .await
            .with_context(|| {
                format!(
                    "set scale-in protection to {protected} for {}",
                    self.instance_id
                )
            })?;
        Ok(())
    }

    pub async fn termination_waiting(&self) -> Result<bool> {
        let response = self
            .auto_scaling
            .describe_auto_scaling_instances()
            .instance_ids(&self.instance_id)
            .send()
            .await
            .context("read Auto Scaling lifecycle state")?;
        let instance = response
            .auto_scaling_instances()
            .iter()
            .find(|instance| instance.instance_id() == Some(self.instance_id.as_str()))
            .with_context(|| format!("{} is not registered in Auto Scaling", self.instance_id))?;
        let state = instance
            .lifecycle_state()
            .context("Auto Scaling response omitted lifecycle state")?;
        Ok(is_termination_wait_state(state))
    }

    pub async fn complete_termination(&self) -> Result<()> {
        self.auto_scaling
            .complete_lifecycle_action()
            .auto_scaling_group_name(&self.auto_scaling_group)
            .lifecycle_hook_name(&self.termination_hook)
            .instance_id(&self.instance_id)
            .lifecycle_action_result("CONTINUE")
            .send()
            .await
            .context("complete Auto Scaling termination lifecycle action")?;
        Ok(())
    }

    pub async fn publish_capacity(&self, capacity: &HarnessCapacity) -> Result<()> {
        let dimension = Dimension::builder()
            .name("AutoScalingGroupName")
            .value(&self.auto_scaling_group)
            .build();
        let metrics = vec![
            metric(
                "QueueDepth",
                capacity.queued,
                StandardUnit::Count,
                &dimension,
            ),
            metric(
                "ActiveRuns",
                capacity.active,
                StandardUnit::Count,
                &dimension,
            ),
            metric("Demand", capacity.demand(), StandardUnit::Count, &dimension),
            metric(
                "OldestQueuedSeconds",
                capacity.oldest_queued_seconds,
                StandardUnit::Seconds,
                &dimension,
            ),
        ];
        self.cloudwatch
            .put_metric_data()
            .namespace(&self.metric_namespace)
            .set_metric_data(Some(metrics))
            .send()
            .await
            .context("publish benchmark capacity to CloudWatch")?;
        Ok(())
    }
}

fn metric(name: &str, value: u64, unit: StandardUnit, dimension: &Dimension) -> MetricDatum {
    MetricDatum::builder()
        .metric_name(name)
        .value(value as f64)
        .unit(unit)
        .dimensions(dimension.clone())
        .build()
}

fn is_termination_wait_state(state: &str) -> bool {
    state == "Terminating:Wait"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_drainable_lifecycle_state_stops_claiming() {
        assert!(is_termination_wait_state("Terminating:Wait"));
        assert!(!is_termination_wait_state("InService"));
        assert!(!is_termination_wait_state("Terminating:Proceed"));
    }
}
