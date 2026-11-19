# Referenz

Jeder Befehl, jede Option, jeder Port und jede Datei, zum Nachschlagen. Optionen lassen sich auch
als Umgebungsvariablen setzen (in Klammern); eine Option auf der Kommandozeile gewinnt gegen die
Variable.

## Zentrale { #zentrale }

```text
stellwerk [OPTIONS]                       die Zentrale starten (der Standard)
stellwerk set-password                    das Admin-Passwort setzen, gelesen von stdin
stellwerk fetch-syncthing <DIR>           das festgelegte Syncthing nach DIR herunterladen und prüfen
```

| Option | Standard | Was sie tut |
|---|---|---|
| `--data-dir` (`STELLWERK_DATA`) | `data` (`/data` im Image) | Datenbank, Hub-Syncthing und die Kopie jedes Ordners auf dem Hub; für jeden Befehl |
| `--listen` (`STELLWERK_LISTEN`) | `0.0.0.0:8080` | Adresse der Weboberfläche, der API und des WebSockets der Agenten |
| `--hub-listen` (`STELLWERK_HUB_LISTEN`) | `tcp://0.0.0.0:22001` | wo das Hub-Syncthing Sync-Verbindungen annimmt |
| `--hub-address` (`STELLWERK_HUB_ADDRESS`) | `tcp://<Hostname>:<Hub-Port>` | wo die Geräte den Hub erreichen; in einem Container und hinter NAT setzen |
| `--public-url` (`STELLWERK_PUBLIC_URL`) | – | die Adresse, die die Geräte für den Webport benutzen, z. B. `https://sync.example.org`; wird im LAN angekündigt |
| `--lan-pairing` (`STELLWERK_LAN_PAIRING`) | `true` | die Zentrale über mDNS ankündigen und Geräte ohne Code auf Freigabe warten lassen; `false` schaltet beides aus |
| `--syncthing` (`STELLWERK_SYNCTHING`) | das festgelegte Release herunterladen | stattdessen diese Syncthing-Programmdatei benutzen (das Image setzt `/opt/syncthing/syncthing`) |
| `--ca-cert` (`STELLWERK_CA_CERT`) | – | das Zertifikat (PEM) einer eigenen CA hinter dem HTTPS der Zentrale; bereitgestellt unter `/ca.crt`, sein Fingerabdruck kommt in den Befehl zum Koppeln ([eigene CA](installation.md#behind-a-private-ca)) |
| `--version`, `--help` | | |

`set-password` liest eine Zeile von der Standardeingabe, speichert ihren argon2id-Hash und meldet
jede Sitzung ab.

### Datenverzeichnis der Zentrale

| Pfad | Inhalt |
|---|---|
| `stellwerk.db` | SQLite-Datenbank: Admin, Sitzungen, Geräte, Ordner, Freigaben, Codes zum Koppeln, Aktivität |
| `hub/` | Home des Hub-Syncthing: `cert.pem`/`key.pem` (seine Identität), `config.xml`, Index-Datenbank, `stellwerk-syncthing.log` |
| `folders/<Syncthing-Ordner-ID>/` | die Kopie jedes Ordners auf dem Hub; alte Versionen in `.stversions/` darin |
| `syncthing-bin/` | das heruntergeladene und geprüfte Syncthing-Release |

## Agent

```text
stellwerk-agent join [--server URL] [--ca-sha256 FINGERPRINT] [CODE]
                                             dieses Gerät koppeln
stellwerk-agent run [--name NAME] [--syncthing PATH]
                                             im Vordergrund laufen
stellwerk-agent service install              ab der Anmeldung im Hintergrund laufen
stellwerk-agent service uninstall            den Hintergrunddienst stoppen und entfernen
```

| Option | Gilt für | Was sie tut |
|---|---|---|
| `--data-dir` (`STELLWERK_AGENT_DATA`) | alle | wo der Agent seinen Schlüssel, sein Syncthing und dessen Datenbank ablegt |
| `--server URL` | `join` | die Zentrale, `https://…` oder `http://…`; ohne Angabe über mDNS gefunden |
| `CODE` | `join` | der Einmal-Code aus **Gerät koppeln**; ohne ihn wartet das Gerät auf Freigabe |
| `--ca-sha256 FINGERPRINT` | `join` | die Zentrale steht hinter einer eigenen CA mit diesem SHA-256-Fingerabdruck: die CA von `/ca.crt` holen, behalten, wenn der Fingerabdruck stimmt, und ihr von da an für die Zentrale vertrauen |
| `--name NAME` | `run` | der Gerätename, der der Zentrale beim Koppeln vorgeschlagen wird; standardmäßig der Hostname. Nur für `run` im Vordergrund: Der Dienst schlägt immer den Hostnamen vor. Später benennst du das Gerät in der Oberfläche um |
| `--syncthing PATH` (`STELLWERK_SYNCTHING`) | `run` | diese Syncthing-Programmdatei benutzen, statt das festgelegte herunterzuladen |

`join` erzeugt den Ed25519-Schlüssel des Geräts, speichert ihn mit Server und Code in
`agent.json` und zeigt den Fingerabdruck des Schlüssels; mit `--ca-sha256` speichert es dort auch
die CA der Zentrale. Ein erneutes `join` ersetzt Server und Code, behält aber den Schlüssel (und
die CA, beim selben Server); mit einem neuen Code kommt ein schon einmal gekoppeltes Gerät als
dasselbe Gerät zurück. Gibt es `agent.json`, lässt sie sich aber nicht lesen, hört `join` auf,
statt einen neuen Schlüssel zu erzeugen; lösch die Datei, um das Gerät als neues zu koppeln.

`service install` richtet den Dienst für die Programmdatei ein, aus der es aufgerufen wird, und
für das Datenverzeichnis, das es bekommt, und startet ihn. Führ es nach dem Verschieben der
Programmdatei noch einmal aus, oder um den Agenten neu zu starten.

Unter Windows gibt es ein zweites Programm, `stellwerk-agent-background.exe`: derselbe Agent ohne
Konsolenfenster, den die Aufgabe bei der Anmeldung startet. Es führt `run` aus und schreibt sein
Log in `agent.log`.

### Datenverzeichnis des Agenten { #agent-data-directory }

Standardmäßig:

| System | Pfad |
|---|---|
| Linux | `~/.local/share/stellwerk-agent` |
| macOS | `~/Library/Application Support/stellwerk-agent` |
| Windows | `%LOCALAPPDATA%\stellwerk-agent` |

| Pfad | Inhalt |
|---|---|
| `agent.json` | die Adresse der Zentrale, der geheime Schlüssel des Geräts und, bis zum Koppeln, der Code; nur für den Benutzer lesbar |
| `syncthing/` | Home des Syncthing des Agenten: Identität, Konfiguration, Index-Datenbank |
| `syncthing-bin/` | das heruntergeladene und geprüfte Syncthing-Release |
| `agent.log` | das Log unter macOS und Windows (unter Linux geht es ins Journal) |
| `stop` | von `service uninstall` geschrieben, um einen laufenden Agenten zum Stoppen aufzufordern; von ihm entfernt |
| `ignore.d/*.list` | Ausnahmen, die andere Programme auf diesem Gerät schreiben (unten) |

Die abgeglichenen Ordner liegen nicht im Datenverzeichnis, sondern dort, wohin ihre Pfade zeigen
(standardmäßig `~/Stellwerk/<Ordner>`).

### Ausnahmen anderer Programme { #exclusions-from-other-programs }

Ein Programm, das manche Dateien selbst überträgt und bei dem Syncthing sie nicht auch noch
schreiben darf, legt eine Liste in `ignore.d/<programm>.list` im Datenverzeichnis des Agenten ab:
ein absoluter Pfad pro Zeile, `#` leitet einen Kommentar ein. Pfade werden genommen, wie sie
dastehen, und einer mit einem Leerzeichen am Anfang oder Ende bekommt kein Muster, weil Syncthing
Musterzeilen trimmt. Spielstand schreibt `spielstand.list` mit den Spielstanddateien, die es
abgleicht.

Der Agent fügt jeden gelisteten Pfad, der in einem Ordner liegt, den Ignoriermustern dieses
Ordners hinzu, als ein Muster ab der Wurzel des Ordners, das genau diesen Pfad trifft (Zeichen, die
Syncthings Muster besonders lesen, werden maskiert, mit `\` oder unter Windows `|`). Ein Pfad, der
den Ordner über einen Link erreicht, zählt als darin. Das Muster steht vor der ersten Zeile mit `!`
oder `#include`, weil Syncthing beim ersten passenden Muster aufhört. Der Agent wendet die Listen
mit jedem Soll-Zustand an und liest sie alle 10 Sekunden neu, so wirkt eine geänderte Liste ohne
die Zentrale; schlägt das Anwenden fehl, versucht er es nach einer Pause wieder, die bis auf eine
Stunde wächst. Die Listen bleiben auf dem Gerät: Die Muster von der Zentrale sind unverändert, und
jedes andere Gerät bekommt dieselben Ausnahmen aus seiner eigenen Liste.

## Logs

Beide Programme loggen auf die Standardfehlerausgabe, standardmäßig auf Stufe `info`. `RUST_LOG`
ändert das, z. B. `RUST_LOG=debug` oder `RUST_LOG=stellwerk_engine=debug,info`. Die
Syncthing-Instanzen loggen in `stellwerk-syncthing.log` in ihrem Home-Verzeichnis; das Log des
vorigen Starts bleibt als `stellwerk-syncthing.log.1`. Der Agent warnt einmal vor einem
Verbindungsproblem; solange sich dasselbe Problem wiederholt, loggt er jeden weiteren Versuch nur
auf Stufe `debug`, so wächst `agent.log` nicht, während die Zentrale nicht erreichbar ist.

## Ports

| Port | Wo | Was |
|---|---|---|
| 8080/tcp | Zentrale | Weboberfläche, REST-API (`/api/…`), WebSocket der Oberfläche (`/api/ws`), WebSocket der Agenten (`/api/agent/ws`) |
| 22001/tcp | Zentrale | Sync-Port des Hub-Syncthing |
| 5353/udp | Zentrale, Agent während `join` | mDNS: `_stellwerk._tcp` |
| zufällig, 127.0.0.1 | Server und Geräte | die GUI/REST-API jedes Syncthing, mit einem neuen API-Schlüssel bei jedem Start; nur lokal erreichbar |
| zufällig, alle Schnittstellen | Geräte | der Listen-Port des Syncthing des Agenten; nie angewählt, da die Geräte den Hub anwählen |

## Zeiten und Grenzen

| Was | Wert |
|---|---|
| Code zum Koppeln | gilt 15 Minuten, funktioniert einmal; nur sein Hash wird gespeichert |
| API-Token | bis zum Widerruf oder bis ein neues Admin-Passwort gesetzt wird; nur sein Hash wird gespeichert |
| Anmeldesitzung | 30 Tage; Anmeldungen werden nacheinander geprüft, und eine fehlgeschlagene Anmeldung wartet eine halbe Sekunde, doppelt so lang mit jedem weiteren Fehlschlag in Folge, bis 8 s |
| wartende Geräte (Koppeln im LAN) | höchstens 20, im Speicher, weg, wenn der Agent die Verbindung trennt oder die Zentrale neu startet |
| Neuverbindung des Agenten | nach 1 s, verdoppelt bis 30 s; 60 s nach einer Abweisung |
| Heartbeat des Agenten | Agent und Zentrale pingen je alle 30 s; eine Verbindung, die 90 s still ist, gilt als verloren |
| erneuter Abgleich des Hubs | nach einem Fehler 1 s, verdoppelt bis 60 s, bis es klappt oder sich die Revision ändert |
| Live-Status von einem Gerät | bei Änderung, höchstens etwa einmal pro Sekunde |
| Aktivität „hat N Dateien geändert“ | gezählt je Gerät und Ordner über 30 Sekunden |
| Suche nach Konflikten auf dem Hub | alle 30 Sekunden |
| Diff eines Konflikts | Textdateien bis 256 KB |
| Versionen auf dem Hub | gestaffelt, 365 Tage aufbewahrt |
| Standardpfad einer neuen Freigabe | `~/Stellwerk/<Ordnername>` |
| Suche auf einem Gerät | 8 Ebenen tief, 50 Treffer, 10 s auf dem Gerät; die Zentrale wartet 15 s |
| Antwort der Ordnerauswahl | die Zentrale wartet 5 s |

## REST-API

Die Weboberfläche benutzt die API unter `/api`; sie ist noch keine stabile öffentliche
Schnittstelle. Jede Anfrage außer `/api/session` und `/api/login` braucht das Sitzungs-Cookie, und
jede Änderung zusätzlich den Header `X-CSRF-Token` mit dem Token aus der Anmeldung. Skripte
benutzen stattdessen ein [API-Token](user-guide.md#api-tokens): `Authorization: Bearer stw_…`,
kein CSRF-Token. Die Endpunkte, und welche davon ein Token aufrufen darf, stehen in der
[Architektur](architecture.md#login-and-api). KI-Assistenten benutzen den
[MCP-Server](user-guide.md#claude-code-and-other-assistants-mcp) unter `/mcp` mit denselben
Tokens.
