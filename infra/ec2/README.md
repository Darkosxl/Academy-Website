# EC2 benchmark fleet

`stack.yaml` creates a private Auto Scaling group with one worker by default and a hard
maximum of five. Each worker is an `r8i.4xlarge` (16 vCPU, 128 GiB) with an encrypted
200 GB gp3 root volume. There is no public IP, no ingress rule, and IMDSv2 has a hop limit
of one. Place the group in a private subnet with NAT or the required VPC endpoints;
operators connect through SSM, never SSH.

One instance runs one ARC and one Frontier submission concurrently. Legacy bundled and
Kaggle work waits for both lanes. Academy's authenticated capacity API counts claimable
and leased harness/Kaggle work. Every controller publishes the same
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
{"worker_token":"same-value-as-Academy-WORKER_TOKEN","bedrock_api_key":"...","cerebras_api_keys":["...","...","...","..."]}
```

The instance role can read only that secret, publish the benchmark metric namespace, and
manage protection/lifecycle state for its exact Auto Scaling group. It also carries the
standard SSM role.

The key names are intentionally lowercase. The post-launch deployment canary requires
that exact schema; the EC2 role can read this secret but cannot modify it.

## Prepared AMI

Autoscaling defaults to `false`. A raw Ubuntu replacement does not contain the pinned
benchmark caches. Deploy one base node with `CAPABILITY_IAM` only when performing a manual
recovery:

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
    InstanceType=r8i.4xlarge \
    EnableAutoscaling=false
```

On a fresh dedicated builder, the canonical preparation command is:

```bash
sudo infra/ec2/prepare-worker-image.sh "$(pwd)"
```

It verifies the exact source commit, builds and checksums the host artifacts, recreates
the pinned ARC/Frontier/Harbor caches, installs the services without enabling them, builds
the rootless sandbox image, and runs the complete offline canary. Existing caches make it
stop instead of silently reusing mutable state.

Create the production AMI from a dedicated builder that has never accepted a submission.
Before capture, verify the bundle and sandbox image, stop both services, remove
`/etc/exposure-benchmark/controller.env`, and run `cloud-init clean --logs --machine-id`.
The launch template's cloud-init recreates the instance-specific env file and restarts the
services when a clone boots. Do not image an active worker: its mutable run directories can
contain student code.

Update the stack with that AMI and keep one node while the regional quota is 16 vCPUs:

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
    InstanceType=r8i.4xlarge \
    WorkerEnabled=true \
    EnableAutoscaling=false \
    MaxCapacity=1
```

`WorkerEnabled=false` safely scales the group to zero while the dedicated Packer builder
uses the account's entire 16-vCPU quota. The deployment workflow restores the previous
worker AMI automatically if the image build fails. Raise `MaxCapacity` only after the EC2
quota and full-run canaries support it.

## Automatic image deployment

`.github/workflows/deploy-benchmark-worker.yml` builds and deploys a new immutable AMI
whenever benchmark runtime code reaches `arc-live-viewer`. It does not SSH to a mutable
worker or run `git pull` there. The workflow:

1. authenticates with short-lived GitHub OIDC credentials (there are no AWS access keys in
   GitHub);
2. drains the old worker through its lifecycle hook and scales the fleet to zero;
3. builds a fresh private-subnet AMI from the exact Git commit through SSM;
4. runs the offline image canary before capture;
5. launches one replacement, checks its commit, services, secret schema, health endpoint,
   and rootless sandbox image; and
6. restores the previous AMI and fleet settings automatically if any later step fails.

The one-time bootstrap order is intentionally strict:

1. Deploy `stack.yaml` with `WorkerEnabled=false`, `EnableAutoscaling=false`, and
   `MaxCapacity=1`. This terminates the quarantined builder and frees the entire 16-vCPU
   quota; do not run it while a benchmark is active.
2. Create or reuse the account's `token.actions.githubusercontent.com` IAM OIDC provider.
3. Deploy `github-deploy-role.yaml` with `CAPABILITY_NAMED_IAM`.
4. Set repository variables `BENCHMARK_DEPLOY_ROLE_ARN` and
   `BENCHMARK_CLOUDFORMATION_ROLE_ARN` from that stack's outputs.
5. Merge the deployment branch into `arc-live-viewer`. That push performs the first image
   build and brings the worker back only after all canaries pass.

The OIDC trust is restricted to `Darkosxl/Academy-Website` and the `arc-live-viewer`
branch. The deployment role cannot read the benchmark secret. Infrastructure template
changes still require an explicit operator deployment; normal benchmark code changes are
automatic.

## Capacity gates

Five `r8i.4xlarge` workers require 80 On-Demand standard-instance vCPUs and 640 GiB of
aggregate RAM. The current 16-vCPU regional quota permits one worker; request at least 80
before raising the fleet maximum to five. One run has roughly 20 vCPU of configured ARC and
Frontier ceilings, so the one-node canary must confirm the 16-vCPU worker meets benchmark
timeouts. Its 128 GiB leaves ample memory headroom for the configured workload limits.

Five controllers can expose up to 160 simultaneous model calls with the default
`BEDROCK_MAX_CONCURRENCY=32`. Confirm the provider account's request/token limits before
raising the fleet maximum. Also run the frame load test at 25 active games (five workers x
five ARC slots), while polling all 125 board summaries, and require p95 frame-to-screen
latency below 2.5 seconds.

The generic `cloud-init.yaml` remains the host bootstrap for launches outside
CloudFormation. Production runs `benchmark-controller` and `benchmark-executor`
host-native under systemd; the Docker artifact target is never an outer runtime around
rootless Podman.

During each canary, run `artifacts/infra/verify-isolation.sh` as root while student
containers are active. Roll back by setting `EnableAutoscaling=false`, stopping the Rust
controller, and restarting the old worker. Supabase remains the durable queue throughout.
