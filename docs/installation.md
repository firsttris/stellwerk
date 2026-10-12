# Installation

Stellwerk has two parts to install: the **Zentrale** once on a server, and the **agent** on every
device. This page covers both for real use: TLS, firewall, backups, updates. For a quick first
try, [Getting started](getting-started.md) is shorter.

!!! note "Before v1.0"
    No release is published yet: build the image with
    `docker build -f docker/Dockerfile -t tristanteu/stellwerk .` and the agent with
    `cargo build --release -p stellwerk-agent` (see [Development](development.md#building)).
    From v1.0 on, the image is `tristanteu/stellwerk` on Docker Hub and
    `ghcr.io/firsttris/stellwerk`, and every [release](https://github.com/firsttris/stellwerk/releases)
    carries the agents.

## Requirements

**Server**

- Linux on `amd64` or `arm64`, with Docker or Podman, or the plain binary.
- Disk space for a full copy of every folder plus its versions: the hub keeps old versions of
  changed and deleted files for a year by default.
- Reachable from the devices on two ports:

| Port | Protocol | Used by | Must be reachable |
|---|---|---|---|
| 8080 (default `--listen`) | HTTP, WebSocket | web UI, agents' connection to the Zentrale | from the devices and your browser; from outside only through a TLS proxy (443) |
| 22001 (default `--hub-listen`) | TCP (Syncthing, TLS) | file transfer between devices and the hub | from the devices, directly |
| 5353 | UDP multicast (mDNS) | LAN pairing without a code | in the LAN only; optional |

The hub uses 22001 and not Syncthing's usual 22000, so a Syncthing already running on the server
keeps its port.

**Devices**

- Linux (`x86_64`, `aarch64`; static builds that run on any distribution), Windows 10 or 11
  (`x86_64`), macOS (Apple Silicon and Intel).
- Outgoing connections to the server on the two ports above. Devices need **no** open port; the
  hub never dials them.
- No admin rights: the agent runs as the normal user.

## Zentrale as a container

The image holds the Zentrale with the web UI and the pinned Syncthing. It runs as an
unprivileged user (UID 65532), keeps everything under `/data` and shuts down cleanly on
`SIGTERM`, stopping the hub Syncthing with it.

### Docker Compose

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:8080"   # web UI and agents, only for the TLS proxy on this host
      - "22001:22001"           # sync port of the hub, reachable from the devices
    environment:
      STELLWERK_HUB_ADDRESS: tcp://sync.example.org:22001
      STELLWERK_PUBLIC_URL: https://sync.example.org
    volumes:
      - stellwerk:/data
volumes:
  stellwerk:
```

```bash
docker compose up -d
docker compose exec -T stellwerk stellwerk set-password <<< 'a long password'
```

`STELLWERK_HUB_ADDRESS` tells the devices where to reach the hub. Inside a container the host
name is the container's, so set it always. If you map the hub to another outside port, put that
port into the address.

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
Environment=STELLWERK_PUBLIC_URL=https://sync.example.org
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

`AutoUpdate=registry` lets `podman auto-update` (or its timer) pull new images.

### LAN pairing from a container

mDNS needs the Zentrale in the same network segment as the devices. With port mapping the
announcement does not leave the container; use host networking for it (`network_mode: host` in
Compose, `Network=host` in Quadlet, and drop the port mappings). Without that, pass `--server`
to `stellwerk-agent join`; approving a device without a code works the same.

## Zentrale as a binary

The Zentrale is one file, `stellwerk`, with the UI built in. It downloads the pinned Syncthing
on first start into its data directory, or uses one you give it with `--syncthing`.

```bash
stellwerk --data-dir /srv/stellwerk set-password      # once; reads the password from stdin
stellwerk --data-dir /srv/stellwerk \
  --listen 127.0.0.1:8080 \
  --hub-listen tcp://0.0.0.0:22001 \
  --hub-address tcp://sync.example.org:22001 \
  --public-url https://sync.example.org
```

As a systemd service, `/etc/systemd/system/stellwerk.service`:

```ini
[Unit]
Description=Stellwerk Zentrale
After=network-online.target
Wants=network-online.target

[Service]
User=stellwerk
Environment=STELLWERK_DATA=/srv/stellwerk
Environment=STELLWERK_LISTEN=127.0.0.1:8080
Environment=STELLWERK_HUB_ADDRESS=tcp://sync.example.org:22001
Environment=STELLWERK_PUBLIC_URL=https://sync.example.org
ExecStart=/usr/local/bin/stellwerk
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

```bash
useradd --system --home /srv/stellwerk --create-home stellwerk
systemctl enable --now stellwerk
```

Every option is in the [reference](reference.md#zentrale).

## Reverse proxy and TLS

The web port carries the admin password, the session cookie and the devices' sign-in. Put it
behind a reverse proxy with HTTPS before it is reachable from outside your own network. The
proxy must:

- pass WebSockets on `/api/ws` (the UI's live updates) and `/api/agent/ws` (the agents);
- set `X-Forwarded-Proto: https`, so the Zentrale marks the session cookie `Secure`.

The hub port 22001 does **not** go through the proxy: Syncthing encrypts it itself with TLS and
pinned device IDs.

=== "Caddy"

    ```caddy
    sync.example.org {
        reverse_proxy 127.0.0.1:8080
    }
    ```

    Caddy gets the certificate, passes WebSockets and sets `X-Forwarded-Proto` by itself.

=== "nginx"

    ```nginx
    server {
        listen 443 ssl;
        server_name sync.example.org;
        # ssl_certificate …; ssl_certificate_key …;

        location / {
            proxy_pass http://127.0.0.1:8080;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection $connection_upgrade;
            proxy_set_header Host $host;
            proxy_set_header X-Forwarded-Proto $scheme;
            proxy_read_timeout 1h;   # the WebSockets stay open
        }
    }
    map $http_upgrade $connection_upgrade { default upgrade; '' close; }
    ```

=== "Traefik (Compose labels)"

    ```yaml
        labels:
          - traefik.enable=true
          - traefik.http.routers.stellwerk.rule=Host(`sync.example.org`)
          - traefik.http.routers.stellwerk.entrypoints=websecure
          - traefik.http.routers.stellwerk.tls.certresolver=letsencrypt
          - traefik.http.services.stellwerk.loadbalancer.server.port=8080
    ```

    Traefik passes WebSockets and sets `X-Forwarded-Proto` by itself. Drop the `8080` port
    mapping then.

With the proxy in place, pair devices with `--server https://sync.example.org`. The pairing
dialog takes the address from the browser, so open the UI through the proxy.

## Firewall

On the server, open:

- **443/tcp** to the proxy (or 8080/tcp if you stay in the LAN without a proxy);
- **22001/tcp** to the hub;
- **5353/udp** in the LAN only, if you want pairing without a code.

Syncthing's local discovery, global discovery and relays are off on the hub and on every agent:
devices connect to the hub address directly, nothing else.

## Agent on a device

Download the agent for the device from the release, or build it. Put it where it can stay: the
service starts it from that place.

=== "Linux"

    ```bash
    mkdir -p ~/.local/bin
    install -m 755 stellwerk-agent-linux-x86_64 ~/.local/bin/stellwerk-agent
    stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
    stellwerk-agent service install
    ```

    A user service stops when the user logs out. To keep the agent running without a login
    (a server, a headless PC), allow lingering once: `loginctl enable-linger $USER` (on some
    distributions with `sudo`).

=== "macOS"

    ```bash
    mkdir -p ~/.local/bin
    install -m 755 stellwerk-agent-macos-aarch64 ~/.local/bin/stellwerk-agent
    xattr -d com.apple.quarantine ~/.local/bin/stellwerk-agent 2>/dev/null
    ~/.local/bin/stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
    ~/.local/bin/stellwerk-agent service install
    ```

    The binary is not notarized yet; removing the quarantine flag lets macOS start it. Use
    `-macos-x86_64` on an Intel Mac.

=== "Windows (zip)"

    Unpack `stellwerk-agent-windows-x86_64.zip` into `%LOCALAPPDATA%\Programs\Stellwerk`; it holds
    `stellwerk-agent.exe` and `stellwerk-agent-background.exe`. In PowerShell in that folder:

    ```powershell
    .\stellwerk-agent.exe join --server https://sync.example.org K7Q2-MX9P-41TZ
    .\stellwerk-agent.exe service install
    ```

### Windows installer

`stellwerk-agent-setup-windows-x86_64.exe` does all of the above in one go, for the current user
and without admin rights: it installs both programs into `%LOCALAPPDATA%\Programs\Stellwerk`,
asks for the Zentrale's address and the code (empty for pairing in the LAN), pairs the device
and installs the task. **Settings → Apps → Stellwerk Agent → Uninstall** stops and removes it;
the data directory stays, so installing again keeps the pairing.

Without questions, e.g. for a deployment script:

```powershell
stellwerk-agent-setup-windows-x86_64.exe /S /SERVER=https://sync.example.org /CODE=K7Q2-MX9P-41TZ
```

The installer is not code-signed yet, so SmartScreen may warn on first start (**More info → Run
anyway**).

### Running in the background

`stellwerk-agent service install` starts the agent now and from every login on, as the current
user. `service uninstall` stops the agent and its Syncthing and removes the service; the data
directory stays. `stellwerk-agent run` runs it in the foreground instead, e.g. to watch its log.

| System | Service | Log |
|---|---|---|
| Linux | systemd user unit `~/.config/systemd/user/stellwerk-agent.service` | `journalctl --user -u stellwerk-agent` |
| macOS | launchd agent `~/Library/LaunchAgents/io.github.firsttris.stellwerk.agent.plist` | `agent.log` in the data directory |
| Windows | Task Scheduler task `Stellwerk\Agent` at logon, running `stellwerk-agent-background.exe` (no window) | `agent.log` in the data directory |

The service remembers the path of the binary and the data directory it was installed with. After
moving the binary, run `service install` again from the new place.

### Removing the agent from a device

1. `stellwerk-agent service uninstall` (or uninstall the Windows app).
2. Delete the binary and, to forget the pairing, the data directory
   ([where it is](reference.md#agent-data-directory)).
3. In the UI, **Devices → Remove …**. The synced folders stay on the device as plain files.

## Updating

**Zentrale.** Pull the new image and recreate the container (`docker compose pull && docker
compose up -d`, or `podman auto-update`). The database is migrated on start. Agents reconnect by
themselves within 30 seconds.

**Agents.** Until the self-update arrives: replace the binary with the new one at the same place
(on Windows, run the new installer) and run `stellwerk-agent service install` again, which
restarts it. Agent and Zentrale of neighbouring versions work together; the device list shows
each agent's version.

**Syncthing.** Each Stellwerk version pins one Syncthing version with its checksums. It is
updated together with Stellwerk, never on its own; Syncthing's own auto-upgrade is off.

## Backup and restore

Everything the Zentrale knows is in its data directory (`/data` in the container):

| Path | Holds | Back up |
|---|---|---|
| `stellwerk.db` | devices, folders, shares, sessions, activity (SQLite) | **yes**: the configuration |
| `hub/` | the hub Syncthing's identity (`cert.pem`, `key.pem`), configuration and index | **yes**: the identity; the index is rebuilt if lost |
| `folders/` | the hub's copy of every folder, with `.stversions` | as your data needs it; the devices have the current files, only the server has the versions |
| `syncthing-bin/` | the downloaded Syncthing | no |

The simplest consistent backup is with the Zentrale stopped:

```bash
docker compose stop stellwerk
docker run --rm -v stellwerk_stellwerk:/data -v "$PWD":/backup alpine \
  tar czf /backup/stellwerk-$(date +%F).tgz -C /data stellwerk.db hub
docker compose start stellwerk
```

While it runs, `sqlite3 /data/stellwerk.db ".backup /backup/stellwerk.db"` takes a consistent
copy of the database.

**Restore** by putting the files back into an empty data directory before the first start. If
the hub's identity is lost, the hub gets a new device ID; the Zentrale hands it to every agent
with the next desired state, so the devices follow without re-pairing, and the hub fetches the
folders from them again.

**Moving to another server**: back up, restore on the new one, point the DNS name to it. If the
hub address changes, set the new `STELLWERK_HUB_ADDRESS`; agents get it with their next desired
state. If the web address changes, the agents still use the old one from `agent.json`: pair them
again, or edit `server` in `agent.json` and restart the agent.

## Resetting the admin password

On the server: `stellwerk set-password` (in the container: `docker compose exec -T stellwerk
stellwerk set-password <<< 'new password'`). It logs out every session.
