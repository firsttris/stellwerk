# Sicherheit

Was in Stellwerk was schützt, und wem du vertrauen musst. Kurz: **Wer die Zentrale kontrolliert,
bestimmt, welche Ordner jedes Gerät abgleicht, und wo.** Schütze das Admin-Passwort und den Server
so, wie du die Geräte selbst schützen würdest.

## Vertrauensmodell

| Beteiligter | Kann |
|---|---|
| **Admin** (die eine Anmeldung) | für jedes Gerät festlegen, welche Ordner es abgleicht, in welchem Modus und unter welchem lokalen Pfad; jede Datei auf dem Server lesen; Konflikte lösen und Versionen wiederherstellen |
| **Server** (root auf dem Host oder das Volume des Containers) | jede abgeglichene Datei in der Kopie des Hubs, die Datenbank und die Identität des Hubs lesen und ändern |
| **Ein gekoppeltes Gerät** | seine Änderungen in die Ordner schicken, die es im Modus Beides oder Nur senden hält; sie erreichen den Server und jedes andere Gerät dieser Ordner |
| **Jemand im LAN** | darum bitten, ohne Code gekoppelt zu werden; wird erst ein Gerät, wenn der Admin es freigibt |
| **Jemand im Internet** | die Anmeldeseite und die Anmeldung der Agenten erreichen, wenn der Webport öffentlich ist |

**Der Agent folgt der Zentrale.** Er richtet sein Syncthing so ein, wie der Soll-Zustand es sagt,
einschließlich des lokalen Pfads eines Ordners, mit den Rechten des Benutzers, als der er läuft.
Pfade schränkt er nicht ein. Ein Admin, oder ein Angreifer mit dem Admin-Passwort oder der
Kontrolle über den Server, kann also einen Ordner mit jedem Pfad freigeben, den der Benutzer des
Geräts lesen oder schreiben kann, und so Dateien vom Gerät lesen oder Dateien darauf ablegen. Das
ist der Sinn der zentralen Verwaltung, und auch der Grund, warum die Zentrale der wertvollste Teil
ist, den es zu schützen gilt. Der Agent löscht nie Dateien, und er führt nie Befehle aus, die er
bekommt.

**Der Server hält Klartext.** Der Hub hält eine normale, unverschlüsselte Kopie jedes Ordners, mit
Versionen. Ordner auf dem Hub zu verschlüsseln (die Untrusted Devices von Syncthing) wird noch
nicht unterstützt.

## Verbindungen

```text
Browser ──HTTPS (Proxy)──▶ Zentrale ◀──WSS (Proxy)── Agent
                               │                        │
                       REST, 127.0.0.1           REST, 127.0.0.1
                               ▼                        ▼
                        Hub-Syncthing ◀──TLS 1.3──▶ Geräte-Syncthing
```

