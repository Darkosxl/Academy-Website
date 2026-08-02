# Monopoly worker

The Monopoly evaluator remains a separate GPU deployment. It is not installed on the
CPU-only benchmark EC2 node and is not part of the Rust controller/executor lifecycle.

- `monopoly_ctl.py` starts and stops the GPU worker host according to Academy demand.
- `monopoly_runner.py` owns the existing Monopoly worker protocol and model processes.

Run these scripts from this directory (or pass absolute paths) with the same environment
they used before the monorepo move. Their Academy routes and data model are unchanged.
