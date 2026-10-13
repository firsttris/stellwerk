<div align="center">

<h1>Stellwerk: central web UI to manage Syncthing on all your devices</h1>

**Self-hosted control center for Syncthing: pair devices, share folders and set paths for every PC from one web UI.**<br>
A small agent on each device brings its own Syncthing and follows what the server says; a hub Syncthing on the server keeps a full copy of every folder.<br>
Open source, no cloud, no clicking "accept" on every machine.

[![CI](https://github.com/firsttris/stellwerk/actions/workflows/ci.yml/badge.svg)](https://github.com/firsttris/stellwerk/actions/workflows/ci.yml)
[![Docs](https://img.shields.io/badge/docs-firsttris.github.io%2Fstellwerk-4f46e5?logo=materialformkdocs&logoColor=white)](https://firsttris.github.io/stellwerk/)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/built_with-Rust-b7410e?logo=rust)

```bash
docker compose up -d && docker compose exec -T stellwerk stellwerk set-password <<< 'a long password'
```

[Install](#-install) •
[Why](#-why-stellwerk) •
[Features](#-features) •
[Screenshots](#-screenshots) •
[Alternatives](#-stellwerk-and-the-alternatives) •
[FAQ](#-faq) •
[Documentation](https://firsttris.github.io/stellwerk/)

<img src="docs/screenshot.png" alt="Stellwerk overview: a folder by device matrix showing which device sends, receives or does not hold each Syncthing folder, with a draft of two changes waiting to be applied" width="900">

</div>

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

Server: Linux with Docker or Podman (amd64 and arm64: a NAS, a mini PC, a Raspberry Pi 4/5, a
VPS). Devices: Linux, Windows and macOS.

## 🚀 Install

On the server, a `compose.yaml` (the hub address is where your devices reach the server):

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk:latest
    restart: unless-stopped
    ports: ["8080:8080", "22001:22001"]   # web UI and agents; sync port of the hub
    environment:
      STELLWERK_HUB_ADDRESS: tcp://nas.local:22001
    volumes: ["stellwerk:/data"]
volumes:
  stellwerk:
```

```bash
docker compose up -d
docker compose exec -T stellwerk stellwerk set-password <<< 'a long password'
```

Open `http://nas.local:8080`, click **Pair device** and run the one command it shows in a terminal
on each device, as the normal user. It downloads the agent, pairs it and starts it:

```bash
curl -fsSL https://github.com/firsttris/stellwerk/releases/download/vX.Y.Z/install.sh | sh -s -- http://nas.local:8080 K7Q2-MX9P-41TZ
```

On Windows the dialog shows the same as a PowerShell command; there is also an installer that
asks for the address and the code. Podman Quadlet, a
reverse proxy with HTTPS, backups and updates are in the
[installation guide](https://firsttris.github.io/stellwerk/installation.html); a step-by-step
first run is in [Getting started](https://firsttris.github.io/stellwerk/getting-started.html).

> **Before v1.0**: every [release](https://github.com/firsttris/stellwerk/releases) publishes the
> image `tristanteu/stellwerk` and carries the agents. Everything works and is tested on Linux,
> Windows and macOS; the self-update of the agents is the last piece before v1.0
> ([roadmap](https://firsttris.github.io/stellwerk/architecture.html#roadmap)).

## ✨ Features

### 🗺️ One matrix for everything

- **Folders × devices**: one click per cell switches between both directions, send only, receive
  only and off.
- **Folder picker**: choose the folder on a PC by clicking through its directories in the web
  UI, no path to type.
- **Drafts**: changes collect until you apply them, then go out as one numbered revision.
- **Offline devices catch up**: a device that is off gets the new configuration and the files as
  soon as it is back; the cell says "waiting" until then.

### 🔗 Pairing without clicking on every PC

- **One command per device**: "Pair device" shows a command with a code valid for 15 minutes; it
  downloads the agent, pairs it and starts it, and the device appears in the UI by itself.
- **Your own CA works too**: behind a private CA (e.g. Caddy's `tls internal`) the command carries
  the CA's fingerprint, and the agent trusts it without any setup on the device.
- **In the LAN without a code**: `stellwerk-agent join` finds the Zentrale over mDNS; the device
  waits under "Waiting devices" until you approve it, after comparing its fingerprint.
- **Runs in the background**: `stellwerk-agent service install` (or the Windows installer) starts
  the agent at login as the current user, without admin rights, on Linux, macOS and Windows.
- **Signed sign-in**: after pairing, the agent signs in with its own Ed25519 key; a device can be
  blocked or removed in the UI.

### 🗄️ The server holds everything

- **Hub in every folder**: the server keeps a full copy of each folder, with versions for a year.
- **Live status**: progress per folder and device, what an offline device will get, and an
  activity feed ("laptop changed 18 files in Projects").
- **Conflicts solved in one place**: both versions side by side with a diff; keep one or both,
  the other stays in the versions. Any older version of a file can be restored.
- **German and English**, following the browser or switched in the sidebar.
- **Your Syncthing stays yours**: the agent brings its own Syncthing on its own ports and never
  touches a Syncthing you already run.

## 📸 Screenshots

<table>
  <tr>
    <td><img src="docs/screenshot-devices.png" alt="Device list with online and offline devices, the folders each holds and the agent version"></td>
    <td><img src="docs/screenshot-pairing.png" alt="Pairing dialog with the agent command and a one-time code, waiting for the new device"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshot-folder.png" alt="Folder page with the mode and the local path of the folder on every device"></td>
    <td><img src="docs/screenshot-dark.png" alt="The overview with the folder by device matrix in dark mode"></td>
  </tr>
  <tr>
    <td><img src="docs/screenshot-conflict.png" alt="A Syncthing conflict shown side by side with a line diff and the choice to keep one or both versions"></td>
    <td></td>
  </tr>
</table>

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
Yes, that is what Stellwerk is for: one web UI on your server that configures Syncthing on all
your devices. There is no release yet, but it can be built from source; see [Install](#-install).

### Does it change my existing Syncthing?
No. The agent brings its own Syncthing with its own configuration and ports and leaves a
Syncthing you already run alone.

### Can I add devices that run plain Syncthing, like a phone?
Not yet. Every device needs the agent, which runs on Linux, Windows and macOS. Android and iOS
are not supported.

### Does it run on a Synology, Unraid or Raspberry Pi?
Anywhere Docker or Podman runs on Linux with amd64 or arm64: a Raspberry Pi 4 or 5 with a 64-bit
OS, Unraid, a Synology with Container Manager. The server needs disk space for a full copy of
every folder.

### Do the devices need admin rights or open ports?
No. The agent runs as the normal user and starts at login; devices only connect out to the
server. Only the server needs two ports open: the web port (behind HTTPS) and the hub's sync
port.

### Is my data encrypted?
In transit, yes: Syncthing encrypts every transfer with TLS and accepts only known devices; the
web UI and the agents' connection go through your HTTPS proxy. On the server, the hub keeps a
normal copy of every folder, so protect the server like the devices. Details in
[Security](https://firsttris.github.io/stellwerk/security.html).

### What happens if the server is down?
The devices keep their files and their last configuration. Changes wait on each device and sync
as soon as the server is back.

## 📚 Documentation

The full documentation, with search, is at **[firsttris.github.io/stellwerk](https://firsttris.github.io/stellwerk/)**.

| | |
|---|---|
| [Getting started](https://firsttris.github.io/stellwerk/getting-started.html) | from nothing to a first synced folder |
| [User guide](https://firsttris.github.io/stellwerk/user-guide.html) | every page of the web UI: matrix, devices, pairing, folders, conflicts, versions |
| [Installation](https://firsttris.github.io/stellwerk/installation.html) | Docker Compose, Podman Quadlet, binary; reverse proxy and TLS; the agent on each OS; updates and backups |
| [Reference](https://firsttris.github.io/stellwerk/reference.html) | every option, port, file and limit |
| [Security](https://firsttris.github.io/stellwerk/security.html) | trust model, encryption, login, pairing |
| [Troubleshooting](https://firsttris.github.io/stellwerk/troubleshooting.html) | logs and the common problems |
| [Architecture](https://firsttris.github.io/stellwerk/architecture.html) | Zentrale, hub, agent and engine; protocol; reconciler; roadmap |
| [Development](https://firsttris.github.io/stellwerk/development.html) | building, tests, CI, releases |

## 🛠️ Development

Requires Rust (stable), Node 22 and pnpm.

```bash
git clone https://github.com/firsttris/stellwerk && cd stellwerk
pnpm install && pnpm dev:demo   # the UI on demo data, no server needed
cargo test                      # downloads the pinned Syncthing once and runs real instances
pnpm e2e                        # Playwright against the UI in demo mode
```

Rust workspace: `protocol` (messages, desired state), `engine` (Syncthing control and
reconciler), `agent`, `zentrale` (axum, SQLite); React UI in `web/`, embedded into the Zentrale.
Running a real Zentrale and agent locally, the tests and releases:
[Development](https://firsttris.github.io/stellwerk/development.html).

## 🤝 Contributing

Issues and pull requests are welcome. A good bug report names the OS, the Stellwerk and Syncthing
versions and what the Zentrale showed. Please run `cargo fmt --all --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test` and `pnpm check` before opening a pull
request.

---

<div align="center">

⭐ Like Stellwerk? A [star on GitHub](https://github.com/firsttris/stellwerk) helps others find it.<br>
🐛 [Report a bug](https://github.com/firsttris/stellwerk/issues/new) · 💡 [Request a feature](https://github.com/firsttris/stellwerk/issues/new)

<sub>License: <a href="LICENSE">AGPL-3.0</a> · © Tristan Teufel and contributors<br>
Changed versions you pass on or run for others must offer their source code under the AGPL; a commercial license without these obligations is available via <a href="https://teufel-it.de">teufel-it.de</a>.<br>
Stellwerk is not affiliated with the Syncthing Foundation or Resilio.</sub>

</div>
