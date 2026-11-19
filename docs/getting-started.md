# Erste Schritte

Diese Seite führt dich von null zu einem ersten Ordner, der zwischen einem Server und einem Gerät
abgeglichen wird, auf dem kürzesten Weg, den du trotzdem so lassen kannst. Für einen Server, der
aus dem Internet erreichbar ist, lies danach die [Installation](installation.md); sie ergänzt TLS,
Backups und Updates.

**Du brauchst**

- einen **Server**, der immer an ist: Linux mit Docker oder Podman (ein NAS, ein Mini-PC, ein
  VPS);
- ein oder mehrere **Geräte**, die abgleichen sollen: Linux, Windows oder macOS;
- die Geräte müssen den Server auf zwei Ports erreichen: dem Webport (8080, oder 443 hinter einem
  Proxy) und dem Sync-Port des Hubs (22001).

## 1. Die Zentrale starten

Leg auf dem Server in einem neuen Verzeichnis eine `compose.yaml` an. Ersetze `nas.local` durch
den Namen oder die IP-Adresse, unter der deine Geräte den Server erreichen:

```yaml
services:
  stellwerk:
    image: tristanteu/stellwerk
    restart: unless-stopped
    ports:
      - "8080:8080"     # Weboberfläche und Agenten
      - "22001:22001"   # Sync-Port des Hubs
    environment:
      STELLWERK_HUB_ADDRESS: tcp://nas.local:22001
    volumes:
      - stellwerk:/data
volumes:
  stellwerk:
```

```bash
docker compose up -d
docker compose exec -T stellwerk stellwerk set-password <<< 'ein langes Passwort'
```

Öffne `http://nas.local:8080` und melde dich mit diesem Passwort an. Die Übersicht ist leer: noch
keine Geräte, noch keine Ordner.

!!! warning "Achtung"
    Port 8080 ohne TLS reicht für einen ersten Versuch im eigenen Netz. Bevor du ihn ins Internet
    öffnest, stell einen Reverse Proxy mit HTTPS davor: Das Passwort und die Anmeldung der Geräte
    laufen über diesen Port. Siehe [Reverse Proxy](installation.md#reverse-proxy-and-tls).

## 2. Einen Ordner anlegen

Klicke auf **Neuer Ordner** und gib ihm einen Namen wie `Dokumente`. Der Server hält jetzt einen
leeren Ordner mit Versionen; noch hat ihn kein Gerät. Ist ein Gerät gekoppelt, kann **Neuer
Ordner** auch gleich einen vorhandenen Ordner darauf auswählen (siehe [Einen Ordner auf einem
Gerät auswählen](user-guide.md#picking-a-folder-on-a-device)).

## 3. Ein Gerät koppeln

Klicke in der Oberfläche auf **Gerät koppeln**, wähle das Betriebssystem des Geräts und kopiere
den Befehl. Er sieht so aus, mit einem Einmal-Code, der 15 Minuten gilt:

```bash
curl -fsSL https://github.com/Speicherstadt/stellwerk/releases/download/vX.Y.Z/install.sh | sh -s -- http://nas.local:8080 K7Q2-MX9P-41TZ
```

Führe ihn auf dem Gerät in einem Terminal aus, als dein normaler Benutzer (kein `sudo`, kein
Administrator); unter Windows zeigt der Dialog einen Befehl für PowerShell. Er lädt den Agenten in
der Version der Zentrale herunter und führt `stellwerk-agent join` aus, das den Server und einen
neuen Schlüssel für das Gerät speichert, und dann `stellwerk-agent service install`, das den
Agenten jetzt und bei jeder Anmeldung startet. Mit einem selbst gebauten Agenten führst du diese
beiden direkt aus:
`stellwerk-agent join --server http://nas.local:8080 K7Q2-MX9P-41TZ && stellwerk-agent service install`.
Beim ersten Start lädt der Agent sein eigenes Syncthing herunter (etwa 15 MB), das dauert einen
Moment.

Der Dialog in der Oberfläche bemerkt das neue Gerät von selbst und fragt, welche Ordner es
bekommen soll. Hake **Dokumente** an und klicke auf **Gerät mit 1 Ordner einrichten**.

??? tip "Koppeln im LAN ohne Code"
    Im selben Netz kannst du den Code weglassen: Führe den Befehl ohne den Code am Ende aus. Der
    Agent zeigt einen Fingerabdruck wie `3F2A-91BC-04DE-77A1`, und das Gerät erscheint unter
    **Geräte → Wartende Geräte**; gib es dort frei, wenn der Fingerabdruck übereinstimmt.

## 4. Beim Abgleichen zusehen

Zurück in der **Übersicht** hat die Zeile *Dokumente* eine Zelle für das neue Gerät im Modus
**Beides**. Der Ordner liegt auf dem Gerät unter `~/Stellwerk/Dokumente`
(`%USERPROFILE%\Stellwerk\Dokumente` unter Windows). Leg eine Datei hinein: Nach wenigen Sekunden
meldet der Aktivitätsverlauf, dass das Gerät eine Datei in *Dokumente* geändert hat, und die Datei
liegt auch auf dem Server.

Koppel ein zweites Gerät genauso und gib ihm den Ordner auch: Dateien fließen jetzt
Gerät → Server → Gerät, auch wenn die beiden Geräte nie gleichzeitig an sind.

## Nächste Schritte

- **Pfad** oder **Modus** eines Ordners je Gerät ändern: Ordner öffnen, Reiter **Geräte**
  ([Bedienung](user-guide.md#folder-page)).
- Build-Ausgaben und Caches mit **Ignoriermustern** aus einem Ordner heraushalten
  ([Bedienung](user-guide.md#settings-rename-ignore-patterns-remove)).
- Den Server mit TLS von außen erreichbar machen und Backups einrichten
  ([Installation](installation.md)).
