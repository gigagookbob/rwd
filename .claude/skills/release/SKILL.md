---
name: release
description: Bump version, tag, and push to trigger CI release
user_invocable: true
---

# Release

Bump the version, create a git tag, and push to trigger the CI release pipeline.

## Arguments

The user may provide a version bump type: `patch` (default), `minor`, or `major`.

## Steps

1. **Check preconditions**
   - Current branch must be `dev`
   - Working tree must be clean (`git status --porcelain` is empty)
   - Run `/validate` (cargo fmt + clippy + test) and confirm all pass

2. **Determine new version**
   - Read current version from `Cargo.toml`
   - Bump according to the argument (default: patch)
   - Show the user: `current → new` and ask for confirmation

3. **Apply version bump**
   - Update `version` field in `Cargo.toml`
   - Run `cargo check` to update `Cargo.lock`

4. **Commit and tag**
   - `git add Cargo.toml Cargo.lock`
   - `git commit -m "chore: bump version to X.Y.Z"`
   - `git tag vX.Y.Z`

5. **Push**
   - Ask user for confirmation before pushing
   - `git push origin dev`
   - `git push origin vX.Y.Z`
   - CI (release.yml) will automatically build and create the GitHub Release

## Rules

- Do NOT attach local build artifacts to the release — CI handles this
- Do NOT create the GitHub Release manually — CI handles this via `softprops/action-gh-release`
- If any step fails, stop and report. Do not proceed with partial state.
