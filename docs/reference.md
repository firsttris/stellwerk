# Reference

Every command, option, port and file, for looking things up. Options can also be set as
environment variables (shown in brackets); a command-line option wins over the variable.

## Zentrale

```text
stellwerk [OPTIONS]                       run the Zentrale (the default)
stellwerk set-password                    set the admin password, read from stdin
stellwerk fetch-syncthing <DIR>           download and check the pinned Syncthing into DIR
```

| Option | Default | What it does |
|---|---|---|
| `--data-dir` (`STELLWERK_DATA`) | `data` (`/data` in the image) | database, hub Syncthing and the hub's copy of every folder; for every command |
| `--listen` (`STELLWERK_LISTEN`) | `0.0.0.0:8080` | address of the web UI, the API and the agents' WebSocket |
| `--hub-listen` (`STELLWERK_HUB_LISTEN`) | `tcp://0.0.0.0:22001` | where the hub Syncthing accepts sync connections |
| `--hub-address` (`STELLWERK_HUB_ADDRESS`) | `tcp://<host name>:<hub port>` | where devices reach the hub; set it in a container and behind NAT |
| `--public-url` (`STELLWERK_PUBLIC_URL`) | – | the address devices use for the web port, e.g. `https://sync.example.org`; announced in the LAN |
| `--lan-pairing` (`STELLWERK_LAN_PAIRING`) | `true` | announce the Zentrale over mDNS and let devices without a code wait for approval; `false` turns both off |
| `--syncthing` (`STELLWERK_SYNCTHING`) | download the pinned release | use this Syncthing binary instead (the image sets `/opt/syncthing/syncthing`) |
| `--ca-cert` (`STELLWERK_CA_CERT`) | – | the certificate (PEM) of a private CA behind the Zentrale's HTTPS; served at `/ca.crt`, its fingerprint goes into the pairing command ([private CA](installation.md#behind-a-private-ca)) |
| `--version`, `--help` | | |

`set-password` reads one line from standard input, stores its argon2id hash and logs out every
session.

### Zentrale data directory

| Path | Contents |
|---|---|
| `stellwerk.db` | SQLite database: admin, sessions, devices, folders, shares, pairing codes, activity |
| `hub/` | home of the hub Syncthing: `cert.pem`/`key.pem` (its identity), `config.xml`, index database, `stellwerk-syncthing.log` |
| `folders/<syncthing folder id>/` | the hub's copy of each folder; old versions in `.stversions/` inside it |
| `syncthing-bin/` | the downloaded and checked Syncthing release |

## Agent

```text
stellwerk-agent join [--server URL] [--ca-sha256 FINGERPRINT] [CODE]
                                             pair this device
stellwerk-agent run [--name NAME] [--syncthing PATH]
                                             run in the foreground
stellwerk-agent service install              run in the background from login on
stellwerk-agent service uninstall            stop and remove the background service
```

| Option | Applies to | What it does |
|---|---|---|
| `--data-dir` (`STELLWERK_AGENT_DATA`) | all | where the agent keeps its key, its Syncthing and Syncthing's database |
| `--server URL` | `join` | the Zentrale, `https://…` or `http://…`; found over mDNS if left out |
| `CODE` | `join` | the one-time code from **Pair device**; without it the device waits for approval |
| `--ca-sha256 FINGERPRINT` | `join` | the Zentrale is behind a private CA with this SHA-256 fingerprint: fetch the CA from `/ca.crt`, keep it if the fingerprint matches, and trust it for the Zentrale from then on |
| `--name NAME` | `run` | the device name suggested to the Zentrale when it pairs the device; the host name by default. Only for `run` in the foreground: the service always suggests the host name. Later, rename the device in the UI |
| `--syncthing PATH` (`STELLWERK_SYNCTHING`) | `run` | use this Syncthing binary instead of downloading the pinned one |

`join` creates the device's Ed25519 key, stores it with the server and the code in `agent.json`
and prints the key's fingerprint; with `--ca-sha256` it stores the Zentrale's CA there too.
Running `join` again replaces server and code but keeps the key (and the CA, for the same
server); with a new code, a device that was paired before comes back as the same device. If
`agent.json` exists but cannot be read, `join` stops instead of making a new key; delete the
file to pair the device as a new one.

