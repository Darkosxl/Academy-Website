FROM python:3.12-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential git socat \
    && rm -rf /var/lib/apt/lists/* \
    && python -m pip install --no-cache-dir \
       "arc-agi==0.9.9" "openai>=1,<3" "python-dotenv>=1,<2"

COPY vendor/ARC-AGI-3-Agents /opt/arc-agents
ENV PYTHONPATH=/opt/arc-agents
WORKDIR /submission
