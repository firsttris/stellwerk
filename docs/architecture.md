# Architecture

## Parts

- **Zentrale** (`crates/zentrale`): one Rust binary with login, SQLite, the desired state of
  every device with a revision number, the connections to all agents, and the web UI.
- **Hub Syncthing** on the server: steered by the Zentrale with the same code as the devices,
  member of every folder, keeps versions.
- **Agent** (`crates/agent`) on every device: starts its own Syncthing, brings its configuration
  to the desired state, reports status.
- **`protocol`**: the messages and the desired state. Describes devices, folders, modes and
  paths only.
- **`engine`**: the only crate that knows Syncthing.

The Zentrale never sends commands like "create folder X", always "this is how it should look,
revision 42". Applying is idempotent, so a device that was offline simply gets the current state.

## Protocol

The agent opens a WebSocket to the Zentrale (`/api/agent/ws`); messages are JSON with a `type`
field (`crates/protocol/src/messages.rs`).

| Direction | Message | Content |
|---|---|---|
| Zentrale → agent | `challenge` | a fresh nonce |
| agent → Zentrale | `hello` | public key, signature over the nonce, on first contact the one-time code; host name, OS, agent and Syncthing version, Syncthing device ID, last applied revision |
| Zentrale → agent | `welcome` | the device's ID at the Zentrale |
| Zentrale → agent | `desiredState` | revision, peers (device IDs and addresses), folders with ID, label, mode, path, versioning, ignore patterns, peers |
| agent → Zentrale | `applied` | revision, errors per folder |
| Zentrale → agent | `rejected` | why; the socket closes |

The desired state is always complete, never a diff: an agent that was offline for three days just
gets the current state when it connects.

## Pairing

1. "Pair device" in the UI: the Zentrale creates a one-time code (valid for 15 minutes) and
   stores only its hash.
2. On the device: `stellwerk-agent join --server https://… <code>` creates an Ed25519 key and
   stores server, key and code; `stellwerk-agent run` starts the agent.
3. The agent answers the Zentrale's challenge with a hello signed by its key and carrying the
   code. The Zentrale uses the code up and creates the device; the agent drops the code.
4. From then on the agent signs in with its key only. A blocked device is turned away.

The hub Syncthing listens on its own port (22001 by default, `--hub-listen`), so a Syncthing
already running on the server keeps 22000. Devices dial the hub at `--hub-address`; the hub
never needs to reach the devices.

## Zentrale

### Data model

SQLite with migrations embedded in the binary (`crates/zentrale/migrations`).

| Table | Columns |
|---|---|
| `devices` | id, name, kind, os, public_key, syncthing_id, agent_version, syncthing_version, approved, created_at, last_seen, applied_rev |
| `folders` | id, label, syncthing_folder_id, ignore_patterns, versioning, encrypted_on_hub, created_at |
| `shares` | folder_id, device_id, mode, path (no row = "off") |
| `enrollments` | code_hash, expires_at, used_at, device_id |
| `config_rev` | one revision number, raised in the same transaction as every change |
| `events` | at, device, folder, kind, text (activity feed) |
| `admin`, `sessions` | login |

### Login and API

One admin with a password (argon2id), set with `stellwerk set-password` on the server. A login
sets an HttpOnly, SameSite=Strict session cookie (Secure behind HTTPS) and returns a CSRF token
the UI sends back in `X-CSRF-Token` on every change. Only hashes of session tokens and pairing
codes are stored.

| Endpoint | Does |
|---|---|
| `GET /api/session`, `POST /api/login`, `POST /api/logout` | login state, log in, log out |
| `GET /api/overview` | revision, hub, devices (with online state), folders, shares |
| `POST /api/folders`, `PATCH`/`DELETE /api/folders/{id}` | create, rename, remove a folder |
| `POST /api/shares` | apply a matrix draft as one revision |
| `PATCH`/`DELETE /api/devices/{id}` | rename, block, remove a device |
| `POST /api/enrollments`, `POST /api/enrollments/status` | new pairing code, has a device used it yet |
| `GET /api/events` | activity feed |

