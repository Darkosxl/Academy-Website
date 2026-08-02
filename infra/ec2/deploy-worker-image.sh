#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  echo "usage: $0 SOURCE_ARCHIVE SOURCE_COMMIT" >&2
  exit 2
}

[[ $# -eq 2 ]] || usage
source_archive=$(realpath "$1")
source_commit=$2
[[ -f $source_archive ]] || usage
[[ $source_commit =~ ^[0-9a-f]{40}$ ]] || usage

region=${AWS_REGION:-us-east-1}
stack_name=${BENCHMARK_STACK_NAME:-exposure-benchmark}
cfn_role_arn=${BENCHMARK_CLOUDFORMATION_ROLE_ARN:?set BENCHMARK_CLOUDFORMATION_ROLE_ARN}
script_directory=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
packer_template=$script_directory/worker.pkr.hcl
manifest=$script_directory/packer-manifest.json

for command in aws jq packer session-manager-plugin; do
  command -v "$command" >/dev/null || { echo "STOP: missing command: $command" >&2; exit 1; }
done

scratch_directory=$(mktemp -d)
finished=false
drained=false
new_ami=

stack_json() {
  aws cloudformation describe-stacks \
    --region "$region" --stack-name "$stack_name" --output json
}

parameter() {
  local document=$1 key=$2
  jq -er --arg key "$key" \
    '.Stacks[0].Parameters[] | select(.ParameterKey == $key) | .ParameterValue' <<<"$document"
}

output() {
  local document=$1 key=$2
  jq -er --arg key "$key" \
    '.Stacks[0].Outputs[] | select(.OutputKey == $key) | .OutputValue' <<<"$document"
}

update_stack() {
  local worker_enabled=$1 autoscaling_enabled=$2 prepared_ami=$3 max_capacity=$4
  local parameters_file=$scratch_directory/parameters.json result status deadline
  local before_timestamp current_timestamp

  before_timestamp=$(stack_json | jq -er '.Stacks[0].LastUpdatedTime // .Stacks[0].CreationTime')

  jq \
    --arg worker "$worker_enabled" \
    --arg autoscaling "$autoscaling_enabled" \
    --arg ami "$prepared_ami" \
    --arg maximum "$max_capacity" \
    '[.Stacks[0].Parameters[] |
      if .ParameterKey == "WorkerEnabled" then
        {ParameterKey: .ParameterKey, ParameterValue: $worker}
      elif .ParameterKey == "EnableAutoscaling" then
        {ParameterKey: .ParameterKey, ParameterValue: $autoscaling}
      elif .ParameterKey == "PreparedWorkerAmi" then
        {ParameterKey: .ParameterKey, ParameterValue: $ami}
      elif .ParameterKey == "MaxCapacity" then
        {ParameterKey: .ParameterKey, ParameterValue: $maximum}
      else
        {ParameterKey: .ParameterKey, UsePreviousValue: true}
      end]' <<<"$initial_stack" >"$parameters_file"

  if result=$(aws cloudformation update-stack \
    --region "$region" \
    --stack-name "$stack_name" \
    --use-previous-template \
    --role-arn "$cfn_role_arn" \
    --capabilities CAPABILITY_IAM \
    --parameters "file://$parameters_file" 2>&1); then
    status=0
  else
    status=$?
    if [[ $result == *"No updates are to be performed"* ]]; then
      return 0
    fi
    echo "$result" >&2
    return "$status"
  fi
  echo "$result"

  deadline=$((SECONDS + 7800))
  while (( SECONDS < deadline )); do
    status=$(aws cloudformation describe-stacks \
      --region "$region" --stack-name "$stack_name" \
      --query 'Stacks[0].StackStatus' --output text)
    current_timestamp=$(aws cloudformation describe-stacks \
      --region "$region" --stack-name "$stack_name" \
      --query 'Stacks[0].LastUpdatedTime || Stacks[0].CreationTime' --output text)
    case "$status" in
      CREATE_COMPLETE|UPDATE_COMPLETE)
        [[ $current_timestamp != "$before_timestamp" ]] && return 0
        ;;
      *_FAILED|*_ROLLBACK_COMPLETE|ROLLBACK_COMPLETE)
        echo "STOP: stack update ended in $status" >&2
        return 1
        ;;
    esac
    sleep 20
  done
  echo "STOP: stack update did not finish within 130 minutes" >&2
  return 1
}

wait_for_empty_group() {
  local deadline=$((SECONDS + 7500)) count
  while (( SECONDS < deadline )); do
    count=$(aws autoscaling describe-auto-scaling-groups \
      --region "$region" --auto-scaling-group-names "$asg_name" \
      --query 'AutoScalingGroups[0].Instances | length(@)' --output text)
    [[ $count == 0 ]] && return 0
    echo "Waiting for the existing worker to drain safely ($count instance still attached)..."
    sleep 20
  done
  echo "STOP: worker did not drain before the two-hour lifecycle deadline" >&2
  return 1
}

rollback() {
  local exit_status=$?
  trap - EXIT
  set +e
  if [[ $finished != true && $drained == true ]]; then
    echo "Deployment failed; restoring the previous worker configuration" >&2
    if ! update_stack "$old_worker_enabled" "$old_autoscaling" "$old_ami" "$old_maximum"; then
      echo "STOP: automatic rollback also failed; inspect $stack_name immediately" >&2
    fi
  fi
  rm -rf -- "$scratch_directory"
  rm -f -- "$manifest"
  exit "$exit_status"
}
trap rollback EXIT

