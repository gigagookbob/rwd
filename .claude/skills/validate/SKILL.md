---
name: validate
description: Run full Rust code quality pipeline (fmt, clippy, test)
user_invocable: true
---

# Validate

Run the full code quality pipeline for the rwd project. Execute each step sequentially and stop on first failure.

## Steps

1. `cargo fmt --check` — formatting check
2. `cargo clippy` — lint with zero warnings
3. `cargo test` — run all tests

## Instructions

- Run each command sequentially using the Bash tool
- If any step fails, stop and report the failure with the full error output
- If all steps pass, report success concisely
- Do NOT fix issues automatically — report them and let the user decide
