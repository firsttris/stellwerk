# Getting started

This page takes you from nothing to a first folder synced between a server and one device, in
the shortest way that is still safe to keep. For a server reachable from the internet, read
[Installation](installation.md) afterwards; it adds TLS, backups and updates.

**You need**

- a **server** that is on all the time: Linux with Docker or Podman (a NAS, a mini PC, a VPS);
- one or more **devices** to sync: Linux, Windows or macOS;
- the devices must be able to reach the server on two ports: the web port (8080, or 443 behind
  a proxy) and the hub's sync port (22001).

## 1. Start the Zentrale

On the server, create `compose.yaml` in a new directory. Replace `nas.local` with the name or IP address under which
your devices reach the server:

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk
    restart: unless-stopped
    ports:
      - "8080:8080"     # web UI and agents
      - "22001:22001"   # sync port of the hub
    environment:
      STELLWERK_HUB_ADDRESS: tcp://nas.local:22001
    volumes:
      - stellwerk:/data
volumes:
  stellwerk:
```

```bash
docker compose up -d
docker compose exec -T stellwerk stellwerk set-password <<< 'a long password'
```

Open `http://nas.local:8080` and log in with that password. The overview is empty: no devices,
no folders yet.

!!! warning
    Port 8080 without TLS is fine for a first try in your own network. Before you open it to the
    internet, put a reverse proxy with HTTPS in front of it: the password and the devices'
    sign-in go over this port. See [Reverse proxy](installation.md#reverse-proxy-and-tls).

## 2. Create a folder

**New folder**, give it a name such as `Documents`. The server now holds an empty folder with
versions; no device has it yet. Once a device is paired, **New folder** can also pick an existing
folder on it right away (see [Picking a folder on a
device](user-guide.md#picking-a-folder-on-a-device)).

## 3. Pair a device

In the UI click **Pair device**, pick the device's operating system and copy the command. It
looks like this, with a one-time code valid for 15 minutes:

```bash
curl -fsSL https://github.com/firsttris/stellwerk/releases/download/vX.Y.Z/install.sh | sh -s -- http://nas.local:8080 K7Q2-MX9P-41TZ
```

Run it in a terminal on the device, as your normal user (no `sudo`, no administrator); on
Windows the dialog shows a command for PowerShell. It
downloads the agent of the Zentrale's version, runs `stellwerk-agent join`, which stores the
server and a new key for the device, and `stellwerk-agent service install`, which starts the
agent now and on every login. With an agent you built yourself, run those two directly:
`stellwerk-agent join --server http://nas.local:8080 K7Q2-MX9P-41TZ && stellwerk-agent service install`.
On the first start the agent downloads its own Syncthing (about 15 MB), which takes a moment.

The dialog in the UI notices the new device by itself and asks which folders it should get.
Tick **Documents** and click **Set up device**.

??? tip "Pairing in the LAN without a code"
    In the same network you can skip the code: run the command without the code at the end. The
    agent prints a fingerprint such as `3F2A-91BC-04DE-77A1`, and the device appears under
    **Devices → Waiting devices**; approve it there if the fingerprint matches.

## 4. Watch it sync

Back in the **Overview**, the row *Documents* has a cell for the new device in mode **Both**.
The folder is at `~/Stellwerk/Documents` on the device (`%USERPROFILE%\Stellwerk\Documents` on
Windows). Put a file into it: within seconds the activity feed says that the device changed a
file in *Documents*, and the file is on the server too.

Pair a second device the same way and give it the folder as well: files now flow device → server
→ device, even if the two devices are never on at the same time.

## Next steps

- Change a folder's **path** or **mode** per device: open the folder, tab **Devices**
  ([user guide](user-guide.md#folder-page)).
- Keep build output and caches out of a folder with **ignore patterns**
  ([user guide](user-guide.md#settings-rename-ignore-patterns-remove)).
- Make the server reachable from outside with TLS and set up backups
  ([installation](installation.md)).