- **Browser und Agenten → Zentrale.** Ein Webport für die Oberfläche, die API und den WebSocket
  der Agenten. Stellwerk selbst spricht reines HTTP; TLS kommt vom Reverse Proxy
  ([Installation](installation.md#reverse-proxy-and-tls)). Die Agenten prüfen das Zertifikat des
  Proxys gegen die Stammzertifikate des Systems, dazu eine eigene CA, wenn die Zentrale eine hat
  (`--ca-cert`): Der Agent hat sie beim Koppeln geholt, ohne der Verbindung zu vertrauen, und nur
  behalten, weil ihr SHA-256 zum Fingerabdruck im Befehl zum Koppeln passte, den der Admin über
  eine Verbindung, der er schon vertraut, aus der Oberfläche kopiert hat. Eine Adresse der
  Zentrale mit `http://` ist in einem vertrauenswürdigen LAN in Ordnung, aber dann ist nichts
  verschlüsselt, auch nicht der Code zum Koppeln.
- **Geräte ↔ Hub.** Das eigene Protokoll von Syncthing: TLS mit dem Zertifikat jeder Seite, und
  jede Seite nimmt nur die Geräte-IDs (Hashes der Zertifikate) an, die sie bekommen hat. Die
  Zentrale gibt jedem Agenten die ID des Hubs und dem Hub die IDs aller freigegebenen Geräte, so
  kann niemand sonst einem Ordner beitreten, auch nicht über den offenen Sync-Port. Discovery-Server
  und Relays sind aus; Daten gehen nur zwischen einem Gerät und dem Hub.
- **Stellwerk → Syncthing.** Die GUI/REST-API jedes Syncthing lauscht auf `127.0.0.1` auf einem
  zufälligen Port, mit einem neuen zufälligen API-Schlüssel bei jedem Start. Der Schlüssel geht
  über die Umgebung an Syncthing, nicht über die Kommandozeile, wo jeder lokale Benutzer ihn lesen
  könnte (`ps`). Andere Benutzer auf dem Rechner könnten den Port erreichen, aber nicht ohne den
  Schlüssel.

## Admin-Anmeldung

- Ein Admin, mit einem Passwort, das als **argon2id**-Hash gespeichert ist. Es wird nur auf dem
  Server gesetzt (`stellwerk set-password`), nie über das Web.
- Eine Anmeldung erzeugt ein zufälliges Sitzungstoken. Der Browser bekommt es in einem Cookie, das
  `HttpOnly`, `SameSite=Strict` und, hinter HTTPS (`X-Forwarded-Proto: https`), `Secure` ist. Die
  Datenbank speichert nur den Hash des Tokens. Sitzungen gelten 30 Tage; ein neues Passwort
  beendet alle.
- Jede Änderung braucht den Header `X-CSRF-Token` mit einem zweiten Token aus der Anmeldung, so
  kann eine andere Website deinen Browser nichts ändern lassen.
- Passwörter werden nacheinander geprüft. Eine fehlgeschlagene Anmeldung wartet eine halbe
  Sekunde, doppelt so lang mit jedem weiteren Fehlschlag in Folge, bis 8 Sekunden, so ist
  paralleles Raten nicht schneller als serielles. Eine Sperre gibt es nicht; nimm ein langes
  Passwort, und ist die Anmeldung öffentlich, überleg, sie im Proxy einzuschränken (z. B. nur aus
  deinem VPN).
- Der Live-WebSocket der Oberfläche (`/api/ws`) prüft die Sitzung vor jeder Aktualisierung und
  schließt, sobald die Sitzung weg ist (Abmelden, Ablauf, neues Passwort).
- Hinter einem TLS-Proxy bekommt das Cookie `Secure` nur, wenn der Proxy
  `X-Forwarded-Proto: https` setzt (die Beispiele in der
  [Installation](installation.md#reverse-proxy-and-tls) tun das).

## API-Tokens

- Für Skripte und KI-Assistenten erstellt der Admin API-Tokens in der Weboberfläche
  (**API-Tokens**). Ein Token besteht aus 32 zufälligen Bytes mit dem Präfix `stw_`; die
  Datenbank speichert nur seinen SHA-256, und die Oberfläche zeigt es einmal.
- Ein Token wird als `Authorization: Bearer …` gesendet. Der Browser sendet es nie von selbst,
  deshalb braucht es kein CSRF-Token.
- Sein Umfang begrenzt es: **Lesen** sieht die Übersicht, den Status, die Aktivität und die Ordner
  auf den Geräten; **Verwalten** legt außerdem Ordner an und benennt sie um, setzt Ignoriermuster
  und wendet Freigaben an. Geräte entfernen oder sperren, Koppeln, Ordner löschen, Konflikte,
  Versionen und die Tokens selbst werden jedem Token verweigert (`403`), so kann ein
  durchgesickertes Token kein Gerät hereinlassen oder eines hinauswerfen.
- Ein Token mit **Verwalten** kann trotzdem einen Ordner mit jedem gekoppelten Gerät teilen, unter
  jedem Pfad dort: Behandle es wie das Admin-Passwort. Nimm **Lesen**, wo das reicht, gib jedem
  Skript und jedem Assistenten ein eigenes Token und widerrufe die, die du nicht mehr brauchst (die
  Liste zeigt, wann jedes zuletzt benutzt wurde).
- Ein neues Admin-Passwort widerruft jedes Token, denn wer das alte kannte, könnte eines erstellt
  haben.
- Der MCP-Server unter `/mcp` nimmt nur API-Tokens an, nie das Sitzungs-Cookie, und bietet einem
  Token nur die Werkzeuge seines Umfangs an. Er lehnt Anfragen ab, deren `Origin` einen anderen
  Host nennt, so kann eine Webseite ihn nicht über deinen Browser erreichen. Ein Assistent mit
  einem Token mit **Verwalten** kann jede Freigabe einrichten, die eine Person in der Matrix
  einrichten könnte; Claude Code fragt vor jedem Werkzeug, das etwas ändert, lass diese Rückfrage
  für `apply_shares` also an.

## Koppeln und Anmeldung der Geräte

- **Einmal-Code.** Zufällig, 15 Minuten gültig, einmal benutzbar; die Datenbank speichert nur
  seinen Hash. Er beweist der Zentrale, dass die Person, die `join` ausführt, koppeln durfte.
- **Geräteschlüssel.** `join` erzeugt auf dem Gerät ein **Ed25519**-Schlüsselpaar; der geheime
  Schlüssel bleibt in `agent.json`, nur für den Benutzer lesbar (unter Linux und macOS mit dem
  Dateimodus `600`). Bei jeder Verbindung schickt die Zentrale eine neue zufällige Nonce, und der
  Agent signiert sie zusammen mit allem anderen, was sein Hello sagt (Code, Syncthing-Geräte-ID,
  Name, Betriebssystem, Versionen). So lässt sich eine aufgezeichnete Anmeldung nicht wieder
  abspielen, und wer eine `http://`-Verbindung lesen und ändern kann, kann weder die Signatur
  behalten und seine eigene Syncthing-ID einsetzen noch den Code in ein eigenes Hello übernehmen.
  Den Code lesen kann er trotzdem; nimm HTTPS überall, wo das Netz nicht vertrauenswürdig ist.
  Nach der ersten Anmeldung fällt der Code weg; der Schlüssel allein identifiziert das Gerät. Jedes
  Gerät muss seine eigene Syncthing-Geräte-ID haben: Ein Gerät, das eine ID meldet, die einem
  anderen Gerät gehört (ein kopiertes Syncthing-Home), wird abgewiesen.
- **Koppeln im LAN ohne Code.** Ein Gerät, das ohne Code kommt, wartet nur. Die Oberfläche zeigt
  den **Fingerabdruck** seines Schlüssels (die ersten 8 Bytes des SHA-256 des Schlüssels, z. B.
  `3F2A-91BC-04DE-77A1`), und `join` zeigt denselben auf dem Gerät. Gib es nur frei, wenn beide
  übereinstimmen; sonst könnte sich jemand anderes im Netz als dein Gerät ausgeben.
  `--lan-pairing false` schaltet das aus.
- **Sperren und entfernen.** Ein gesperrtes Gerät wird bei der Anmeldung abgewiesen, bekommt
  keinen Soll-Zustand, und der Hub streicht es als Gegenstelle, so kann es nicht mehr abgleichen.
  Entfernen löscht es; sein Schlüssel ist dann unbekannt, und es müsste neu gekoppelt werden.

## Auf den Geräten

- Der Agent läuft als normaler Benutzer, ohne Administratorrechte, er kann also nur erreichen,
  was dieser Benutzer erreichen kann.
- Er lädt genau die Syncthing-Version, die im Quellcode festgelegt ist, aus den GitHub-Releases
  von Syncthing herunter und prüft ihren **SHA-256** gegen die Prüfsumme, die in Stellwerk
  einkompiliert ist, bevor er sie startet. Das eigene Auto-Upgrade von Syncthing ist aus.
- Ein anderes Syncthing auf demselben Rechner fasst er nie an; sein eigenes läuft mit eigenem
  Home-Verzeichnis und eigenen Ports.
- Auf Anfrage der Zentrale listet er die Unterverzeichnisse eines Verzeichnisses auf (die
  Ordnerauswahl) oder sucht Verzeichnisse nach Namen. Nur Namen und Pfade von Verzeichnissen
  verlassen das Gerät, nie Dateien oder deren Inhalt; eine Suche überspringt versteckte
  Verzeichnisse, folgt keinen symbolischen Links und hört nach 8 Ebenen, 50 Treffern oder
  10 Sekunden auf.

## Updates und Releases

Release-Images und Agenten baut GitHub Actions aus dem getaggten Quellcode, nach der vollständigen
Testsuite. Jedes Release listet die SHA-256-Summen der Agenten in `SHA256SUMS`.

Ein Agent aktualisiert sich nur, wenn die Oberfläche ihn darum bittet. Die Zentrale nennt nur eine
Version: Der Agent lädt sie über HTTPS von der festen Adresse der GitHub-Releases des Projekts
herunter, prüft sie gegen `SHA256SUMS` und installiert nur ein neueres `x.y.z`. Eine kompromittierte
Zentrale kann also um ein echtes neueres Release bitten, aber kein eigenes Programm auf die Geräte
bringen oder zu einem älteren zurückgehen. Eine Signatur obendrauf gibt es nicht: Wer die
GitHub-Releases des Projekts kontrolliert, könnte eine schädliche Version veröffentlichen, und die
Agenten würden ihr vertrauen.

## Checkliste

- [ ] Der Webport ist von außen nur über HTTPS erreichbar.
- [ ] Das Admin-Passwort ist lang und wird nirgends sonst benutzt.
- [ ] Es gibt nur die API-Tokens, die in Gebrauch sind, jedes mit dem engsten Umfang, der reicht.
- [ ] Das Datenverzeichnis (oder Volume) wird gesichert, und die Sicherung ist so geschützt wie
      der Server.
- [ ] Wartende Geräte werden erst nach dem Vergleich des Fingerabdrucks freigegeben.
- [ ] Ein verlorenes oder verkauftes Gerät wird in der Oberfläche **gesperrt** oder **entfernt**.

## Eine Schwachstelle melden

Bitte eröffne kein öffentliches Issue. Nutze stattdessen das private
[Security Advisory](https://github.com/Speicherstadt/stellwerk/security/advisories/new) von GitHub.
