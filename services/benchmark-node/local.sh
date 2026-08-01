#!/usr/bin/env bash
set -Eeuo pipefail

benchmark_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$benchmark_dir/../.." && pwd)"
state_dir="${BENCHMARK_DEV_STATE_DIRECTORY:-$benchmark_dir/state}"
cache_dir="${HARNESS_CACHE_DIRECTORY:-$repo_root/worker/cache}"
venv_dir="${BENCHMARK_DEV_VENV:-$state_dir/venv}"
python_command="${BENCHMARK_DEV_PYTHON:-python3.12}"
lock_file="$benchmark_dir/adapters/requirements.lock"
adapter="$benchmark_dir/adapters/runner.py"

die() {
    echo "benchmark local: $*" >&2
    exit 1
}

usage() {
    cat <<'EOF'
Usage: bash services/benchmark-node/local.sh COMMAND

Commands:
  setup        create/update the locked local Python environment
  game         run one sandboxed, model-backed ARC game
  game-engine  run the free ARC engine-only development loop
  stack        run Academy, benchmark-executor, and benchmark-controller locally
EOF
}

require_arc_cache() {
    [[ -x "$cache_dir/arc-starter/.venv/bin/python" ]] ||
        die "ARC cache missing at $cache_dir/arc-starter (restore it or set HARNESS_CACHE_DIRECTORY)"
    [[ -d "$cache_dir/arc-starter/environment_files" ]] ||
        die "ARC environment cache missing at $cache_dir/arc-starter/environment_files"
}

require_full_cache() {
    require_arc_cache
    [[ -d "$cache_dir/frontier-bench/frontier-bench" ]] ||
        die "Frontier cache missing at $cache_dir/frontier-bench/frontier-bench"
}

setup_venv() {
    command -v "$python_command" >/dev/null 2>&1 ||
        die "$python_command is required (set BENCHMARK_DEV_PYTHON to override)"
    mkdir -p "$state_dir"
    if [[ ! -x "$venv_dir/bin/python" ]]; then
        "$python_command" -m venv "$venv_dir"
    fi
    lock_digest="$(sha256sum "$lock_file")"
    lock_digest="${lock_digest%% *}"
    stamp="$venv_dir/.requirements-sha256"
    if [[ ! -f "$stamp" ]] || [[ "$(<"$stamp")" != "$lock_digest" ]]; then
        "$venv_dir/bin/python" -m pip install --require-hashes -r "$lock_file"
        printf '%s\n' "$lock_digest" >"$stamp"
    fi
}

