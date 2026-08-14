# Runs monopoly_controller.py as a Dokploy Compose service (see
# compose.dokploy.yml). Build context is this directory — the worker is
# self-contained (monopoly_game_engine is a subpackage here), it doesn't need
# the rest of the repo.
FROM python:3.12-slim-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends git git-lfs ca-certificates docker.io \
    && rm -rf /var/lib/apt/lists/* \
    && git lfs install --system

# monopoly_controller.py finds the repo root via Path(__file__).parents[1],
# so it needs to sit two directories deep, mirroring services/monopoly-worker
# in the real checkout, not flattened into /app.
WORKDIR /app/services/monopoly-worker
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY . .

# Needs docker.sock mounted in (see compose.dokploy.yml) to run submitted
# agents as sibling containers on the host's own Docker daemon — that socket
# grant is root-equivalent on the host regardless of the in-container user,
# so this doesn't bother dropping to a non-root user.
ENTRYPOINT ["python3", "monopoly_controller.py"]
CMD ["--max-colab", "0", "--host-workers", "2"]
