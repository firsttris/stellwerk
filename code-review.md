# Code review — Stellwerk v0.1.1

Reviewed at commit `3edcc76` (Release v0.1.1). Every source file of the four crates and of `web/`
was read in full; each bug claim was traced through the code before it was written down. Nothing
in the repository was changed by the review.

Severity:

- **bug** — wrong behaviour that a user or an agent can hit today;
- **risk** — security, robustness or operational problems that need a specific situation;
- **cleanup** — Clean Code (CLAUDE.md): duplication, dead code, placement, language, tests;
- **simplification** — faster or simpler ways to do the same thing.

## Checks

All project checks pass on `main`:

| Check | Result |
|---|---|
| `cargo fmt --all --check` | ok |
| `cargo clippy --all-targets -- -D warnings` | ok |
| `cargo test` (incl. integration tests against the pinned Syncthing) | 85 tests, all green |
| `pnpm --filter web check` (Biome, tsc, Vitest, vite build) | ok |

## Top findings (fix first)

1. A paired device that runs `join` again with a new code is rejected forever — Z1.
2. The Syncthing API key is on the command line, readable by every local user via `ps` — E4.
3. A failed hub reconcile is never retried until the next revision — Z2.
4. No heartbeat on the agent WebSocket: a silently dropped link stays "online" for hours — A3.
5. The web UI never handles 401: after session expiry or a password change it shows empty pages
   and never redirects to login — W8 (+ W9, W12).
6. The hello signature covers only the nonce, not `syncthing_id` or the code — A2 (plain HTTP in
   the LAN is a supported setup).
7. The conflict diff can allocate gigabytes and freeze the tab on a 256 KB file — W1.
8. A conflict of a conflict copy is parsed wrongly and resolves onto a wrong name — E1; in-flight
   `.syncthing.*.tmp` files are listed as conflicts — E2.
9. A pattern with leading whitespace makes every reconcile rewrite `.stignore` — E3.
10. `service install` does not restart a running agent on Linux and Windows, contrary to the
    docs — A1.
11. The conflict view follows symlinks inside a synced folder, so a device user can show the
    admin any readable file on the hub server — Z7/E12.
12. `rust-version = "1.85"` while the code uses let chains (stable since 1.88) — A4.
13. Graceful shutdown is dead code: `stop` aborts the server right after the signal — Z4.
14. Nobody notices when the Syncthing child exits (`--no-restart`) — E8.

Cross-cutting themes:

- **Config re-read and full scans on timers** (Z13–Z15, Z14/E25, E24): the Zentrale re-reads
  SQLite per subscriber per event and walks every hub folder on disk every 30 s; the monitor
  polls F×D requests per second. Publishing `Arc<Config>` over the watch channel and reacting to
  Syncthing events instead of polling removes most of it.
- **Errors that go nowhere** (W4, Z2, E8, E10, A12): several failure paths only log at debug or
  not at all; the UI then shows stale or "fine" state.
- **Duplication across crates** (A26, E16, E21, Z clean code, W29): escape helpers, host name,
  "original of a conflict copy", SystemTime → seconds, share-holder counting.
- **Pure functions without tests** (A8, E22, W30, Z clean code).

---

## Zentrale (`crates/zentrale`)

Verified as correct, not reported: `Admin` on every non-session route; CSRF check on every
non-GET/HEAD route (body extractors run after `Admin`); constant-time CSRF compare; hashed session
tokens and pairing codes; per-connection nonce with a domain-separated signature payload (no
replay); `BEGIN IMMEDIATE` write transactions with the revision bump inside the same transaction
(the docs' claim holds); `ON DELETE CASCADE`/`SET NULL`; every client path goes through
`engine::files::safe_relative`; no `std::sync::Mutex` guard is held across an `await`;
`desired.rs` is pure.

### Bugs

**Z1 — `src/agents.rs:137-146` with `src/store/devices.rs:227-239` — bug.**
An already-paired device that runs `stellwerk-agent join <new code>` again is rejected forever.
`join` keeps the existing key (`crates/agent/src/lib.rs:43-46`); the hello carries the known key
plus a valid code. `identify` calls `store.enroll` first, `insert_device` returns
`Invalid("this key is already enrolled")`, the transaction rolls back (the code stays unused) and
the session sends `Rejected`. The agent only drops its code on `Welcome`
(`crates/agent/src/session.rs:78`), so it repeats the same hello after every pause: a permanent
reject loop, and the pairing dialog never sees the code used.
Fix: in `identify` look the key up first and ignore the code for a known key (or let `enroll`
return the existing device when the key matches and consume the code). Unit test "known key +
fresh code".

**Z2 — `src/hub.rs:19-47` — bug.**
The hub is reconciled only on `revisions.changed()`. When `stellwerk_engine::reconcile` errors
(hub REST briefly unavailable, restart, `PlanError`) or returns `failures`, it is logged and
nothing happens until the next configuration change. A device added right then stays unknown to
the hub and can never connect.
Fix: retry with a backoff inside the loop (`select!` on `revisions.changed()` and a retry timer)
or re-reconcile periodically.

**Z3 — `src/desired.rs:55-62`, `src/store/devices.rs:149-160`, `migrations/0001_init.sql:14` —
bug (unlikely, total impact).**
`devices.syncthing_id` is not UNIQUE and `device_seen` writes whatever the agent reports.
`for_hub` builds `peers` as a `Vec` without dedup, so two approved devices with the same
Syncthing ID make `engine::plan` return `PlanError::DuplicatePeer` and the hub gets no updates at
all until the duplicate disappears. A misconfigured or copied agent (same Syncthing home on two
machines) triggers this.
Fix: dedupe peers in `for_hub` (map keyed by Syncthing ID) and refuse a `device_seen` report whose
ID belongs to another device; unit test in `desired.rs`.

**Z4 — `src/server.rs:113-117` — bug (dead graceful shutdown, leaked tasks).**
`stop` sends the shutdown signal and immediately `server.abort()`s, so `with_graceful_shutdown`
never runs. `axum::serve` spawns one task per connection; aborting the serve future does not abort
those, so agent and UI WebSocket sessions keep running (reading the store, ticking) after `stop()`
returns and after the hub Syncthing was stopped. Tests mask it because agents are stopped first.
Fix: `let _ = self.shutdown.send(()); let _ = tokio::time::timeout(d, self.server).await;` without
the abort, or drop the graceful-shutdown wrapper if abort is intended — either way make it
explicit.

### Risks

**Z5 — `src/api/session.rs:103-121` — no real login rate limit.**
The 500 ms sleep is per request and serialises nothing: thousands of concurrent `POST /api/login`
each spawn an argon2 verification on the blocking pool, which `files::scan`/`resolve` share
(CPU exhaustion of the whole Zentrale).
Fix: a per-IP (or, with one admin, global) token bucket, or a `Semaphore(1)` around verification
with a delay that grows after failures.

**Z6 — `src/api/live.rs:44-70` — a live WebSocket outlives its session.**
`Admin` is checked only at the upgrade; the socket then streams revision, online devices,
conflicts and waiting agents after `POST /api/logout`, session expiry or `stellwerk set-password`
(which deletes all sessions).
Fix: re-check `store.session` every N ticks or on a timer and close when it is gone.

**Z7 — `src/files.rs:165-184` and `crates/engine/src/files.rs:176-212, 235-259` — symlinks are
followed when reading and resolving conflicts.**
`scan_conflicts` uses `DirEntry::metadata` (does not follow links), so a symlink named like a
conflict copy is listed; `read` then uses `std::fs::metadata`/`std::fs::read`, which follow it.
Syncthing syncs symlinks on Unix, so a device user can put
`x.sync-conflict-20261012-103112-ABCDEFG.txt -> /etc/passwd` into a shared folder and the conflict
view shows the hub server's file (up to 256 KB). `Keep::Copy` renames the link over the original.
Fix: `symlink_metadata` in `read`; skip symlinks in `scan_conflicts` and refuse them in
`engine::files::resolve`.

**Z8 — `src/api/routes.rs:284-287` — internal error text reaches the client.**
`FilesError::Failed(message)` becomes the 500 body (paths under `data/folders`, reqwest errors
with the hub API URL). `StoreError` is handled better in `api/error.rs:36-39`.
Fix: log, return "internal error", as `error.rs` does.

**Z9 — `src/app.rs:117-122` and `src/live.rs:117-129` — stale device status.**
`Live.devices` is only inserted into. After a disconnect `view` still reports the last folder
states with the old `at`; after `delete_device` the entry stays until restart (bounded leak).
Fix: remove the entry in `set_offline` and in the delete route, or carry `online` into
`DeviceLiveView`.

**Z10 — `src/api/live.rs:44`, `src/agents.rs:22` — no `Origin` check on WebSocket upgrades
(low).** Cross-site WebSocket hijacking of `/api/ws` is mitigated only by `SameSite=Strict`; a
same-site sibling origin (other subdomain or port on the same host) gets the cookie.
Fix: compare `Origin` with `Host`/`--public-url` for `/api/ws`.

**Z11 — `src/agents.rs:100-101, 109-117` — `Applied.revision` is trusted verbatim (low).**
An agent can report any revision, including a future one, and clear the "waiting" marker.
Fix: ignore values above the current revision.

**Z12 — `src/store/devices.rs:204-206` — `PATCH /api/devices/{id}` with the stored `approved`
value still bumps the revision**, so every agent and the hub reconcile for nothing.
Fix: compare with the old value, as `device_seen` does for the Syncthing ID.

### Simplification / performance

**Z13 — `src/store/auth.rs:51-62` — one SQLite write per authenticated request.**
`session()` runs `DELETE FROM sessions WHERE expires_at <= ?` on every `Admin` extraction, which
contends with the `BEGIN IMMEDIATE` writers.
Fix: `AND expires_at > ?` in the `SELECT`; prune in `login`/`create_session` or on a timer.

**Z14 — `src/files.rs:139-147` — a full directory walk of every hub folder every 30 s.**
Constant IO on large folders. Fix: react to the hub monitor's events (`ItemFinished` /
`RemoteChangeDetected` with `.sync-conflict-` in the path) and rescan only that folder, or at least
skip folders whose `sequence` did not change.

