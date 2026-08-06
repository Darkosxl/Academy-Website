# Exposure Academy monorepo

The Academy web application remains on its current host and persists all durable state in
Supabase. Only Agentic Harness execution moves to the dedicated EC2 benchmark node.

```text
services/academy/          Axum website and authenticated worker API
services/benchmark-node/   Rust controller/executor plus Python SDK adapters
services/monopoly-worker/  Separate GPU-backed Monopoly deployment
crates/benchmark-protocol/ Shared worker DTOs and benchmark version
infra/ec2/                 Ubuntu 24.04 EC2 provisioning and systemd units
docs/                      Specs, design directions, and implementation decision logs
```

The browser still talks only to Academy. Live ARC data follows
`EC2 -> Academy API -> Supabase -> Academy API -> browser`; no EC2 service port or worker
credential is exposed to students.

See [services/academy/README.md](services/academy/README.md) for the website and
[services/benchmark-node/README.md](services/benchmark-node/README.md) for benchmark-node
operation and rollout. Planning material is indexed in [docs/README.md](docs/README.md).

Student-facing assets — Beginner Track briefs, cheat sheets, demos — are served by the
website and belong under `services/academy/static/`, never at the repo root.
