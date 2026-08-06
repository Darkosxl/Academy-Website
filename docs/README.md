# Docs

Planning and design material — specs written before the code, decision logs kept
alongside it. None of it is read at runtime; the app ships nothing from this folder.

```text
projects-pool.md              Beginner Track concept pool the görev panosu tasks draw from
nlp-game-spec.md              SLM finetuning game / AI Monopoly feature notes
agentic-harness-spec.md       Agentic Harness feature spec (Kaggle submission constraints)
harness-ui-designs.html       Four Agentic Harness design directions, side by side
implementation/               Decision log, live TODO, and the AWS blocked-model list
```

Operational documentation lives with the thing it operates:
[services/academy](../services/academy/README.md),
[services/benchmark-node](../services/benchmark-node/README.md),
[infra/ec2](../infra/ec2/README.md).

## Beginner Track briefs

The student-facing project PDFs are **not** kept here. They are served by the website
and live in `services/academy/static/beginner-projects/`, named `NN-slug.pdf` to match
the `BEGINNER_PROJECTS` table in `services/academy/src/html.rs`. Adding a project means
dropping the PDF there and adding its row — nowhere else. Loose copies at the repo root
are duplicates; delete them rather than committing them.