**Z15 — `src/agents.rs:88`, `src/hub.rs:31`, `src/api/live.rs:73` — the configuration is re-read
from SQLite per subscriber per event.**
Every revision makes every agent session plus the hub read all three tables; every UI tick (up to
3/s per browser socket, driven by all agents' status messages) reads them again.
Fix: `App::changed()` reads `config()` once and publishes `Arc<Config>` over the watch channel
(`watch::Sender<Arc<Config>>` instead of `u64`); subscribers derive their desired state from the
snapshot. This also closes the (benign) gap between the revision in the channel and the config
read later.

**Z16 — `src/api/routes.rs:48-56, 67-71` — `app.online_devices()` locks and clones the whole map
once per device** in `overview`. Fix: take the map once, pass `&BTreeMap` into `device_view`.

**Z17 — `src/api/live.rs:53-58` — during the 300 ms throttle sleep the socket is not polled**, so
a close is noticed late. Sleep before the `select!` on the next loop instead. Minor.

### Clean code

- **Duplication:** `pairing.rs:73-78 hex_sha256` = `auth.rs:48-53 token_hash`;
  `live.rs:184-186` re-implements `files.rs:68-73 device_by_short_id`; `files.rs:174-178` (`read`)
  re-implements engine's private `modified_secs`; `agents.rs:235-247 report` builds `NewDevice`
  while `store::test_device` and three test modules build the same struct by hand (one test
  helper in `model`/`store`).
- **Placement:** `impl From<FilesError> for ApiError` lives in `routes.rs:277-290` instead of
  `api/error.rs` next to the `StoreError` impl.
- **Dead:** `migrations/0001_init.sql:29 encrypted_on_hub` is read by nothing;
  `Hello.applied_revision` (`protocol/src/messages.rs`) is sent by the agent and ignored by the
  Zentrale (use it in `admit` to seed `applied_rev` after a reconnect, or drop it);
  `app.rs:40 lan_pairing: AtomicBool` is set once right after `App::new` (`server.rs`) and could be
  a constructor argument.
- **Language:** `files.rs:208` fallback label `"konflikt"` is German and becomes part of file
  names; CLAUDE.md asks for English in code (`"conflict"`).
- **Unbounded tables:** `events` (an online/offline pair per reconnect, `agents.rs:60-74`) and
  `enrollments` (expired and used codes are never deleted, `store/auth.rs:72-79`) grow forever;
  keep N events, delete enrollments past `expires_at`.
- **`web.rs:16`:** `GET /api` (no trailing slash) is not caught by `starts_with("api/")` and
  returns `index.html`; use `path == "api" || path.starts_with("api/")`.
- **`store/folders.rs`** sits at the ~400-line limit mostly through tests; split the tests out
  with the next feature.
- **Missing unit tests for pure functions:** `main.rs default_hub_address` (host:port parsing,
  IPv6 `tcp://[::]:22001`); `agents.rs:234-247 report` (OS → `DeviceKind`); `files.rs:187-199
  content`'s derivation of the original path from a nested copy path; `api/live.rs:72-80 update`'s
  JSON shape (the contract with `web/src/api/types.ts`); `web.rs:29-48 file` cache headers. No
  test covers Z1 (known key + valid code) or Z3 (duplicate Syncthing IDs).

### Not confirmed

- `session.rs:71-78`: behind a TLS proxy that does not set `x-forwarded-proto` the cookie has no
  `Secure` flag. Deployment-dependent, not a code error; worth a line in the docs.
- Enrollment codes are 60 bits with a 15-minute window and a WebSocket round trip per guess; brute
  force is impractical, but there is no rate limit on `/api/agent/ws` hellos either.

---

## Agent and protocol (`crates/agent`, `crates/protocol`, installer, CI)

Verified as correct, not reported: TLS verification by default on both WebSocket
(`rustls-tls-native-roots`) and HTTP (`rustls-platform-verifier`), no dangerous overrides;
Ed25519 via `ed25519-dalek` 3 with `verify_strict` on the server; 32 random bytes for key and
nonce; atomic write + rename of `agent.json`; no realistic panics (`fingerprint`, `expect`s on
`Path::parent`, `receive` handles close/binary/ping frames); `status.changed()` and
`stream.next()` are cancel-safe in the `select!`; a code lost before `Welcome` is covered by the
`device_by_key` fallback; systemd `%` escaping and launchd/task XML escaping are correct and
tested; the installer's silent mode, error logging and HKCU uninstall entry match
`installer-smoke.sh`.

### Bugs

**A1 — `src/service/linux.rs:17-18`, `src/service/windows.rs:38-51` — bug.**
`service install` does not restart a running agent, although `docs/installation.md:314-316`
promises it. Linux rewrites the unit, but `systemctl --user enable --now` is a no-op for an active
unit; Windows `/Create /F` replaces the task, but `/Run` is ignored while the old instance runs
(`MultipleInstancesPolicy = IgnoreNew`, `definitions.rs:97`). Only macOS (bootout, then bootstrap)
restarts. A user who replaces the binary and runs `service install` stays on the old version; the
CI smoke test never installs twice.
Fix: Linux `daemon-reload` + `enable` + `restart`; Windows `request_stop` and wait for
`agent_running()` to clear before `/Create` + `/Run` (the NSIS installer already does this by
calling `service uninstall` first).

**A8 — `src/run.rs:98-102` — bug (minor, contradicts docs).**
On a connection error `pause` is doubled before it is used, so the first retry waits 2 s, not the
documented 1 s (`docs/architecture.md:243-244`); the sequence is 2, 4, 8, 16, 30. `End::Rejected`
does not reset `pause`, so a rejection followed by a network error resumes an old, long pause.
Fix: `let wait = pause; pause = (pause * 2).min(MAX_PAUSE); wait`; reset on `Rejected`; extract a
pure `next_pause` with a unit test (there is none).

### Risks

**A2 — `src/session.rs:60-61` with `crates/zentrale/src/pairing.rs:54-71` — security.**
The signature covers only `stellwerk-agent-auth:<nonce>`, not the hello body. `name`, `os`,
`syncthing_id` and `enrollment_code` are unauthenticated; the Zentrale stores `syncthing_id` from
the hello and the hub then shares folders with that Syncthing ID. Over `wss://` TLS protects the
body, but plain `http://` is a supported LAN setup (`docs/security.md:41`, installer,
getting-started). An on-path attacker on plain HTTP can relay the valid signature with their own
`syncthing_id` and read the one-time code.
Fix: sign over nonce + a canonical encoding (or SHA-256) of the hello fields; verify the same on
the Zentrale; keep the domain prefix.

**A3 — `src/session.rs:72-105` and `crates/zentrale/src/agents.rs:79-107` — no heartbeat.**
Neither end sends ping frames or applies a read timeout; the agent writes only on a status
change, the Zentrale only on a new revision. After a silent link loss (suspend/resume, NAT
timeout, Wi-Fi roam) the agent sits in `receive()` until the OS TCP keepalive fires (default
≈2 h) while the Zentrale shows the device online and never delivers the next desired state.
Fix: the agent sends `Message::Ping` every ~30 s inside the select and treats no frame within
~90 s as an error (backoff); axum answers pings automatically.

**A4 — `Cargo.toml:9` — build.** `rust-version = "1.85"` is wrong: the workspace uses let chains
(`crates/engine/src/monitor.rs:171-172`), stable since 1.88. A 1.85–1.87 toolchain the manifest
claims to support fails with a syntax error. Fix: `rust-version = "1.88"`.

**A5 — `src/lib.rs:43-46` — identity loss.** `join` generates a *new* key whenever
`config::load` fails for any reason — a corrupt, truncated or unreadable `agent.json` included.
The device silently loses its identity: with a code it enrolls as a duplicate device, without one
it appears as a new waiting device, and the old pairing cannot be resumed.
Fix: generate a key only on `ErrorKind::NotFound`; otherwise return the error.

**A6 — `src/service/mod.rs:70-79` with `windows.rs:57-58` — 30 s for nothing.** `request_stop`
always waits the full `STOP_TIMEOUT` when no agent is running, because nothing removes the stop
file. `service uninstall` with a stopped agent, the NSIS uninstaller (`stellwerk-agent.nsi:180`)
and every upgrade install (`.nsi:150-153` calls `service uninstall` when an exe exists) block for
30 s. Fix: check `agent_running()` before `request_stop`, or return as soon as `!agent_running()`.

**A7 — `src/config.rs:42-44` — key exposure window.** `std::fs::write(&tmp, …)` creates
`agent.json.tmp` with the umask default (usually 0644) and only then `restrict()`s it to 0600; the
secret key is briefly world-readable, and a crash in between leaves a 0644 copy.
Fix: on unix open the temp file with `OpenOptions::new().write(true).create_new(true).mode(0o600)`
and drop the separate chmod.

**A9 — `src/main.rs:100-102` vs `:92-96`.** `install` absolutises `--data-dir`, `uninstall` does
not. With a relative `--data-dir` from another directory, Windows `uninstall` writes the stop file
into the wrong directory, waits 30 s, then `/End`s the task, which kills the agent without a
signal and orphans its Syncthing — the very case the stop file exists for.
Fix: `std::path::absolute` in the uninstall branch too (or in `service::uninstall`).

**A10 — `.github/workflows/ci.yml:6,9` — CI.** `paths-ignore: ["**.md", "docs/**"]` means a
docs-only PR runs no checks, and `mkdocs build --strict` (listed in CLAUDE.md) does not run in CI
at all; broken docs surface only in `docs.yml` after the merge. Fix: a small `docs` job in
`ci.yml` for docs paths.

**A15 — `src/main.rs:38-41, 83-91`, `src/service/definitions.rs:23,47-50,109` — `--name` only
works for `run`.** `service install` writes definitions without it and `background.rs` hard-codes
`name: None`, so a service-installed agent always reports the host name; `docs/reference.md:52`
documents the flag as if usable. Store the name in `agent.json` (set by `join --name`) or drop the
flag.

**A17 — `src/bin/background.rs:27-30`, `definitions.rs:37,56-59` — `agent.log` grows forever**
(Windows and macOS): every reconnect warns, so an unreachable Zentrale grows it continuously.
Fix: `tracing_appender` rolling file or truncate at start.

**A21 — `.github/workflows/agents.yml:35` — reproducibility.** `pip install ziglang` and
`cargo install --locked cargo-zigbuild` are unpinned; a new release can break the musl build
without any change in the repo. Pin both.

**A22 — `src/discover.rs:33,52-56` — arbitrary address.** `service.addresses` is a `HashSet`, so
"first usable IPv4" is arbitrary when the Zentrale host has several (LAN address vs. a Docker
bridge 172.17.0.1); `join` may store an unreachable URL and the user learns it only from connect
failures. Fix: sort for determinism and print all candidates; better, have the Zentrale always set
the `url` TXT record.

**A18 — `src/service/mod.rs:55-66` — uncertain, filesystem dependent.** `modified >= since`
relies on mtime resolution; on FAT/exFAT (2 s) a stop request within ~2 s of start can be ignored.
Irrelevant for `%LOCALAPPDATA%` on NTFS.

### Protocol

**A11 — `crates/protocol/src/messages.rs:76-78` — dead field.** `Hello.applied_revision` is sent
(`session.rs:68`) and never read (`agents.rs::report` ignores it; `serve` always sends the state
on connect). Use it (skip the initial send when equal, seed `applied_rev`) or remove it and the
`last_applied` plumbing.

**A12 — `src/session.rs:86-88`, `crates/zentrale/src/agents.rs:109-117` — semantics.**
`Applied.revision` is recorded as `applied_rev` even when `errors` is non-empty; a device whose
folder failed (bad path) is shown as having applied the revision (`web/src/lib/matrix.ts:98`
uses `appliedRev` for "waiting"), the failure only appears as an event. Decide: record only on
success, or carry both.

**A13 — `src/session.rs:85-89` — confirmed fine, with a cost.** `apply` is awaited inline, so a
second `desiredState` during an apply is queued and applied afterwards (no concurrent apply).
Status messages (and pings, once added) stall for the duration, and N queued states mean N full
reconciles. Simplification: after an apply, drain already-received `DesiredState` messages and
apply only the last.

**A14 — `crates/protocol/src/lib.rs:42-49` — mixed wire conventions.** `Versioning` uses
`rename_all = "camelCase"` for the tag but its field is `max_age_days`; `Peer`/`Folder`/
`DesiredState` are snake_case, `FolderMode` camelCase. Harmless while the only consumer shares the
crate (and `types.ts` copies it); settle one convention before an external engine or UI consumes
it.

### Service, installer, scripts

- **A16 — `docs/reference.md:81` vs `windows.rs:49`:** the table says `task.xml` stays in the
  data directory; `install` deletes it right after `schtasks /Create`.
- **A19 — `installer/windows/stellwerk-agent.nsi:67-104`:** `ParameterValue` cuts at the next
  space and has no quote handling (`/SERVER=` cannot carry spaces); `StrCmp $R4 $R1` is a
  substring match. Not a practical failure; document the limit.
- **A20 — `scripts/service-smoke.sh:29`:** `pgrep -f "$data/syncthing"` also matches the binary
  path `$data/syncthing-bin/…`; works by accident. `pgrep -f -- "--home $data/syncthing"` is
  precise.
- **A23 — `src/discover.rs:36-37`:** dedup by instance name "Stellwerk on <host>"; two Zentralen on
  hosts with the same hostname collapse into one entry. Key by URL.

### Clean code

- **A24 — `src/service/mod.rs:83`:** `#[cfg(any(linux, macos, windows))]` on `command` is
  pointless; `install`/`uninstall` reference `platform`, which does not exist elsewhere, so the
  crate does not compile there anyway. Gate the module consistently or drop the cfg.
- **A25 — `src/service/mod.rs:70-80`:** `request_stop` is used only by `windows.rs`; on Unix it is
  compiled but unused outside tests. `#[cfg(any(windows, test))]` or move it into `windows.rs`.
- **A26 — duplication:** `definitions.rs:117-122 xml_escape` ≈ `crates/engine/src/process.rs:
  208-212`; `run.rs:116-120 host_name` = `crates/zentrale/src/discovery.rs:10-12`;
  `main.rs:58-62` and `bin/background.rs:31-37` duplicate the subscriber setup. A `fn log_filter()`
  in `lib.rs` and one shared escape helper remove them.
- **A27 — `src/session.rs:26`:** `Report.syncthing_id: String` is converted to `DeviceId` on every
  hello; store it as `DeviceId`.
- **A28 — `src/run.rs:55`:** for `Network::Lan` a `free_port()` is computed on every start but only
  the first start uses it (`process::start` keeps an existing `config.xml`); the comment is true,
  the code suggests otherwise. `tcp://0.0.0.0` is IPv4-only in Syncthing (`tcp://:port` is
  dual-stack); irrelevant while devices only dial out, worth a comment.
- **A29 — `Cargo.toml:44`:** `tokio-tungstenite = "0.30"` while axum 0.8.9 pulls 0.29, so the
  lockfile carries two tungstenite stacks; 0.29 dedupes until axum moves.
- **A30 — `src/lib.rs:51-53`:** `join` trims only trailing slashes of `server` and trims `code`;
  a leading space in `--server` fails later in `ws_url` with a confusing error, and an empty code
  is stored as `Some("")`, which the Zentrale treats as a bad code instead of waiting for
  approval. Trim the server; map an empty code to `None`.
- **Missing tests for pure logic:** the backoff policy (A8); `config::save` leaves `agent.json`
  at 0600 (would also pin A7); `windows::user()` / `definitions::task_xml` UTF-16 BOM (`FF FE`)
  prefix; `session::apply`'s `Failure → ApplyError` mapping (split into a pure
  `fn errors_of(&Outcome) -> Vec<ApplyError>`).

