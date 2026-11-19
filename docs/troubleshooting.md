# Fehlersuche

## Wo die Logs liegen

| Was | Wo |
|---|---|
| Zentrale (Container) | `docker compose logs stellwerk`, `journalctl -u stellwerk` bei Quadlet |
| Zentrale (Programmdatei mit systemd) | `journalctl -u stellwerk` |
| Hub-Syncthing | `hub/stellwerk-syncthing.log` im Datenverzeichnis der Zentrale |
| Agent unter Linux | `journalctl --user -u stellwerk-agent` |
| Agent unter macOS und Windows | `agent.log` im [Datenverzeichnis des Agenten](reference.md#agent-data-directory) |
| das Syncthing des Geräts | `syncthing/stellwerk-syncthing.log` im Datenverzeichnis des Agenten |

Mehr Details: Setz `RUST_LOG=debug` für die Zentrale (ein Eintrag unter `environment:`) oder lass
den Agenten nach `service uninstall` mit `RUST_LOG=debug stellwerk-agent run` im Vordergrund
laufen.

## Koppeln

**Der Dialog zum Koppeln wartet und wartet.**
Der Agent hat die Zentrale nicht erreicht. Schau auf dem Gerät in sein Log:

- `connection to the Zentrale failed`: Die Adresse im Befehl ist vom Gerät aus nicht erreichbar.
  Der Dialog nimmt die Adresse aus der Adressleiste des Browsers; hast du die Oberfläche als
  `localhost` oder über eine Adresse geöffnet, die das Gerät nicht erreicht, koppel mit von Hand
  gesetztem `--server`.
- ein Zertifikatsfehler: Das Gerät vertraut dem Zertifikat des Proxys nicht, weil es von einer
  eigenen CA kommt (z. B. Caddys `tls internal`). Gib der Zentrale diese CA mit `--ca-cert` und
  erzeuge einen neuen Code: Der Befehl enthält dann den Fingerabdruck der CA (siehe
  [Hinter einer eigenen CA](installation.md#behind-a-private-ca)). Oder nimm ein Zertifikat einer
  öffentlichen CA.
- `the Zentrale turned the agent away`: Der Code ist verbraucht oder älter als 15 Minuten. Erzeuge
  einen neuen und führe `join` damit noch einmal aus; das Gerät behält seinen Schlüssel.

**`join` ohne `--server` findet keine Zentrale.**
mDNS funktioniert nur im selben Netzsegment und nicht aus einem Container mit Port-Mapping (siehe
[Koppeln im LAN aus einem Container](installation.md#lan-pairing-from-a-container)). Gastnetze und
manche WLAN-Access-Points blockieren Multicast. Gib stattdessen `--server` an; das Freigeben
funktioniert genauso.

**Ein Gerät erscheint nicht unter Wartende Geräte.**
Es erscheint nur, solange sein Agent läuft (`service install` nach `join`), und nur, wenn das
Koppeln im LAN an ist (`--lan-pairing`). Ein abgelehntes Gerät bleibt abgelehnt, bis die Zentrale
neu startet.

## Abgleich

**Eine Zelle zeigt lange „wartet“.**
Das Gerät ist offline, oder sein Agent läuft nicht. Es wendet die Änderung an, sobald es wieder da
ist. Auf dem Gerät: Läuft der Dienst (`systemctl --user status stellwerk-agent`, die
Aufgabenplanung unter Windows, `launchctl print gui/$(id -u)/io.github.firsttris.stellwerk.agent`
unter macOS)?

**Das Gerät ist online, aber es kommen keine Dateien an.**
Die Konfiguration ist angekommen, aber das Syncthing des Geräts erreicht den Hub nicht. Prüf vom
Gerät aus, ob die Hub-Adresse erreichbar ist (`nc -vz sync.example.org 22001`, oder
`Test-NetConnection sync.example.org -Port 22001` unter Windows):

- Der Hub-Port 22001 ist in der Firewall des Servers nicht offen oder im Container nicht
  abgebildet;
- `STELLWERK_HUB_ADDRESS` ist nicht gesetzt, also versuchen die Geräte den Hostnamen des
  Containers. Setz es auf den öffentlichen Namen und Port des Servers.

**Unter Linux stoppt der Agent, wenn ich mich abmelde.**
So funktionieren systemd-Benutzerdienste. `loginctl enable-linger $USER` hält ihn am Laufen.

**„Fehler in … auf …“ unter Braucht Aufmerksamkeit.**
Syncthing auf diesem Gerät konnte manche Dateien nicht abgleichen. Der Text sagt, warum; die
häufigsten Gründe:

- *path is not writable*: Der lokale Pfad des Ordners existiert nicht oder gehört einem anderen
  Benutzer. Ändere ihn auf der Ordnerseite, Reiter **Geräte**.
- *folder marker missing*: Das Verzeichnis des Ordners wurde gelöscht oder ein Laufwerk ist nicht
  eingehängt. Syncthing stoppt den Ordner, statt alles als gelöscht zu behandeln. Häng das
  Laufwerk ein oder setz den Pfad neu.
- Dateien, die ein anderes Programm geöffnet oder gesperrt hat (häufig unter Windows); sie werden
  erneut versucht.

**Ein Ordner auf einem Gerät steht auf „Nur empfangen“, hat aber lokale Änderungen.**
Im Modus Nur empfangen bleiben Änderungen auf dem Gerät dort und werden nicht gesendet. Das ist so
gewollt. Stell den Modus auf Beides, oder entferne die lokalen Änderungen.

**Eine Datei, die ich gelöscht habe, ist wieder da.**
Ein anderes Gerät im Modus Beides hatte sie noch und hat sie wieder gesendet, bevor es von der
Löschung erfuhr, oder das Gerät, das sie gelöscht hat, steht auf Nur empfangen. Alte Versionen
gelöschter Dateien liegen im Reiter **Versionen** des Ordners.

## Konflikte

**Warum gibt es überhaupt Konfliktkopien?**
Zwei Geräte haben dieselbe Datei geändert, bevor eines die Änderung des anderen bekommen hat,
typischerweise ein Laptop, der offline war. Syncthing behält beide; Stellwerk zeigt sie unter
**Konflikte**. Löse sie einmal auf dem Server; jedes Gerät bekommt das Ergebnis.

**Der Diff zeigt nichts.**
Die Datei ist binär oder größer als 256 KB. Vergleiche Größe und Zeit, oder behalte beide und
vergleiche auf einem Gerät.

## Zentrale

**Ich habe das Admin-Passwort vergessen.**
`stellwerk set-password` auf dem Server setzt ein neues (im Container:
`docker compose exec -T stellwerk stellwerk set-password <<< 'neues Passwort'`).

**Die Anmeldung hält nicht / ich werde sofort abgemeldet.**
Hinter HTTPS ist das Cookie `Secure`: Der Proxy muss `X-Forwarded-Proto: https` setzen, und du
musst die Oberfläche über `https://` öffnen. Öffnest du sie über `http://` mit einem Proxy, der
HTTPS behauptet, geht das Cookie verloren.

**Die Live-Aktualisierung funktioniert nicht, die Oberfläche aktualisiert sich nur alle
30 Sekunden.**
Der Proxy reicht WebSockets auf `/api/ws` nicht durch. Siehe die
[Beispiele für den Proxy](installation.md#reverse-proxy-and-tls). Die Agenten brauchen dasselbe für
`/api/agent/ws`.

**Die Zentrale startet nicht: Adresse belegt.**
Ein anderes Programm benutzt 8080 oder 22001. Ändere `--listen` oder `--hub-listen` (und dann
`--hub-address`).

## Auf einem Gerät neu anfangen

```bash
stellwerk-agent service uninstall
rm -rf ~/.local/share/stellwerk-agent     # das Datenverzeichnis; für macOS und Windows siehe die Referenz
```

Entferne das Gerät in der Oberfläche und koppel es dann neu. Die abgeglichenen Ordner bleiben, wo
sie sind; bekommt das Gerät sie wieder unter denselben Pfaden, gleicht Syncthing die vorhandenen
Dateien ab, statt sie noch einmal herunterzuladen.

## Einen Fehler melden

Eröffne ein [Issue](https://github.com/Speicherstadt/stellwerk/issues/new) mit dem Betriebssystem,
der Stellwerk-Version (`stellwerk --version`, `stellwerk-agent --version`), dem, was die Oberfläche
gezeigt hat, und den passenden Zeilen aus den Logs oben.
