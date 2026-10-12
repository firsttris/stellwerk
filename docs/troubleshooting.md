# Troubleshooting

## Where the logs are

| What | Where |
|---|---|
| Zentrale (container) | `docker compose logs stellwerk`, `journalctl -u stellwerk` for Quadlet |
| Zentrale (binary with systemd) | `journalctl -u stellwerk` |
| hub Syncthing | `hub/stellwerk-syncthing.log` in the Zentrale's data directory |
| agent on Linux | `journalctl --user -u stellwerk-agent` |
| agent on macOS and Windows | `agent.log` in the [agent's data directory](reference.md#agent-data-directory) |
| the device's Syncthing | `syncthing/stellwerk-syncthing.log` in the agent's data directory |

More detail: set `RUST_LOG=debug` for the Zentrale (an `environment:` entry) or run the agent in
the foreground with `RUST_LOG=debug stellwerk-agent run` after `service uninstall`.

## Pairing

**The pairing dialog keeps waiting.**
The agent has not reached the Zentrale. On the device, look at its log:

- `connection to the Zentrale failed`: the address in the command is not reachable from the
  device. The dialog uses the address in the browser's address bar; if you opened the UI as
  `localhost` or through an address the device cannot reach, pair with `--server` set by hand.
- a certificate error: the proxy's certificate is not trusted by the device (self-signed). Use a
  certificate from a public CA, e.g. through Caddy.
- `the Zentrale turned the agent away`: the code is used up or older than 15 minutes. Create a
  new one and run `join` again with it; the device keeps its key.

**`join` without `--server` finds no Zentrale.**
mDNS works only in the same network segment, and not from a container with port mapping (see
[LAN pairing from a container](installation.md#lan-pairing-from-a-container)). Guest networks and
some Wi-Fi access points block multicast. Pass `--server` instead; approval works the same.

**A device does not appear under Waiting devices.**
It appears only while its agent runs (`service install` after `join`) and only if LAN pairing is
on (`--lan-pairing`). A rejected device stays rejected until the Zentrale restarts.

## Syncing

**A cell says "waiting" for a long time.**
The device is offline, or its agent is not running. It applies the change as soon as it is back.
On the device: is the service running (`systemctl --user status stellwerk-agent`, Task Scheduler
on Windows, `launchctl print gui/$(id -u)/io.github.firsttris.stellwerk.agent` on macOS)?

**The device is online, but no files arrive.**
The configuration arrived, but the device's Syncthing cannot reach the hub. Check from the device
that the hub address is reachable (`nc -vz sync.example.org 22001`, or
`Test-NetConnection sync.example.org -Port 22001` on Windows):

- the hub port 22001 is not open in the server's firewall or not mapped in the container;
- `STELLWERK_HUB_ADDRESS` is not set, so devices try the container's host name. Set it to the
  server's public name and port.

**On Linux the agent stops when I log out.**
That is how systemd user services work. `loginctl enable-linger $USER` keeps it running.

**"Error in … on …" in Needs attention.**
Syncthing on that device could not sync some files. The text says why; the most common reasons:

- *path is not writable*: the folder's local path does not exist or belongs to another user.
  Change it on the folder page, tab **Devices**.
- *folder marker missing*: the folder's directory was deleted or a drive is not mounted.
  Syncthing stops the folder instead of treating everything as deleted. Mount the drive or set
  the path again.
- files that are open or locked by another program (common on Windows); they are retried.

**A folder on a device shows "Receive only" but has local changes.**
In mode Receive only, changes made on the device stay there and are not sent. That is by design.
Switch the mode to Both, or remove the local changes.

**A file I deleted came back.**
Another device in mode Both still had it and sent it again before it learned of the deletion,
or the device that deleted it is in Receive only. Old versions of deleted files are on the
folder's **Versions** tab.

## Conflicts

**Why are there conflict copies at all?**
Two devices changed the same file before either got the other's change, typically a laptop that
was offline. Syncthing keeps both; Stellwerk shows them under **Conflicts**. Resolve once on the
server; every device gets the result.

**The diff shows nothing.**
The file is binary or larger than 256 KB. Compare size and time, or keep both and compare on a
device.

## Zentrale

**I forgot the admin password.**
`stellwerk set-password` on the server sets a new one (in the container:
`docker compose exec -T stellwerk stellwerk set-password <<< 'new password'`).

**The login does not stick / I am logged out right away.**
Behind HTTPS the cookie is `Secure`: the proxy must set `X-Forwarded-Proto: https`, and you must
open the UI through `https://`. Opening it over `http://` with a proxy that claims HTTPS loses
the cookie.

**Live updates do not work, the UI only refreshes every 30 seconds.**
The proxy does not pass WebSockets on `/api/ws`. See the
[proxy examples](installation.md#reverse-proxy-and-tls). Agents need the same for
`/api/agent/ws`.

**The Zentrale does not start: address in use.**
Another program uses 8080 or 22001. Change `--listen` or `--hub-listen` (and then
`--hub-address`).

## Starting over on a device

```bash
stellwerk-agent service uninstall
rm -rf ~/.local/share/stellwerk-agent     # the data directory; see the reference for macOS and Windows
```

Remove the device in the UI, then pair it again. The synced folders stay where they are; when
the device gets them again at the same paths, Syncthing matches the existing files instead of
downloading them again.

## Reporting a bug

Open an [issue](https://github.com/firsttris/stellwerk/issues/new) with the OS, the Stellwerk
version (`stellwerk --version`, `stellwerk-agent --version`), what the UI showed and the relevant
lines from the logs above.