---

## Engine (`crates/engine`)

Syncthing v2.1.6 (the tag pinned in `src/pin.rs`) was cloned into the scratchpad and consulted
where a finding depends on it. Verified as correct, not reported: `PUT /rest/config/{folders,
devices}/{id}` starts from `DefaultFolder()`/`DefaultDevice()` and unmarshals on top
(`lib/api/confighandler.go:143,187`), so omitted fields get defaults as the module doc says;
`DELETE` removes the device and `prepare` drops it from folders; the shapes of `/rest/db/ignores`,
`/rest/folder/versions`, `/rest/events` (`since`/`events`/`timeout`, per-mask buffering) and
`/rest/db/status`; all ten event names in `EVENTS` exist; the XML element names in `seed_config`;
every CLI flag used; `/rest/system/shutdown` flushes its response before exiting, so `stop()`'s
check is sound. `DeviceConfiguration.prepare` turns `[]` into `["dynamic"]` and
`FolderConfiguration.prepare` sorts devices and adds the own ID, matching `device_config`/
`folder_config`/`same_folder`; it does not rewrite `path`, so the "moved" detection does not
loop. `plan` is pure and deterministic; `simulate` in `reconcile_tests.rs` mirrors Syncthing's
`SetDevice/SetFolder/RemoveFolder/RemoveDevice` faithfully. `safe_relative` rejects `..`,
absolute paths, prefixes and Syncthing internals. `kill_on_drop(true)` plus `Instance::stop`
leave no orphan Syncthing. `free_port` never returns 8384/22000, only 127.0.0.1.

