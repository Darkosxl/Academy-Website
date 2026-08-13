FROM python:3.12-slim-bookworm@sha256:4766d8b510c428e595d74b9cc5bbb2fae8e26316fffb4adc89908d79aacd58a2

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PATH=/home/agent/.local/bin:$PATH

RUN useradd --create-home --uid 10001 agent
COPY --chown=agent:agent submission/ /app/submission/
COPY --chown=root:root agent_runner.py /opt/exposure-agent-runner.py
COPY --chown=root:root wheelhouse/ /opt/exposure-wheels/
COPY --chown=root:root requirements.lock /opt/exposure-requirements.lock

USER agent
WORKDIR /app/submission
RUN if [ -s /opt/exposure-requirements.lock ]; then \
      python -m pip install --user --no-cache-dir --no-index --only-binary=:all: \
        --find-links=/opt/exposure-wheels --requirement /opt/exposure-requirements.lock; \
    fi

ENTRYPOINT ["python", "/opt/exposure-agent-runner.py"]
