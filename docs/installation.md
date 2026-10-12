# Installation

The Zentrale runs as a container (Docker or Podman) or as a single binary; the agent is a single
binary per device. Until v1.0 is released, build the image and the binaries from source; from
v1.0 on the image is `tristanteu/stellwerk` on Docker Hub and `ghcr.io/firsttris/stellwerk`.

## Zentrale as a container

The image holds the Zentrale with the UI and the pinned Syncthing, runs as an unprivileged user
and keeps everything under `/data`. Build it locally with
`docker build -f docker/Dockerfile -t stellwerk .`.

### Docker Compose

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:8080"   # web UI and agents, behind a TLS proxy
      - "22001:22001"           # sync port of the hub, reachable from the devices
    environment:
      STELLWERK_HUB_ADDRESS: tcp://sync.example.org:22001
    volumes:
      - stellwerk:/data
volumes:
  stellwerk:
```

```bash
docker compose up -d
docker compose exec -T stellwerk stellwerk set-password <<< 'a long password'
```

### Podman Quadlet

`/etc/containers/systemd/stellwerk.container`:

```ini
[Unit]
Description=Stellwerk Zentrale

[Container]
Image=docker.io/tristanteu/stellwerk:latest
PublishPort=127.0.0.1:8080:8080
PublishPort=22001:22001
Environment=STELLWERK_HUB_ADDRESS=tcp://sync.example.org:22001
Volume=stellwerk.volume:/data
AutoUpdate=registry

[Service]
Restart=always

[Install]
WantedBy=multi-user.target
```

`/etc/containers/systemd/stellwerk.volume` holds only `[Volume]`. Then:

```bash
systemctl daemon-reload && systemctl start stellwerk
podman exec -i systemd-stellwerk stellwerk set-password <<< 'a long password'
```

Set `STELLWERK_HUB_ADDRESS` in either case: inside a container the host name is the container's.
The container stops cleanly on `SIGTERM`; the hub Syncthing is shut down with it.

## Build

Requires Rust (stable), Node 22 and pnpm.

```bash
git clone https://github.com/firsttris/stellwerk && cd stellwerk
pnpm install && pnpm --filter web build      # the UI, embedded into the Zentrale
cargo build --release                        # target/release/stellwerk and stellwerk-agent
```

## Zentrale as a binary

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
| `--lan-pairing` (`STELLWERK_LAN_PAIRING`) | `true` | announce the Zentrale over mDNS and let devices without a code wait for approval in the UI |
| `--public-url` (`STELLWERK_PUBLIC_URL`) | – | the address devices use, e.g. `https://sync.example.org`; announced in the LAN |

The hub port must be reachable from the devices; the web port only through the proxy.

## Agent on a device

Every release carries the agent per platform (`stellwerk-agent-linux-x86_64`, `-linux-aarch64`,
`-macos-aarch64`, `-macos-x86_64`, and `stellwerk-agent-windows-x86_64.zip` with both Windows
programs) and their `SHA256SUMS`; the Linux builds are static and run on any distribution.
Before v1.0, build it with `cargo build --release -p stellwerk-agent`. Put it somewhere it can
stay, e.g. `~/.local/bin` or `%LOCALAPPDATA%\Programs\Stellwerk`: the service starts it from
there.

In the UI, **Gerät koppeln** shows the command with a one-time code:

```bash
stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
stellwerk-agent service install
```

In the same network it also works without a code: `stellwerk-agent join` finds the Zentrale over
mDNS (or pass `--server`), prints the device's fingerprint and saves the address. After
`stellwerk-agent service install` the device shows up under **Geräte → Wartende Geräte**; approve it there if
the fingerprint matches.

mDNS needs the Zentrale in the same network segment: in a container that means host networking
(`network_mode: host` in Compose, `Network=host` in Quadlet). With port mapping, pass `--server`
to `join`; waiting for approval works the same.

The agent keeps its key, its own Syncthing and Syncthing's database in its data directory
(`~/.local/share/stellwerk-agent` on Linux, `--data-dir` to change it). It downloads the pinned
Syncthing on first start and never touches a Syncthing that is already installed.

### Running in the background

`stellwerk-agent service install` starts the agent now and from every login on, as the current
user and without admin rights; `service uninstall` stops it (and its Syncthing) and removes the
service, the data directory stays. `stellwerk-agent run` runs it in the foreground instead.

| System | Service | Log |
|---|---|---|
| Linux | systemd user unit `~/.config/systemd/user/stellwerk-agent.service` | `journalctl --user -u stellwerk-agent` |
| macOS | launchd agent `~/Library/LaunchAgents/io.github.firsttris.stellwerk.agent.plist` | `agent.log` in the data directory |
| Windows | task `Stellwerk\Agent` at logon, running `stellwerk-agent-background.exe` (no window) | `agent.log` in the data directory |

On Windows the release also carries `stellwerk-agent-setup-windows-x86_64.exe`: it installs both
programs for the current user into `%LOCALAPPDATA%\Programs\Stellwerk` (no admin rights), asks
for the Zentrale's address and the code (empty for pairing in the LAN), pairs and installs the
task; **Apps → Stellwerk Agent → Uninstall** stops and removes it, the data directory stays.
Without questions: `stellwerk-agent-setup-windows-x86_64.exe /S /SERVER=https://… /CODE=…`.

On Linux a user service stops when the user logs out; `loginctl enable-linger $USER` keeps it
running (on some distributions with `sudo`). On Windows the agent runs while the user is logged
in. CI installs, checks and removes the service on all three systems
(`scripts/service-smoke.sh`), and the Windows installer on every agent build
(`scripts/installer-smoke.sh`).

## Releases

**Actions → Bump version** raises the version in `Cargo.toml` (and the `package.json` files), tags
it and starts `release.yml`: the checks from `ci.yml`, the agents from `agents.yml`, then the
multi-arch image (`linux/amd64`, `linux/arm64`) on Docker Hub and GHCR and the GitHub release with
the agents attached. Docker Hub needs the `DOCKER_PAT` secret.