### Bugs

**E1 — `src/files.rs:47` — bug.** `parse_conflict_name` splits at the *first* `.sync-conflict-`,
so a conflict of a conflict copy is parsed wrongly. Syncthing names it
`a.sync-conflict-20260101-000000-AAAAAAA.sync-conflict-20260102-000000-BBBBBBB.txt`
(`conflictName` in `lib/model/folder_sendrecv.go`: `name[:len-len(ext)] + mark + lastModBy +
ext`). With `find` the original becomes `a.sync-conflict-20260102-000000-BBBBBBB.txt` (does not
exist) with the time and device of the first mark; `scan_conflicts` reports
`original_size: None`, `Keep::Copy` renames onto a non-existent name, `Keep::Original` archives
with the wrong metadata. Fix: `name.rfind(CONFLICT_MARK)`; test with a double conflict.

**E2 — `src/files.rs:176-212` — bug.** `scan_conflicts` reports Syncthing's in-flight temporary
files as conflicts. While the hub pulls a copy it exists as
`.syncthing.config.sync-conflict-20261012-103112-ABCDEFG.ts.tmp` (`lib/fs/tempname.go`:
`.syncthing.` / `~syncthing~` prefix, `.tmp` suffix). The Zentrale writes a "conflict" event for
it, the UI shows it, and resolving races with Syncthing's rename. Fix: skip temp names as
`fs.IsTemporary` does; test.

