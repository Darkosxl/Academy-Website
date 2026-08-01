# EC2 benchmark node

`stack.yaml` creates one private `c8i.8xlarge` (32 vCPU, 64 GiB) with an encrypted
200 GB gp3 root volume. It has no public IP, no ingress rules, IMDSv2-only metadata,
and outbound TCP 443 only. Place it in a private subnet with NAT or the required VPC
endpoints; operators connect through SSM, never SSH.

The instance role can use SSM and read only the supplied benchmark secret. Store this JSON
in Secrets Manager:

```json
{"worker_token":"same-value-as-Academy-WORKER_TOKEN","bedrock_api_key":"..."}
```

The generic `cloud-init.yaml` is the host bootstrap for launches outside CloudFormation.
Build the checksum-covered bundle from the repository root:

```bash
docker build --target artifacts \
  --output type=local,dest=/tmp/exposure-benchmark-artifacts \
  -f services/benchmark-node/Dockerfile .
```

Transfer that directory through SSM or an approved private artifact store. Populate the
pinned ARC/Frontier sources under `/var/lib/exposure-benchmark/cache`, then run:

```bash
sudo /path/to/artifacts/infra/install-artifacts.sh /path/to/artifacts
```

The installer validates `SHA256SUMS`, recreates the Python venv from the local hash-locked
wheelhouse, installs systemd assets, and builds the rootless sandbox image when the ARC
cache is ready. It enables but does not start the services unless passed `--start`.

Production runs `benchmark-controller` and `benchmark-executor` directly under systemd.
The artifact image is a build/output envelope only; do not run Podman inside it.

During the canary, run `artifacts/infra/verify-isolation.sh` as root while student
containers are active. It checks credential unreadability, controller UID pinning, systemd
hardening, EC2 metadata denial, zero container capabilities, absence of engine-socket
mounts, and external network denial.
