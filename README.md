# Exposure Academy monorepo

The Academy web application remains on its current host and persists all durable state in
Supabase. Only Agentic Harness execution moves to the dedicated EC2 benchmark node.

```text
services/academy/          Axum website and authenticated worker API
services/benchmark-node/   Rust controller/executor plus Python SDK adapters
services/monopoly-worker/  CPU-only AI Monopoly tournament controller and workers
crates/benchmark-protocol/ Shared worker DTOs and benchmark version
infra/ec2/                 Ubuntu 24.04 EC2 provisioning and systemd units
docs/                      Specs, design directions, and implementation decision logs
```

The browser still talks only to Academy. Live benchmark and tournament data flows through the
authenticated Academy API; no worker service port or worker credential is exposed to students.

See [services/academy/README.md](services/academy/README.md) for the website and
[services/benchmark-node/README.md](services/benchmark-node/README.md) for benchmark-node
operation and rollout. The Monopoly fleet is documented in
[services/monopoly-worker/README.md](services/monopoly-worker/README.md). Planning material is
indexed in [docs/README.md](docs/README.md).

Student-facing assets — Beginner Track briefs, cheat sheets, demos — are served by the
website and belong under `services/academy/static/`, never at the repo root.