**E3 — `src/reconcile.rs:215-221` — bug (reconciles forever).** `ignore_lines` trims only trailing
whitespace; Syncthing trims both sides (`lib/ignore/ignore.go parseIgnoreFile`:
`strings.TrimSpace`) and `GET /rest/db/ignores` returns trimmed lines. A desired pattern
`"  *.log"` never equals the read-back `"*.log"`, so every reconcile emits `SetIgnores`
(rewrites `.stignore`, triggers a rescan). Fix: `line.trim()`; extend the ignore test.

**E4 — `src/process.rs:110` — security risk (verified).** The API key is passed as
`--gui-apikey=…` on the command line, visible to every local user via `ps` /
`/proc/<pid>/cmdline`. Syncthing accepts `STGUIAPIKEY` (and `STGUIADDRESS`) from the environment
(`cmd/syncthing/main.go:160`). Anyone on the host can drive the agent's or hub's Syncthing. Fix:
`.env("STGUIAPIKEY", &api_key)` instead of the arg. `Client` and `Instance` also derive `Debug`
including `api_key` (`client.rs:14-19`, `process.rs:51-57`); no current log prints them, but a
`{:?}` would — implement `Debug` manually or wrap the key.

**E5 — `src/reconcile.rs:113-131` — bug (low).** A "fresh" folder is assumed to have no ignores,
but a pre-existing directory may already hold a `.stignore` (a re-added folder, a hub dir left
from a removed folder). `have` is forced to `[]`; if `want` is also empty no `SetIgnores` is
planned and the stale patterns stay in force until the next revision (then `actual.ignores` shows
the diff and it self-heals). Files are silently not synced in between. Fix: for fresh folders
always emit `SetIgnores { lines: want }` (an empty list removes the file), or re-read ignores
after `PutFolder` in `apply`. The comment "an empty list on a new folder is what Syncthing has
anyway" is not generally true.

**E6 — `src/download.rs:40-45` — race (verified).** The binary is renamed into place before it is
made executable. Two processes calling `ensure_syncthing` on the same empty `.syncthing-cache/`
(e.g. `cargo test` running `engine/tests` and `zentrale/tests` concurrently) can see
`binary.is_file()` in that window and spawn it → `EACCES`. Fix: set 0o755 on the temp file before
the rename. Both also download the ~30 MB archive twice; a lock file would avoid that (waste,
not failure).

**E7 — `src/process.rs:120-125` — latent bug.** `Instance::listen_address()` returns the address
from `SpawnOptions`, not the one Syncthing listens on when `config.xml` already existed (`start`
only seeds `listenAddress` when the file is absent). Today the only caller is `phase0_gate.rs`
with fresh homes, so nothing breaks yet. Fix: read it back from `GET /rest/config`, or document
the field as "the seeded address" and drop the getter.

### Risks

**E8 — `src/process.rs` (whole), `crates/agent/src/run.rs:60`, `crates/zentrale/src/server.rs:69`
— nobody notices when the Syncthing child exits.** `--no-restart` makes Syncthing exit instead of
restarting after an API restart or crash. Agent and Zentrale keep running with a dead child:
`monitor::run` logs at *debug* every 5 s, every reconcile fails, the UI shows stale state.
`Instance` offers no `wait()`. Fix: expose `Instance::wait()` (or a `select!` arm on
`child.wait()`) and let callers restart or exit loudly; at least `warn` on repeated monitor
failures.

**E9 — `src/files.rs:45-64, 222-224` — local time vs UTC.** Conflict times are parsed as UTC, but
Syncthing writes them in local time (`time.Now().Format` in `conflictName`), and `archive` writes
a UTC tag that Syncthing reads with `time.ParseInLocation(…, time.Local)`
(`lib/versioner/util.go:118`). Outside the Docker image (TZ ≠ UTC) `Conflict::at` and listed
version times are off by the zone offset, and the staggered cleaner ages versions by it.
`docs/architecture.md` mentions it for versions only. Fix: document for conflicts too, or convert
with the local offset, or at least name the field `at_local_as_utc`.

**E10 — `src/monitor.rs:115-141` — one failing `GET /rest/db/status` aborts the whole snapshot.**
It answers 404 for a folder not in the model (`lib/api/api.go:793-801`), e.g. one the hub
reconciler deleted between `config()` and `folder_summary()`; nothing is published until the next
event or the 30 s idle refresh. Every revision that removes a folder can hit it. Fix: tolerate
per-folder failures (skip with `debug!`) or re-read config on 404.

**E11 — `src/client.rs:47` — `reqwest::Client::new()` has no timeouts.** Every call except the
intentional long-poll can hang forever if Syncthing stalls; `monitor::run` and `apply` then block,
and `wait_until_ready` misses its 60 s deadline because a single `ping` never returns. Fix:
`ClientBuilder::timeout(…)` and a per-request `.timeout(timeout_secs + margin)` for `events`.

**E12 — `src/files.rs:150-166` — `safe_relative` does not consider symlinks.** Same issue as Z7,
seen from the engine side: refuse symlinks (`symlink_metadata`) in `resolve` and in the content
reader.

**E13 — `src/process.rs:215-223, 98` — TOCTOU on `free_port` (minor).** Another process can take
the GUI port between `free_port()` and the spawn; Syncthing fails to bind and `start` times out
after 60 s with an unhelpful log tail. Rare; worth a comment and a clearer startup error.

