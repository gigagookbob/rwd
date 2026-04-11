# Release Notes Guide — rwd

## Goal

`rwd` release notes should feel polished, calm, and useful.
We want the tone of a product changelog, but scaled to a small Rust CLI project.

## Preferred Shape

Each GitHub Release should have two layers.

1. Structured summary
   - One-sentence overview
   - `New Features` section when applicable
   - `Highlights` section
   - Small category sections such as `Bug Fixes`
   - Upgrade/install reminder
2. Auto-generated details
   - Full PR list from GitHub release notes
   - Contributors and compare links handled by GitHub

This keeps the top of the release readable while still preserving a full audit trail below.

## Writing Rules

- Start with what changed for the user, not how it was implemented.
- Use counts when they clarify scope, not to exaggerate impact.
- Keep the custom summary short enough to scan in under a minute.
- Let GitHub's generated section carry the exhaustive list.
- Exclude release-only chores such as version bump PRs.
- If setup, config, or behavior changed, mention it in `Highlights`.

## Category Mapping

| PR label | Release section |
| --- | --- |
| `enhancement` | `New Features` |
| `bug` | `Bug Fixes` |
| `performance` | `Performance` |
| `documentation` | `Documentation` |
| `chore`, `dependencies` | `Maintenance` |
| `skip-release-notes` | Excluded from release notes |

## Example Tone

```md
## Release Notes

This release focuses on session discovery reliability and cleaner daily summaries.

### New Features
- support multi-root Claude/Codex log discovery (#85)

### Highlights
- Compared against `v0.13.1` to keep the summary scoped to this release.
- Install with `cargo install --git https://github.com/gigagookbob/rwd.git` or update with `rwd update`.

### Bug Fixes
- parse GitHub release tag_name correctly in installer (#84)
```

## Maintainer Workflow

- `Cargo.toml` version bump still defines the release tag (`vX.Y.Z`).
- `.github/scripts/generate_release_notes.cjs` writes the structured summary.
- `.github/release.yml` controls the GitHub-generated categories.
- PRs opened from `chore/release-vX.Y.Z` branches get `skip-release-notes` automatically.

## When to Edit Manually

The automated summary is the default.
After publishing, it is still worth editing the release body by hand when:

- a release contains behavior changes that users must react to,
- a release introduces a new command or config key,
- a release fixes a subtle data-loss or correctness bug,
- or the automatic bullets miss the real story of the release.
