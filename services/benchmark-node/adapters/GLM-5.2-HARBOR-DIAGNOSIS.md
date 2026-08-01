# GLM-5.2 Harbor Run Diagnosis

Date: 2026-07-30

## Incident

The GLM-5.2 Harbor run on `music-harmony` was stopped after 41 minutes:

- 18 agent turns
- 610k prompt tokens
- 455k completion tokens
- no verifier verdict
- Harbor process and `music-harmony__uysazn4__env-main-1` required `SIGKILL`

The development server on port 3000 was deliberately left running. The unrelated
Academy server on port 3999, its `eamig` Postgres container, and
`respectaso-web-1` were left untouched.

## Diagnosis

This was primarily a model/agent-budget problem, not a stuck verifier and probably
not a slow provider.

`music-harmony/task.toml` gives the agent 7,200 seconds and estimates one hour for
a human expert. Its verifier timeout is only 60 seconds. Because GLM never declared
the task complete, the verifier had not started; "no verdict" meant that the agent
phase was still running.

The token and timing numbers show uncontrolled generation:

- 41 minutes / 18 turns = about 137 seconds per turn.
- 455k completion tokens / 41 minutes = about 185 completion tokens per second,
  averaged across model calls and shell work.
- 1.065 million total tokens were processed for one task.
- At turn 14 the run had used about 354k prompt and 41k completion tokens. By turn
  18 it reported 610k prompt and 455k completion tokens.

That final jump strongly suggests Terminus 2 approached the model context limit and
activated its automatic three-call summarization flow. Summarization then amplified
an already verbose reasoning model.

The reference `HarborAgent` sets `max_turns=40`, but that limits rounds rather than
tokens or time per round. At the observed rate, the task could have run for roughly
91 minutes before exhausting its turn budget, still inside the task's two-hour
timeout.

## What Was Misconfigured

1. An expert-hour, specialized music task was used as the first model viability
   test.
2. The model was allowed 40 turns and the task's native two-hour agent timeout.
3. There was no per-call output or reasoning-token budget.
4. Terminus 2 automatic summarization remained enabled.
5. A missing verdict was initially treated as possible benchmark delay even though
   the agent had not entered the verifier phase.

## Full-Run Projection

Seventy tasks at 41 minutes each would take about 48 hours serially. The runner uses
four-way concurrency, so the idealized projection is approximately 12 hours, almost
exactly equal to the runner's global Frontier timeout.

This is still unsafe:

- provider rate limits may prevent four-way scaling;
- task durations vary and some have two-hour agent budgets;
- environment startup and verification add overhead;
- the projected traffic is about 42.7M prompt and 31.9M completion tokens if this
  task were representative.

## Recommended Model Screen

Before authorizing another full paid run:

- test 3-4 tasks from different domains;
- exclude `music-harmony` from the initial screen;
- cap the agent at 10-12 turns;
- cap each task at 10-15 minutes;
- cap each model response at roughly 2k-4k output tokens;
- disable Terminus 2 summarization during screening;
- require at least one clean verifier verdict;
- compare per-request latency separately from shell-command time.

## Conclusion

GLM-5.2 was a poor fit for this task under the current settings. The provider may
have contributed latency, but the observed throughput does not support provider
slowness as the primary cause. The harness's real mistake was allowing a verbose
reasoning model to consume a two-hour, 40-turn budget without a response-token or
screening-time ceiling.