Every change raises the revision and sends connected agents and the hub their new desired state.

### Desired states

`crates/zentrale/src/desired.rs` turns the stored configuration into one desired state per
device and one for the hub, as pure functions. The topology is a star:

- a device has the hub as its only peer and shares each of its folders with the hub;
- the hub has every approved device as a peer and holds every folder under
  `<data>/folders/<syncthing folder id>`, in mode "both", with staggered versioning (one year by
  default), shared with every device that has the folder;
- a blocked device gets no desired state and disappears from the hub's peers and folders.

## Web UI

`web/`: React with TanStack Router (file-based routes), Query and Table, Tailwind 4 with design
tokens for light and dark, components in shadcn style on Radix, lucide icons. The release build
of the Zentrale embeds `web/dist`; paths outside `/api` fall back to `index.html`.

- **Übersicht**: devices online, folders, revision; the folder × device matrix. Clicks collect in
  a draft ("3 ungespeicherte Änderungen") and "Anwenden" sends it as one revision. The server
  column is fixed: the hub holds every folder. Cells of an offline device that has not applied the
  current revision show "wartet".
- **Geräte**: list with search and filter, rename, block, remove; "Gerät koppeln" creates a code,
  shows the command per OS and waits until the device shows up, then asks for its name and
  folders.
- **Ordner**: mode and local path per device, rename, ignore patterns (one per line, on every
  device and the hub), remove.

The overview is polled every five seconds. `src/api/types.ts` checks every answer with Zod.
`pnpm dev:demo` runs the UI on fixtures (`src/demo/`) without a Zentrale. The Playwright tests in
`e2e/` and the screenshots (`pnpm screenshots`, or **Actions → Update screenshots**) use the same
demo mode, so they need neither Rust nor Syncthing.

## Engine

| Module | Does |
|---|---|
| `pin` | the pinned Syncthing version and the SHA-256 of each platform's release archive |
| `download` | fetches the archive once into a cache directory, checks it, unpacks binary and license |
| `process` | starts Syncthing with its own `--home`, GUI/API on 127.0.0.1 with a random port and a fresh API key; no auto-upgrade, no usage reporting |
| `client` | Syncthing REST API: `/rest/config`, `/rest/config/{devices,folders}/{id}`, `/rest/db/scan`, … |
| `reconcile` | pure function: actual config + desired state → list of API calls |
| `apply` | runs those calls; the only side effects of reconciling |

### Reconciler

1. Read `GET /rest/config` and the own device ID.
2. Compare with the desired state: add missing devices and folders, change differing ones,
   remove those no longer wanted. The own device is never removed.
3. Write the changes with `PUT`/`DELETE` on `/rest/config/devices/{id}` and
   `/rest/config/folders/{id}`, devices before the folders that name them; then the ignore
   patterns with `POST /rest/db/ignores?folder={id}` where they differ from `GET` (empty lines
   and trailing spaces do not count).

A folder with a new path is removed and added again, so Syncthing starts with a fresh index
instead of treating the files at the old path as deleted. The reconciler only changes
configuration; removing a folder leaves its files alone.

| Mode | Syncthing folder type |
|---|---|
| Both | `sendreceive` |
| Send only | `sendonly` |
| Receive only | `receiveonly` |
| Off | folder not present on the device |

## Roadmap

| Phase | Content | Gate |
|---|---|---|
| 0 · Spike ✅ | engine starts Syncthing, reconciler applies a fixed desired state | integration test "file from A arrives on B" is green |
| 1 · MVP ✅ | Zentrale with SQLite, agent over WebSocket, pairing with a one-time code, hub Syncthing, matrix and device list in the UI | 3-node test with an offline device green (`crates/zentrale/tests/phase1_gate.rs`), own devices one week in daily use (open) |
| 2 · Comfort | live status, activity, conflicts on the hub, versions, ignore patterns, German and English | E2E green, conflict and restore in the integration test |
| 3 · Distribution | installers, LAN discovery, self-update with signature, Docker image and Quadlet | v1.0 published through `release.yml` |
