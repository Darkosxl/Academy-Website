-- Live ARC-AGI-3 boards: one row per frame the engine handed back, so a run can be
-- watched while it plays and replayed after it ends. Idempotent for the same reason as
-- 002 — main.rs replays every migration at every boot, there is no ledger.

create table if not exists harness_arc_frames_exposure_academy (
  run_id uuid not null references harness_runs_exposure_academy(id) on delete cascade,
  game text not null,
  seq int not null,
  grids text not null,                       -- newline-separated 4096-char hex grids, >= 1
  state text not null,                       -- NOT_PLAYED | NOT_FINISHED | WIN | GAME_OVER
  levels_completed int not null default 0,
  baseline int[],                            -- per-level human baseline actions
  action text, action_x int, action_y int,   -- action producing this frame; null at seq 0
  created_at timestamptz not null default now(),
  -- Serves both reads: the per-game latest row (distinct on) and the focus tail
  -- (seq > n). A second index would only duplicate its leading columns.
  primary key (run_id, game, seq)
);
