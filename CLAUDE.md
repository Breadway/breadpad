# CLAUDE.md — Repo hygiene

Scope: this file covers *repo hygiene* — branching, remotes, CI, cleanup. It is not project documentation.

This repo follows the branch/release workflow documented in `CONTRIBUTING.md`
— read and follow it for any git, branch, or release work here (the
single-trunk model, `feature/x`/`fix/x` branch naming, how RC tags work,
etc). Don't improvise a different workflow. The short version: there is one
long-lived branch, `main` — no `dev` or `beta` branch exists. `main`
auto-publishes a dev-track build on every push. "Beta" and "stable" are both
just tags, not branches: push a `vX.Y.Z-rc.N` tag to publish a beta-track
build, push a plain `vX.Y.Z` tag to cut the signed stable release.
"Freezing" for stabilization means pausing pushes to `main`, not moving a
branch. This replaced an earlier three-branch (`dev`/`beta`/`main`) model
after `main` was found to have silently rotted out of sync with `dev`/`beta`
across most repos in this ecosystem.

## Remotes
- `origin` — Forgejo (`git.breadway.dev` via Hestia, SSH) — authoritative.
- `github` — GitHub mirror. Push both when publishing.

## CI
- `check.yml` — clippy + test, triggers on push to `feature/**`/`fix/**`.
- `dev-release.yml` — triggers on push to `main`.
- `rc-release.yml` — triggers on `vX.Y.Z-rc.N` tag push.
- `release.yml` — triggers on any other `v*` tag push.

All four run on a self-hosted runner (`hestia`) inside a pinned Arch
container — not the host's native environment. The Containerfile/build
script are shared across bread-ecosystem products and live in
`bread-ecosystem/ci/`; this repo's `ci/build.sh` clones that repo at the
sha in `ci/bread-ecosystem.rev` (deliberately pinned, not `main`) and
delegates to it. Nothing runs automatically on plain commits or PRs
beyond what's listed.

## Don't
- Don't embed credentials in remote URLs — SSH or a credential helper only.
