# Agentic Harness implementation decisions

Status: accepted for implementation on 2026-07-31. Update this file before
implementing any deviation.

## Product contract

- One public GitHub submission produces independent ARC-AGI-3, Frontier Sprint,
  and RAM-bench results. A failure in one benchmark does not erase completed
  results from another.
- Local evaluation has a hard 600-second wall-clock deadline from queue claim.
  The official Kaggle workflow is explicit, asynchronous, and never changes the
  local leaderboard.
- Leaderboards are versioned. `harness-2026-sprint-v2` pins the model profile
  fingerprint, dataset revisions, task/game lists, limits, and scoring rules.
  Configuration changes require a new version rather than mixing scores.
- v2 supersedes `harness-2026-sprint-v1` because the scored ARC set grew from
  ten games to thirteen. An ARC score is an aggregate over the pinned set, so a
  v1 total and a v2 total measure different things and ranking them together
  would be wrong. The resulting leaderboard reset is deliberate: v1 rows stay
  readable under their own version instead of being rescored or migrated.
- The existing left-side submission, right-side switchable leaderboards,
  history, team administration, and instructions remain the product shape.

## AWS Bedrock

- AWS Bedrock is the only LLM backend. The harness uses xAI Grok 4.3 through
  Bedrock Mantle's OpenAI-compatible API at
  `https://bedrock-mantle.us-east-1.api.aws/openai/v1`, with model ID
  `xai.grok-4.3`. Grok 4.3 supports Mantle Responses and Chat Completions, but
  not Bedrock Runtime Converse/Invoke or cross-Region inference. The harness
  uses native Chat Completions for `xai.*`; `openai.*` models continue through
  the Responses adapter.
- `BEDROCK_API_KEY` is passed to the trusted Mantle client as its bearer API
  key. It never reaches a submission container. Explicit `AWS_REGION`,
  `BEDROCK_MODEL_ID`, and `BEDROCK_PROFILE_NAME` override the defaults;
  concurrency defaults to 32.
- Grok reasoning defaults to `none` through `BEDROCK_REASONING_EFFORT`; accepted
  values are `none`, `low`, `medium`, and `high`. Both default and `low` spent
  an entire 128-token smoke budget on hidden reasoning without visible output,
  while `none` returned the requested text in six output tokens. `none` is
  therefore selected for reliable actions and the 100-turns-per-30-seconds target.
  Requests do not set sampling parameters. The gateway records counts, token usage,
  latency, stop reason, and errors; it never records prompts, responses, AWS
  credentials, or displays the API key.
- Student agents keep an OpenAI-compatible HTTP contract. A trusted local
  gateway translates Chat Completions messages and function calls to/from
  Bedrock Mantle Responses. Each run receives a short-lived gateway bearer
  token.
- Automatic LLM summarization is disabled. The task/system prompt and latest
  12 turns are retained, individual tool results are capped at 8 KiB, and a
  foreground tool command is capped at 30 seconds.

## Throughput and deadlines

- ARC and Frontier each enforce 100 aggregate completed turns per rolling
  30-second window. A turn completes when a Bedrock response has been converted
  and its game action or tool command has been dispatched.
- The target is prorated by model-eligible active time. Terminal sessions and
  sessions waiting for an already-dispatched command are excluded. Sampling
  begins after the first full window and runs every five seconds; two
  consecutive misses abort the cohort as an infrastructure throughput failure.
- Infrastructure/Bedrock failures do not post leaderboard scores. Genuine game
  or task timeouts score zero only for the affected item.

## Benchmark definitions

- ARC uses `arc-agi` 0.9.9, starter commit
  `eeb1535404f321d280a8f9194bbc1d7aca5f05fc`, and these thirteen pinned public
  games in parallel: `ls20`, `vc33`, `ar25`, `cn04`, `s5i5`, `sp80`, `bp35`,
  `ft09`, `m0r0`, `re86`, `cd82`, `sb26`, `r11l`. The original ten keep their
  order and `cd82`, `sb26`, and `r11l` are appended, so a game's position is
  stable across versions.
- ARC has no harness action cap. The trusted controller ends a game on its first
  `WIN` or `GAME_OVER`; it does not reset after death. A game still active at
  the global deadline scores zero.
