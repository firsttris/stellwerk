# Security

What protects what in Stellwerk, and what you have to trust. In short: **whoever controls the
Zentrale controls which folders every device syncs, and where.** Protect the admin password and
the server like you would protect the devices themselves.

## Trust model

| Party | Can |
|---|---|
| **Admin** (the one login) | decide for every device which folders it syncs, in which mode and at which local path; read every file on the server; resolve conflicts and restore versions |
| **Server** (root on the host, or the container's volume) | read and change every synced file in the hub's copy, the database and the hub's identity |
| **A paired device** | send its changes into the folders it holds in mode Both or Send only; these reach the server and every other device of those folders |
| **Someone in the LAN** | ask to be paired without a code; becomes a device only when the admin approves it |
| **Someone on the internet** | reach the login page and the agents' sign-in, if the web port is public |

**The agent follows the Zentrale.** It configures its Syncthing to whatever the desired state
says, including the local path of a folder, with the rights of the user it runs as. It does not
restrict paths. An admin, or an attacker with the admin password or control of the server, can
therefore share a folder with any path the device's user can read or write, and so read files
from the device or put files on it. That is the point of central management, and also why the
Zentrale is the most valuable part to protect. The agent never deletes files, and it never runs
commands it receives.

**The server holds plaintext.** The hub keeps an ordinary, unencrypted copy of every folder, with
versions. Encrypting folders on the hub (Syncthing's untrusted devices) is not supported yet.

## Connections

```text
browser ──HTTPS (proxy)──▶ Zentrale ◀──WSS (proxy)── agent
                               │                        │
                       REST, 127.0.0.1           REST, 127.0.0.1
                               ▼                        ▼
                         hub Syncthing ◀──TLS 1.3──▶ device Syncthing
```

- **Browser and agents → Zentrale.** One web port for the UI, the API and the agents'
  WebSocket. Stellwerk itself speaks plain HTTP; TLS comes from the reverse proxy
  ([installation](installation.md#reverse-proxy-and-tls)). Agents check the proxy's certificate
  against the system's root certificates. A Zentrale address with `http://` is fine inside a
  trusted LAN, but nothing is encrypted then, including the pairing code.
- **Devices ↔ hub.** Syncthing's own protocol: TLS with each side's certificate, and each side
  only accepts the device IDs (certificate hashes) it was given. The Zentrale hands every agent
  the hub's ID and the hub the IDs of all approved devices, so nobody else can join a folder,
  even on the open sync port. Discovery servers and relays are off; data goes only between a
  device and the hub.
- **Stellwerk → Syncthing.** Each Syncthing's GUI/REST API listens on `127.0.0.1` on a random
  port, with a fresh random API key on every start. Other users on the machine could reach the
  port, but not without the key.

## Admin login

- One admin, with a password stored as an **argon2id** hash. It is set only on the server
  (`stellwerk set-password`), never over the web.
- A login creates a random session token. The browser gets it in a cookie that is `HttpOnly`,
  `SameSite=Strict` and, behind HTTPS (`X-Forwarded-Proto: https`), `Secure`. The database
  stores only the token's hash. Sessions last 30 days; a new password ends all of them.
- Every change needs the header `X-CSRF-Token` with a second token from the login, so another
  website cannot make your browser change anything.
- Passwords are checked one at a time. A failed login waits half a second, doubling with each
  further failure in a row up to 8 seconds, so parallel guessing is no faster than serial. There
  is no lockout; use a long password, and if the login is public, consider limiting it in the
  proxy (e.g. only from your VPN).
- The UI's live WebSocket (`/api/ws`) checks the session before each update and closes once the
  session is gone (logout, expiry, new password).
- Behind a TLS proxy, the cookie gets `Secure` only if the proxy sets `X-Forwarded-Proto: https`
  (the examples in [installation](installation.md#reverse-proxy-and-tls) do).

## Pairing and device sign-in

- **One-time code.** Random, valid for 15 minutes, usable once; the database stores only its
  hash. It proves to the Zentrale that the person running `join` was allowed to pair.
- **Device key.** `join` creates an **Ed25519** key pair on the device; the secret key stays in
  `agent.json`, readable only by the user (on Linux and macOS, file mode `600`). On every
  connection the Zentrale sends a fresh random nonce and the agent signs it, so a recorded
  sign-in cannot be replayed. After the first sign-in the code is dropped; the key alone
  identifies the device. Each device must have its own Syncthing device ID: a device reporting
  an ID that belongs to another device (a copied Syncthing home) is turned away.
- **LAN pairing without a code.** A device that comes without a code only waits. The UI shows
  the **fingerprint** of its key (the first 8 bytes of the key's SHA-256, e.g.
  `3F2A-91BC-04DE-77A1`), and `join` prints the same on the device. Approve only if they match;
  otherwise someone else in the network could pose as your device. `--lan-pairing false` turns
  this off.
- **Block and remove.** A blocked device is turned away at sign-in, gets no desired state, and
  the hub drops it as a peer, so it can no longer sync. Removing deletes it; its key is then
  unknown and it would have to be paired again.

## On the devices

- The agent runs as the normal user, without admin rights, so it can only reach what that user
  can reach.
- It downloads exactly the Syncthing version pinned in the source, from Syncthing's GitHub
  releases, and checks its **SHA-256** against the checksum compiled into Stellwerk before
  running it. Syncthing's own auto-upgrade is off.
- It never touches another Syncthing on the same machine; its own runs with its own home
  directory and ports.

## Updates and releases

Release images and agents are built by GitHub Actions from the tagged source, after the full test
suite. Each release lists the agents' SHA-256 sums in `SHA256SUMS`. The planned self-update of
the agents will only install a new version whose signature matches a public key built into the
agent ([roadmap](architecture.md#roadmap)), so a compromised Zentrale cannot push its own
program to the devices.

## Checklist

- [ ] The web port is reachable from outside only through HTTPS.
- [ ] The admin password is long and used nowhere else.
- [ ] The data directory (or volume) is backed up, and the backup is protected like the server.
- [ ] Waiting devices are approved only after comparing the fingerprint.
- [ ] A lost or sold device is **blocked** or **removed** in the UI.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private
[security advisory](https://github.com/firsttris/stellwerk/security/advisories/new) instead.
