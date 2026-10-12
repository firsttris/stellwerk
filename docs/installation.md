# Installation

Until v1.0 there are no packages yet: Zentrale and agent are built from source. The Docker image,
installers and self-update come in phase 3.

## Build

Requires Rust (stable), Node 22 and pnpm.

```bash
git clone https://github.com/firsttris/stellwerk && cd stellwerk
pnpm install && pnpm --filter web build      # the UI, embedded into the Zentrale
cargo build --release                        # target/release/stellwerk and stellwerk-agent
```

## Zentrale on the server

```bash
stellwerk --data-dir /srv/stellwerk set-password      # once; reads the password from stdin
stellwerk --data-dir /srv/stellwerk \
  --listen 0.0.0.0:8080 \
  --hub-listen tcp://0.0.0.0:22001 \
  --hub-address tcp://sync.example.org:22001
```

| Option | Default | |
|---|---|---|
| `--data-dir` (`STELLWERK_DATA`) | `data` | database, hub Syncthing and the hub's copy of every folder |
| `--listen` (`STELLWERK_LISTEN`) | `0.0.0.0:8080` | web UI and the agents' WebSocket; put it behind a TLS proxy such as Caddy |
| `--hub-listen` (`STELLWERK_HUB_LISTEN`) | `tcp://0.0.0.0:22001` | sync port of the hub; not 22000, so a Syncthing already running keeps its port |
| `--hub-address` (`STELLWERK_HUB_ADDRESS`) | host name + hub port | where devices reach the hub |
| `--syncthing` (`STELLWERK_SYNCTHING`) | download | a Syncthing binary instead of the pinned release |

The hub port must be reachable from the devices; the web port only through the proxy.

## Agent on a device

In the UI, **Gerät koppeln** shows the command with a one-time code:

```bash
stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
stellwerk-agent run
```

The agent keeps its key, its own Syncthing and Syncthing's database in its data directory
(`~/.local/share/stellwerk-agent` on Linux, `--data-dir` to change it). It downloads the pinned
Syncthing on first start and never touches a Syncthing that is already installed. Running it as a
service (systemd, Windows service, launchd) comes with the installers in phase 3.