- Frontier uses Harbor 0.20.0 and dataset commit
  `3d3a3b63152c76eaf4ade56cea9ffac1a1bcafe3`. Frontier Sprint v1 runs
  `html-js-filter`, `vllm-deepseek-streaming`, `session-window-debug`,
  `mvcc-lsm-compaction`, and `embedding-drift-monitor` concurrently.
- Frontier has no turn cap. Each task gets 120 seconds of agent time and a
  60-second verifier cap. Its score is 100 times the mean of the five verifier
  rewards. This is explicitly labelled a sprint, not a full Frontier-bench
  result.
- RAM-bench runs `main.py` once and then ten times concurrently. Every process
  reads the same fixed prompt, must make exactly one successful gateway request,
  emit non-empty stdout, and exit within ten seconds. Aggregate descendant PSS
  is sampled every 20 ms inside the run cgroup; peak PSS is ranked, while cgroup
  `memory.peak` is diagnostic only. `HARNESS_RAM_PROBE` is removed.
- Fixed RAM prompt: `Reply with exactly three concise bullets explaining how
  you would inspect this repository for a failing test. Do not edit files.`

## Live ARC viewing

- Students watch their team's ARC run as a grid of live boards, one per pinned
  game, and open one game to follow it closely. Frames are captured in the
  trusted game controller, never in a submission container, so what the grid
  shows is engine state a student agent cannot forge.
- Frame posts to `POST /api/worker/harness/arc/frames` are fire-and-forget: a
  slow, failed, or rejected post is swallowed and the game continues at full
  speed. The feed is cosmetic and must never fail or delay a scored run. The
  site answers HTTP 409 for an unknown or already-terminal run so a zombie
  controller stops posting instead of retrying forever.
- A grid is stored as text: one lowercase hex character per cell of the fixed
  64x64 board, 4096 characters row-major, grids in one frame joined by
  newlines. Text needs no image pipeline, no blob storage, and no decoder on
  either side. `FrameData.frame` is an intra-action animation buffer rather
  than a history, so buffers longer than sixteen grids keep the first fifteen
  and the last one; the resulting board stays the final element.
- Visibility is own-team only and is resolved from the session, never from the
  run ID in the URL. `GET /agentic-harness/arc/live` honours a run parameter
  only when that run belongs to the caller's team, and falls back to the team's
  latest run when the parameter is absent. Guessing a UUID reveals nothing.
- The stored feed is both the live view and the replay archive. There is no
  separate recording step and no expiry: a finished run replays from the same
  rows the live grid polled, so replay correctness follows from the live path
  being correct.
- Recording frames introduces no harness action cap. ARC still ends a game on
  its first `WIN` or `GAME_OVER`, and per-game frame volume stays unbounded.

## Isolation and trust boundaries

- The trusted worker never imports or directly executes student Python.
  Cloning, dependency installation, module imports, smoke checks, and benchmark
  agents run in rootless Podman with no capabilities, no-new-privileges,
  cgroup/PID limits, read-only benchmark mounts, and per-run writable storage.
- Runtime submissions have no general external network. A local TCP-to-Unix-
  socket relay exposes only the authenticated model gateway. Build containers
  may resolve dependencies but receive no deployment secrets.
- GitHub URLs must use HTTPS with the exact `github.com` host and no embedded
  credentials, query, fragment, or submodules. Repository size and clone/build
  time are bounded.
- The whole repository is mounted so auxiliary imports work. Shared ARC and
  Harbor environments are immutable; no student dependency is installed into
  them. Scores come from trusted game/verifier state, never student stdout.
- Harbor 0.20 loads a custom agent in its controller process and needs a
  container API socket. The controller therefore runs inside Bubblewrap with
  no home-directory mount, no external network, only the run repository,
  sprint dataset, output directory, Bedrock Unix socket, and a rootless Podman
  socket. Production requires `HARNESS_HARBOR_USER` to name a dedicated
  unprivileged service account; sharing the website account's rootless Podman
  socket is allowed only in explicit local-development mode because that
  account remains the isolation ceiling.
- Submission repositories are capped at 100 MiB, individual files at 10 MiB,
  and `requirements.txt` at 32 KiB. Symlinks, submodules, editable installs,
  direct URLs/VCS requirements, pip options, and local-path requirements are
  rejected. These keep the disposable build boundary inspectable and prevent
  dependency resolution from becoming a second arbitrary fetch mechanism.

## Queue, storage, and API

