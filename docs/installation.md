# Installation

Stellwerk hat zwei Teile zum Installieren: die **Zentrale** einmal auf einem Server und den
**Agenten** auf jedem Gerät. Diese Seite beschreibt beides für den echten Betrieb: TLS, Firewall,
Backups, Updates. Für einen schnellen ersten Versuch sind die [Ersten Schritte](getting-started.md)
kürzer.

Keinen eigenen Server oder keine eigene IT? Stellwerk gibt es auf Anfrage gehostet oder über einen
IT-Dienstleister, siehe [Gehostet oder über einen Partner](hosted.md).

Jedes [Release](https://github.com/Speicherstadt/stellwerk/releases) veröffentlicht das Image
`tristanteu/stellwerk` auf Docker Hub und `ghcr.io/speicherstadt/stellwerk` und enthält die
Agenten. Releases von vor dem Umzug in die Organisation Speicherstadt bleiben unter
`ghcr.io/firsttris/stellwerk`, das keine neuen Versionen mehr bekommt; wechsle zu Docker Hub oder
zum neuen Namen. Wie du sie selbst baust, steht unter [Entwicklung](development.md#building).

## Voraussetzungen

**Server**

- Linux auf `amd64` oder `arm64`, mit Docker oder Podman, oder die einzelne Programmdatei.
- Platz für eine vollständige Kopie jedes Ordners samt Versionen: Der Hub behält alte Versionen
  geänderter und gelöschter Dateien standardmäßig ein Jahr.
- Von den Geräten auf zwei Ports erreichbar:

| Port | Protokoll | Benutzt von | Muss erreichbar sein |
|---|---|---|---|
| 8080 (Standard für `--listen`) | HTTP, WebSocket | Weboberfläche, Verbindung der Agenten zur Zentrale | von den Geräten und deinem Browser; von außen nur über einen TLS-Proxy (443) |
| 22001 (Standard für `--hub-listen`) | TCP (Syncthing, TLS) | Dateiübertragung zwischen den Geräten und dem Hub | von den Geräten, direkt |
| 5353 | UDP-Multicast (mDNS) | Koppeln im LAN ohne Code | nur im LAN; optional |

Der Hub nutzt 22001 und nicht den üblichen Syncthing-Port 22000, so behält ein Syncthing, das
schon auf dem Server läuft, seinen Port.

**Geräte**

- Linux (`x86_64`, `aarch64`; statische Builds, die auf jeder Distribution laufen), Windows 10
  oder 11 (`x86_64`), macOS (Apple Silicon und Intel).
- Ausgehende Verbindungen zum Server auf den beiden Ports oben. Die Geräte brauchen **keinen**
  offenen Port; der Hub wählt sie nie an.
- Keine Administratorrechte: Der Agent läuft als normaler Benutzer.

## Zentrale als Container

Das Image enthält die Zentrale mit der Weboberfläche und das festgelegte Syncthing. Es läuft als
Benutzer ohne Rechte (UID 65532), legt alles unter `/data` ab und fährt bei `SIGTERM` sauber
herunter, das Hub-Syncthing mit.

### Docker Compose

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk:latest
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:8080"   # Weboberfläche und Agenten, nur für den TLS-Proxy auf diesem Host
      - "22001:22001"           # Sync-Port des Hubs, von den Geräten erreichbar
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
docker compose exec -T stellwerk stellwerk set-password <<< 'ein langes Passwort'
```

`STELLWERK_HUB_ADDRESS` sagt den Geräten, wo sie den Hub erreichen. In einem Container ist der
Hostname der des Containers, setz die Adresse also immer. Bildest du den Hub auf einen anderen
Port nach außen ab, gehört dieser Port in die Adresse.

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

`/etc/containers/systemd/stellwerk.volume` enthält nur `[Volume]`. Dann:

```bash
systemctl daemon-reload && systemctl start stellwerk
podman exec -i systemd-stellwerk stellwerk set-password <<< 'ein langes Passwort'
```

`AutoUpdate=registry` lässt `podman auto-update` (oder seinen Timer) neue Images holen.

### Koppeln im LAN aus einem Container { #lan-pairing-from-a-container }

mDNS braucht die Zentrale im selben Netzsegment wie die Geräte. Mit Port-Mapping verlässt die
Ankündigung den Container nicht; nimm dafür das Host-Netzwerk (`network_mode: host` in Compose,
`Network=host` in Quadlet, und lass die Port-Mappings weg). Ohne das gib `--server` an
`stellwerk-agent join` mit; ein Gerät ohne Code freigeben funktioniert genauso.

## Zentrale als Programmdatei

Die Zentrale ist eine Datei, `stellwerk`, mit eingebauter Oberfläche. Sie lädt das festgelegte
Syncthing beim ersten Start in ihr Datenverzeichnis herunter oder nimmt eines, das du ihr mit
`--syncthing` gibst.

```bash
stellwerk --data-dir /srv/stellwerk set-password      # einmal; liest das Passwort von stdin
stellwerk --data-dir /srv/stellwerk \
  --listen 127.0.0.1:8080 \
  --hub-listen tcp://0.0.0.0:22001 \
  --hub-address tcp://sync.example.org:22001 \
  --public-url https://sync.example.org
```

Als systemd-Dienst, `/etc/systemd/system/stellwerk.service`:

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

Jede Option steht in der [Referenz](reference.md#zentrale).

## Reverse Proxy und TLS { #reverse-proxy-and-tls }

Über den Webport laufen das Admin-Passwort, das Sitzungs-Cookie und die Anmeldung der Geräte. Stell
ihn hinter einen Reverse Proxy mit HTTPS, bevor er von außerhalb deines eigenen Netzes erreichbar
ist. Der Proxy muss:

- WebSockets auf `/api/ws` (die Live-Aktualisierung der Oberfläche) und `/api/agent/ws` (die
  Agenten) durchreichen;
- `X-Forwarded-Proto: https` setzen, damit die Zentrale das Sitzungs-Cookie als `Secure` markiert.

Der Hub-Port 22001 geht **nicht** über den Proxy: Syncthing verschlüsselt ihn selbst mit TLS und
festen Geräte-IDs.

=== "Caddy"

    ```caddy
    sync.example.org {
        reverse_proxy 127.0.0.1:8080
    }
    ```

    Caddy holt das Zertifikat, reicht WebSockets durch und setzt `X-Forwarded-Proto` von selbst.

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
            proxy_read_timeout 1h;   # die WebSockets bleiben offen
        }
    }
    map $http_upgrade $connection_upgrade { default upgrade; '' close; }
    ```

=== "Traefik (Compose-Labels)"

    ```yaml
        labels:
          - traefik.enable=true
          - traefik.http.routers.stellwerk.rule=Host(`sync.example.org`)
          - traefik.http.routers.stellwerk.entrypoints=websecure
          - traefik.http.routers.stellwerk.tls.certresolver=letsencrypt
          - traefik.http.services.stellwerk.loadbalancer.server.port=8080
    ```

    Traefik reicht WebSockets durch und setzt `X-Forwarded-Proto` von selbst. Lass das
    Port-Mapping für `8080` dann weg.

Steht der Proxy, koppelst du Geräte mit `--server https://sync.example.org`. Der Dialog zum
Koppeln nimmt die Adresse aus dem Browser, öffne die Oberfläche also über den Proxy. Ein
Zertifikat von einer eigenen CA braucht `--ca-cert`, siehe
[Hinter einer eigenen CA](#behind-a-private-ca).

## Firewall

Öffne auf dem Server:

- **443/tcp** zum Proxy (oder 8080/tcp, wenn du ohne Proxy im LAN bleibst);
- **22001/tcp** zum Hub;
- **5353/udp** nur im LAN, wenn du ohne Code koppeln willst.

Local Discovery, Global Discovery und Relays von Syncthing sind auf dem Hub und auf jedem Agenten
aus: Die Geräte verbinden sich direkt mit der Hub-Adresse, mit nichts anderem.

## Agent auf einem Gerät { #agent-on-a-device }

**Gerät koppeln** in der Oberfläche zeigt einen Befehl mit einem frischen Code und der Version der
Zentrale anstelle von `X.Y.Z`; führe ihn auf dem Gerät in einem Terminal als normaler Benutzer
aus:

=== "Linux, macOS"

    ```bash
    curl -fsSL https://github.com/Speicherstadt/stellwerk/releases/download/vX.Y.Z/install.sh | sh -s -- https://sync.example.org K7Q2-MX9P-41TZ
    ```

=== "Windows (PowerShell)"

    ```powershell
    & ([scriptblock]::Create((irm https://github.com/Speicherstadt/stellwerk/releases/download/vX.Y.Z/install.ps1))) https://sync.example.org K7Q2-MX9P-41TZ
    ```

Das Skript (`installer/install.sh`, `install.ps1`) hängt an jedem Release; der Dialog nimmt das
der eigenen Version der Zentrale. Es lädt den Agenten dieser Version aus dem Release herunter,
prüft ihn gegen die `SHA256SUMS` des Releases, koppelt ihn und installiert den Dienst: Unter Linux
und macOS kommt die Programmdatei nach `~/.local/bin/stellwerk-agent`, unter Windows läuft der
[Installer](#windows-installer) ohne Rückfragen. Ohne den Code wartet das Gerät auf die Freigabe
in der Oberfläche. Führst du den Befehl auf einem gekoppelten Gerät noch einmal aus, wird der Agent
aktualisiert und die Kopplung bleibt. Das Gerät muss `github.com` erreichen; ohne das installierst
du von Hand wie unten.

### Hinter einer eigenen CA { #behind-a-private-ca }

Kommt das Zertifikat des Reverse Proxys von einer eigenen CA, zum Beispiel von Caddys
`tls internal` oder von step-ca, vertraut ihm noch kein Gerät: `curl` und der Agent lehnen die
Verbindung ab. Gib der Zentrale das Zertifikat der CA mit `--ca-cert` (`STELLWERK_CA_CERT`). Sie
stellt die CA unter `/ca.crt` bereit, und der Dialog zum Koppeln fügt dem Befehl den
SHA-256-Fingerabdruck der CA hinzu (`--ca-sha256 …`, unter Windows `-CaSha256 …`). Der Agent holt
die CA von der Zentrale, behält sie nur, wenn der Fingerabdruck stimmt, und vertraut ihr von da an,
neben den Stammzertifikaten des Systems; auf dem Gerät muss nichts von Hand installiert werden. Der
Fingerabdruck kommt aus der Oberfläche, die du über eine Verbindung geöffnet hast, der du schon
vertraust, so kann ein Rechner dazwischen keine eigene CA unterschieben.

Mit Caddy in Docker Compose gibst du Stellwerk das Stammzertifikat von Caddy:

```yaml
services:
  stellwerk:
    # …
    environment:
      STELLWERK_CA_CERT: /caddy/pki/authorities/local/root.crt
    volumes:
      - stellwerk:/data
      - caddy_data:/caddy:ro    # das Volume, in dem Caddy /data ablegt
```

`openssl x509 -in root.crt -noout -fingerprint -sha256` zeigt denselben Fingerabdruck wie der
Dialog, ohne die Doppelpunkte.

### Von Hand { #by-hand }

Lade den Agenten für das Gerät aus dem Release herunter oder bau ihn. Leg ihn dort ab, wo er
bleiben kann: Der Dienst startet ihn von diesem Ort.

=== "Linux"

    ```bash
    mkdir -p ~/.local/bin
    install -m 755 stellwerk-agent-linux-x86_64 ~/.local/bin/stellwerk-agent
    stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
    stellwerk-agent service install
    ```

    Ein Benutzerdienst stoppt, wenn sich der Benutzer abmeldet. Damit der Agent ohne Anmeldung
    weiterläuft (ein Server, ein PC ohne Bildschirm), erlaube einmal Lingering:
    `loginctl enable-linger $USER` (auf manchen Distributionen mit `sudo`).

=== "macOS"

    ```bash
    mkdir -p ~/.local/bin
    install -m 755 stellwerk-agent-macos-aarch64 ~/.local/bin/stellwerk-agent
    xattr -d com.apple.quarantine ~/.local/bin/stellwerk-agent 2>/dev/null
    ~/.local/bin/stellwerk-agent join --server https://sync.example.org K7Q2-MX9P-41TZ
    ~/.local/bin/stellwerk-agent service install
    ```

    Die Programmdatei ist noch nicht notarisiert; entfernst du das Quarantäne-Flag, startet macOS
    sie. Nimm `-macos-x86_64` auf einem Intel-Mac.

=== "Windows (zip)"

    Entpacke `stellwerk-agent-windows-x86_64.zip` nach `%LOCALAPPDATA%\Programs\Stellwerk`; darin
    liegen `stellwerk-agent.exe` und `stellwerk-agent-background.exe`. In PowerShell in diesem
    Ordner:

    ```powershell
    .\stellwerk-agent.exe join --server https://sync.example.org K7Q2-MX9P-41TZ
    .\stellwerk-agent.exe service install
    ```

### Windows-Installer { #windows-installer }

`stellwerk-agent-setup-windows-x86_64.exe` erledigt all das in einem Durchgang, für den aktuellen
Benutzer und ohne Administratorrechte: Er installiert beide Programme nach
`%LOCALAPPDATA%\Programs\Stellwerk`, fragt nach der Adresse der Zentrale und dem Code (leer für das
Koppeln im LAN), koppelt das Gerät und richtet die Aufgabe ein. **Einstellungen → Apps → Stellwerk
Agent → Deinstallieren** stoppt und entfernt ihn; das Datenverzeichnis bleibt, so bleibt beim
erneuten Installieren die Kopplung erhalten.

Ohne Rückfragen, z. B. für ein Verteilungsskript:

```powershell
stellwerk-agent-setup-windows-x86_64.exe /S /SERVER=https://sync.example.org /CODE=K7Q2-MX9P-41TZ
```

Der Installer ist noch nicht codesigniert, SmartScreen kann also beim ersten Start warnen
(**Weitere Informationen → Trotzdem ausführen**).

### Im Hintergrund laufen { #running-in-the-background }

`stellwerk-agent service install` startet den Agenten jetzt und ab jeder Anmeldung, als der
aktuelle Benutzer. `service uninstall` stoppt den Agenten und sein Syncthing und entfernt den
Dienst; das Datenverzeichnis bleibt. `stellwerk-agent run` lässt ihn stattdessen im Vordergrund
laufen, z. B. um sein Log zu verfolgen.

| System | Dienst | Log |
|---|---|---|
| Linux | systemd-Benutzer-Unit `~/.config/systemd/user/stellwerk-agent.service` | `journalctl --user -u stellwerk-agent` |
| macOS | launchd-Agent `~/Library/LaunchAgents/io.github.firsttris.stellwerk.agent.plist` | `agent.log` im Datenverzeichnis |
| Windows | Aufgabe `Stellwerk\Agent` in der Aufgabenplanung bei der Anmeldung, startet `stellwerk-agent-background.exe` (ohne Fenster) | `agent.log` im Datenverzeichnis |

Der Dienst merkt sich den Pfad der Programmdatei und das Datenverzeichnis, mit denen er
installiert wurde. Hast du die Programmdatei verschoben, führe `service install` vom neuen Ort aus
noch einmal aus.

### Den Agenten von einem Gerät entfernen

1. `stellwerk-agent service uninstall` (oder die Windows-App deinstallieren).
2. Lösch die Programmdatei und, um die Kopplung zu vergessen, das Datenverzeichnis
   ([wo es liegt](reference.md#agent-data-directory)).
3. In der Oberfläche **Geräte → Entfernen …**. Die abgeglichenen Ordner bleiben als normale
   Dateien auf dem Gerät.

## Aktualisieren { #updating }

**Zentrale.** Hol das neue Image und erzeuge den Container neu (`docker compose pull && docker
compose up -d`, oder `podman auto-update`). Die Datenbank wird beim Start migriert. Die Agenten
verbinden sich innerhalb von 30 Sekunden von selbst neu.

**Agenten.** Unter Linux und macOS zeigt **Geräte** neben einem Agenten, der älter ist als die
Zentrale, „Update verfügbar“ mit einem Knopf, der ihn aktualisiert: Der Agent lädt die Version der
Zentrale aus dem GitHub-Release herunter, prüft sie gegen `SHA256SUMS` und ob sie läuft, ersetzt
sich selbst, und der Dienst startet ihn neu. Das Gerät braucht also Zugang zu github.com. Der Agent
muss als Dienst laufen, und die erste Version mit diesem Knopf muss einmal von Hand installiert
werden. Von Hand, und unter Windows immer: Ersetze die Programmdatei am selben Ort durch die neue
(unter Windows: führe den neuen Installer aus) und führe `stellwerk-agent service install` noch
einmal aus, das startet ihn neu. Agent und Zentrale benachbarter Versionen arbeiten zusammen, mit
einer Ausnahme: Agenten bis v0.1.1 melden sich auf die ältere Art an (nur über die Nonce), die
neuere Zentralen abweisen; aktualisiere diese Agenten also zusammen mit der Zentrale. Die
Geräteliste zeigt die Version jedes Agenten.

**Syncthing.** Jede Stellwerk-Version legt eine Syncthing-Version mit ihren Prüfsummen fest. Sie
wird zusammen mit Stellwerk aktualisiert, nie für sich allein; das eigene Auto-Upgrade von
Syncthing ist aus.

## Sichern und wiederherstellen

Alles, was die Zentrale weiß, liegt in ihrem Datenverzeichnis (`/data` im Container):

| Pfad | Enthält | Sichern |
|---|---|---|
| `stellwerk.db` | Geräte, Ordner, Freigaben, Sitzungen, Aktivität (SQLite) | **ja**: die Konfiguration |
| `hub/` | die Identität des Hub-Syncthing (`cert.pem`, `key.pem`), Konfiguration und Index | **ja**: die Identität; der Index wird neu aufgebaut, wenn er fehlt |
| `folders/` | die Kopie jedes Ordners auf dem Hub, mit `.stversions` | wie es deine Daten brauchen; die Geräte haben die aktuellen Dateien, nur der Server hat die Versionen |
| `syncthing-bin/` | das heruntergeladene Syncthing | nein |

Am einfachsten sicherst du konsistent bei gestoppter Zentrale:

```bash
docker compose stop stellwerk
docker run --rm -v stellwerk_stellwerk:/data -v "$PWD":/backup alpine \
  tar czf /backup/stellwerk-$(date +%F).tgz -C /data stellwerk.db hub
docker compose start stellwerk
```

Während sie läuft, nimmt `sqlite3 /data/stellwerk.db ".backup /backup/stellwerk.db"` eine
konsistente Kopie der Datenbank.

**Wiederherstellen**: Leg die Dateien vor dem ersten Start in ein leeres Datenverzeichnis zurück.
Fehlt die Identität des Hubs, bekommt der Hub eine neue Geräte-ID; die Zentrale gibt sie mit dem
nächsten Soll-Zustand an jeden Agenten weiter, so folgen die Geräte ohne neues Koppeln, und der Hub
holt die Ordner wieder von ihnen.

**Umzug auf einen anderen Server**: sichern, auf dem neuen wiederherstellen, den DNS-Namen auf ihn
zeigen lassen. Ändert sich die Hub-Adresse, setz das neue `STELLWERK_HUB_ADDRESS`; die Agenten
bekommen es mit ihrem nächsten Soll-Zustand. Ändert sich die Webadresse, benutzen die Agenten
weiter die alte aus `agent.json`: Koppel sie neu, oder ändere `server` in `agent.json` und starte
den Agenten neu.

## Das Admin-Passwort zurücksetzen

Auf dem Server: `stellwerk set-password` (im Container: `docker compose exec -T stellwerk
stellwerk set-password <<< 'neues Passwort'`). Es meldet jede Sitzung ab.
