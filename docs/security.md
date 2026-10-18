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
  against the system's root certificates, plus a private CA when the Zentrale has one
  (`--ca-cert`): the agent fetched it at pairing without trusting the connection and kept it
  only because its SHA-256 matched the fingerprint in the pairing command, which the admin
  copied from the UI over a connection they already trust. A Zentrale address with `http://` is fine inside a
  trusted LAN, but nothing is encrypted then, including the pairing code.
- **Devices ↔ hub.** Syncthing's own protocol: TLS with each side's certificate, and each side
  only accepts the device IDs (certificate hashes) it was given. The Zentrale hands every agent
  the hub's ID and the hub the IDs of all approved devices, so nobody else can join a folder,
  even on the open sync port. Discovery servers and relays are off; data goes only between a
  device and the hub.
- **Stellwerk → Syncthing.** Each Syncthing's GUI/REST API listens on `127.0.0.1` on a random
  port, with a fresh random API key on every start. The key goes to Syncthing in its
  environment, not on the command line, where every local user could read it (`ps`). Other
  users on the machine could reach the port, but not without the key.

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

## API tokens

- For scripts and AI assistants, the admin creates API tokens in the web UI (**API tokens**).
  A token is 32 random bytes with the prefix `stw_`; the database stores only its SHA-256, and
  the UI shows it once.
- A token is sent as `Authorization: Bearer …`. The browser never sends it on its own, so it
  needs no CSRF token.
- Its scope limits it: **Read** sees the overview, status, activity and the folders on devices;
  **Manage** also creates and renames folders, sets ignore patterns and applies shares. Removing
  or blocking devices, pairing, deleting folders, conflicts, versions and the tokens themselves
  are refused (`403`) to every token, so a leaked token cannot let a device in or throw one out.
- A **Manage** token can still share a folder with any paired device, at any path there: treat it
  like the admin password. Prefer **Read** where that is enough, give each script or assistant
  its own token, and revoke the ones you no longer use (the list shows when each was last used).
- A new admin password revokes every token, since whoever knew the old one may have made one.
- The MCP server at `/mcp` takes only API tokens, never the session cookie, and offers a token
  only the tools of its scope. It refuses requests whose `Origin` names another host, so a web
  page cannot reach it through your browser. An assistant with a **Manage** token can set up
  any share a person could in the matrix; Claude Code asks before each tool that changes
  something, so keep that prompt on for `apply_shares`.

## Pairing and device sign-in

- **One-time code.** Random, valid for 15 minutes, usable once; the database stores only its
  hash. It proves to the Zentrale that the person running `join` was allowed to pair.
- **Device key.** `join` creates an **Ed25519** key pair on the device; the secret key stays in
  `agent.json`, readable only by the user (on Linux and macOS, file mode `600`). On every
  connection the Zentrale sends a fresh random nonce and the agent signs it together with
  everything else its hello says (code, Syncthing device ID, name, OS, versions), so a recorded
  sign-in cannot be replayed, and someone who can read and change an `http://` connection
  cannot keep the signature but put in their own Syncthing ID, nor move the code into a hello
  of their own. They can still read the code; use HTTPS wherever the network is not trusted.
  After the first sign-in the code is dropped; the key alone identifies the device. Each device
  must have its own Syncthing device ID: a device reporting an ID that belongs to another
  device (a copied Syncthing home) is turned away.
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
- On request of the Zentrale it lists the subdirectories of a directory (the folder picker) or
  searches for directories by name. Only names and paths of directories leave the device, never
  files or their contents; a search skips hidden directories, follows no symbolic links and stops
  after 8 levels, 50 matches or 10 seconds.

## Updates and releases

Release images and agents are built by GitHub Actions from the tagged source, after the full test
suite. Each release lists the agents' SHA-256 sums in `SHA256SUMS`. The planned self-update of
the agents will only install a new version whose signature matches a public key built into the
agent ([roadmap](architecture.md#roadmap)), so a compromised Zentrale cannot push its own
program to the devices.

## Checklist

- [ ] The web port is reachable from outside only through HTTPS.
- [ ] The admin password is long and used nowhere else.
- [ ] Only the API tokens in use exist, each with the narrowest scope that works.
- [ ] The data directory (or volume) is backed up, and the backup is protected like the server.
- [ ] Waiting devices are approved only after comparing the fingerprint.
- [ ] A lost or sold device is **blocked** or **removed** in the UI.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private
[security advisory](https://github.com/firsttris/stellwerk/security/advisories/new) instead.