- Claiming moves to `POST /api/worker/harness/claim` using `FOR UPDATE SKIP
  LOCKED`. A claim carries a 90-second lease, renewed every 30 seconds. Every
  worker mutation supplies the lease token; stale updates return HTTP 409.
- Expired claims are retried at most three times, then become infrastructure
  failures. The worker polls every two seconds.
- Run states are `queued`, `preparing`, `running`, `done`, `partial`, `failed`,
  and `infra_failed`. Per-benchmark status/progress/error data is JSON; existing
  score columns remain for simple leaderboard queries.
- Successfully completed benchmark scores appear even when the overall run is
  partial. The status endpoint returns independent benchmark cards plus the run
  deadline, version, commit, and safe telemetry.
- Terminal results are idempotent for the same lease. The benchmark execution
  deadline remains 600 seconds; the active lease has a 30-second result-only
  grace so controller cancellation and the final Academy round trip can finish.
  Progress and frame mutations stop at the hard deadline.

## EC2 split and deployment

- The repository is a Cargo workspace with Academy in `services/academy`, the
  CPU benchmark service in `services/benchmark-node`, the unchanged GPU worker
  in `services/monopoly-worker`, and wire DTOs plus the version constant in
  `crates/benchmark-protocol`.
- Academy stays on its current instance and Supabase remains the only durable
  database. The benchmark host exposes no application ingress; its only TCP
  listener is loopback health/metrics. Browsers continue to use Academy routes.
- The trusted Rust controller and restricted Rust executor are distinct systemd
  users. The controller owns Academy/AWS/provider credentials. The executor sees
  only bounded NDJSON, a run-scoped model capability, and the files needed for
  rootless Podman/Bubblewrap execution.
- Production is host-native. A root-context multi-stage Docker build emits
  checksum-covered binaries, adapters, locked wheels, the sandbox Containerfile,
  and systemd assets; it is not an outer runtime container.
- Private Ubuntu 24.04 x86-64 workers are `c8i.8xlarge`, each with a 200 GB
  encrypted gp3 volume, IMDSv2, SSM management, Secrets Manager read access, no
  public IP or ingress, and outbound TCP 443 only. Mutable state is rooted at
  `/var/lib/exposure-benchmark`.
- One worker runs one complete benchmark at a time. Academy reports global
  claimable plus leased demand; controllers publish it to CloudWatch, and an ASG
  scales from one prepared-AMI worker to at most five. Claimed nodes use EC2
  scale-in protection and a 15-minute termination hook drains the claim/protection
  race. Autoscaling remains disabled until the golden AMI and two-node canary pass.

## Kaggle

- Teams may save a Kaggle username and API token. The token is encrypted at
  rest with XChaCha20-Poly1305 using deployment key
  `KAGGLE_CREDENTIAL_KEY`; it is never returned or logged and can be replaced
  or deleted.
- A completed local ARC run exposes an explicit official-submit action. The
  worker re-clones the exact commit, builds a self-contained notebook from the
  official starter, pushes it, and submits it to
  `arc-prize-2026-arc-agi-3`.
- Official states are `queued`, `kernel_running`, `submitted`, `scored`, and
  `failed`. Official runtime and score are separate from all local statuses and
  rankings.

## Verification policy

- Incremental paid Bedrock smoke requests are authorized when the user supplies
  the deployment API key and asks for a live test. Run a single minimal gateway
  request before starting ARC, Frontier, or RAM cohorts so model-access or quota
  failures stop cheaply.
- Local operators may run `services/benchmark-node/adapters/live_arc.py` against one pinned public game
  before starting a scored cohort. The diagnostic uses the production ARC game
  controller, rootless submission container, run-scoped gateway, and required
  action tool calls. Its short wall-clock deadline is diagnostic only: it never
  posts a score, changes the thirteen-game rules, or imposes an action cap.
- The first `ls20` live probe used the official ARC `FastLLM` template. It made
  six valid Grok requests with zero errors, but sent 127,578 input tokens and
  completed only four actions in the final 30 seconds. The operator fixture now
  uses one stateless request per action with a run-length encoded final grid and
  a short action history. This isolates Bedrock/gateway throughput from the
  official template's raw multi-frame history overhead; it is a diagnostic
  policy, not a mandated student-agent implementation.
