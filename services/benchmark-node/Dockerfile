# syntax=docker/dockerfile:1.7
# Builds the deterministic EC2 artifact bundle and the Dokploy Compose runtime.
# The `artifacts` target remains the host-native deployment bundle used by the AMI workflow.
FROM rust:1.95.0-slim-bookworm AS rust-builder
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY services/academy/Cargo.toml ./services/academy/Cargo.toml
COPY services/academy/src ./services/academy/src
COPY services/academy/migrations ./services/academy/migrations
COPY services/academy/videos.dat ./services/academy/videos.dat
COPY services/benchmark-node/Cargo.toml ./services/benchmark-node/Cargo.toml
COPY services/benchmark-node/src ./services/benchmark-node/src
RUN cargo build --locked --release -p benchmark-node --bins

FROM python:3.12.11-slim-bookworm AS python-builder
WORKDIR /bundle
COPY services/benchmark-node/adapters/requirements.lock ./requirements.lock
RUN python -m pip wheel --require-hashes --no-deps \
      --wheel-dir /bundle/wheels -r /bundle/requirements.lock
COPY services/benchmark-node/adapters ./adapters
RUN python -m compileall -q /bundle/adapters

FROM debian:bookworm-slim AS packager
WORKDIR /artifacts
COPY --from=rust-builder /source/target/release/benchmark-controller ./bin/benchmark-controller
COPY --from=rust-builder /source/target/release/benchmark-executor ./bin/benchmark-executor
COPY --from=python-builder /bundle/adapters ./adapters
COPY --from=python-builder /bundle/requirements.lock ./python/requirements.lock
COPY --from=python-builder /bundle/wheels ./python/wheels
COPY services/benchmark-node/sandbox ./sandbox
COPY services/benchmark-node/systemd ./systemd
COPY infra/ec2/install-artifacts.sh infra/ec2/verify-isolation.sh \
     infra/ec2/verify-running-worker.sh infra/ec2/verify-worker-image.sh ./infra/
RUN find . -type f ! -name SHA256SUMS -print0 \
      | sort -z | xargs -0 sha256sum > /artifacts/SHA256SUMS

FROM scratch AS artifacts
COPY --from=packager /artifacts /
LABEL org.opencontainers.image.title="Exposure benchmark-node deterministic artifacts"

# Pinned public benchmark inputs. They are copied into the runtime image as a seed and
# populated into the persistent Docker volume only on its first boot.
FROM python:3.12.11-slim-bookworm AS cache-builder
ARG ARC_STARTER_COMMIT=eeb1535404f321d280a8f9194bbc1d7aca5f05fc
ARG ARC_AGENTS_COMMIT=10213de83f01df0ef4f0149ee9f8408dcc3772fb
ARG FRONTIER_COMMIT=3d694e919871dbf21ea5ff618782c99a3cb3663f
ARG HARBOR_VERSION=0.20.0
WORKDIR /seed
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*
COPY --from=python-builder /bundle/wheels /wheelhouse
COPY --from=python-builder /bundle/requirements.lock /requirements.lock
RUN set -eux; \
    clone_exact() { \
      git clone --filter=blob:none --no-checkout "$1" "$2"; \
      git -C "$2" checkout --detach "$3"; \
      test "$(git -C "$2" rev-parse HEAD)" = "$3"; \
    }; \
    python -m venv /seed/venv; \
    /seed/venv/bin/python -m pip install --disable-pip-version-check --no-index --no-deps --require-hashes \
      --find-links /wheelhouse -r /requirements.lock; \
    install -d -m 0700 /seed/cache; \
    clone_exact https://github.com/arcprize/ARC-AGI-3-Kaggle-Starter.git \
      /seed/cache/arc-starter "$ARC_STARTER_COMMIT"; \
    install -d -m 0700 /seed/cache/arc-starter/vendor; \
    clone_exact https://github.com/arcprize/ARC-AGI-3-Agents.git \
      /seed/cache/arc-starter/vendor/ARC-AGI-3-Agents "$ARC_AGENTS_COMMIT"; \
    python -m venv /seed/cache/arc-starter/.venv; \
    /seed/cache/arc-starter/.venv/bin/python -m pip install --disable-pip-version-check --no-index --no-deps --require-hashes \
      --find-links /wheelhouse -r /requirements.lock; \
    install -d -m 0700 /seed/cache/frontier-bench; \
    clone_exact https://github.com/harbor-framework/frontier-bench.git \
      /seed/cache/frontier-bench/repo "$FRONTIER_COMMIT"; \
    ln -s repo/tasks /seed/cache/frontier-bench/frontier-bench; \
    install -d -m 0700 /seed/executor/.local/share/uv/tools; \
    python -m venv /seed/executor/.local/share/uv/tools/harbor; \
    /seed/executor/.local/share/uv/tools/harbor/bin/python -m pip install --disable-pip-version-check --no-cache-dir \
      "harbor==$HARBOR_VERSION"; \
    /seed/executor/.local/share/uv/tools/harbor/bin/harbor --help >/dev/null
