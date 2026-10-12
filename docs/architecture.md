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
| Zentrale → agent | `desiredState` | revision, peers (device IDs and addresses), folders with ID, label, mode, path, versioning, peers |
| agent → Zentrale | `applied` | revision, errors per folder |
| Zentrale → agent | `rejected` | why; the socket closes |

The desired state is always complete, never a diff: an agent that was offline for three days just
gets the current state when it connects.

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
   `/rest/config/folders/{id}`, devices before the folders that name them.

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
| 1 · MVP | Zentrale with SQLite, agent over WebSocket, pairing with a one-time code, hub Syncthing, matrix and device list in the UI | 3-node test with an offline device green, own devices one week in daily use |
| 2 · Comfort | live status, activity, conflicts on the hub, versions, ignore patterns, German and English | E2E green, conflict and restore in the integration test |
| 3 · Distribution | installers, LAN discovery, self-update with signature, Docker image and Quadlet | v1.0 published through `release.yml` |
