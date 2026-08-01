# EC2 benchmark fleet

`stack.yaml` creates a private Auto Scaling group with one worker by default and a hard
maximum of five. Each worker is a `c8i.8xlarge` (32 vCPU, 64 GiB) with an encrypted
200 GB gp3 root volume. There is no public IP, no ingress rule, and IMDSv2 has a hop limit
of one. Place the group in a private subnet with NAT or the required VPC endpoints;
operators connect through SSM, never SSH.

One instance runs one complete benchmark at a time. Academy's authenticated capacity API
counts claimable and leased harness/Kaggle work. Every controller publishes the same
`Demand = queued + active` snapshot to `Exposure/Benchmark`; CloudWatch uses `Maximum`, so
duplicate publishers do not multiply demand. The group adds the missing number of slots
and removes one idle node only after 15 quiet minutes.

Scale-in safety has two layers:

- a controller protects its instance as soon as it claims a lease and releases protection
  only after the terminal result has been attempted;
- a 15-minute termination lifecycle hook catches the claim/protection race and lets a
  running benchmark drain before the controller completes termination.

Store this JSON in Secrets Manager:

```json
{"worker_token":"same-value-as-Academy-WORKER_TOKEN","bedrock_api_key":"..."}
```

The instance role can read only that secret, publish the benchmark metric namespace, and
manage protection/lifecycle state for its exact Auto Scaling group. It also carries the
standard SSM role.

## First node and prepared AMI

Autoscaling defaults to `false`. This is intentional: a raw Ubuntu replacement does not
contain the large pinned benchmark caches. First deploy with the default Ubuntu AMI and
one node, using `CAPABILITY_IAM`:

```bash
aws cloudformation deploy \
  --stack-name exposure-benchmark \
  --template-file infra/ec2/stack.yaml \
  --capabilities CAPABILITY_IAM \
  --parameter-overrides \
    VpcId=vpc-REPLACE \
    PrivateSubnetId=subnet-REPLACE \
    BenchmarkSecretArn=arn:aws:secretsmanager:REGION:ACCOUNT:secret:REPLACE \
    AcademyBaseUrl=https://academy.example.com \
    EnableAutoscaling=false
```

Build the checksum-covered bundle from the repository root:

```bash
docker build --target artifacts \
  --output type=local,dest=/tmp/exposure-benchmark-artifacts \
  -f services/benchmark-node/Dockerfile .
```

Transfer it through SSM or an approved private artifact store. Populate the pinned
ARC/Frontier sources under `/var/lib/exposure-benchmark/cache`, then install without
starting the worker:

```bash
sudo /path/to/artifacts/infra/install-artifacts.sh /path/to/artifacts
```

Create the production AMI from a dedicated builder that has never accepted a submission.
Before capture, verify the bundle and sandbox image, stop both services, remove
`/etc/exposure-benchmark/controller.env`, and run `cloud-init clean --logs --machine-id`.
The launch template's cloud-init recreates the instance-specific env file and restarts the
services when a clone boots. Do not image an active worker: its mutable run directories can
contain student code.

Update the stack with that AMI and begin with two nodes:

```bash
aws cloudformation deploy \
  --stack-name exposure-benchmark \
  --template-file infra/ec2/stack.yaml \
  --capabilities CAPABILITY_IAM \
  --parameter-overrides \
    VpcId=vpc-REPLACE \
    PrivateSubnetId=subnet-REPLACE \
    BenchmarkSecretArn=arn:aws:secretsmanager:REGION:ACCOUNT:secret:REPLACE \
    AcademyBaseUrl=https://academy.example.com \
    PreparedWorkerAmi=ami-REPLACE \
    EnableAutoscaling=true \
    MaxCapacity=2
```

Raise `MaxCapacity` to 3, then 5 only after each canary passes. AWS changes the group and
alarms; no application restart or browser configuration change is required.

## Capacity gates

Five `c8i.8xlarge` workers require 160 On-Demand standard-instance vCPUs and 320 GiB of
aggregate RAM. Request at least 192 vCPUs of regional quota for headroom. One run can use
roughly 20 vCPU and 44 GiB across its concurrent ARC and Frontier phases, leaving adequate
host margin on `c8i.8xlarge`; use `c8i.12xlarge` if load tests show memory or CPU pressure.

Five controllers can expose up to 160 simultaneous model calls with the default
`BEDROCK_MAX_CONCURRENCY=32`. Confirm the provider account's request/token limits before
raising the fleet maximum. Also run the frame load test at 65 games (five workers x 13 ARC
games) and require p95 frame-to-screen latency below 2.5 seconds.

The generic `cloud-init.yaml` remains the host bootstrap for launches outside
CloudFormation. Production runs `benchmark-controller` and `benchmark-executor`
host-native under systemd; the Docker artifact target is never an outer runtime around
rootless Podman.

During each canary, run `artifacts/infra/verify-isolation.sh` as root while student
containers are active. Roll back by setting `EnableAutoscaling=false`, stopping the Rust
controller, and restarting the old worker. Supabase remains the durable queue throughout.
