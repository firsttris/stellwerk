# Development

How the repository is laid out, how to build and run Stellwerk from source, how it is tested and
how a release is made. The rules for changes are in
[CLAUDE.md](https://github.com/firsttris/stellwerk/blob/main/CLAUDE.md); they apply to people too.

## Repository

| Path | What |
|---|---|
| `crates/protocol` | the messages between agent and Zentrale and the desired state; knows devices, folders, modes and paths, not Syncthing |
| `crates/engine` | everything about Syncthing: the pinned version, download, starting it, REST client, reconciler, monitor, conflict and version files |
| `crates/agent` | `stellwerk-agent`: pairing, the connection to the Zentrale, the background service; `src/bin/background.rs` is the Windows program without a console |
| `crates/zentrale` | `stellwerk`: axum server, SQLite store with migrations, desired-state derivation, hub, live status, pairing, mDNS, the embedded UI |
| `web/` | the web UI: React, TanStack Router and Query, Tailwind, Paraglide; `src/demo/` holds the demo data |
| `e2e/` | Playwright tests and the screenshots, against the UI in demo mode |
| `docker/Dockerfile` | the Zentrale's image |
| `installer/windows/` | the NSIS script of the Windows installer |
| `scripts/` | the service and installer smoke tests CI runs on real systems |
| `docs/` | this documentation (MkDocs Material) |

## Prerequisites

- Rust stable (the workspace needs at least 1.94, edition 2024)
- Node 22 and pnpm (`corepack enable`)
- for the docs: Python and `pip install -r requirements-docs.txt`

## Building

```bash
pnpm install
pnpm --filter web build          # web/dist, embedded into the Zentrale's release build
cargo build --release            # target/release/stellwerk and stellwerk-agent
docker build -f docker/Dockerfile -t tristanteu/stellwerk .   # the image, UI included
```

A release build of the Zentrale built before `web/dist` exists answers every page with a hint to
build the UI first. Debug builds read `web/dist` from disk at run time.

## Running it locally

**The UI alone**, on demo data, without Rust or Syncthing:

```bash
pnpm dev:demo                    # http://localhost:5173
```

**A real Zentrale and agent** on one machine, with ports that do not collide with a Syncthing
you run yourself:

```bash
# terminal 1: the Zentrale
cargo run -p stellwerk-zentrale -- --data-dir /tmp/stw/zentrale set-password <<< 'dev'
cargo run -p stellwerk-zentrale -- --data-dir /tmp/stw/zentrale \
  --listen 127.0.0.1:8080 --hub-listen tcp://127.0.0.1:22101 --lan-pairing false

# terminal 2: the UI with hot reload, /api proxied to :8080
pnpm dev

# terminal 3: an agent, after "Pair device" in the UI
cargo run -p stellwerk-agent -- --data-dir /tmp/stw/agent join --server http://127.0.0.1:8080 <CODE>
cargo run -p stellwerk-agent -- --data-dir /tmp/stw/agent run --name dev-agent
```

Two agents on one machine need two data directories. Never start a test Syncthing on 8384 or
22000, and never touch a Syncthing you did not start.

## Tests

| Command | What runs |
|---|---|
| `cargo test` | unit tests of every crate, and the integration tests in `crates/*/tests` against real Syncthing instances (the pinned version is downloaded once into `.syncthing-cache/`) |
| `pnpm --filter web check` | Biome, `tsc --noEmit`, Vitest, `vite build` |
| `pnpm e2e` | Playwright against the demo build; `PW_CHROMIUM=/path/to/chrome` outside the Playwright image, `PW_PORT` when 4173 is taken |
| `mkdocs build --strict` | the docs, with broken links as errors |

The integration tests are the gates of the roadmap:

| Test | Shows |
|---|---|
| `engine/tests/phase0_gate.rs` | the reconciler sets up two Syncthings and a file arrives |
| `zentrale/tests/pairing.rs` | a device pairs with a code and syncs with the hub |
| `zentrale/tests/lan_pairing.rs` | a device without a code waits, is approved and syncs |
| `zentrale/tests/phase1_gate.rs` | three nodes; a device that was offline catches up on files and configuration |
| `zentrale/tests/phase2_gate.rs` | a conflict is resolved on the hub and a version restored |
| `zentrale/tests/api.rs` | login, CSRF, folders, drafts and pairing codes over HTTP |

Every Syncthing in a test runs with `SpawnOptions::isolated`: random ports on 127.0.0.1, no
discovery, no relays.

## Continuous integration

| Workflow | When | Does |
|---|---|---|
| `ci.yml` | every push to `main` and pull request (not for docs only) | Rust: fmt, clippy and tests on Linux, Windows and macOS; the service smoke test on all three; web checks; E2E in the Playwright image; the Docker image builds |
| `agents.yml` | from `release.yml`, or by hand | the agent for every platform; on Windows also the installer and its silent install/uninstall test; `SHA256SUMS` |
| `docs.yml` | pushes to `main` and pull requests that touch `docs/`, `mkdocs.yml` or `requirements-docs.txt` | builds the docs with `mkdocs build --strict`; on `main` also publishes them to GitHub Pages |
| `screenshots.yml` | by hand: **Actions → Update screenshots** | renders the pictures in `docs/` from the demo data and commits them |
| `bump.yml` | by hand: **Actions → Bump version** | raises the version and tags it, which starts `release.yml` |
| `release.yml` | a tag `vX.Y.Z`, or by hand | CI, agents, the multi-arch image on Docker Hub and GHCR, the GitHub release with the agents |

The reusable parts (bump, Docker release, screenshots) come from
[firsttris/workflows](https://github.com/firsttris/workflows).

## Releasing

1. **Actions → Bump version → patch/minor/major.** It sets the version in `Cargo.toml`,
   `web/package.json` and `e2e/package.json`, commits, and tags `vX.Y.Z`.
2. The tag starts `release.yml`: the full CI, the agents, then the image `tristanteu/stellwerk`
   and `ghcr.io/firsttris/stellwerk` for `linux/amd64` and `linux/arm64` with the tags `x.y.z`,
   `x.y` and `latest`, the Docker Hub description from the README, and the GitHub release with
   the agents, the Windows installer and `SHA256SUMS`.

Run by hand on `main`, `release.yml` publishes the image as `edge` only, without a release: a
dry run of the publishing part.

Secrets: `DOCKER_PAT` (Docker Hub access token for `tristanteu`). GHCR uses the workflow's own
token.

## Changing Syncthing's version

The version and the SHA-256 of every platform's archive are in `crates/engine/src/pin.rs`. To
raise it, put in the new version and checksums from the release's `sha256sum.txt.asc`, and run
`cargo test`: the integration tests must be green with the new version. The Syncthing sources
(`lib/config`, `lib/api`) are the reference for anything the engine relies on.

## Documentation

```bash
pip install -r requirements-docs.txt
mkdocs serve                     # http://127.0.0.1:8000, reloads on change
```

Pictures come from `pnpm screenshots` or **Actions → Update screenshots**; never edit them by
hand. The same command renders `docs/social-preview.png` (1280 × 640) from
`scripts/social-preview/social-preview.html` and the dark screenshot; upload it under
**Settings → General → Social preview** when it changed. Links between pages are relative (`installation.md#…`); links from the README go to the
published site.