run_game() {
    require_arc_cache
    setup_venv
    game="${GAME:-ls20}"
    game_seconds="${GAME_SECONDS:-45}"
    submission_repo="${SUBMISSION_REPO:-$benchmark_dir/adapters/live_submission}"
    if [[ "$submission_repo" != /* ]]; then
        submission_repo="$repo_root/$submission_repo"
    fi
    arguments=(
        --game "$game"
        --seconds "$game_seconds"
        --repo "$submission_repo"
    )
    if [[ -n "${RUN_ID:-}" ]]; then
        arguments+=(--run-id "$RUN_ID")
    fi
    cd "$repo_root"
    exec env \
        ENVIRONMENT=DEV \
        HARNESS_CACHE_DIRECTORY="$cache_dir" \
        "$venv_dir/bin/python" "$benchmark_dir/adapters/live_arc.py" "${arguments[@]}"
}

run_engine_game() {
    require_arc_cache
    game="${GAME:-ls20}"
    steps="${STEPS:-50}"
    engine_repo="${ENGINE_SUBMISSION_REPO:-$cache_dir/arc-starter}"
    if [[ "$engine_repo" != /* ]]; then
        engine_repo="$repo_root/$engine_repo"
    fi
    exec "$cache_dir/arc-starter/.venv/bin/python" \
        "$benchmark_dir/adapters/play_local_offline.py" \
        --game "$game" \
        --max-steps "$steps" \
        --cache "$cache_dir/arc-starter" \
        --repo "$engine_repo"
}

link_development_cache() {
    mkdir -p "$state_dir"
    local state_cache="$state_dir/cache"
    if [[ ! -e "$state_cache" && ! -L "$state_cache" ]]; then
        ln -s "$cache_dir" "$state_cache"
    fi
    [[ "$(realpath "$state_cache")" == "$(realpath "$cache_dir")" ]] ||
        die "$state_cache already points somewhere else; set BENCHMARK_DEV_STATE_DIRECTORY"
}

link_development_harbor() {
    harbor_binary="$(type -P harbor || true)"
    [[ -n "$harbor_binary" ]] ||
        die "Harbor is required for the full stack (install it with uv tool install harbor)"
    harbor_root="$(cd "$(dirname "$(readlink -f "$harbor_binary")")/.." && pwd)"
    executor_tools="$state_dir/executor/.local/share/uv/tools"
    executor_harbor="$executor_tools/harbor"
    mkdir -p "$executor_tools"
    if [[ ! -e "$executor_harbor" && ! -L "$executor_harbor" ]]; then
        ln -s "$harbor_root" "$executor_harbor"
    fi
    [[ "$(realpath "$executor_harbor")" == "$harbor_root" ]] ||
        die "$executor_harbor already points somewhere else; set BENCHMARK_DEV_STATE_DIRECTORY"
}

run_stack() {
    [[ -f "$repo_root/.env" ]] || die "copy .env.example to .env and fill in local credentials"
    require_full_cache
    setup_venv
    link_development_cache
    link_development_harbor

    runtime_base="${XDG_RUNTIME_DIR:-/tmp}/exposure-benchmark-dev-${UID}"
    executor_socket="$runtime_base/executor.sock"
    gateway_directory="$runtime_base/gateways"
    academy_url="${BENCHMARK_DEV_ACADEMY_URL:-http://127.0.0.1:3000}"
    academy_bind="${BENCHMARK_DEV_ACADEMY_BIND:-127.0.0.1:3000}"
    mkdir -p "$runtime_base" "$gateway_directory"
    [[ ! -e "$executor_socket" ]] ||
        die "$executor_socket already exists; another local stack may be running"

    cd "$repo_root"
    cargo build -p academy -p benchmark-node --bins

    process_ids=()
    cleanup() {
        status=$?
        trap - EXIT INT TERM
        if ((${#process_ids[@]})); then
            kill "${process_ids[@]}" 2>/dev/null || true
            wait "${process_ids[@]}" 2>/dev/null || true
        fi
        rm -f "$executor_socket"
        exit "$status"
    }
    trap cleanup EXIT INT TERM

    ENVIRONMENT=DEV \
    BENCHMARK_EXECUTOR_SOCKET="$executor_socket" \
    BENCHMARK_STATE_DIRECTORY="$state_dir" \
    BENCHMARK_ADAPTER="$adapter" \
    BENCHMARK_PYTHON="$venv_dir/bin/python" \
    "$repo_root/target/debug/benchmark-executor" &
    process_ids+=("$!")

    for _ in {1..100}; do
        [[ -S "$executor_socket" ]] && break
        kill -0 "${process_ids[0]}" 2>/dev/null || wait "${process_ids[0]}"
        sleep 0.1
    done
    [[ -S "$executor_socket" ]] || die "benchmark-executor did not create $executor_socket"

    ENVIRONMENT=DEV \
    BIND="$academy_bind" \
    APP_BASE_URL="$academy_url" \
    "$repo_root/target/debug/academy" &
    process_ids+=("$!")

    ENVIRONMENT=DEV \
    ACADEMY_BASE_URL="$academy_url" \
    BENCHMARK_EXECUTOR_SOCKET="$executor_socket" \
    BENCHMARK_GATEWAY_DIRECTORY="$gateway_directory" \
    "$repo_root/target/debug/benchmark-controller" &
    process_ids+=("$!")

    echo "Local harness is running at $academy_url"
    echo "Submit through the browser; Ctrl-C stops all three processes."
    wait -n "${process_ids[@]}"
}

case "${1:-}" in
    setup) setup_venv ;;
    game) run_game ;;
    game-engine) run_engine_game ;;
    stack) run_stack ;;
    -h|--help|help) usage ;;
    *) usage; exit 2 ;;
esac