`service install` sets up the service for the binary it is run from and the data directory it
is given, and starts it. Run it again after moving the binary or to restart the agent.

On Windows there is a second program, `stellwerk-agent-background.exe`: the same agent without a
console window, which the task at logon starts. It runs `run` and writes its log to `agent.log`.

### Agent data directory

By default:

| System | Path |
|---|---|
| Linux | `~/.local/share/stellwerk-agent` |
| macOS | `~/Library/Application Support/stellwerk-agent` |
| Windows | `%LOCALAPPDATA%\stellwerk-agent` |

| Path | Contents |
|---|---|
| `agent.json` | the Zentrale's address, the device's secret key and, until pairing, the code; readable only by the user |
| `syncthing/` | home of the agent's Syncthing: identity, configuration, index database |
| `syncthing-bin/` | the downloaded and checked Syncthing release |
| `agent.log` | the log on macOS and Windows (on Linux it goes to the journal) |
| `stop` | written by `service uninstall` to ask a running agent to stop; removed by it |
| `ignore.d/*.list` | exclusions written by other programs on this device (below) |

The synced folders are not in the data directory; they are wherever their paths point
(`~/Stellwerk/<folder>` by default).

### Exclusions from other programs

A program that moves some files itself, and must not have Syncthing write them too, puts a
list into `ignore.d/<program>.list` in the agent's data directory: one absolute path per line,
`#` starts a comment. Spielstand writes `spielstand.list` with the save files it synchronises.

The agent adds each listed path that lies inside a folder to that folder's ignore patterns, as
a pattern rooted at the folder that matches exactly that path (characters Syncthing's patterns
read specially are escaped, with `\` or on Windows `|`). It applies the lists with every desired state and reads them
again every 10 seconds, so a changed list takes effect without the Zentrale. The lists stay on
the device: the patterns from the Zentrale are unchanged, and every other device gets the same
exclusions from its own list.

## Logging

Both programs log to standard error, at level `info` by default. `RUST_LOG` changes it, e.g.
`RUST_LOG=debug` or `RUST_LOG=stellwerk_engine=debug,info`. The Syncthing instances log into
`stellwerk-syncthing.log` in their home directory; the log of the previous start is kept as
`stellwerk-syncthing.log.1`. The agent warns about a connection problem once; while the same
problem repeats, it logs each retry only at level `debug`, so `agent.log` does not grow while
the Zentrale is out of reach.

## Ports

| Port | Where | What |
|---|---|---|
| 8080/tcp | Zentrale | web UI, REST API (`/api/…`), UI WebSocket (`/api/ws`), agents' WebSocket (`/api/agent/ws`) |
| 22001/tcp | Zentrale | sync port of the hub Syncthing |
| 5353/udp | Zentrale, agent during `join` | mDNS: `_stellwerk._tcp` |
| random, 127.0.0.1 | server and devices | the GUI/REST API of each Syncthing, with a fresh API key on every start; reachable only locally |
| random, all interfaces | devices | the agent's Syncthing listen port; never dialed, since devices dial the hub |

## Times and limits

| What | Value |
|---|---|
| pairing code | valid 15 minutes, works once; only its hash is stored |
| login session | 30 days; logins are checked one at a time, and a failed login waits half a second, doubling with each failure in a row up to 8 s |
| waiting devices (LAN pairing) | at most 20, in memory, gone when the agent disconnects or the Zentrale restarts |
| agent reconnect | after 1 s, doubling up to 30 s; 60 s after being turned away |
| agent heartbeat | agent and Zentrale each ping every 30 s; a connection silent for 90 s counts as lost |
| hub reconcile retry | after a failure 1 s, doubling up to 60 s, until it succeeds or the revision changes |
| live status from a device | on change, at most about once per second |
| activity "changed N files" | counted per device and folder over 30 seconds |
| conflict scan on the hub | every 30 seconds |
| conflict diff | text files up to 256 KB |
| versions on the hub | staggered, kept for 365 days |
| default path of a new share | `~/Stellwerk/<folder name>` |

## REST API

The web UI uses the API under `/api`; it is not a stable public interface yet. Every request but
`/api/session` and `/api/login` needs the session cookie, and every change also the header
`X-CSRF-Token` with the token from the login. The endpoints are listed in
[Architecture](architecture.md#login-and-api).