**E14 — `src/files.rs:222-231` — `archive` silently overwrites a version with the same second
tag** (`rename` replaces on Unix). Two resolves of the same path within one second lose the first
archived side. Fix: bump the tag or add a counter when the target exists.

### Clean code

- **E15 — `src/reconcile.rs:53-144`:** `plan` is ~92 lines (> ~80). Split into `plan_devices`,
  `plan_folders` (incl. `moved`), `plan_ignores`; `plan` validates and chains.
- **E16 — `src/files.rs`:** 415 lines (> ~400 incl. tests); the "original of a copy" logic is
  written three times (`files::resolve:236-240`, `zentrale/src/files.rs:190-193` and `:205-207`:
  `rsplit('/')` + `parse_conflict_name` + `strip_suffix`). Add one pure
  `pub fn conflict_original(copy: &str) -> Option<(String, i64, String)>` in the engine; move the
  calendar helpers (`parse_tag`, `format_tag`, `days_from_civil`, `civil_from_days`) into
  `files/tag.rs`. The `rfind('/')` dir/file split appears twice (`tag_filename`, `free_name`).
- **E17 — `src/error.rs:40-41` + `files.rs:162,238,243`:** `Error::Archive` ("archive error") is
  reused for "not a path inside the folder" and "not a conflict copy". The Zentrale maps
  `Archive` → HTTP 400, so a real archive error is reported as a client error, and the name
  misleads. Add `Error::InvalidPath` / `Error::NotAConflict`.
- **E18 — `src/files.rs:131-147` (uncertain whether intended):** `tag_filename` claims
  Syncthing's naming, but for dot-files they differ: Go's `filepath.Ext(".bashrc")` is `.bashrc`,
  so `TagFilename` gives `~20261012-103112.bashrc`; the engine produces `.bashrc~20261012-103112`.
  Syncthing still recognises it through its "old style tag" branch (`util.go:69-72`), so it
  works; either match Go or say in the comment that the old form is used on purpose.
- **E19 — `src/client.rs:71-74`:** `config_raw` is used only by `tests/phase0_gate.rs`; mark it as
  test support or let the test call `/rest/config` itself.
- **E20 — `src/client.rs:253-261`:** `method_name` hand-maps `Method` to `&'static str`;
  `Error::Api::method` as `String` from `method.as_str()` removes it. `FileVersion` mixes
  `use serde::Deserialize` with the `serde::Serialize` path; `Ignores::ignore` has a redundant
  `#[serde(default)]` on an `Option`.
- **E21 — `src/files.rs:168-173`, `zentrale/src/files.rs:171-175`, `files.rs:288-292`:** the same
  "SystemTime → i64 seconds" conversion three times. Make `modified_secs` public (or add
  `pub fn secs(SystemTime) -> i64`) and use it from the Zentrale and from `now()`.
- **E22 — `src/reconcile.rs:27-36`:** `ApiCall::folder()` is public, pure, relied on by the agent
  for per-folder error reporting (`agent/src/session.rs:115`), and untested.
- **E23 — `src/process.rs:101`:** `File::create` truncates `stellwerk-syncthing.log` on every
  start, so the log of a crash that caused a restart is gone. Open with `append(true)`.

### Simplification / performance

- **E24 — `src/monitor.rs:115-141`:** `snapshot` re-reads `/rest/config` and (on the hub)
  `/rest/system/status` on every refresh and makes F + F×D sequential requests up to once per
  second. `my_id` never changes: fetch once in `run`. The config only changes on `ConfigSaved`,
  which is already in `EVENTS`: keep the folder list and refresh it on that event. Per-device
  completion can be updated incrementally from `FolderCompletion` events (`folder`, `device`,
  `completion`, `needBytes`, `needItems`), removing the F×D polling; at minimum fire the requests
  concurrently with `join_all`.
- **E25 — `src/files.rs:176-212` + `zentrale/src/files.rs:100-141`:** see Z14; the hub receives
  conflict copies as remote changes, so `RemoteChangeDetected`/`LocalChangeDetected` events with
  `.sync-conflict-` in `path` (plus one full walk at startup) are enough, with the periodic walk
  as a rare fallback. Alternatively `GET /rest/db/browse?folder=…` lists from the index without
  touching the disk.
- **E26 — `src/apply.rs:49-51` / `client.rs:77-88`:** `actual()` fetches ignores per folder
  sequentially (N+2 requests per reconcile); `join_all` cuts latency on hubs with many folders.
- **E27 — `src/download.rs:30-35`:** the SHA-256 of the ~30 MB archive is computed on the async
  runtime thread; move `check_sha256` into the `spawn_blocking` with `extract`.
- **E28 — `src/process.rs:98` and `agent/src/run.rs:55`:** see A28.

---

## Web UI (`web/`)

Verified as correct, not reported: **`src/api/types.ts` mirrors the Rust API exactly** — every
field name, optionality, nullability and enum value matches the serde output (`DeviceView`
flattening, `csrfToken`/`error` skipped when `None` → `.optional()`, `Versioning`
`{type:"staggered", max_age_days}`, `ShareChange.path` `#[serde(default)]`); demo fixtures are in
step with the types; `de.json` and `en.json` have identical key sets and placeholders, every key
is used and none is missing. Draft/diff logic in `lib/matrix.ts`; `byCount` with `_one/_other`
everywhere except W2; `ago/duration/since` boundaries; `isWaiting`; `folderBadge` priority; WS
reconnect/backoff and StrictMode double-effect cleanup; CSRF token capture on `session()`/
`login()` and clearing on logout; one code per open of the pairing dialog (`gcTime: 0`);
`validateSearch` for tabs; `diffLines` output; `bothName` vs the server's `free_name`/`split_ext`;
pnpm/Biome/tsconfig (strict, `noUncheckedIndexedAccess`).

### Bugs

**W1 — `src/components/conflict-card.tsx:103-104` + `src/lib/diff.ts:10-12` — bug (freeze).**
`diffLines` allocates an `(a+1)×(b+1)` matrix of JS numbers and runs in the component body on
every render, un-memoised. The server sends text up to 256 KB; a file with ~13 000 short lines
means 169 M doubles ≈ 1.3 GB — the tab freezes or is killed, and every WebSocket push re-renders
the route. Fix: `useMemo` on `(original?.text, copy?.text)`, cap at e.g. 2 000 lines per side
(show `conflict_no_preview` above), and use two `Int32Array` rows (or Myers) instead of nested
arrays.

**W2 — `src/lib/live.ts:48` + `messages/*.json error_files_failed` — bug (pluralisation).**
No `_one` variant: with `errors === 1` the UI shows "1 Dateien nicht synchronisiert" / "1 files
not synced". Fix: `error_files_failed_one`/`_other` with `byCount`, as everywhere else.

**W3 — `src/lib/live.ts:13-23` — bug (formatting).** The unit is chosen before rounding:
`formatBytes(999_990)` → `"1.000 KB"` (de) / `"1,000 KB"` (en), read as 1000 KB or 1 KB instead
of "1 MB" (verified in node). Fix: loop on `Math.round(value * 10) / 10 >= 1000`; add the case to
`live.test.ts`.

