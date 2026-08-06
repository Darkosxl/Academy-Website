# Agentic Harness implementation TODO

- [x] Create the decision log and live TODO.
- [x] Capture the baseline build and dirty-worktree boundaries (`cargo check`,
  Python bytecode compilation, and JavaScript syntax checking pass; four
  unrelated Monopoly dead-code warnings remain).
- [x] Add the additive database migration and benchmark-version backfill.
- [x] Add atomic claims, leases, heartbeats, retries, and partial results.
- [x] Add independent benchmark state to the worker and student status APIs.
- [x] Build the Bedrock Mantle Responses gateway and student-facing Chat
  Completions compatibility adapter.
- [x] Select the first account-available production model: `xai.grok-4.3` via
  Bedrock Mantle Responses.
- [x] Verify Grok through the real local gateway for plain text and a complete
  two-turn function-call/result loop.
- [x] Run one pinned ARC public game through the real sandbox and Grok gateway.
  Native Chat completed 16 valid calls with zero errors in 45 seconds, but the
  terminal rolling window was 9/10, so throughput is not yet accepted.
- [x] Bound ARC execution to five concurrent games and refill slots until all games finish.
- [x] Expand the scored ARC set to thirteen pinned public games and move the
  leaderboard pin to `harness-2026-sprint-v2`.
- [x] Expand v3 to all 25 public games, add manual cancellation, and keep v2 scores historical.
- [x] Capture ARC frames in the trusted controller and post them fire-and-forget
  to `POST /api/worker/harness/arc/frames`.
- [x] Store frames as 64x64 hex grids and serve the team's own run from
  `GET /agentic-harness/arc/live`.
- [x] Render the live board grid, click-to-focus, and finished-run replay on the
  student page.
- [x] Split the repository into Academy, benchmark-node, Monopoly, protocol, and
  EC2 infrastructure directories without discarding the dirty v2 work.
- [x] Move worker DTOs and the benchmark version into `benchmark-protocol` and
  add serialization compatibility tests.
- [x] Add the Rust controller/executor split, bounded NDJSON, run-scoped Unix
  gateway, Secrets Manager loading, health, and Prometheus metrics.
- [x] Add the root-context artifact build, hash-locked Python wheelhouse,
  systemd users/units, Ubuntu 24.04 cloud-init, and private EC2 stack.
- [x] Add queue-aware one-to-five-node ASG scaling, per-claim scale-in protection,
  lifecycle draining, and a prepared-AMI activation gate.
- [ ] Ask AWS Sales/account support to enable `openai.gpt-5.6-terra` for this
  account. The key and `provider_data_share` retention mode are valid, but the
  model-specific `/v1/models/{model}` status and inference both deny account
  access even though the bulk catalog lists Terra.
- [ ] Include `openai.gpt-5.6-sol` and `openai.gpt-5.6-luna` in that AWS case;
  their detailed model status and real inference calls fail identically to
  Terra despite compatible retention and bulk-catalog visibility.
- [ ] Include the Anthropic account-access inconsistency in the same AWS case:
  all current Mantle Claude models report detailed status `unavailable`, and
  Fable 5, Sonnet 5, and Opus 5 fail real Runtime invocation even though the
  Runtime control plane reports them authorized and agreement-available.
- [x] Add Bedrock token/latency/error telemetry and the rolling-rate watchdog.
- [x] Replace host execution and shared venv mutation with rootless isolation.
- [x] Replace the thirteen-game all-at-once cohort with all 25 public games and
  a five-slot refill queue that does not abort on a throughput miss.
- [ ] Run one complete paid 25-game v3 canary and exercise manual cancellation
  on a separate development run.
- [ ] Run the five pinned Terminal Sprint tasks with 120-second deadlines.
- [x] Replace RAM probing with cgroup-wide descendant PSS measurement.
- [x] Add encrypted team Kaggle credentials and explicit official submission.
- [x] Update the dashboard, progress UI, history, instructions, and mobile CSS.
- [x] Add repeatable canary load and EC2 isolation verification scripts.
- [x] Run compile/build-only verification and correct discovered defects.
- [x] Audit the final diff, update both implementation documents, and hand off.

## Deferred live checks (do not run in this implementation pass)

- [ ] Simultaneous worker claims cannot receive the same run.
- [ ] Lease expiry/retry and stale-worker HTTP 409 behavior.
- [ ] Hostile repository cannot access host files, credentials, or general egress.
- [ ] Bedrock throttling and two-window throughput failure behavior.
- [ ] ARC `WIN`, `GAME_OVER`, and global-deadline scoring.
- [ ] Thirteen boards stay current in the browser while a run plays, and the
  same run replays from the stored frames after it finishes.
- [ ] A failing or slow frame endpoint leaves game pace and scores untouched.
- [ ] Five simultaneous team runs scale to five workers, then return to one only
  after all leases finish and the 15-minute idle window passes.
- [ ] Five-worker model traffic stays within account quotas and 65 live ARC games
  retain p95 frame-to-screen latency below 2.5 seconds.
- [ ] Terminal task timeout, verifier timeout, partial reward, and crash handling.
- [ ] RAM descendants remain in the cgroup and short-lived peaks are captured.
- [ ] Partial runs retain every completed leaderboard result.
- [ ] Kaggle token save/replace/delete and official polling.
