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
- [ ] Pass a small concurrent ARC probe before starting all ten public games.
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
- [ ] Add Bedrock token/latency/error telemetry and the rolling-rate watchdog.
- [ ] Replace host execution and shared venv mutation with rootless isolation.
- [ ] Run ten pinned ARC public games concurrently with natural termination.
- [ ] Run the five pinned Frontier Sprint tasks with 120-second deadlines.
- [ ] Replace RAM probing with cgroup-wide descendant PSS measurement.
- [ ] Add encrypted team Kaggle credentials and explicit official submission.
- [ ] Update the dashboard, progress UI, history, instructions, and mobile CSS.
- [ ] Run compile/build-only verification and correct discovered defects.
- [ ] Audit the final diff, update both implementation documents, and hand off.

## Deferred live checks (do not run in this implementation pass)

- [ ] Simultaneous worker claims cannot receive the same run.
- [ ] Lease expiry/retry and stale-worker HTTP 409 behavior.
- [ ] Hostile repository cannot access host files, credentials, or general egress.
- [ ] Bedrock throttling and two-window throughput failure behavior.
- [ ] ARC `WIN`, `GAME_OVER`, and global-deadline scoring.
- [ ] Frontier task timeout, verifier timeout, partial reward, and crash handling.
- [ ] RAM descendants remain in the cgroup and short-lived peaks are captured.
- [ ] Partial runs retain every completed leaderboard result.
- [ ] Kaggle token save/replace/delete and official polling.