RUN ARC_CACHE=/seed/cache/arc-starter \
    /seed/cache/arc-starter/.venv/bin/python - <<'PY'
import os
import arc_agi
from arc_agi import OperationMode

arcade = arc_agi.Arcade(
    operation_mode=OperationMode.NORMAL,
    environments_dir=os.path.join(os.environ["ARC_CACHE"], "environment_files"),
)
for game in (
    "bp35", "m0r0", "ft09", "ar25", "s5i5", "sp80", "g50t", "lp85", "r11l",
    "sc25", "tn36", "sk48", "re86", "wa30", "cn04", "tu93", "tr87", "sb26",
    "su15", "ls20", "ka59", "cd82", "dc22", "vc33", "lf52",
):
    assert arcade.make(game) is not None
PY
RUN printf '%s\n' \
      "arc-starter=$ARC_STARTER_COMMIT" \
      "arc-agents=$ARC_AGENTS_COMMIT" \
      "frontier=$FRONTIER_COMMIT" \
      "harbor=$HARBOR_VERSION" \
      > /seed/.cache-version

# A single Compose service owns both trusted processes. The service needs privileged
# runtime settings in compose.yaml for nested *rootless* Podman; never mount the host
# Docker socket into it.
FROM python:3.12.11-slim-bookworm AS production
ENV DEBIAN_FRONTEND=noninteractive
RUN set -eux; \
    apt-get update; \
    apt-get install -y --no-install-recommends \
      bash bubblewrap ca-certificates curl fuse-overlayfs git gnupg jq \
      passwd podman procps slirp4netns socat tini uidmap; \
    install -m 0755 -d /etc/apt/keyrings; \
    curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc; \
    chmod a+r /etc/apt/keyrings/docker.asc; \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian bookworm stable" \
      > /etc/apt/sources.list.d/docker.list; \
    apt-get update; \
    apt-get install -y --no-install-recommends docker-buildx-plugin docker-ce-cli docker-compose-plugin; \
    rm -rf /var/lib/apt/lists/*; \
    groupadd --system --gid 10000 exposure-benchmark; \
    groupadd --system --gid 10001 exposure-controller; \
    groupadd --system --gid 10002 exposure-executor; \
    useradd --system --uid 10001 --gid exposure-controller --groups exposure-benchmark \
      --home-dir /var/lib/exposure-benchmark/controller --create-home --shell /usr/sbin/nologin exposure-controller; \
    useradd --system --uid 10002 --gid exposure-executor --groups exposure-benchmark \
      --home-dir /var/lib/exposure-benchmark/executor --create-home --shell /usr/sbin/nologin exposure-executor; \
    printf '%s\n' 'exposure-executor:100000:65536' >> /etc/subuid; \
    printf '%s\n' 'exposure-executor:100000:65536' >> /etc/subgid
WORKDIR /opt/exposure-benchmark
COPY --from=rust-builder /source/target/release/benchmark-controller ./bin/benchmark-controller
COPY --from=rust-builder /source/target/release/benchmark-executor ./bin/benchmark-executor
COPY --from=python-builder /bundle/adapters ./adapters
COPY --from=python-builder /bundle/requirements.lock ./python/requirements.lock
COPY --from=python-builder /bundle/wheels ./python/wheels
COPY services/benchmark-node/sandbox ./sandbox
COPY --from=cache-builder --chown=exposure-executor:exposure-executor /seed ./seed
COPY --chmod=0755 services/benchmark-node/docker/entrypoint.sh /usr/local/bin/benchmark-entrypoint
RUN sha256sum /opt/exposure-benchmark/sandbox/Containerfile > /opt/exposure-benchmark/sandbox.sha256 \
    && chmod 0755 /opt/exposure-benchmark/bin/benchmark-controller /opt/exposure-benchmark/bin/benchmark-executor \
    && chown -R root:root /opt/exposure-benchmark/bin /opt/exposure-benchmark/adapters /opt/exposure-benchmark/python /opt/exposure-benchmark/sandbox
ENV ENVIRONMENT=PROD \
    BENCHMARK_STATE_DIRECTORY=/var/lib/exposure-benchmark \
    BENCHMARK_EXECUTOR_SOCKET=/run/exposure-benchmark/executor/executor.sock \
    BENCHMARK_GATEWAY_DIRECTORY=/run/exposure-benchmark/gateways \
    BENCHMARK_ADAPTER=/opt/exposure-benchmark/adapters/runner.py \
    BENCHMARK_PYTHON=/var/lib/exposure-benchmark/venv/bin/python \
    BENCHMARK_SANDBOX_IMAGE=localhost/exposure-harness-arc:0.9.9 \
    BENCHMARK_METRICS_BIND=127.0.0.1:9108
HEALTHCHECK --interval=30s --timeout=5s --start-period=15m --retries=3 \
  CMD curl --fail --silent --show-error http://127.0.0.1:9108/healthz || exit 1
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/benchmark-entrypoint"]