initial_stack=$(stack_json)
stack_status=$(jq -er '.Stacks[0].StackStatus' <<<"$initial_stack")
[[ $stack_status == CREATE_COMPLETE || $stack_status == UPDATE_COMPLETE || $stack_status == UPDATE_ROLLBACK_COMPLETE ]] || {
  echo "STOP: stack is not stable: $stack_status" >&2
  exit 1
}

old_worker_enabled=$(parameter "$initial_stack" WorkerEnabled)
old_autoscaling=$(parameter "$initial_stack" EnableAutoscaling)
old_ami=$(parameter "$initial_stack" PreparedWorkerAmi)
old_maximum=$(parameter "$initial_stack" MaxCapacity)
instance_type=$(parameter "$initial_stack" InstanceType)
source_ami=$(jq -er \
  '.Stacks[0].Parameters[] | select(.ParameterKey == "UbuntuAmi") | .ResolvedValue' \
  <<<"$initial_stack")
asg_name=$(output "$initial_stack" AutoScalingGroupName)
subnet_id=$(parameter "$initial_stack" PrivateSubnetId)
security_group_id=$(output "$initial_stack" SecurityGroupId)
instance_profile_name=$(output "$initial_stack" InstanceProfileName)

[[ $old_worker_enabled == true || $old_worker_enabled == false ]] || {
  echo "STOP: invalid WorkerEnabled value: $old_worker_enabled" >&2
  exit 1
}
[[ $old_autoscaling == false ]] || { echo "STOP: disable autoscaling before image deployment" >&2; exit 1; }
[[ $old_maximum == 1 ]] || { echo "STOP: MaxCapacity must be 1 under the 16-vCPU quota" >&2; exit 1; }
[[ $instance_type == r8i.4xlarge ]] || { echo "STOP: expected r8i.4xlarge, found $instance_type" >&2; exit 1; }

if [[ $old_worker_enabled == true ]]; then
  echo "Draining the existing worker before consuming the 16-vCPU quota"
  update_stack false false "$old_ami" 1
else
  echo "Worker fleet is already disabled for the initial image build"
fi
drained=true
wait_for_empty_group

rm -f -- "$manifest"
packer init "$packer_template"

packer_environment=(
  "PKR_VAR_aws_region=$region"
  "PKR_VAR_source_ami=$source_ami"
  "PKR_VAR_subnet_id=$subnet_id"
  "PKR_VAR_security_group_id=$security_group_id"
  "PKR_VAR_instance_profile_name=$instance_profile_name"
  "PKR_VAR_instance_type=$instance_type"
  "PKR_VAR_source_archive=$source_archive"
  "PKR_VAR_source_commit=$source_commit"
)

env "${packer_environment[@]}" packer validate "$packer_template"
env "${packer_environment[@]}" packer build "$packer_template"

new_ami=$(jq -er '.builds[-1].artifact_id | split(":")[-1]' "$manifest")
[[ $new_ami =~ ^ami-[0-9a-f]+$ ]] || { echo "STOP: Packer did not return an AMI ID" >&2; exit 1; }
aws ec2 wait image-available --region "$region" --image-ids "$new_ami"

echo "Deploying verified AMI $new_ami"
update_stack true false "$new_ami" 1

deadline=$((SECONDS + 1200))
instance_id=
while (( SECONDS < deadline )); do
  instance_id=$(aws autoscaling describe-auto-scaling-groups \
    --region "$region" --auto-scaling-group-names "$asg_name" \
    --query 'AutoScalingGroups[0].Instances[?LifecycleState==`InService` && HealthStatus==`Healthy`].InstanceId | [0]' \
    --output text)
  [[ $instance_id == i-* ]] && break
  sleep 15
done
[[ $instance_id == i-* ]] || { echo "STOP: replacement worker never became healthy" >&2; exit 1; }

actual_image=$(aws ec2 describe-instances --region "$region" --instance-ids "$instance_id" \
  --query 'Reservations[0].Instances[0].ImageId' --output text)
[[ $actual_image == "$new_ami" ]] || { echo "STOP: replacement launched from $actual_image" >&2; exit 1; }

deadline=$((SECONDS + 900))
while (( SECONDS < deadline )); do
  managed=$(aws ssm describe-instance-information --region "$region" \
    --filters "Key=InstanceIds,Values=$instance_id" \
    --query 'InstanceInformationList[0].PingStatus' --output text)
  [[ $managed == Online ]] && break
  sleep 15
done
[[ $managed == Online ]] || { echo "STOP: replacement worker never became SSM-ready" >&2; exit 1; }

ssm_parameters=$(jq -cn --arg command \
  "sudo /opt/exposure-benchmark/infra/verify-running-worker.sh $source_commit" \
  '{commands: [$command]}')
command_id=$(aws ssm send-command \
  --region "$region" \
  --instance-ids "$instance_id" \
  --document-name AWS-RunShellScript \
  --parameters "$ssm_parameters" \
  --query 'Command.CommandId' --output text)
aws ssm wait command-executed \
  --region "$region" --command-id "$command_id" --instance-id "$instance_id"
aws ssm get-command-invocation \
  --region "$region" --command-id "$command_id" --instance-id "$instance_id" \
  --query '{Status:Status,Output:StandardOutputContent,Error:StandardErrorContent}' --output json

finished=true
rm -rf -- "$scratch_directory"
rm -f -- "$manifest"
trap - EXIT
echo "READY: source commit $source_commit is live on $instance_id using $new_ami"