- The first compact probe reduced six requests to 5,303 input tokens and about
  eight seconds of completion time, but its sixth 64-token response contained
  no action tool call. The live fixture therefore uses the same 128-token budget
  that passed the standalone Grok tool-call smoke; missing tool calls remain a
  hard diagnostic failure and are never replaced by a random action.
- A 128-token rerun reached 12 completions in about 30 seconds, but one response
  still ended at the token limit without choosing among four separate action
  tools. The diagnostic now exposes one forced `take_action` function with the
  currently legal actions as an enum argument. Grok still selects the move,
  while Mantle can enforce the exact function instead of only requiring some
  tool; malformed or missing calls remain hard failures.
- The forced single-tool Responses probe still produced one token-limit failure
  after 17 valid actions. A direct native Mantle Chat Completions probe then
  returned valid forced tool calls on 20/20 requests (48.07 seconds total,
  2.40-second mean, 12.85-second worst case). Grok therefore uses native Chat
  Completions in the gateway; the Chat-to-Responses translation remains for
  OpenAI models only.
- The first sandboxed `ls20` run after that switch completed 16 valid native
  Chat action calls in 45 seconds with zero gateway errors, 14,192 input tokens,
  and 37.84 seconds of aggregate model latency. Its terminal rolling window was
  9 completions versus the prorated 10-per-30-second target. This is enough to
  validate the route and isolation path, but not enough to authorize the full
  thirteen-game cohort; concurrency must first pass a smaller aggregate probe.
- The 2026-07-31 Sonnet 5 smoke reached Bedrock through the real gateway but
  returned `AccessDeniedException`. A direct in-Region request confirmed
  `anthropic.claude-sonnet-5` is not available to this AWS account. No ARC,
  Frontier, RAM, or Kaggle run was started.
- A direct 2026-07-31 GPT-5.6 Terra Mantle smoke also reached AWS but returned
  HTTP 401 `access_denied`: `openai.gpt-5.6-terra` is not available to this
  account. The gateway remains configured for Terra so it is ready when AWS
  enables model access.
- The follow-up Mantle diagnostics use the management route
  `/v1/models/openai.gpt-5.6-terra` (not Terra's inference-only
  `/openai/v1` prefix). The same key reports account retention mode
  `provider_data_share`, which is included in Terra's allowed modes, but the
  model-specific status remains `unavailable` with an account-access reason.
  The bulk model catalog reports Terra as `available`; therefore catalog and
  regional availability are not proof of account invocation entitlement.
- Anthropic diagnostics show the same catalog-versus-account split. Detailed
  Mantle status is `unavailable` for Fable 5, Haiku 4.5, Sonnet 5, Opus 4.7,
  Opus 4.8, and Opus 5. The account retention mode is compatible with every
  model (`provider_data_share`, required by Fable 5). Runtime control-plane
  checks report authorization, entitlement, and Region as available; they also
  report agreements available for Fable 5, Sonnet 5, and Opus 5, but real
  Runtime calls to all three still return account-level `AccessDeniedException`.
  Opus 4.7 and 4.8 additionally report agreement status `NOT_AVAILABLE`.
- Opus 4.8 was also tested through both documented inference routes: the
  Runtime US profile `us.anthropic.claude-opus-4-8` returned
  `AccessDeniedException`, and Mantle `/anthropic/v1/messages` returned HTTP
  403 `permission_error`. The bare model ID is not a valid in-Region Runtime
  target for Opus 4.8; it is valid only on Mantle.
- GPT-5.6 Sol and Luna match Terra: each appears `available` in the bulk Mantle
  catalog, has a compatible `provider_data_share` account retention mode, but
  reports `unavailable` from `/v1/models/{model}` and returns HTTP 401
  `access_denied` from a real `/openai/v1/responses` request.
- `xai.grok-4.3` is the first confirmed working model for this account. Its
  account-specific status is `available`, and a paid Mantle Responses smoke
  completed inference with token usage. Harness v1 therefore selects Grok 4.3
  instead of the account-blocked GPT-5.6 and current Claude models.
- End-to-end gateway verification with reasoning effort `none` returned visible
  `OK` in 1.82 seconds using 27 input and 6 output tokens. A separate two-turn
  tool smoke produced a required `echo` function call, accepted its tool result,
  and returned a final answer; both requests were HTTP 200 with zero errors.
- Live benchmark cohorts and Kaggle submission remain deferred until the
  selected model passes the one-request smoke test.
