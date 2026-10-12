<div align="center">

<h1>Stellwerk: central web UI to manage Syncthing on all your devices</h1>

**Self-hosted control center for Syncthing: pair devices, share folders and set paths for every PC from one web UI.**<br>
A small agent on each device brings its own Syncthing and follows what the server says; a hub Syncthing on the server keeps a full copy of every folder.<br>
Open source, no cloud, no clicking "accept" on every machine.

[![CI](https://github.com/firsttris/stellwerk/actions/workflows/ci.yml/badge.svg)](https://github.com/firsttris/stellwerk/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-firsttris.github.io%2Fstellwerk-4f46e5?logo=materialformkdocs&logoColor=white)](https://firsttris.github.io/stellwerk/)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/built_with-Rust-b7410e?logo=rust)

[Status](#-status) •
[Why](#-why-stellwerk) •
[Alternatives](#-stellwerk-and-the-alternatives) •
[FAQ](#-faq) •
[Documentation](https://firsttris.github.io/stellwerk/)

</div>

## 🚧 Status

Early development, not usable yet. Phase 0 is done: the engine downloads a pinned Syncthing,
starts it on its own ports and brings its configuration to a desired state, tested with two real
Syncthing instances. Next is phase 1: Zentrale, agent, pairing and the folder × device matrix.
The [roadmap](https://firsttris.github.io/stellwerk/architecture.html#roadmap) has the details.

## 💡 Why Stellwerk?

Syncthing has no central management: every device is set up in its own web UI, and every new
share has to be accepted on the other side. Resilio Sync offers a management console, but only
commercially and closed source.

- **Configure once, on the server**: pair a device, pick folders, modes and paths; both sides
  are set up automatically.
- **No standstill when a PC is off**: the server is a member of every folder and holds a full
  copy, so changes reach a device as soon as it is back.
- **A clear overview**: folders × devices as one matrix, sync status at a glance.
- **Syncthing does the syncing**: proven, encrypted transfer; Stellwerk only steers it.

Server: Linux with Docker or Podman. Devices: Linux, Windows and macOS.

## 🔀 Stellwerk and the alternatives

Stellwerk is made for one admin who wants all their own devices synced through their own server.

| | What it is | Where Stellwerk differs |
|---|---|---|
| [Syncthing](https://syncthing.net) alone | peer-to-peer sync, configured on each device | one place to configure all devices, a server that is always in every folder |
| [Resilio Sync](https://www.resilio.com) with Management Console | sync with central management | open source and free; no commercial license needed |
| [Seafile](https://www.seafile.com) | self-hosted file sync and share server with its own clients | uses Syncthing; plain folders on every device, no own storage format |
| [Nextcloud](https://nextcloud.com) | self-hosted cloud suite with file sync | does only sync, light on the server, direct LAN transfer through Syncthing |
| [SyncWings](https://forum.syncthing.net/t/syncwings-web-ui-for-syncthing-filestash-single-or-multi-user-easy-file-management-looking-for-testers/25202) | web file manager on top of Syncthing instances | manages the Syncthing on every device, not only on the server |

## ❓ FAQ

### Is there a central management UI for Syncthing?
That is what Stellwerk is for. It is not released yet; see [Status](#-status).

### Does it change my existing Syncthing?
No. The agent brings its own Syncthing with its own configuration and ports and leaves a
Syncthing you already run alone.

## 📚 Documentation

The documentation is at **[firsttris.github.io/stellwerk](https://firsttris.github.io/stellwerk/)**.

| | |
|---|---|
| [Architecture](https://firsttris.github.io/stellwerk/architecture.html) | Zentrale, hub, agent and engine; reconciler; roadmap |

## 🛠️ Development

Requires Rust (stable), Node 22 and pnpm.

```bash
git clone https://github.com/firsttris/stellwerk && cd stellwerk
pnpm install && pnpm dev:demo   # the UI on demo data, no server needed
cargo test                      # downloads the pinned Syncthing once and runs real instances
```

Rust workspace: `protocol` (messages, desired state), `engine` (Syncthing control and
reconciler), `agent`, `zentrale` (axum, SQLite); React UI in `web/`, embedded into the Zentrale.

## 🤝 Contributing

Issues and pull requests are welcome. A good bug report names the OS, the Stellwerk and Syncthing
versions and what the Zentrale showed. Please run `cargo fmt --all --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo test` before opening a pull request.

---

<div align="center">

⭐ Like Stellwerk? A [star on GitHub](https://github.com/firsttris/stellwerk) helps others find it.<br>
🐛 [Report a bug](https://github.com/firsttris/stellwerk/issues/new) · 💡 [Request a feature](https://github.com/firsttris/stellwerk/issues/new)

<sub>License: <a href="LICENSE">AGPL-3.0</a> · © Tristan Teufel and contributors<br>
Changed versions you pass on or run for others must offer their source code under the AGPL; a commercial license without these obligations is available via <a href="https://teufel-it.de">teufel-it.de</a>.<br>
Stellwerk is not affiliated with the Syncthing Foundation or Resilio.</sub>

</div>
