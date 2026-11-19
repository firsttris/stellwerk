# Entwicklung

Wie das Repository aufgebaut ist, wie du Stellwerk aus dem Quellcode baust und startest, wie es
getestet wird und wie ein Release entsteht. Die Regeln für Änderungen stehen in
[CLAUDE.md](https://github.com/Speicherstadt/stellwerk/blob/main/CLAUDE.md); sie gelten auch für
Menschen.

## Repository

| Pfad | Was |
|---|---|
| `crates/protocol` | die Nachrichten zwischen Agent und Zentrale und der Soll-Zustand; kennt Geräte, Ordner, Modi und Pfade, nicht Syncthing |
| `crates/engine` | alles rund um Syncthing: die festgelegte Version, Download, Start, REST-Client, Reconciler, Monitor, Konflikt- und Versionsdateien |
| `crates/agent` | `stellwerk-agent`: Koppeln, die Verbindung zur Zentrale, der Hintergrunddienst; `src/bin/background.rs` ist das Windows-Programm ohne Konsole |
| `crates/zentrale` | `stellwerk`: axum-Server, SQLite-Speicher mit Migrationen, Ableitung des Soll-Zustands, Hub, Live-Status, Koppeln, mDNS, die eingebettete Oberfläche |
| `web/` | die Weboberfläche: React (gebaut mit der TanStack-Redact-Runtime), TanStack Router und Query, Tailwind, Paraglide; `src/demo/` enthält die Demodaten |
| `e2e/` | Playwright-Tests und die Screenshots, gegen die Oberfläche im Demomodus |
| `docker/Dockerfile` | das Image der Zentrale |
| `installer/windows/` | das NSIS-Skript des Windows-Installers |
| `scripts/` | die Smoke-Tests für Dienst und Installer, die die CI auf echten Systemen ausführt |
| `docs/` | diese Dokumentation (MkDocs Material) |

## Voraussetzungen

- Rust stable (der Workspace braucht mindestens 1.94, Edition 2024)
- [Bun](https://bun.com) (die Version in `packageManager` der `package.json`): installiert die
  Pakete und führt die Skripte, Vite und den Playwright-Runner aus
- für die Doku: Python und `pip install -r requirements-docs.txt`

## Bauen { #building }

```bash
bun install
bun run --filter web build       # web/dist, eingebettet in den Release-Build der Zentrale
cargo build --release            # target/release/stellwerk und stellwerk-agent
docker build -f docker/Dockerfile -t tristanteu/stellwerk .   # das Image, mit Oberfläche
```

Ein Release-Build der Zentrale, der gebaut wurde, bevor es `web/dist` gab, beantwortet jede Seite
mit dem Hinweis, zuerst die Oberfläche zu bauen. Debug-Builds lesen `web/dist` zur Laufzeit von
der Platte.

## Lokal starten

**Nur die Oberfläche**, mit Demodaten, ohne Rust oder Syncthing:

```bash
bun run dev:demo                 # http://localhost:5173
```

**Eine echte Zentrale und ein Agent** auf einem Rechner, mit Ports, die einem Syncthing, das du
selbst betreibst, nicht in die Quere kommen:

```bash
# einmal: das Admin-Passwort
cargo run -p stellwerk-zentrale -- --data-dir /tmp/stw/zentrale set-password <<< 'dev'

# Terminal 1: die Zentrale (:8080) und die Oberfläche mit Hot Reload (:5173, /api an :8080 weitergeleitet)
bun run dev

# Terminal 2: ein Agent, nach „Gerät koppeln“ in der Oberfläche
cargo run -p stellwerk-agent -- --data-dir /tmp/stw/agent join --server http://127.0.0.1:8080 <CODE>
cargo run -p stellwerk-agent -- --data-dir /tmp/stw/agent run --name dev-agent
```

`bun run dev` führt `bun run --parallel dev:server dev:web` aus: `dev:server` ist
`cargo run -p stellwerk-zentrale -- --data-dir /tmp/stw/zentrale --listen 127.0.0.1:8080
--hub-listen tcp://127.0.0.1:22101 --lan-pairing false`, `dev:web` der Vite-Dev-Server
(`bun run --cwd web dev`; unter `bun run --filter` würde er nicht mit dem anderen stoppen). Beide
laufen nebeneinander, jede Zeile mit dem Namen ihres Skripts davor, und beendet sich einer mit
einem Fehler, wird der andere gestoppt. Strg+C, Strg+Z und das Schließen des Terminals erreichen
beide, da das Terminal die ganze Vordergrund-Prozessgruppe benachrichtigt; `fg` setzt sie nach
Strg+Z fort. In einem Terminal behalten beide ihre Farben (`FORCE_COLOR`, `CARGO_TERM_COLOR`).

Zwei Agenten auf einem Rechner brauchen zwei Datenverzeichnisse. Starte nie ein Test-Syncthing auf
8384 oder 22000, und fass nie ein Syncthing an, das du nicht selbst gestartet hast.

## Tests

| Befehl | Was läuft |
|---|---|
| `cargo test` | die Unit-Tests jedes Crates und die Integrationstests in `crates/*/tests` gegen echte Syncthing-Instanzen (die festgelegte Version wird einmal nach `.syncthing-cache/` heruntergeladen) |
| `bun run --filter web check` | Biome, `tsc --noEmit`, Vitest, `vite build` |
| `bun checks` | Dateien über 400 Zeilen, die Oberflächentexte und Import-Ringe zwischen Modulen (den Ordnern von `web/src`), mit den [Checks aus firsttris/workflows](https://github.com/firsttris/workflows#checks) und `checks.json`; ältere große Dateien und Texte stehen in `.checks/` und dürfen nur weniger werden. `bun checks pr` gibt Hinweise auf dem Branch, etwa zu einer Funktion, die neu oder über 80 Zeilen gewachsen ist |
| `bun run e2e` | Playwright gegen den Demo-Build, Runner und Vite auf Bun (`bun --bun playwright test`); `PW_CHROMIUM=/pfad/zu/chrome` außerhalb des Playwright-Images, `PW_PORT`, wenn 4173 belegt ist |
| `zensical build --strict` | die Doku, mit kaputten Links als Fehler |

Die Integrationstests sind die Meilensteine der Roadmap:

| Test | Zeigt |
|---|---|
| `engine/tests/phase0_gate.rs` | der Reconciler richtet zwei Syncthings ein und eine Datei kommt an |
| `zentrale/tests/pairing.rs` | ein Gerät koppelt sich mit einem Code und gleicht mit dem Hub ab |
| `zentrale/tests/lan_pairing.rs` | ein Gerät ohne Code wartet, wird freigegeben und gleicht ab |
| `zentrale/tests/phase1_gate.rs` | drei Knoten; ein Gerät, das offline war, holt Dateien und Konfiguration nach |
| `zentrale/tests/phase2_gate.rs` | ein Konflikt wird auf dem Hub gelöst und eine Version wiederhergestellt |
| `zentrale/tests/api.rs` | Anmeldung, CSRF, Ordner, Entwürfe und Codes zum Koppeln über HTTP |

Jedes Syncthing in einem Test läuft mit `SpawnOptions::isolated`: zufällige Ports auf 127.0.0.1,
keine Discovery, keine Relays.

## Continuous Integration

| Workflow | Wann | Tut |
|---|---|---|
| `ci.yml` | jeder Push auf `main` und jeder Pull Request (nicht bei reinen Doku-Änderungen) | Rust: fmt, clippy und Tests (`cargo nextest`, jeder Test in einem eigenen Prozess, alle gleichzeitig) unter Linux, Windows und macOS, danach der Smoke-Test des Dienstes mit dem Agenten, den die Tests gebaut haben; Web-Checks; `bun checks` (bei einem Pull Request mit `bun checks pr`); E2E im Playwright-Image; das Docker-Image wird gebaut und gestartet. Ein Pull Request führt nur aus, was er kaputt machen kann (`scripts/ci-changes.sh`): Eine Änderung nur in `web/` überspringt Rust, eine nur in `crates/` die Checks der Oberfläche, und das Image wird nur gebaut, wenn sich `docker/`, eine Lockfile oder eine `package.json` geändert hat |
| `agents.yml` | Pushes auf `main`, aus `release.yml` oder von Hand | der Agent für jede Plattform; unter Windows auch der Installer und sein Test für stilles Installieren und Deinstallieren; `SHA256SUMS` und die Installationsskripte. Auf `main` hält er den Cache der Release-Builds warm, und ein kaputter Release-Build fällt dort auf |
| `docs.yml` | Pushes auf `main` und Pull Requests, die `docs/`, `mkdocs.yml` oder `requirements-docs.txt` ändern | baut die Doku mit [Zensical](https://zensical.org/), dem Nachfolger von MkDocs Material (`zensical build --strict`; das Datum unter jeder Seite schreibt `scripts/docs-revision-date` aus dem letzten Commit ihrer Datei, bis Zensical es selbst kann; das Plugin `privacy` lädt Schriften und das Mermaid-Skript beim Bauen herunter, so lädt die Seite nichts von Dritten); auf `main` veröffentlicht er sie auch auf GitHub Pages |
| `screenshots.yml` | von Hand: **Actions → Update screenshots** | erzeugt die Bilder in `docs/` aus den Demodaten und committet sie |
| `bump.yml` | von Hand: **Actions → Bump version** | erhöht die Version und taggt sie, das startet `release.yml` |
| `release.yml` | ein Tag `vX.Y.Z`, oder von Hand | CI, Agenten, das Multi-Arch-Image auf Docker Hub und GHCR, das GitHub-Release mit den Agenten |

Die wiederverwendbaren Teile (Bump, „tested on main“, Docker-Release, Screenshots) kommen aus
[firsttris/workflows](https://github.com/firsttris/workflows).

## Ein Release machen

1. **Actions → Bump version → patch/minor/major.** Das setzt die Version in `Cargo.toml`,
   `web/package.json` und `e2e/package.json`, committet und taggt `vX.Y.Z`.
2. Der Tag startet `release.yml`, das nichts doppelt ausführt. `tested-on-main.yml` aus
   firsttris/workflows prüft, ob die CI auf `main` für den Commit vor dem Release-Commit grün war,
   der nur die Version erhöht; nur wenn nicht, führt das Release die CI zuerst aus. Die Agenten
   werden von Anfang an gebaut, neben dem Image `tristanteu/stellwerk` und
   `ghcr.io/speicherstadt/stellwerk` für `linux/amd64` und `linux/arm64` (Tags `x.y.z`, `x.y`,
   `latest`, die Beschreibung auf Docker Hub aus der README). Der Build-Cache des Images ist der,
   den der Docker-Job der CI füllt, und `cargo-chef` hält darin die kompilierten Abhängigkeiten,
   so werden nur die eigenen Crates von Stellwerk kompiliert. Zuletzt bekommt das GitHub-Release
   die Agenten, den Windows-Installer, die Installationsskripte und `SHA256SUMS`.

Von Hand auf `main` gestartet, veröffentlicht `release.yml` das Image nur als `edge`, ohne
Release: ein Probelauf des Veröffentlichens.

Secrets: `DOCKER_PAT` (Access Token für Docker Hub für `tristanteu`). GHCR nutzt das eigene Token
des Workflows.

## Die Syncthing-Version ändern

Die Version und der SHA-256 des Archivs jeder Plattform stehen in `crates/engine/src/pin.rs`.
Renovate (`.github/renovate.json5`) öffnet für jedes neue Syncthing-Release einen Pull Request; auf
dessen Branch trägt `syncthing-pin.yml` die Prüfsummen aus der `sha256sum.txt.asc` des Releases ein
und startet die CI, deren Integrationstests mit der neuen Version grün sein müssen. Von Hand: Trag
die Version und die Prüfsummen ein und führe `cargo test` aus. Die Quellen von Syncthing
(`lib/config`, `lib/api`) sind die Referenz für alles, worauf sich die Engine verlässt.

## Dokumentation

```bash
pip install -r requirements-docs.txt
zensical serve                   # http://127.0.0.1:8000, lädt bei Änderungen neu
```

Die README und diese Dokumentation gibt es nur auf Deutsch; Code, Kommentare, Commits und Pull
Requests sind auf Englisch, die Oberfläche der App auf Deutsch und Englisch.

Die Bilder kommen aus `bun run screenshots` oder **Actions → Update screenshots**; bearbeite sie
nie von Hand. Derselbe Befehl erzeugt `docs/social-preview.png` (1280 × 640) aus
`scripts/social-preview/social-preview.html` und dem dunklen Screenshot; lade es unter
**Settings → General → Social preview** hoch, wenn es sich geändert hat. Links zwischen den Seiten
sind relativ (`installation.md#…`); Links aus der README gehen auf die veröffentlichte Seite.