**W4 — `src/routes/_app/devices.tsx:251-303` — bug (errors swallowed).** `update.error` and
`remove.error` are never rendered. A 400 ("a device needs a name"), 403 (CSRF), 404 or network
failure fails silently; the menu just closes. Fix: render the error like every other mutation
does, or a shared `MutationCache.onError` toast.

**W5 — `src/routes/_app/folders/$folderId.tsx:204` — i18n rule.** `<StatusDot online /> Online`
is a hardcoded string; `m.devices_online()` exists. `matrix.tsx:270` hardcodes `{progress} %`
while the messages carry the "· {percent} %" format.

**W6 — `index.html:2` — accessibility.** `<html lang="de">` is static; with locale `en` screen
readers pronounce the English UI with German rules. Fix: `document.documentElement.lang =
getLocale()` in `main.tsx` (Paraglide reloads on `setLocale`, so once at startup is enough).

**W7 — `src/components/pair-dialog.tsx:88-92` — false success on plain HTTP.**
`navigator.clipboard` is `undefined` on insecure origins, which the project supports for the LAN.
`await navigator.clipboard?.writeText(...).catch(...)` resolves `undefined`, then
`setCopied(true)` shows "Kopiert" although nothing was copied. Fix: select the `<code>` text /
`execCommand('copy')` fallback; set `copied` only on success.

**W8 — `src/routes/_app.tsx:7-10` + `src/api/http.ts:34-40` — 401 is never handled.**
`ensureQueryData(sessionQuery)` returns the cached session without revalidating, and nothing else
subscribes to it inside `_app`, so after the first load the session is never checked again. When
the 30-day session expires or the password is changed (which ends all sessions), every query
throws `ApiError(401)`: the overview shows "not logged in", devices/folders/conflicts show nothing
(they ignore `error`), mutations fail, the WebSocket reconnects every ≤30 s forever, and the user
is never sent to `/login`. Fix: on 401 in `request()` → `queryClient.removeQueries(sessionQuery)`
and `router.navigate({to:"/login"})` (or a `QueryCache.onError`), and/or `beforeLoad` with
`fetchQuery` + a 60 s `staleTime`.

**W9 — `src/components/layout.tsx:39-43` — logout can strand the user.** `await api.logout()`
throws on 401/403/network; then neither the cache is cleared nor navigation happens, with no
error display. With W8 an expired session cannot be left via the button. Fix:
`try { await api.logout() } finally { removeQueries; navigate }`.

**W10 — `src/components/pair-dialog.tsx:56-61` — polls forever.** The code expires after 15 min,
but `enrollmentStatus` polls every 2 s indefinitely (`store/auth.rs:82 enrolled_device` ignores
`expires_at`), so the dialog shows "Warte auf das Gerät …" forever with a dead code; `expiresAt`
from the response is never used. Fix: stop at `expiresAt`, show "code expired" with a "new code"
button (reset the `new-enrollment` query).

**W11 — `src/components/matrix.tsx:201`, `src/routes/_app/folders/index.tsx:27`,
`src/routes/_app/folders/$folderId.tsx:65`, `src/routes/_app/devices.tsx:104-106` — count
disagrees with the model.** Blocking a device keeps its shares (`store/devices.rs:189-209` only
flips `approved`; `model.rs:48` says a blocked device is removed from every folder in the desired
state). These four places count `overview.shares` regardless of approval, so "auf 3 Geräten"
includes a blocked device that gets nothing, while `lib/live.ts:31 holders()` (used for badges)
filters `approved`. Fix: export `holders()` and use it everywhere (removes the triplicated
filter).

**W12 — `src/api/http.ts:33` — wrong error surfaced.** `JSON.parse(text)` runs before the status
check; a non-JSON body (reverse-proxy 502/504 HTML, nginx 413, the embedded `web.rs` 404 text for
an unknown `/api/…`) throws `SyntaxError: Unexpected token '<'` instead of `ApiError(status)`, so
status-based handling (W8) is bypassed. Fix: parse inside try/catch, fall back to
`response.statusText`.

### Risks

- **W13 — `src/api/live.ts:31-32`:** `JSON.parse` is unguarded (a bad frame throws in
  `onmessage`), and a `safeParse` failure is silently dropped: a new `SyncState`/`DeviceKind`
  variant or `UiMessage` type on the server silently stops WS updates while `GET /api/status`
  errors. Suggest `syncStateSchema.catch("unknown")`, `deviceKindSchema.catch("desktop")`,
  `console.warn` on parse failure.
- **W14 — `src/api/live.ts:34` vs `src/api/queries.ts:20-24`:** stale-overwrite race —
  `statusQuery` keeps `refetchInterval: 30_000` while the WS writes the same key with
  `setQueryData`; a REST response in flight when a push arrives overwrites newer data with older.
  Disable the interval while the socket is open (expose `connected` from `useLiveUpdates`).
- **W15 — `src/components/layout.tsx:94-101`:** "Server online" is a hard-coded green dot; it stays
  green while the WS is in its reconnect loop or the Zentrale is down. Drive it from the socket
  state (W14).
- **W16 — `src/main.tsx:8-9`:** no `defaultErrorComponent`/`defaultNotFoundComponent`; a failing
  `beforeLoad` or an unknown URL shows TanStack's unstyled English defaults. `new QueryClient()`
  keeps `retry: 3`: a 401/403/404 (`conflictContent` 404, wrong CSRF) is retried 3× (~7 s) before
  the message appears. Suggest `retry: (n, e) => !(e instanceof ApiError) && n < 3`.
- **W17 — `src/routes/_app/folders/$folderId.tsx:119`:** `key={overview.revision}` remounts
  `FolderDevices` on every revision bump, including ones caused elsewhere (another tab, a LAN
  approval), so a half-typed path is discarded without warning. Keep the rows and reset only
  after the component's own save.
- **W18 — `$folderId.tsx:317-320, 346-360, 394`:** rename and ignore-save share one `update`
  mutation; a rename error is rendered under the *ignore patterns* card, and the rename button
  ignores `update.isPending` (double submit). Split into two mutations.
- **W19 — `src/routes/_app/index.tsx:293`:** `key={`${folder.id}-${device}`}` uses the device
  *name*; names are not unique, so two same-named devices with errors in one folder collide. Have
  `folderErrors` return the device id.
- **W20 — `src/routes/_app/devices.tsx:189-195`:** no `getRowId`; rows are keyed by index, so after
  filtering, `DeviceActions` instances (and their pending state/errors) are reused for a different
  device. Add `getRowId: (d) => d.id`.
- **W21 — `src/components/pair-dialog.tsx:23-28` (uncertain which shell is targeted):** the Windows
  command joins with `;`, which is PowerShell-only (cmd.exe does not understand it); `&&` works
  in cmd and PowerShell ≥ 7 but not 5.1. Say so in the dialog or show both.
- **W22 — `src/lib/live.ts:117-131` (uncertain):** `deviceSummary` uses `percent < 100` to decide
  "loading" but `waiting` (needItems) for the count; Syncthing reports `completion < 100` with
  `needItems == 0` when only deletes are pending → "Lädt 0 Änderungen · 99 %". Guard
  `waiting === 0`.
- **W23 — `src/components/matrix.tsx:71`:** the draft lives in component state; any navigation
  discards unsaved cells silently (no `beforeunload`/block). After a concurrent overview change a
  draft cell equal to the stored value is still counted and sent (server no-op). Minor.
- **W24 — `src/demo/api.ts:85`:** `id: folder-${folders.length+1}` collides after deleting a
  non-last folder (duplicate React keys, `findFolder` hits the first). Use a counter.
- **W25 — `src/components/conflict-card.tsx:102-104`:** when the original was deleted but the copy
  is text, no preview is shown at all although the copy's text is available. Show it as
  all-`added`.
- **W26 — accessibility, minor:** `$folderId.tsx:92-117` and `pair-dialog.tsx:97-115` use
  `role="tab"` without `aria-controls`/`tabpanel` and without arrow-key navigation;
  `conflict-card.tsx:48` scrollable `<pre>` lacks `tabIndex={0}`; `devices.tsx:204` `<th>` lacks
  `scope="col"`; `layout.tsx:103-114` labels the select twice; badges are `text-[10px]`.
- **W27 — raw server error strings** everywhere (`login.tsx:62`, `matrix.tsx:138`,
  `conflict-card.tsx:120/148`, …): "wrong password", "not logged in", "missing or wrong CSRF
  token" are English regardless of locale. Map known statuses/messages to keys (at least login).

### Clean code

- **W28 — size:** `src/routes/_app/folders/$folderId.tsx` is 414 lines: move `FolderDevices` +
  `shareStatus` (140-310) to `components/folder-devices.tsx` and `FolderSettings` (312-414) to
  `components/folder-settings.tsx`. `components/matrix.tsx`: `Matrix` is ~225 lines (70-296):
  extract `DraftBar` (109-146), `MatrixHead` (163-198), `MatrixCell` (234-282).
  `routes/_app/devices.tsx`: `DeviceTable` ~140 lines: columns into `useDeviceColumns()`,
  `HubRow` and `DeviceActions` into `components/`. `routes/_app/index.tsx`: `Attention` ~100
  lines; split per item type.
- **W29 — duplication:** share-holder counting in four places (W11); the grid header/cell styling
  `grid-cols-[minmax(160px,1fr)_220px_…]` three times in `$folderId.tsx:191,197,231`;
  `nowSeconds()` in `lib/format.ts:59` and `demo/api.ts:12`; the search inputs in
  `devices.tsx:58-71` and `versions-list.tsx:44-57` → `SearchInput`; the success handler
  `Promise.all([invalidate status, refresh])` in `conflict-card.tsx:93-98`,
  `waiting-devices.tsx:36-40`, `versions-list.tsx:31-34` → `refresh({ status: true })`.
- **W30 — pure functions in component files without tests:** `bothName` (`conflict-card.tsx:15`,
  mirrors `engine free_name`/`split_ext`; they agree, nothing pins it), `pairCommand`
  (`pair-dialog.tsx:23`), `hostOf` (`layout.tsx:24`), `shareStatus` (`$folderId.tsx:287`),
  `formatDate` (`versions-list.tsx:12`). Move to `lib/`, add Vitest. Also untested:
  `completion()`, `describeEvent` for `paired/online/error/conflict/resolved/restored` and the
  default branch, `nextMode`'s guard, the `formatBytes` boundary (W3).
- **W31 — unused dev dependencies:** `@testing-library/react`, `@testing-library/user-event` and
  `afterEach(cleanup)` in `src/test-setup.ts:7` — there is no component test. Add some (`Matrix`
  draft bar, `ConflictCard` preview) or drop them and jsdom.
- **W32 — `src/api/http.ts:71-73`:** `applyShares` is the only call that casts
  (`json as { revision: number }`) instead of parsing; add `revisionSchema`.
- **W33 — `src/routes/_app/index.tsx:56`:** `React.ReactNode` via the global namespace without
  import; import `type ReactNode`. The `Overview` type is aliased to `OverviewData` because the
  component is called `Overview` — rename the component `OverviewPage`.
- **W34 — raw palette classes instead of tokens** (`styles.css:5` says components use semantic
  names only): `text-zinc-700 dark:text-zinc-300` in `layout.tsx:21,103`, `matrix.tsx:47`,
  `pair-dialog.tsx:109`, `devices.tsx:84`; `bg-amber-700`/`text-white` in `layout.tsx:87`.
- **W35 — `$folderId.tsx:222`:** `folderSize(live, folder.id)` twice in one expression.
- **W36 — `src/components/pair-dialog.tsx:163-173`:** `save` issues `updateDevice` then
  `applyShares` sequentially; if the second fails the name is already changed and the error reads
  as if nothing happened. Acceptable, but say so or apply shares first.

### Simplification / performance

- **W37 — `src/routes/_app/devices.tsx`:** `@tanstack/react-table` is used for a four-row table
  with one string filter and no sorting/paging; a plain filter + `<table>` removes the dependency
  and ~80 lines. If it stays: the `columns` `useMemo` deps include `now` (`:187`), which changes
  every second, defeating the memo — compute `now` inside the cell.
- **W38 — `src/components/versions-list.tsx:12-15`:** `new Intl.DateTimeFormat` per call, twice
  per version row; create once per render.
- **W39 — `src/api/types.ts:1`:** full `zod` v4 build; `zod/mini` would cut the bundle's largest
  non-React dependency (only `object/array/record/enum/literal/discriminatedUnion/nullable/
  optional/default` are used). Gain unmeasured.
- **W40 — `src/components/matrix.tsx:81-94`:** `apply.onSuccess` awaits `refresh()` then clears
  the draft, while the WS push for the same revision triggers a second `refresh()`. Harmless
  double fetch per apply; clear first and let the WS reconcile.
- **W41 — `src/lib/format.ts:7-12` vs `pair-dialog.tsx:16-20`:** two OS label tables; derive from
  one list.

### e2e (`e2e/tests`)

- **W42 —** no test asserts nothing, but gaps: logout/login (demo is always logged in — add a
  demo flag or test the real login form at least for "no password set"), rejecting a waiting
  device, removing a device, "Diese Fassung behalten" (only "Beide behalten" is covered), the
  conflicts empty state, the versions empty-search state, the folder "gone" state.
  `folder.spec.ts:13 expect(save).toBeDisabled()` passes while the save is merely pending, so it
  does not prove the save landed — wait for the toast or revision. "versions are listed…" lives in
  `conflicts.spec.ts:21` but is a folder feature.
