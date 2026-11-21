# Panorama Stitcher: Benutzerhandbuch

*Andere Sprachen: [English](manual_en.md) · [Česky](manual_cs.md) · [Español](manual_es.md) · [Français](manual_fr.md) · [Polski](manual_pl.md)*

Ein Desktop-Programm, das überlappende Bilder zu einem Panorama oder Mosaik
zusammenfügt: JPEG, TIFF, PNG, Kamera-RAW und astronomische FITS-Bilder. Jeder
Schritt läuft erst, wenn Sie seine Schaltfläche drücken, und jedes Ergebnis
lässt sich vor dem Weitermachen prüfen und korrigieren. Ein zweiter Reiter
stackt Sternaufnahmen zu einem rauscharmen Nachthimmel oder zu Strichspuren
([Sterne stacken](#13-sterne-stacken)).

Die Bedienelemente stehen in diesem Handbuch **fett**, so wie sie auf dem
Bildschirm auf Deutsch erscheinen. Die Sprache des Fensters (Deutsch, Englisch,
Tschechisch, Spanisch, Französisch, Polnisch) wird oben rechts umgeschaltet; dieses Handbuch gibt es im Programm
auch unter **? Hilfe** (F1).

---

## Inhalt

1. [Voraussetzungen](#1-voraussetzungen)
2. [Programm starten](#2-programm-starten)
3. [Das Fenster im Überblick](#3-das-fenster-im-überblick)
4. [Schnellstart](#4-schnellstart)
5. [Schritt für Schritt](#5-schritt-für-schritt)
6. [In der Ansicht navigieren](#6-in-der-ansicht-navigieren)
7. [Projektionen](#7-projektionen)
8. [RAW-Dateien](#8-raw-dateien)
9. [FITS-Dateien (Astronomie)](#9-fits-dateien-astronomie)
10. [Speichern](#10-speichern)
11. [Fehlerbehebung](#11-fehlerbehebung)
12. [Tipps für gute Panoramen](#12-tipps-für-gute-panoramen)
13. [Sterne stacken](#13-sterne-stacken)
14. [Verwendung ohne Fenster](#14-verwendung-ohne-fenster)
15. [Dateien](#15-dateien)
16. [Nachbearbeitung](#16-nachbearbeitung)
17. [Zeitraffer](#17-zeitraffer)

---

## 1. Voraussetzungen

- **Nichts zu installieren.** Das Programm ist eine einzige Datei,
  `panostitch` (`panostitch.exe` unter Windows), dazu `panostack` zum
  Sterne-Stacken von der Kommandozeile. Unter Linux braucht es einen
  Grafiktreiber mit OpenGL sowie X11 oder Wayland, wie jeder Desktop sie hat.
- **Kamera-RAW-Dateien** brauchen nichts weiter: ein RAW-Decoder ist
  eingebaut. Sind `dcraw_emu` (LibRaw), RawTherapee, darktable oder dcraw
  installiert, kann das Programm sie ebenfalls nutzen (siehe
  [RAW-Dateien](#8-raw-dateien)).
- **FITS- und XISF-Dateien** brauchen nichts weiter.
- **Dateiauswahl-Dialoge** brauchen unter Windows und macOS nichts weiter.
  Unter Linux verwenden sie `zenity`, `kdialog` oder `yad`, je nachdem, was
  installiert ist (die meisten Desktops haben eines davon). Ohne sie tippen
  Sie Pfade in das Pfadfeld.

Um das Programm aus dem Quellcode zu bauen, brauchen Sie Rust
(<https://rustup.rs>): `cargo build --release` erzeugt
`target/release/panostitch` und `target/release/panostack`.

---

## 2. Programm starten

Doppelklicken Sie auf `panostitch`, oder starten Sie es im Terminal:

```
panostitch
```

Bilder oder eine Projektdatei lassen sich auch direkt beim Start öffnen:

```
panostitch ~/Pictures/pano/*.NEF
panostitch mein_pano.panoproj
panostitch nacht.stackproj          # öffnet sich im Tab Sterne stacken
panostitch nacht.tlproj             # öffnet sich im Tab Timelapse
```

Hinweise:

- Das Programm nutzt die Prozessorkerne automatisch. Lange Aufgaben nehmen
  höchstens 85 % davon und lassen 15 % des Speichers frei, damit der Rechner
  bedienbar bleibt.
- Das Fenster öffnet sich in ein bis zwei Sekunden; vorher muss nichts
  vorbereitet werden.
- Erscheint kein Fenster, starten Sie das Programm im Terminal: Dort stehen
  die Fehlermeldungen.
- Die Schalter für das Aussehen (Dark, Modern, Simple) stehen in `ui.toml` im
  Einstellungsordner: `~/.config/panostitch` unter Linux,
  `~/Library/Application Support/panostitch` unter macOS,
  `%APPDATA%\panostitch` unter Windows.
- **Sprache:** Das Fenster spricht Deutsch, Englisch, Tschechisch, Spanisch,
  Französisch oder Polnisch; das
  Menü oben rechts schaltet sofort um (der erste Start folgt der Sprache des
  Systems).
- **Hilfe:** **? Hilfe** oben rechts (oder F1) öffnet dieses Handbuch im
  Programm, mit Inhalt, Suche und Links; F1 öffnet das Kapitel des angezeigten
  Tabs.
- **Bis zum nächsten Start gemerkt:** der Tab, in dem Sie waren, die
  Einstellungen fürs Sterne-Stacken, die Videoeinstellungen der Timelapse
  (Deflicker, Kamerafahrt, Ausgabe, RAW via) und die RAW-Einstellungen des
  Panorama-Stitchers. Dateien, Masken und Looks nicht; die hält eine
  Projektdatei.
- **Ziehen und Ablegen:** Bilder oder einen Ordner ins Fenster ziehen fügt
  sie dem angezeigten Tab hinzu (Post-processing öffnet das erste Bild). Eine
  abgelegte Projektdatei öffnet sich in ihrem Tab.
- **Letzte Projekte** (obere Leiste) listet die zuletzt geöffneten oder
  gespeicherten Projekte jeder Art; ein Klick öffnet eines in seinem Tab.
- Dateidialoge beginnen im zuletzt benutzten Ordner.
- Nach dem Speichern öffnet **Im Ordner zeigen** (im Bericht oder neben Save in
  Post-processing) den Ordner im Dateimanager.

---

## 3. Das Fenster im Überblick

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ [Open…] [Save proj.] │ [Image+keypoints] [Pair matches]     │ 3 · Projection      │
│ Images               │ [Panorama] [Zoom +] [Zoom -] [Fit]   │  Projektionsmenü    │
│  Add Remove Clear ▲▼ │ (o) overlays  [Undo] [Redo]          │  Focal, Yaw, Pitch, │
│  Pfadfeld            │                                      │  Roll               │
│  Bildliste           │                                      │ 4 · Compose         │
│  Reference           │            Hauptansicht              │  Blend, Preview size│
│  RAW via, half size  │                                      │  Exposure,Brightness│
│  Preset              │                                      │  show unlinked      │
│ 1 · Features         │                                      │  Auto crop, Uncrop  │
│                      │                                      │  Nudge ← ↑ ↓ →      │
│ 2 · Matching         │                                      │ 5 · Stretch         │
│  Pairs, Model, Pair  │                                      │  Histogramm, Regler │
│  Paar-Details        │  Statuszeile (roter Text)            │ [Preview] [Save…]   │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

- **Linke Spalte:** Projekte öffnen und speichern, Bilder auswählen, Merkmale finden, Bilder zuordnen.
- **Mitte:** die Hauptansicht mit Schaltflächen zum Wechseln der Ansicht, zum
  Zoomen, zum Ausblenden der Überlagerungen sowie zum Rückgängigmachen und
  Wiederherstellen.
- **Rechte Spalte:** Projektion, Belichtung, Überblendung, Helligkeit,
  manuelles Platzieren, Histogramm-Streckung, Vorschau und Speichern.
- **Rote Statuszeile** (unter der Ansicht): was das Programm gerade tut,
  Ergebnisse und Fehlermeldungen.

Die Schalter oben rechts ändern das Aussehen: **Dunkel**, **Modern**
(runder, luftiger) und **Einfach**. Der einfache Modus blendet
die Detaileinstellungen aus (Merkmalsuche und Zuordnung, Überblendung,
Streckung, Verschieben) und behält, was die meisten Panoramen brauchen: die
Bilder, eine Vorgabe, **Alle Schritte ausführen**, die Projektion mit Brennweite,
Gieren (Yaw), Neigen (Pitch) und Rollen (Roll), die Belichtung, **Angleichen**,
Zuschnitt, Vorschau und Speichern. Ausgeblendete Einstellungen behalten die
Werte der Vorgabe. Das Programm merkt sich die Schalter.

---

## 4. Schnellstart

**Foto-Panorama:**

1. Lassen Sie **halbe Größe** eingeschaltet. Das ist schneller und reicht zum
   Prüfen des Ergebnisses völlig aus.
2. Wählen Sie ein passendes **Vorgabe** (siehe [Voreinstellungen](#50-voreinstellungen)).
3. Tippen Sie den Ordner mit den Bildern in das Pfadfeld und drücken Sie
   **Enter**, oder klicken Sie auf **Hinzufügen…** und wählen Sie die Bilder aus.
   **Die Reihenfolge spielt keine Rolle.**
4. Klicken Sie auf **Alle Schritte ausführen** und warten Sie, bis die Statuszeile
   *Done* meldet.
5. Wählen Sie rechts eine **Projektion**: **zylindrisch** für eine einzelne
   Reihe, **sphärisch** für mehrere Reihen oder Aufnahmen, die weit nach oben
   oder unten reichen. Ist das Ergebnis zu dunkel, klicken Sie auf **Auto
   exposure**; ist der Horizont gebogen, passen Sie **Pitch** an. Mit **Auto
   crop** schneiden Sie die ausgefransten Ränder ab.
6. Klicken Sie auf **Panorama speichern…**. Wenn Sie später weiterarbeiten
   möchten, klicken Sie auch auf **Projekt speichern…**.

**Deep-Sky-Mosaik aus FITS-Dateien:**

1. Wählen Sie die Voreinstellung **Deep-Sky-Mosaik**.
2. Laden Sie die FITS-Kacheln (gestackt oder Einzelbilder). Sie werden zur
   Anzeige automatisch gestreckt.
3. Klicken Sie auf **Alle Schritte ausführen** und dann auf **Automatisch strecken**, damit die
   Streckung zum ganzen Mosaik passt. Bei Bedarf feinjustieren.
4. Speichern Sie als **`.fits`**, um die linearen Daten weiterzuverarbeiten,
   oder als **`.tif`**, um das gestreckte Bild so zu erhalten, wie Sie es sehen.

---

## 5. Schritt für Schritt

**Werte eintippen:** Neben jedem Schieberegler steht ein Zahlenfeld. Klicken
Sie hinein, tippen Sie einen Wert und drücken Sie **Enter**. Der Wert wird auf
Schrittweite und Bereich des Reglers gerundet.

### 5.0 Voreinstellungen

Das Menü **Vorgabe** (linke Spalte, unter *RAW über*) setzt alle Werte für
Zuordnung und Zusammensetzung für eine Motivart auf einmal. Jeder Wert lässt
sich danach weiter ändern.

| Voreinstellung | Für | Wichtigste Einstellungen |
|---|---|---|
| **Tageslandschaft** | Tageslicht-Panoramen, Kamera auf der Stelle gedreht | leichtes Entrauschen, Homographie, zylindrische Projektion |
| **Astro / Nachtlandschaft** | Nachthimmel, Milchstraße über einer Landschaft | starkes Entrauschen, lockerere Zuordnung, sphärische Projektion, +1 EV (dann **Automatische Belichtung**), höhere Eckenschwelle (weniger falsche Ecken im Rauschen) |
| **Deep-Sky-Mosaik** | Teleskop-Mosaike von Sternfeldern, z. B. FITS | wenig Entrauschen (Sterne sind klein), Ähnlichkeitsmodell (Verschiebung + Bildfeldrotation), keine Verfeinerung, geradlinige Projektion; **Automatisch strecken** verwenden |
| **Makro / Mikroskop** | Objektträger oder Tisch unter einer festen Kamera verschoben | Verschiebungsmodell (Bilder verschieben sich nur), keine Verfeinerung, geradlinige Projektion |
| **Scan / flaches Dokument** | Karten, Gemälde, Dokumente in Teilen fotografiert | affines Modell, keine Verfeinerung, geradlinige Projektion |

### 5.1 Bilder (Images)

| Element | Funktion |
|---|---|
| **Hinzufügen…** | Öffnet einen Dateidialog. Mehrere Dateien sind möglich (Strg/Umschalt + Klick). |
| **Pfadfeld** | Pfad zu einer Datei oder einem **Ordner** eintippen und Enter drücken. Ein Ordner lädt alle Bilder darin, nach Namen sortiert. Dateien namens `panorama…` (frühere Ergebnisse) werden übersprungen. |
| **Bildliste** | Ein Klick wählt ein Bild aus und zeigt es an. **Strg + Klick** fügt ein Bild der Auswahl hinzu oder entfernt es, **Umschalt + Klick** wählt einen Bereich, **Strg+A** (Maus über der Liste) wählt alles. Mit dem Mausrad blättern. |
| **Entfernen** | Entfernt die ausgewählten Bilder. Die **Entf**-Taste tut dasselbe, wenn die Maus über der Liste ist. |
| **Alles leeren** | Entfernt alle Bilder. |
| **▲ / ▼** | Verschiebt die ausgewählten Bilder in der Liste nach oben oder unten. Nur die Paar-Modi *sequential* und *grid* achten auf die Reihenfolge. |
| **Referenz** | Das Bild, um das herum die anderen platziert werden; es bleibt unverzerrt. **auto (Mitte)** wählt das Bild in der Mitte der Gruppe, was meist das gleichmäßigste Ergebnis gibt. |
| **RAW über** | Welcher RAW-Konverter verwendet wird, siehe [RAW-Dateien](#8-raw-dateien). Gilt für danach hinzugefügte Bilder. |
| **halbe Größe** | Lädt RAW-Dateien und FITS-Dateien von Farbkameras in halber Auflösung: etwa 4× schneller, ¼ des Speichers. Gilt für danach hinzugefügte Bilder. Beim Speichern mit einer **Ausgabegröße** über 50 % werden RAW-Dateien erneut in voller Größe gelesen. |

Markierungen in der Liste:

| Markierung | Bedeutung |
|---|---|
| blauer Balken | ausgewählt; der dunklere ist das aktuelle Bild (für Anzeige, Helligkeit und Verschieben) |
| `R` | Referenzbild |
| `4345kp` | Anzahl gefundener Merkmalspunkte |
| `UNLINKED` | das Bild ließ sich nicht mit dem Panorama verbinden (siehe [5.3](#53-schritt-2-zuordnung-matching)) |

Zuordnungsergebnisse bleiben erhalten, wenn Sie Bilder umsortieren oder andere
Bilder entfernen. Sie müssen also nicht neu zuordnen.

### 5.2 Schritt 1: Merkmale (Features)

Merkmalspunkte sind markante Stellen wie Ecken, Kanten und Sterne, die sich in
zwei überlappenden Bildern wiedererkennen lassen.

| Regler | Standard | Bedeutung |
|---|---|---|
| **Erkennungsgröße (px)** | 2000 | Das Bild wird vor der Suche auf diese Größe (längere Seite) verkleinert. Größer ist genauer, aber langsamer. |
| **Max. Merkmale** | 5000 | Obergrenze pro Bild. Die Punkte werden gleichmäßig über das ganze Bild verteilt. |
| **Eckenschwelle** | 0,001 | Mindeststärke einer Ecke. Niedriger findet auch schwache Ecken (nützlich bei Himmel oder Nebel), höher nur kräftige. |
| **Entrauschen (σ px)** | 1,5 | Glättet vor der Suche das Sensorrauschen. Für **Nachtaufnahmen** und verrauschte Hoch-ISO-Bilder erhöhen (2–3); für Sternfelder und saubere Tageslichtbilder mit feinen Details verringern (0,5–1). |

Klicken Sie auf **Merkmale finden**. Die Ansicht zeigt das aktuelle Bild mit
den Punkten als **gelbe Kreise**. Sie sollten über die Bildteile verteilt sein,
die sich mit anderen Bildern überlappen. Neu verarbeitet werden nur Bilder,
deren Einstellungen sich geändert haben. Dunkle und lineare Bilder
(Nachtaufnahmen, FITS) sind kein Problem: Die Erkennung passt den Kontrast
intern an.

### 5.3 Schritt 2: Zuordnung (Matching)

Die Zuordnung findet dieselben Punkte in zwei Bildern. **RANSAC** behält dann
nur die Zuordnungen, die zu einem gemeinsamen geometrischen Modell passen (die
*Inlier*).

**Welche Paare probiert werden (Menü Pairs):**

| Pairs | Wann verwenden |
|---|---|
| **alle Paare** | Standard. Jedes Bild wird mit jedem anderen probiert, daher **spielt die Reihenfolge keine Rolle**, und mehrere Reihen funktionieren automatisch. Bei vielen Bildern langsamer (74 Fotos ≈ 35 s). |
| **nacheinander** | Eine Reihe, Bilder in Aufnahmereihenfolge. Jedes Bild wird nur mit dem nächsten in der Liste verglichen. Am schnellsten. |
| **Raster** | Mehrere Reihen in bekannter Reihenfolge: **Rasterspalten** Bilder pro Reihe, Reihe für Reihe. Probiert werden die Nachbarn rechts, unten und diagonal. Schalten Sie **Schlange** ein, wenn jede zweite Reihe in Gegenrichtung aufgenommen wurde. |

**Einstellungen:**

| Element | Standard | Bedeutung |
|---|---|---|
| **Rasterspalten** | 5 | Bilder pro Reihe (nur im Modus grid). |
| **Verhältnistest** | 0,8 | Um wie viel besser die beste Zuordnung als die zweitbeste sein muss. Niedriger ist strenger (weniger, aber verlässlichere Zuordnungen); höher ergibt mehr Zuordnungen. |
| **Max. Abstand** | 0,35 | Wie unterschiedlich zwei Punkte aussehen dürfen, um noch zugeordnet zu werden. |
| **RANSAC-Toleranz (px)** | 5,0 | Wie weit (in Pixeln) eine Zuordnung vom Modell abweichen darf und noch als Inlier zählt. |
| **Min. Inlier** | 20 | Mindestanzahl an Inliern, damit ein Paar als echte Überlappung gilt. |
| **Modell** | homography | Wie ein Bild auf ein anderes abgebildet wird, siehe unten. |
| **verfeinern** | an | Globale Verfeinerung, siehe unten. |

**Wann ein Paar als echte Überlappung gilt:** Es braucht mindestens *Min.
inliers* Inlier, eine klare Mehrheit seiner Zuordnungen muss übereinstimmen,
und die Transformation muss plausibel sein (sie darf das Bild weder umklappen
noch extrem verzerren). So werden falsche Zuordnungen zwischen Bildern
aussortiert, die sich gar nicht überlappen.

**Modelle:**

| Modell | Wann verwenden |
|---|---|
| **Homographie** | Normale Panoramen: Kamera auf der Stelle gedreht. Standard. |
| **affin** | Scans oder Fotos eines flachen Objekts aus großer Entfernung. |
| **Ähnlichkeit** | Nur Verschiebung, Drehung und gleichmäßige Skalierung, z. B. Teleskop-Mosaike mit Bildfeldrotation. Stabil auch bei wenigen Zuordnungen. |
| **Verschiebung** | Nur Verschiebung, z. B. Scanner oder Mikroskoptisch. Am robustesten, am wenigsten flexibel. |

**Platzierung und globale Verfeinerung (refine):** Die Bilder werden über ihre
stärksten Paare verbunden, ausgehend vom Referenzbild. Mit eingeschaltetem
**verfeinern** (nur beim Modell homography) werden danach alle Paare gemeinsam als
eine sich drehende Kamera gelöst. Kleine Fehler verteilen sich so
gleichmäßig, statt sich aufzusummieren, und die Schleifen zwischen den Reihen
schließen sich; das beseitigt die meisten „Geisterbilder“. Außerdem wird die
tonnenförmige Verzeichnung des Objektivs geschätzt (stark bei Weitwinkeln) und
beim Rendern korrigiert. Paare, die nicht zur gemeinsamen Lösung passen, werden
**automatisch verworfen**, da es meist falsche Zuordnungen oder bewegte Dinge
sind. Die Statuszeile meldet, wie viele verworfen wurden, und den **typischen
Paarfehler** (1–3 px ist gut).

**Schaltflächen:**

- **Zuordnen**: ordnet die im Menü Pairs gewählten Paare zu (parallel),
  platziert und verfeinert dann.
- **Paar neu zuordnen**: ordnet nur das angezeigte Paar mit den aktuellen
  Einstellungen neu zu.
- **Paar verwerfen**: verwendet das angezeigte Paar nicht mehr (für eine
  sichtbar falsche Zuordnung); die Platzierung wird ohne es neu berechnet.
- **Alle Schritte ausführen**: Merkmale, Zuordnung, Platzierung und Vorschau in einem.

Die Ansicht **Paarzuordnungen** zeigt zwei Bilder nebeneinander:

- **grüne Linien**: Inlier (richtige Zuordnungen)
- **rote Linien**: Ausreißer. Mit dem Schalter **Ausreißer** ausblenden.

Das Menü **Paar** listet die angenommenen Paare mit ihrer Inlier-Anzahl auf.
Mit *rejected* markierte Paare haben eine Prüfung nicht bestanden; der Text
unter dem Menü nennt den Grund.

**So sieht ein gutes Ergebnis aus:** Dutzende grüne Linien oder mehr, die alle
dieselbe Verschiebung beschreiben, und ein mittlerer Fehler von 1–2 px.

**Nicht verbundene Bilder:** Ein Bild ohne angenommenes Paar zu den übrigen
wird in der Liste mit `UNLINKED` markiert und **nicht ins Panorama
übernommen**, damit es dieses nicht verdirbt. Schalten Sie **show unlinked
images** (rechte Spalte) ein, um solche Bilder in einer Reihe unter dem
Panorama anzuzeigen und sie mit **Strg + Ziehen** von Hand zu platzieren.

### 5.4 Schritt 3: Projektion (Projection)

Wie die einzelnen Projektionen aussehen, beschreibt das Kapitel
[Projektionen](#7-projektionen).

| Element | Bedeutung |
|---|---|
| **Projektionsmenü** | 14 Projektionen, siehe [Projektionen](#7-projektionen) |
| **Brennweite (× auto)** | Die Brennweite wird aus den Zuordnungen geschätzt. Dieser Regler skaliert sie. Wirkt das Panorama seitlich gestaucht oder gedehnt, probieren Sie Werte zwischen 0,8 und 1,2. |
| **Gieren (°)** | Dreht das ganze Panorama nach links oder rechts: bestimmt, welche Richtung in der Mitte liegt. Nützlich bei 360°-Panoramen (z. B. damit das Zentrum der Milchstraße in der Mitte statt am Rand liegt). |
| **Neigung (°)** | Neigt das ganze Panorama nach oben oder unten. Damit **begradigen Sie einen gebogenen Horizont** (etwa wenn die Kamera nach unten zeigte). |
| **Rollen (°)** | Dreht das ganze Panorama. Damit richten Sie einen schiefen Horizont aus. |
| *Feineinstellung* | Yaw, Pitch und Roll haben je einen Regler, ein Zahlenfeld und **◀ ▶**. Die Regler für Pitch und Roll sind nahe 0° fein (wo Korrekturen meist liegen) und weiter außen grob. Langsames Ziehen am Zahlenfeld ergibt 0,02° pro Pixel; per Doppelklick lässt sich der Wert eintippen. **◀ ▶** gehen in Schritten von 0,1° (mit Umschalt 1°). |
| **Brennweite / Gieren / Neigen / Rollen zurücksetzen** | Setzt diese vier Regler zurück. |

Der Titel der Ansicht zeigt Panoramagröße, Brennweite in Pixeln, Bildwinkel des
Referenzbilds und den typischen Ausrichtungsfehler.

Diese Elemente aktualisieren die Vorschau sofort (sobald eine Vorschau
existiert).

### 5.5 Schritt 4: Zusammensetzen (Compose)

| Element | Bedeutung |
|---|---|
| **Überblendung** | **intelligent** (Standard): Mehrband-Überblendung mit Geisterbild-Entfernung. Feine Details stammen an jeder Stelle aus einem einzigen Bild, sodass keine Doppelbilder von Menschen, Lichtern oder nahen Objekten entstehen. Helligkeitsunterschiede werden über eine breite Zone überblendet, sodass keine harten Übergänge entstehen. Wo sich überlappende Bilder widersprechen (etwas hat sich bewegt), wird der ganze Bereich aus einem Bild übernommen. **weiche Überblendung**: einfaches weiches Mischen; zeigt Geisterbilder, wo sich etwas bewegt hat. **Naht**: jedes Pixel aus dem zentralsten Bild; keine Geisterbilder, aber harte Übergänge. **Mittelwert**: einfacher Mittelwert; hauptsächlich zum Prüfen der Ausrichtung. |
| **Vorschaugröße (px)** | Größe der Vorschau (längere Seite). Größer ist schärfer, aber langsamer. Beim Hineinzoomen werden Details ohnehin nachberechnet (siehe [In der Ansicht navigieren](#6-in-der-ansicht-navigieren)). |
| **Belichtung (EV)** | Hellt das ganze Panorama auf oder dunkelt es ab, von −3 bis +8 EV (+1 EV = doppelt so hell). Dunkle Astro-Panoramen brauchen oft +1,5 bis +4. |
| **Helligkeit (Ausw.)** | Helligkeitsfaktor der **ausgewählten** Bilder. |
| **nicht verbundene Bilder zeigen** | Zeigt auch nicht verbundene Bilder in einer Reihe unter dem Panorama, zum Platzieren von Hand. |
| **Automatische Belichtung** | Setzt **Belichtung** anhand der aktuellen Vorschau: typische Helligkeit auf Mittelgrau, ohne mehr als die hellsten 0,5 % (Sterne, Lampen) zu beschneiden. |
| **Angleichen** | Berechnet Helligkeitsfaktoren je Bild, damit die Überlappungen zusammenpassen. |
| **Verschiebungen zurücksetzen** | Macht alle manuellen Verschiebungen rückgängig. |
| **Automatisch beschneiden** | Beschneidet das Panorama auf das größte Rechteck, das vollständig von Bildern bedeckt ist, sodass die schwarzen, ausgefransten Ränder verschwinden. Die Vorschau zeigt dann das beschnittene Ergebnis, und **Panorama speichern…** speichert es. Die Statuszeile nennt, wie viel vom Panorama erhalten bleibt. Richten Sie zuerst den Horizont aus (Pitch, Roll); von einem gebogenen Panorama bleibt nur ein schmaler Streifen. |
| **Zuschnitt aufheben** | Hebt den Beschnitt auf. Der Beschnitt wird auch automatisch aufgehoben, wenn Sie etwas ändern, das die Bilder verschiebt (Projektion, Brennweite, Yaw, Pitch, Roll, Verschiebungen, Zuordnung). |
| **Nudge ← ↑ ↓ →** | Verschiebt das aktuelle Bild um ein Vorschau-Pixel (siehe unten). |

In der Ansicht **Panorama** wird der Umriss jedes Bildes gezeichnet und
nummeriert:

- **orange**: das aktuelle Bild
- **hellblau**: weitere ausgewählte Bilder
- **cyan**: das Referenzbild
- **weiß**: die übrigen Bilder
- **gestrichelte gelbe Linien**: die zur Platzierung verwendeten Verbindungen

Mit dem Schalter **Überlagerungen** in der Werkzeugleiste blenden Sie diese aus und
sehen das Panorama selbst.

**Ein Klick in einen Umriss** wählt das Bild aus (funktioniert auch mit
ausgeblendeten Überlagerungen).

**Bilder von Hand verschieben:** **Strg** gedrückt halten und ein Bild ziehen,
oder die Pfeiltasten bzw. die **Auswahl verschieben**-Schaltflächen verwenden. Das Bild
bewegt sich zusammen mit den Bildern, die weiter vom Referenzbild entfernt an
ihm hängen (entlang der gelben Verbindungen). Das Referenzbild bewegt sich
nicht; das ganze Panorama verschieben Sie mit Pitch und Roll.

### 5.6 Schritt 5: Histogramm-Streckung (Stretch)

Astronomische Bilder (und dunkle RAW-Umwandlungen) sind meist **linear**: Die
interessanten Details liegen in den dunkelsten paar Prozent des
Helligkeitsbereichs, und das Bild wirkt fast schwarz. Die Histogramm-Streckung
macht sie sichtbar.

| Element | Bedeutung |
|---|---|
| **Histogramm** | Helligkeitsverteilung der Panorama-Vorschau (in der Ansicht Panorama) bzw. des aktuellen Bildes (in den anderen Ansichten), logarithmisch. Die **blaue Linie** markiert den Schwarzpunkt, die **weiße Linie** den Weißpunkt, und die **orange Kurve** zeigt, wie die Eingangshelligkeit (links nach rechts) auf die Ausgangshelligkeit (unten nach oben) abgebildet wird. |
| **Schwarzpunkt** | Alles mit dieser oder geringerer Helligkeit wird schwarz. Knapp unter den Himmelshintergrund setzen (linker Rand des Hauptgipfels im Histogramm). |
| **Mitteltöne** | Hellt die Mitteltöne auf (kleinere Werte) oder dunkelt sie ab (größere Werte), ohne zu beschneiden. 0,5 ist neutral. Deep-Sky-Bilder brauchen typischerweise 0,005–0,05. |
| **Weißpunkt** | Alles mit dieser oder höherer Helligkeit wird weiß. |
| **Automatisch strecken** | Setzt alle drei Werte anhand des aktuellen Histogramms: Schwarzpunkt knapp unter dem Himmelshintergrund, Mitteltöne so, dass der Hintergrund bei etwa 25 % Grau landet (dasselbe Prinzip wie die automatische Bildschirmstreckung in PixInsight). |
| **Strecken zurücksetzen** | Zurück zu keiner Streckung (0 / 0,5 / 1). |

- Die Streckung ändert nur, wie Bilder **angezeigt** werden und wie das
  Panorama als TIFF, PNG oder JPEG gespeichert wird. Die Zuordnung arbeitet
  immer mit den Originaldaten.
- **FITS-Dateien** werden beim Laden automatisch zur Anzeige gestreckt,
  basierend auf allen geladenen FITS-Bildern zusammen.
- Änderungen der Streckung wirken sofort; nichts muss neu berechnet werden.
- **Belichtung** wird vor der Streckung angewendet.

### 5.7 Rückgängig, Wiederherstellen und Projekte

**Rückgängig / Wiederherstellen:** **Rückgängig** und **Wiederholen** in der Werkzeugleiste
oder **Strg+Z** und **Strg+Y** (auch **Strg+Umschalt+Z**). Die Statuszeile
nennt, was rückgängig gemacht wurde, z. B. *„Undone: Pitch (°) 0 → 10“*.

- Rückgängig machen lässt sich: jeder Regler, jedes Menü und jeder Schalter
  der Einstellungen; Hinzufügen, Entfernen und Umsortieren von Bildern;
  Zuordnungsergebnisse (Match, Re-match, Discard); das Referenzbild; manuelle
  Verschiebungen; Helligkeit; automatischer Beschnitt.
- Ein Ziehen am Regler zählt als **ein** Schritt, egal wie lange Sie ziehen.
  Änderungen an zwei verschiedenen Elementen sind immer zwei Schritte.
- Entfernte Bilder bleiben für das Rückgängigmachen im Speicher und sind
  daher sofort wieder da, samt ihrer Zuordnungsergebnisse.
- Die letzten 100 Schritte werden behalten. Nicht rückgängig zu machen:
  reine Anzeigedinge (Zoom, gewählte Ansicht, Überlagerungen), *RAW über* und
  *half size* sowie das Speichern von Dateien.

**Projekte:** **Projekt speichern…** (oben in der linken Spalte) schreibt eine
Projektdatei mit der Endung **`.panoproj`**. **Projekt öffnen…** lädt sie wieder.
Sie können den Pfad einer `.panoproj`-Datei auch in das Pfadfeld tippen und
Enter drücken oder das Programm damit starten:
`panostitch mein_pano.panoproj`.

Ein Projekt speichert:

- die Bilder (vollständiger Pfad und Pfad relativ zur Projektdatei)
- alle Einstellungen: Regler, Menüs, Schalter, einschließlich der Streckung
- die Zuordnungsergebnisse, sodass nichts neu zugeordnet werden muss
- manuelle Verschiebungen, Helligkeitsfaktoren, das Referenzbild und den
  Beschnitt
- *RAW über* und *half size*, damit die Bilder wieder genauso geladen werden

Die Bilder selbst sind **nicht** enthalten; sie werden aus ihren Dateien
geladen. Verschieben Sie die Fotos zusammen mit der Projektdatei (z. B. den
ganzen Ordner), werden sie über den relativen Pfad gefunden. Fehlende Dateien
werden gemeldet und ausgelassen.

Die Datei ist lesbarer Text (TOML-Format). Ihre Größe besteht hauptsächlich aus
den gespeicherten Zuordnungspunkten: etwa 50 kB pro Bild.

---

## 6. In der Ansicht navigieren

| Aktion | Maus | Tastatur | Schaltfläche |
|---|---|---|---|
| Verschieben | mit der **linken** Taste ziehen | Pfeiltasten (Ansichten Image und Pair) | |
| Hineinzoomen | Mausrad nach oben | `+` | **Zoom +** |
| Herauszoomen | Mausrad nach unten | `-` | **Zoom -** |
| Zoom zurücksetzen | | `F`, `0` oder `Pos1` | **Einpassen** |
| Bild auswählen (Ansicht Panorama) | in seinen Umriss klicken | | |
| Umrisse, Nummern, Verbindungen ein/aus | | | Schalter **Überlagerungen** |
| Bild verschieben (Ansicht Panorama) | **Strg + Ziehen** | Pfeiltasten (`Umschalt` = 10 Schritte) | **← ↑ ↓ →** |
| Rückgängig | | `Strg+Z` | **Rückgängig** |
| Wiederherstellen | | `Strg+Y` oder `Strg+Umschalt+Z` | **Wiederholen** |

Die Schaltflächen **Bild + Merkmale**, **Paarzuordnungen** und **Panorama**
wechseln zwischen den drei Ansichten.

**Weitere Tastenkürzel** (zeigt der Tooltip einer Schaltfläche):

| Tasten | Panorama stitcher | Star stacking |
|---|---|---|
| `Strg+Enter` | **Alle Schritte ausführen** | **Start** |
| `P` / `Strg+P` | `P`: **Panoramavorschau** | `Strg+P`: **Schnellvorschau** |
| `Strg+S` | Projekt speichern (in die offene Projektdatei sofort; beim ersten Mal mit Namensabfrage) | ebenso |
| `Strg+Umschalt+S` | **Panorama speichern…** | |
| `Strg+O` | **Projekt öffnen…** | ebenso |
| `Esc` | | laufendes Stacking abbrechen |
| `F` | Bild in die Ansicht einpassen | ebenso |

Unter macOS `Cmd` statt `Strg`.

**Beim Schließen des Fensters** wird zuerst gefragt, wenn ein Stacking läuft
oder die Arbeit in einem Tab nicht in einer Projektdatei gespeichert ist. Sie
können dann das Projekt speichern, trotzdem beenden oder abbrechen.

**Scharfer Zoom:** Die Vorschau ist ein verkleinertes Bild. Wenn Sie in das
Panorama hineinzoomen und kurz warten, wird der sichtbare Bereich in
Bildschirmauflösung aus den Originalbildern neu berechnet. So lassen sich
feine Details und Übergänge prüfen.

---

## 7. Projektionen

Ein Panorama umfasst mehr, als ein flaches Foto zeigen kann, daher muss das
Programm die Ansicht auf eine Ebene *projizieren*. Jede Projektion verzerrt
etwas:

| Projektion | Wirkt wie | Am besten für | Grenzen |
|---|---|---|---|
| **geradlinig** | ein normales Foto: gerade Linien bleiben gerade | schmale Panoramen (bis ~100°), Architektur, Teleskop-Mosaike | mit wachsendem Bildwinkel werden die Ränder extrem gedehnt |
| **zylindrisch** | waagerechte Linien biegen sich, senkrechte bleiben senkrecht | die meisten waagerechten Panoramen | nicht für hohe Aufnahmen (oben und unten gedehnt) |
| **sphärisch** (sphärisch, equirektangular) | wie zylindrisch, aber vertikal weniger gedehnt | sehr breite oder 360°-Panoramen, Panorama-Viewer | gerade Linien krümmen sich |
| **Mercator** | zwischen zylindrisch und sphärisch | breite Panoramen mit etwas Höhe | Dehnung nahe dem oberen und unteren Rand |
| **stereographisch** (stereografisch) | fischaugenähnlich, Formen bleiben rund | sehr große Bildwinkel, kreative Aufnahmen | starke Krümmung |
| **transversal zylindrisch** | zylindrisch, auf die Seite gelegt: senkrechte Linien biegen sich, waagerechte bleiben gerade | **senkrechte Panoramen** (Horizont bis Zenit), der aufgerichtete Milchstraßenbogen | nicht für breite waagerechte Schwenks (sie laufen über den oberen und unteren Rand) |
| **transversaler Mercator** (transversal Mercator) | wie transversal zylindrisch, seitlich weniger gedehnt | senkrechte Panoramen mit etwas Breite | wie oben |
| **Fisheye** (Fischauge, abstandstreu) | rundes Bild, Abstand zur Mitte proportional zum Winkel | **Ganzhimmelansichten**: mit Pitch so neigen, dass der Zenit in der Mitte liegt | Ränder gestaucht |
| **Fisheye flächentreu** (Fischauge, flächentreu) | rundes Bild, das Flächenverhältnisse erhält | Ganzhimmelansichten, so wie die meisten Fischaugenobjektive abbilden | Ränder stärker gestaucht |
| **kleiner Planet** | der Boden in der Mitte, der Himmel als Ring darum | 360°-Panoramen mit Boden, kreativ | das Panorama muss weit nach unten reichen |
| **Tunnel** | der Himmel in der Mitte, der Horizont darum | 360°-Nachthimmel, Sternspuren, Bäume von unten | das Panorama muss weit nach oben reichen |
| **orthographisch** (orthografisch) | eine Kugel von außen gesehen | kreativ; zeigt eine Hälfte der Kugel | die Ränder sind stark gestaucht |
| **Hammer** (Hammer–Aitoff) | ovale Karte der ganzen Kugel mit richtigen Flächenverhältnissen | Panoramen der ganzen Kugel als ein Bild, Ganzhimmel-Sternkarten | Formen am Rand verzerrt |
| **Mollweide** | ovale flächentreue Karte, an den Polen flacher | wie Hammer; die klassische Ganzhimmelkarte der Astronomie | Formen am Rand verzerrt |

Sieht der Horizont wie ein Lächeln oder ein Stirnrunzeln aus, passen Sie
**Neigung** an. Ist er schief, passen Sie **Rollen** an. **Gieren** dreht die Ansicht
seitlich. Bei Fischauge, kleinem Planeten und Tunnel bestimmt Pitch, was in
der Mitte liegt.

---

## 8. RAW-Dateien

Unterstützt: **Canon CR2 und CR3, Nikon NEF und NRW, Sony ARW und SRF, DNG,
Fuji RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW** und weitere.
Die genaue Kameraunterstützung hängt vom Decoder ab; der eingebaute kennt die
meisten Kameras der letzten zwei Jahrzehnte, LibRaw (falls installiert) etwa
1250 Kameras.

**RAW über** wählt den Decoder:

| Option | Decoder | Eigenschaften |
|---|---|---|
| **auto** | der erste, der funktioniert, in der Reihenfolge unten; hat ein Foto eine `.pp3`-Datei, zuerst RawTherapee | empfohlen |
| **eingebaut** | der eigene Decoder des Programms | immer verfügbar, schnell; neutrale Umwandlung mit den Farben und dem Weißabgleich der Kamera, ohne automatisches Aufhellen, sodass alle Bilder dieselbe Belichtung behalten |
| **libraw** | `dcraw_emu` (LibRaw) | ähnlich wie built-in; für Kameras, die der eingebaute Decoder nicht kennt |
| **rawtherapee** | `rawtherapee-cli` | **verwendet Ihre RawTherapee-Bearbeitungen** aus `foto.NEF.pp3`; sonst das Standard-Aussehen von RawTherapee |
| **darktable** | `darktable-cli` | Standard-Umwandlung von darktable (einschließlich Objektivkorrektur, wo verfügbar); am langsamsten |
| **dcraw** | `dcraw` | Notlösung; kann kein CR3 lesen |

Nur built-in ist immer verfügbar; die anderen erscheinen, wenn das jeweilige
Programm auf dem Computer installiert ist. Das Sterne-Stacken braucht lineare
Daten und verwendet daher nur built-in, libraw oder dcraw.

Die externen Decoder schreiben ein 16-Bit-Bild in einen temporären Ordner,
der danach gelöscht wird. Neben Ihre Fotos wird nichts geschrieben.

**Tipp:** Für einheitliche Farben im ganzen Panorama verwenden Sie für alle
Bilder dieselben Bearbeitungen (dasselbe `.pp3`-Profil), oder nehmen Sie
**eingebaut** und korrigieren Sie die Farben anschließend.

---

## 9. FITS-Dateien (Astronomie)

Dateien mit der Endung **`.fits`, `.fit` oder `.fts`** werden direkt gelesen,
ohne zusätzliche Software. Unterstützt:

| Inhalt | Beispiele | Wie es gelesen wird |
|---|---|---|
| **8-, 16-, 32- und 64-Bit-Ganzzahlen, 32- und 64-Bit-Gleitkomma** | Kameraaufnahmen, Stacking-Ergebnisse | auf 0…1 skaliert (16 Bit vorzeichenlos: 0…65535) |
| **Farbe (3 Ebenen)** | gestackte Ergebnisse aus Siril, PixInsight, DSS | als RGB |
| **Mono** | Monochromkameras, einzelne Filter | als Grau |
| **Rohdaten von Farbkameras (Bayer)** | Bilder von ZWO, ToupTek, QHY … mit Header `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | Debayering; mit **halbe Größe** ein Farbpixel pro 2×2-Zelle (schnell, sauber), sonst Interpolation in voller Größe |
| **Zeilenreihenfolge** | `ROWORDER` = `TOP-DOWN` oder `BOTTOM-UP` (FITS-Standard) | das Bild wird richtig herum angezeigt |

Nicht unterstützt: **kachelkomprimierte Dateien (`.fz`)**. Entpacken Sie diese
zuerst, z. B. mit `funpack`. Weitere Teile einer Datei (Tabellen,
ICC-Profile) werden ignoriert; verwendet wird das erste Bild.

**Hinweise zu Deep-Sky-Mosaiken:**

- Verwenden Sie die Voreinstellung **Deep-Sky-Mosaik**. Teleskop-Kacheln
  verschieben und drehen sich nur (Bildfeldrotation), was das Modell
  *similarity* genau beschreibt.
- **Gestackte, kalibrierte Kacheln** lassen sich am besten zusammenfügen.
  Einzelne kurze Aufnahmen haben wenige sichtbare Sterne und viel Rauschen.
- Rohdaten von Farbkameras sind nicht weißabgeglichen und wirken daher grün;
  die Streckung ändert die Farbbalance nicht.
- Speichern Sie als **FITS**, um in Siril oder PixInsight weiterzuarbeiten
  (Gradientenentfernung, Farbkalibrierung, endgültige Streckung). Die
  FITS-Ausgabe behält die linearen Werte.

---

## 10. Speichern

Klicken Sie auf **Panorama speichern…** und wählen Sie einen Dateinamen. Der
Dateityp richtet sich nach der Endung:

| Endung | Ergebnis |
|---|---|
| `.tif` / `.tiff` | 16 Bit pro Kanal, am besten für die Weiterbearbeitung; gespeichert **wie angezeigt** (mit Belichtung und Streckung) |
| `.png` | 16 Bit pro Kanal; wie angezeigt |
| `.jpg` | 8 Bit pro Kanal, kleinere Datei; wie angezeigt |
| `.fits` / `.fit` / `.fts` | 32-Bit-Gleitkomma, drei Farbebenen; **linear** (Belichtung und Helligkeitsfaktoren angewendet, aber **keine Streckung** und kein Beschneiden), für Astronomie-Software |

Sind alle geladenen Bilder FITS, endet der vorgeschlagene Dateiname auf
`.fits`.

**Ausgabegröße** (über der Speichern-Schaltfläche) legt die Größe des
gespeicherten Panoramas fest, in % der vollen Kameraauflösung; die Zeile
darunter zeigt die Größe in Pixeln (nach einer Vorschau). ¼, ½, full und 2×
sind Schnellwahlen. 100 % behält alle Details; mit **halbe Größe** geladene
RAW-Dateien werden beim Speichern erneut in voller Größe gelesen, das braucht
mehr Zeit und Speicher. 50 % oder 25 % ergeben schnell eine kleinere Datei,
über 100 % wird vergrößert (ohne neue Details).

Darunter zeigt eine Schätzung, wie viel Speicher das Speichern braucht und wie
groß die Datei werden kann, neben dem freien Platz. Passt es nicht, wird sie
orange (⚠); der Tooltip sagt, was zu tun ist. Das Speichern lehnt dann mit
demselben Rat ab, statt den Speicher zu füllen, wodurch der ganze Computer
nicht mehr reagieren würde. Das Laden hält bei fast vollem Speicher früher an
und meldet es.

Ein großes Panorama, das nicht auf einmal in den Speicher passt, wird in
Streifen von Zeilen berechnet; die Statuszeile zählt dann die Streifen. Ganz
gehalten wird nur das fertige Panorama, im Format der Datei (3 Byte pro Pixel
für JPEG, 6 für 16-Bit-TIFF und PNG), und Fotos, die in voller Größe neu
gelesen werden, in halber Genauigkeit. Ein 16-Bit-TIFF mit 20 000 × 8 000 px
braucht so etwa 1 GB plus einen Streifen und die Fotos, wo es früher etwa
8 GB brauchte. Beim Überblenden **Smart** wird jeder Streifen mit Überlappung
berechnet, sodass die Streifen ohne sichtbare Stufe aneinander anschließen.
Das Speichern nutzt nie mehr als 85 % des Speichers.

Panoramen, die breiter oder höher als 60 000 px würden, werden abgelehnt. Das
bedeutet fast immer, dass ein Paar falsch zugeordnet ist oder die Projektion
ungeeignet ist; probieren Sie cylindrical oder spherical.

Während der Arbeit zeigt die Statuszeile die vergangene Zeit. Nach dem
Speichern fasst ein kurzer **Bericht** unter dem Save-Knopf alles zusammen:
wie viele Fotos im Panorama sind (und wie viele nicht passten), die Zeit zum
Zusammenfügen (Laden, Zuordnen und Vorschauen seit dem ersten Foto), die Zeit
zum Speichern, die Größe in Pixeln und die Datei mit ihrer Größe, oder den
Fehler, falls das Speichern scheiterte. × schließt ihn.

---

## 11. Fehlerbehebung

| Problem | Lösungsansatz |
|---|---|
| **Kein Fenster erscheint** | Unter Windows nennt eine Meldung den Grund; unter Linux und macOS das Programm im Terminal starten und die Fehlermeldung dort lesen. Das Fenster braucht OpenGL 2.0 oder neuer: Grafiktreiber aktualisieren (bei alten PCs den des Grafikchip-Herstellers). |
| **Das Programm wurde unerwartet beendet** | Beim nächsten Start meldet es die Statuszeile. Die Details stehen in `crash.log` im Einstellungsordner (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`); bitte beim Melden des Problems anhängen. |
| **Bedienelemente am Rand abgeschnitten** | Den Rand zwischen Seitenleiste und Bild breiter ziehen. |
| **„Busy – wait…“** | Ein Schritt läuft noch; warten Sie, bis sich die Statuszeile ändert. |
| **Ein Bild wird beim Laden übersprungen** | Die Statuszeile nennt Datei und Grund. Bei RAW einen anderen Konverter unter **RAW über** probieren. `.fz`-FITS-Dateien zuerst entpacken. |
| **Panorama ist fast schwarz** | Fotos: **Automatische Belichtung** klicken oder **Belichtung (EV)** erhöhen (Nachtaufnahmen brauchen oft +1,5 bis +4), oder die Fotos in RawTherapee bearbeiten (deren `.pp3`-Dateien werden dann verwendet). FITS und andere lineare Daten: **Automatisch strecken** klicken. |
| **FITS-Bilder sind nach dem Laden schwarz oder weiß** | Die automatische Streckung beim Laden beruht auf allen FITS-Bildern; ein stark über- oder unterbelichtetes Bild kann sie verfälschen. Solche Bilder entfernen, dann **Automatisch strecken** klicken (oder **Strecken zurücksetzen** und von Hand einstellen). |
| **FITS-Bilder wirken grün** | Rohdaten von Farbkameras sind nicht weißabgeglichen. Zuerst stacken und farbkalibrieren, oder die Farben des Ergebnisses in Astronomie-Software korrigieren. |
| **Wenige gelbe Punkte** | **Eckenschwelle** senken; **Max. Merkmale** erhöhen; **Erkennungsgröße** erhöhen. |
| **Punkte vor allem auf Gras, Himmel oder Rauschen** | **Entrauschen** auf 2–3 erhöhen. |
| **Bilder mit UNLINKED markiert** | Ansehen: Überlappen sie wirklich mit einem anderen Bild? **Verhältnistest** (0,85–0,9) und **Max. Abstand** (0,4) erhöhen, **Min. Inlier** senken, **Entrauschen** erhöhen, dann erneut **Zuordnen**. Klappt es weiterhin nicht, **nicht verbundene Bilder zeigen** einschalten und die Bilder mit **Strg + Ziehen** platzieren. |
| **Wolken, Wasser oder andere bewegte Dinge** | Sie verändern sich zwischen den Aufnahmen und lassen sich daher nicht zuverlässig zuordnen (Bilder nur mit Wolken bleiben meist unverbunden). Solche Teile schnell nacheinander aufnehmen oder von Hand platzieren. |
| **Ein falsches Paar wird verwendet** (Linien in alle Richtungen, Bilder am falschen Platz) | Im Menü **Paar** auswählen und **Paar verwerfen** klicken. Mit eingeschaltetem **verfeinern** werden die meisten solchen Paare automatisch verworfen. |
| **Geisterbilder (doppelte Objekte)** | **verfeinern** eingeschaltet lassen und **Blend → smart** (Standard) verwenden. Verbleibende Probleme stammen von Dingen, die sich zwischen den Aufnahmen verändert haben (z. B. eine Taschenlampe, die nur auf einem Foto leuchtet), oder von **Parallaxe**: Nahe Objekte verschieben sich gegenüber dem Hintergrund, wenn die Kamera nicht um ihr Objektiv (den „Nodalpunkt“) gedreht wird. |
| **Sichtbare Helligkeitssprünge** | **Angleichen**, dann einzelne Bilder mit **Helligkeit (Ausw.)** feinjustieren. |
| **Gebogener Horizont** | **Neigung** anpassen (und **Rollen**, wenn er schief ist). |
| **Oben oder unten stark gedehnt** | Projektion **sphärisch** verwenden; die Statuszeile schlägt sie vor, wenn das Panorama weit nach oben oder unten reicht. |
| **Panorama gestaucht oder gedehnt** | **Brennweite (× auto)** anpassen. |
| **Riesiges, verzerrtes Panorama** | Ein falsches Paar (Paare prüfen), oder **sphärisch** / **zylindrisch** statt rectilinear verwenden. |
| **Langsam oder Speicher reicht nicht** | Vor dem Laden **halbe Größe** einschalten; **Vorschaugröße** senken. Bilder in voller Auflösung brauchen etwa 12 Byte pro Pixel, rund 300 MB für ein 24-MP-Bild. Bei vielen Bildern dauert die Zuordnung *all pairs* länger; *grid* ist schneller, wenn Sie die Anordnung kennen. |
| **Auto crop lässt nur einen schmalen Streifen übrig** | Das Panorama ist gebogen. Zuerst mit **Neigung** und **Rollen** begradigen (und eine passende Projektion wählen), dann erneut **Automatisch beschneiden**. |
| **Ein Projekt öffnet sich ohne einige Bilder** | Die Statuszeile listet die fehlenden Dateien auf. Sie wurden verschoben oder umbenannt, und nicht zusammen mit der Projektdatei. Zurücklegen oder erneut hinzufügen und zuordnen. |
| **Rückgängig tut nichts** | Rückgängig wirkt auf abgeschlossene Änderungen; nach der letzten Änderung kurz warten. Während ein Schritt läuft (*Busy*), wartet es. |
| **Alles ist langsam** | Vor dem Laden **halbe Größe** einschalten (RAW und FITS von Farbkameras); **Vorschaugröße** verringern. Im Stacking-Tab **Halbe Größe** unter RAW conversion. |

---

## 12. Tipps für gute Panoramen

- **Überlappung** benachbarter Aufnahmen von 30–50 %, auch zwischen den Reihen.
  Bei Teleskop-Mosaiken genügen meist 15–25 %.
- **Drehen Sie die Kamera** um das Objektiv, nicht um sich selbst. Wenn
  möglich, ein Stativ verwenden, am besten mit Panoramakopf.
- **Belichtung, Weißabgleich und Fokus fixieren** (manueller Modus), damit alle
  Bilder zusammenpassen.
- Bewegte Objekte in den Überlappungsbereichen vermeiden oder nur in einer
  Aufnahme festhalten.
- **Kamera waagerecht halten.** Eine geneigte Kamera biegt den Horizont
  (Korrektur mit Pitch und Roll).
- Bei langen Serien das Ergebnis zuerst mit eingeschaltetem **halbe Größe**
  prüfen.
- Wenn Sie Bilder ordnerweise laden, speichern Sie das Ergebnis nicht in den
  Ordner der Quellbilder. Dateien namens `panorama…` werden übersprungen,
  andere Namen aber nicht.

---

## 13. Sterne stacken

Der zweite Reiter, **Sterne stacken (Landschaft / Spuren)**, vereint viele
Aufnahmen derselben Szene zu einem Bild: einem rauscharmen Himmel mit scharfen
Sternen oder zu Strichspuren. Er ist für Landschafts-Astrofotografie gedacht:
Die Kamera steht auf einem Stativ, der Himmel bewegt sich zwischen den
Aufnahmen, der Vordergrund nicht.

Er arbeitet wie Sequator, mit dem, was Sequator fehlt: Bias- und
Flat-Dark-Bilder, lineare 32-Bit-Ausgabe, ein eigenes Vordergrundbild,
Strichspuren ohne Lücken, automatische Himmelserkennung und keine Blackbox:
Jede Aufnahme wird vermessen und bewertet, ein Diagramm zeigt die Messwerte,
und ein Protokoll listet jeden Schritt mit dem, was er getan hat, und seiner
Dauer.

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ Eingabeliste         │ [Base image] [Result] [Frame report] │ Post-processing     │
│  Star images, Base,  │ [Fit] [Open project…] [Save project…]│  Background extr.   │
│  Kalibrierung, Fore- │                                      │  Color calibration  │
│  ground, Output      │                                      │                     │
│ Add Remove Clear     │   Basisbild (hier wird der Himmel    │  Colour noise       │
│ Use as base          │   markiert), Ergebnis oder Bericht   │  Light pollution    │
│ Einstellungsliste    │                                      │  Brightness, Tone   │
│ Optionen des         │                                      │  Color space        │
│ markierten Eintrags  │                                      │  [Save result…]     │
│                      │                                      │  [Use in Stitcher]  │
│ [Quick prev.] [Start]│  Statuszeile                         │                     │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

**Klicken Sie auf einen Eintrag** in der Eingabeliste oder der
Einstellungsliste: Darunter erscheinen seine Optionen und eine Erklärung.

**Vorgaben** (über der Einstellungsliste) stellen alles für ein typisches
Motiv mit einem Klick ein: **Milchstraßen-Landschaft** (Vordergrund eingefroren,
Himmelsregion automatisch erkannt, Verzeichnungskorrektur,
Lichtverschmutzung), **Strichspuren** (Lücken gefüllt) und **Deep sky
(telescope)** (das ganze Bild ist Himmel, Hintergrundextraktion,
Farbkalibrierung, automatische Streckung). Jede Einstellung lässt sich
weiterhin ändern. Während eines Laufs zeigt ein Fortschrittsbalken unter der
Ansicht den Schritt, die vergangene Zeit und die geschätzte Restzeit. Am Ende
erscheint oben im rechten Bereich ein kurzer **Bericht**: wie viele Bilder von
wie vielen gestackt wurden (ausgelassene und ausgeschlossene mitgezählt), die
Kalibrierbilder, die benötigte Zeit, die Größe des Ergebnisses und die
gespeicherte Datei mit ihrer Größe, oder was schiefging. × schließt ihn.

### 13.1 Schnellstart

1. Markieren Sie **Lights**, klicken Sie auf **Hinzufügen…** und wählen Sie die
   Himmelsaufnahmen (oder tippen Sie einen Datei- oder Ordnerpfad in das
   Pfadfeld und drücken Sie Enter).
2. Markieren Sie **Himmelsregion**, wählen Sie **Grenzlinie** und ziehen Sie in
   der Basisbild-Ansicht eine Linie entlang des Horizonts, oder klicken Sie
   auf **Himmel erkennen** und korrigieren Sie die Maske mit dem Pinsel. Sterne
   werden nur im Himmel gesucht.
3. Klicken Sie auf **Schnellvorschau**: ein kleines, schnelles Ergebnis aus
   höchstens 10 Aufnahmen, um die Einstellungen zu prüfen. Es wird nichts
   gespeichert.
4. Prüfen Sie die Zeile **Speichern als** unter der Eingabeliste: Dateinamen
   eintippen oder einen vorgeschlagenen Namen in der Liste daneben wählen, in
   der zweiten Liste den Dateityp, mit **Ordner…** den Ordner (Vorgabe:
   `stacked.tif` neben der ersten Himmelsaufnahme). Existiert die Datei schon,
   sagt die Zeile das; **nächster freier Name** hängt eine Nummer an. Darunter zeigt
   eine Schätzung, wie viel Platz die Aufnahmen beim Stacken brauchen (nach
   Anzahl und Größe) und ob sie im Speicher oder auf der Festplatte liegen;
   reicht der Platz nicht, wird sie orange (⚠). Dann klicken Sie auf
   **Start**.
5. Passen Sie rechts die Nachbearbeitung an; das Ergebnis ändert sich sofort.
   **Ergebnis speichern…** speichert es erneut mit den neuen Einstellungen.

### 13.2 Eingabedateien

| Eintrag | Was aufnehmen |
|---|---|
| **Lights** (*Sternbilder* im einfachen Modus) | Die Himmelsaufnahmen: gleiche Brennweite, Blende und Belichtung. RAW ist am besten; TIFF, FITS oder XISF nur, wenn die Daten noch linear (unbearbeitet) sind. **Belichtung vereinheitlichen** gleicht Aufnahmen unter verschiedenen Bedingungen an. |
| **Basisbild** (*Basisbild* im einfachen Modus) | Das Bild, an dem die anderen ausgerichtet werden. Leer = das mittlere Light-Bild. Ein anderes wählen Sie in der Auswahlliste neben **Basisbild**, oder wählen Sie ein Light-Bild in dessen Liste und klicken Sie auf **Als Basis verwenden**. |
| **Darks** (*Rauschbilder* im einfachen Modus) | Dunkelbilder (Darks): Objektivdeckel drauf, gleiche ISO und Belichtungszeit, direkt vor oder nach den Himmelsaufnahmen. Entfernen thermisches Rauschen und Hotpixel. |
| **Bias-Bilder** (*Bias-Bilder* im einfachen Modus) | Kürzeste Belichtung, Objektivdeckel drauf, gleiche ISO. |
| **Flats** (*Vignettierungsbilder* im einfachen Modus) | Flats: eine gleichmäßig beleuchtete weiße Fläche, gleiche Brennweite und Blende. Korrigieren dunklere Ecken und Staubschatten. |
| **Flat-Darks** (*Flat-Darks* im einfachen Modus) | Darks für die Flats: gleiche ISO und Belichtungszeit wie die Vignettierungsbilder. |
| **Vordergrundbild** | Freiwillig: eine Aufnahme des Bodens vom selben Stativstandort – zur blauen Stunde, lang belichtet oder mit niedriger ISO. Sie ersetzt unterhalb der Himmelsregion den (verrauschten, dunklen) gestackten Vordergrund. **Helligkeit angleichen** passt sie an die Helligkeit des gestackten Vordergrunds an; **Belichtung (EV)** macht sie von dort aus heller oder dunkler. Braucht eine Himmelsregion. |
| **Ausgabe** | Wird in der Zeile **Speichern als** gesetzt: Name, Typ und Ordner. `.tif` / `.png` (16 Bit), `.jpg` (8 Bit) oder `.fits` / `.xisf` (32-Bit-Gleitkomma, linear, zur Weiterverarbeitung in Siril, PixInsight usw.). |

Alle Kalibrierungsbilder sind freiwillig. Bei hoher ISO helfen schon wenige
Dunkelbilder viel.

**Die Dateilisten:** Jede Gruppe zeigt ihre Dateien in einer Auswahlliste
neben ihrem Namen, sodass auch Hunderte von Light-Bildern kein Scrollen
brauchen. Wählen Sie dort eine Datei, um sie in der Vorschau zu sehen;
**Entfernen** und **Als Basis verwenden** wirken auf diese Datei, **Leeren** leert die
ganze Gruppe.

**Verrauschte Flats** (zu wenige, oder in einer Farbe zu dunkel, z. B. bei
einer blauen Lichtquelle) werden automatisch geglättet; das Ablaufprotokoll
meldet das, auch wie verrauscht sie waren. Sonst würde ihr Rauschen jedem Bild
aufgeprägt und beim Wandern der Sterne zu feinen Streifen.

**Temporäre Dateien:** Zwischenbilder werden nicht gespeichert. Die Bilder
liegen im Arbeitsspeicher oder – wenn sie mehr als die Hälfte des freien
Speichers brauchen – in einer temporären Datei im Ausgabeordner, die sofort
nach dem Lauf gelöscht wird (sie ist währenddessen nicht einmal sichtbar).
Das Ablaufprotokoll nennt, was zutrifft. Reicht der Platz nicht, hält der
Lauf schon vor dem Start mit einer Meldung an: **Halbe Größe** einschalten (ein
Viertel des Platzes) oder die Ausgabe auf einem Laufwerk mit mehr freiem
Platz speichern. Auch Master (Dark, Bias, Flat) werden nicht gespeichert.

### 13.3 Einstellungen

| Einstellung | Wirkung |
|---|---|
| **Komposition** | **Sterne ausrichten**: Die Aufnahmen werden an den Sternen ausgerichtet und vereint. **Akkumulation** mittelt sie; **Beste Pixel wählen** (Vorgabe) lässt Ausreißer weg (Flugzeuge, Satelliten); **Boden einfrieren** richtet nur den Himmel aus, sodass Himmel und Vordergrund scharf bleiben (braucht eine Himmelsregion); **Nur ausrichten** schreibt nur die ausgerichteten Aufnahmen. **Strichspuren**: keine Ausrichtung, der Himmel behält den hellsten Wert jedes Pixels. **Spuren ausblenden**: **Einblenden** (die Spuren beginnen schwach, Sequators Bewegungseffekt), **Ausblenden** oder **Beide Enden** (kometenartig an beiden Enden). **Lücken zwischen den Bildern füllen**: Die kurzen Pausen des Kamera-Intervalls werden entlang der gemessenen Sternbewegung gefüllt, sodass die Spuren durchgehende Linien statt Striche sind. |
| **Himmelsregion** | Wo der Himmel ist. **Ganzer Himmel**: das ganze Bild ist Himmel (kein Vordergrund, z. B. Teleskopaufnahmen). **Grenzlinie**: eine Linie entlang des Horizonts ziehen. **Verlauf**: die Linie mit weichem Übergang. **Unregelmäßige Maske**: den Himmel mit der linken Maustaste malen, mit der rechten radieren; **Pinselgröße** (Schieberegler oder Mausrad). **Himmel erkennen** malt eine erste Maske aus Helligkeit und Sternen – ungefähr, also prüfen und mit dem Pinsel korrigieren. **Maske weichzeichnen** macht den Rand der Maske weich, nur zum Himmel hin, sodass der Vordergrund nichts vom Himmel abbekommt (keine Säume am Horizont). **Gefundene Sterne zeigen** kreist die Sterne ein, die der letzte Lauf in der gezeigten Aufnahme gefunden hat. |
| **Sigma-Clipping** | Wie streng Ausreißer entfernt werden (unten / oben × σ). Üblich sind 2–3; kleinere Werte entfernen mehr, lassen aber mehr Rauschen. Braucht mindestens 4 Aufnahmen, am besten 10 und mehr. |
| **Aussortieren** | Lässt Aufnahmen weg, die deutlich schlechter als die typische sind: Wolken oder Tau (schwächere Sterne), wenige Sterne, unscharfe oder längliche Sterne (Fokus, Wind, angestoßenes Stativ). |
| **RAW-Konvertierung** | Weißabgleich (Kamera, Bilddurchschnitt, Tageslicht), Demosaicing (Vorgabe AHD; DHT oder AAHD haben bei hoher ISO oft weniger Farbrauschen) und **halbe Größe** (4× weniger Pixel, viel schneller). |
| **Dynamisches Rauschen mindern** | Findet und entfernt Hotpixel auch ohne Dunkelbilder. |
| **Verzeichnungseffekte mindern** | Korrigiert Objektivverzeichnung, die in den Ecken langer Serien kurze Spuren hinterlässt: **Objektiv**, **Komplex** (Ultraweitwinkel, Refraktion am Horizont) oder **Auto**. |
| **4 Pixel zusammenfassen** | Addiert 2×2 Pixel: heller und rauschärmer, halbe Breite und Höhe. |
| **Zeitraffer** | Stackt jeweils N aufeinanderfolgende Aufnahmen zu einer; ergibt eine nummerierte Folge rauscharmer Bilder in einem Ordner neben der Ausgabe. |

### 13.4 Die Ansichten

- **Basisbild**: das Bild, an dem ausgerichtet wird; hier wird die
  Himmelsregion eingezeichnet. Ein Klick auf eine Datei in der Eingabeliste
  zeigt sie an.
- **Ergebnis**: der Stack nach der Nachbearbeitung. Ziehen verschiebt, das
  Mausrad zoomt, **Einpassen** zeigt alles.
- **Bildbericht**: ein Diagramm aller Messwerte pro Aufnahme, jeweils durch
  ihren Median geteilt, sodass eine schlechte Aufnahme auffällt (rot =
  weggelassen, blau = Basisbild); darunter eine Zeile pro Himmelsaufnahme:
  gefundene Sterne, Sterngröße (FWHM), Elongation, Himmelshelligkeit, Licht
  (Helligkeit der Sterne gegenüber dem Basisbild), Ausrichtungsfehler, ob die
  Aufnahme verwendet wurde und warum nicht. Unter der Tabelle steht das
  **Verarbeitungsprotokoll**: jeder Schritt des Laufs, was er getan hat
  (Kalibrierung, Himmelsregion, verwendete Aufnahmen, Verzeichnung,
  Lückenfüllung, Vordergrund, Nachbearbeitung) und wie lange er gedauert hat.
  **Ein Klick auf eine Aufnahme** im Diagramm oder in der Tabelle schließt sie
  aus (oder wieder ein); danach erneut **Start** klicken.

### 13.5 Nachbearbeitung

Wird sofort auf das Ergebnis angewendet; **Start** speichert mit diesen
Einstellungen, **Ergebnis speichern…** speichert erneut.

| Einstellung | Wirkung |
|---|---|
| **Hintergrundextraktion** | Zieht ein glattes Modell des Himmelshintergrunds ab (Verläufe durch Lichtverschmutzung oder Mondlicht). |
| **Farbkalibrierung** | Macht den Himmelshintergrund neutral grau und den durchschnittlichen Stern weiß. |
| **Farbrauschen mindern** | Glättet nur die Farben und entfernt die bunten Pünktchen von Aufnahmen mit hoher ISO. |
| **Lichtverschmutzung** | **Deep Sky**: entfernt ein glattes Glühen. **Ungleichmäßig**: folgt dem Glühen Kachel für Kachel (Lichtglocken von Städten im Weitwinkel). **Stärke** bestimmt, wie viel entfernt wird. Zuerst die Vignettierung korrigieren (Vignettierungsbilder), sonst wird es schlimmer. |
| **Sternlicht verstärken** | Hebt die Sterne über den Himmelshintergrund, ohne dessen Rauschen zu verstärken. |
| **Automatische Helligkeit** | Bringt den Himmel auf eine angenehme Helligkeit, ohne die hellsten Sterne zu beschneiden. |
| **Hoher Dynamikumfang** | Komprimiert helle Sterne und Lichter, statt sie zu beschneiden. |
| **Ton** | **sRGB-Kurve** (wie eine Kamera), **Strecken** (Schwarzpunkt, Mitteltöne, Weißpunkt; automatisch oder von Hand) oder **Linear** (keine Kurve). |
| **Farbraum** | sRGB oder Adobe RGB für TIFF- und JPEG-Ausgabe. |
| **Linear (zur Bearbeitung)** | Schaltet alles ab und speichert ein lineares 32-Bit-FITS: das, was Siril oder PixInsight erwarten. |
| **Im Stitcher verwenden** | Fügt das gespeicherte Ergebnis als ein Teilbild eines Panoramas in den Reiter Panorama stitcher ein (siehe 13.7). |

### 13.6 Projekte

**Projekt speichern…** schreibt eine `.stackproj`-Datei mit den Dateilisten, der
Himmelsregion und allen Einstellungen. **Projekt öffnen…** (oder `panostitch
nacht.stackproj`) lädt sie wieder. Dateien werden über ihren
vollständigen Pfad gefunden, oder relativ zur Projektdatei, wenn der Ordner
verschoben wurde.

### 13.7 Panoramen aus gestackten Teilbildern

Ein rauscharmes Milchstraßen-Panorama: Nehmen Sie an jeder Position des
Panoramas mehrere Aufnahmen auf (mit Überlappung zwischen den Positionen),
dann:

1. Stacken Sie die Aufnahmen der ersten Position (Start) und klicken Sie auf
   **Im Stitcher verwenden**.
2. Ersetzen Sie die Himmelsaufnahmen durch die der nächsten Position, geben
   Sie der Ausgabe einen neuen Namen, Start, wieder **Im Stitcher verwenden**;
   und so weiter.
3. Im Reiter Panorama stitcher fügt **Alle Schritte ausführen** die gestackten
   Teilbilder zusammen.

Speichern Sie jedes Teilbild als `.tif`; dieselbe Nachbearbeitung für alle
Teilbilder hält Helligkeit und Farben gleich.

### 13.8 Tipps

- Nehmen Sie mindestens 10–20 Himmelsaufnahmen mit gleichen Einstellungen
  auf, direkt danach 10–20 Dunkelbilder, ohne die Kamera zu bewegen.
- Setzen Sie die Himmelsregion sorgfältig: Bäume und Gebäude, die als Himmel
  zählen, verschmiert die Ausrichtung. **Himmel erkennen** ist ein schneller
  Anfang, nicht das letzte Wort.
- Sehen Sie nach einer Quick preview in den Frame report: Das Diagramm zeigt
  sofort, welche Aufnahmen schlechter sind (Wolken, Wind, Tau).
- Nehmen Sie **Boden einfrieren**, wenn der Vordergrund wichtig ist, sonst
  **Beste Pixel wählen**.
- Zur Weiterverarbeitung in anderen Programmen als `.fits` speichern und
  **Linear (zur Bearbeitung)** einschalten.
- Im Terminal stackt `panostack` ohne Fenster; `panostack --help` zeigt die
  Optionen (siehe [Verwendung ohne Fenster](#14-verwendung-ohne-fenster)).

---

## 14. Verwendung ohne Fenster

**Sterne stacken im Terminal:** `panostack` stackt genauso wie der Tab
Sterne stacken. `panostack --help` zeigt alle Optionen. Beispiele:

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o nacht.tif lights/
panostack --mode trails --fade in --fill-gaps --auto-sky -o spuren.jpg lights/
panostack --preview lights/            # schnelle Kontrolle, nichts wird gespeichert
```

Am Ende werden der Bildbericht und das Ablaufprotokoll ausgegeben.

**Zusammenfügen aus einem eigenen Rust-Programm:** PanoStitch als Abhängigkeit
eintragen (`panostitch = { path = "…/PanoStitch" }` in der `Cargo.toml`), dann:

```rust
use panostitch::io;
use panostitch::stitcher::{self, Projection, StitchParams};

fn main() -> Result<(), String> {
    let paths = vec!["a.NEF".to_string(), "b.NEF".into(), "c.NEF".into()];
    let p = StitchParams { projection: Projection::Spherical, ..Default::default() };
    let pano = stitcher::stitch_files(&paths, &p, true, &mut |m| println!("{m}"))?;
    io::save_panorama("pano.tif", &pano)
}
```

Ein vollständiges Beispiel ist `examples/scripted.rs`
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(pfade, parameter, raw_half, log)` lädt die Bilder und fügt sie
zusammen; `stitch(bilder, parameter, log)` fügt Bilder zusammen, die schon im
Speicher sind. Die Felder von `StitchParams` mit ihren Standardwerten:

| Feld | Standard | Bedeutung |
|---|---|---|
| `layout` | Layout::All | welche Paare zugeordnet werden: `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (all images), `false` | Gitteranordnung |
| `refine` | `true` | globale Verfeinerung |
| `feature_scale` | `0.5` | Maßstab der Merkmalsuche |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | jede aus [Projektionen](#7-projektionen), z. B. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | Modus feather: höher = schmalere Übergänge |

**FITS-Mosaik mit gestrecktem TIFF und linearem FITS-Ergebnis:**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // linear
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Sterne stacken im Code: einen `stacker::StackConfig` füllen (ausgehend von
`StackConfig::default()`) und `stacker::stack(&config, false, &progress, None)` aufrufen.

---

## 15. Dateien

| Datei | Zweck |
|---|---|
| `panostitch` (`panostitch.exe`) | Das Programm (das Fenster). |
| `panostack` (`panostack.exe`) | Sterne stacken von der Kommandozeile (`panostack --help`). |
| `*.panoproj`, `*.stackproj`, `*.tlproj` | Ihre Projektdateien (wo immer Sie sie speichern). |
| `ui.toml` im Einstellungsordner | Die Schalter für das Aussehen (siehe [Programm starten](#2-programm-starten)). |
| `remembered/`, `recent_projects.txt`, `last_folder.txt` im Einstellungsordner | Die für den nächsten Start gemerkten Einstellungen, die letzten Projekte, der Ordner, in dem Dateidialoge beginnen. Löschen, um frisch anzufangen. |
| `docs/manual_de.md` | Dieses Handbuch (Deutsch). |
| `docs/manual_en.md`, `docs/manual_cs.md`, `docs/manual_es.md`, `docs/manual_fr.md`, `docs/manual_pl.md` | Das Handbuch auf Englisch, Tschechisch, Spanisch, Französisch und Polnisch. |

Quellcode (um das Programm selbst zu bauen):

| Pfad | Inhalt |
|---|---|
| `Cargo.toml` | Das Projekt und die verwendeten Bibliotheken. |
| `src/image.rs`, `src/io/` | Bilder; Laden und Speichern (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Zusammenfügen: Merkmale, Zuordnung, Platzierung, Verfeinerung, Projektionen, Überblendung. |
| `src/stacker/` | Sterne stacken. |
| `src/finish.rs` | Nachbearbeitung: Geometrie, Streckungen (MTF, Arcsinh, GHS), Weißabgleich, Licht, Kurven, Farbe, Detail. |
| `src/gui/` | Das Fenster: die vier Tabs, die Dateidialoge, das eingebaute Handbuch und die Texte des Fensters in sechs Sprachen (`i18n_*.rs`). |
| `src/bin/` | Die beiden Programme, `panostitch` und `panostack`. |
| `tests/` | Tests: Stacken mit künstlichen Nachtszenen, Zusammenfügen, Nachbearbeitung, Dateiformate und die Tabs des Fensters. Ausführen mit `cargo test --release`. |
| `examples/scripted.rs` | Das Zusammenfügen aus eigenem Rust-Code verwenden. |

---

## 16. Nachbearbeitung

*Neu in 0.4 (in Entwicklung).* Der Tab **Nachbearbeitung** erledigt die
letzten Schritte an einem gestackten oder zusammengefügten Bild, ohne dass
ein anderes Programm nötig ist: drehen und zuschneiden, lineare Daten
strecken, Weißabgleich, Licht, Kurven, Farbe, Rauschen, Sterne, Schärfen und
Speichern in der gewünschten Größe.

**Ein Bild hineinbringen:** **Öffnen…** (Strg+O) oder einen Pfad eingeben und
Enter drücken; **✎ In der Nachbearbeitung verwenden** im Tab Sterne stacken; oder
**✎ Nachbearbeiten** im Stitcher nach dem Speichern eines Panoramas. FITS- und
XISF-Dateien gelten als lineare Daten und beginnen mit der automatischen
Streckung; JPEG, TIFF und PNG als fertige Bilder (ohne Streckung). **Linear
data (needs stretching)** schaltet das von Hand um.

Die Vorschau arbeitet mit einer Kopie von höchstens 2000 px und folgt den
Reglern daher fast sofort. **Speichern** bearbeitet das ganze Bild im Hintergrund
mit genau denselben Einstellungen; vom Programm gemessene Werte (automatische
Streckung, neutraler Hintergrund) werden aus der Vorschau übernommen, sodass
die gespeicherte Datei wie die Vorschau aussieht.

Die Schritte laufen immer in dieser Reihenfolge, links von oben nach unten:

| Abschnitt | Was er tut |
|---|---|
| **Drehen und beschneiden** | ⟲ ⟳ Vierteldrehungen, **Spiegeln**, **Drehen** um einen beliebigen Winkel (fein um 0°, wie Yaw/Pitch/Roll im Stitcher), **Leere Ecken abschneiden** (leere Ecken nach dem Drehen abschneiden), von jeder Seite zuschneiden oder auf ein Format (3:2, 4:3, 16:9, 4:5, 1:1, 2:1). |
| **Strecken** (lineare Daten) | Zuerst **Hintergrundextraktion** und **Farbkalibrierung** (beide optional, auf den linearen Daten), dann die Streckmethode (siehe unten). |
| **Weißabgleich** | Auf linearem Licht, vor der Streckung. **◎ Grau messen**, dann ein Rechteck über etwas ziehen, das neutral sein soll (Himmelshintergrund, eine Wolke): Danach werden die Farbfaktoren gesetzt. **Neutraler Hintergrund** macht die dunklere Hälfte grau. **Temperatur** und **Tönung** zur Feinabstimmung. Fertige Bilder werden dafür aus ihrer sRGB-Kurve genommen und zurückgesetzt, sodass der Abgleich bei jeder Helligkeit stimmt. |
| **Licht** | **Belichtung** (EV), **Kontrast**, **Lichter** (um 75 %), **Mitteltöne** (50 %), **Tiefen** (25 %), **Weiß** und **Schwarz** (die Endpunkte), wie in Fotoprogrammen. |
| **Kurven** | Eine Kurve für alle Farben (**RGB**) und je eine für **Rot**, **Grün**, **Blau**. Klicken fügt einen Punkt hinzu, Ziehen verschiebt ihn, Rechtsklick entfernt ihn. Das Histogramm hinter der Kurve zeigt, was in sie hineingeht. • markiert geänderte Kurven. |
| **Farbe** | **Sättigung** (0 = schwarzweiß) und **Dynamik** (blasse Farben mehr als kräftige). |
| **Details** | **Rauschminderung** (glättet flache Bereiche, erhält Kanten und Sterne), **Farbrauschen**, **Sternreduktion** (kleinere Sterne), **Schärfen** und sein Radius. Radien sind in Pixeln des ganzen Bildes, daher entspricht die Vorschau dem gespeicherten Ergebnis. |
| **Himmel und Vordergrund** | Eigene Looks für Himmel und Vordergrund, zusätzlich zu allem oben, über eine Maske (siehe unten). |

**Streckmethoden:**

| Methode | Verwendung |
|---|---|
| **Aus** | Das Bild ist schon fertig. |
| **sRGB-Kurve** | Wie eine Kamera: für lineare Daten, die nur die normale Gammakurve brauchen. |
| **MTF, automatisch** | Schwarzpunkt knapp unter dem Himmelshintergrund, Mitteltöne so, dass der Hintergrund die Helligkeit **Hintergrund** erhält (wie AutoStretch in Siril / STF in PixInsight). **Jede Farbe für sich** (Standard) misst Rot, Grün und Blau getrennt, sodass ein Farbstich des Stacks das Bild nicht verdeckt; die Daten selbst bleiben unverändert. **Automatisch → manuell** macht von Hand mit den gemessenen Werten weiter. |
| **MTF, manuell** | Schwarzpunkt, Mitteltöne und Weißpunkt (Midtones Transfer Function). |
| **Arcsinh** | Inverser hyperbolischer Sinus: starke Streckung, die Sternfarben erhält. **Streckfaktor** 1–10000; tiefe Stacks mit schwachem Hintergrund brauchen einige Tausend. |
| **Verallgemeinert hyperbolisch (GHS)** | **Strecken ln(D+1)** (Stärke), **Lokale Intensität b** (wie eng sich die Streckung um den Symmetriepunkt konzentriert), **Symmetriepunkt** (wo die Streckung am stärksten ist: auf den Himmelshintergrund legen), **Schatten schützen** und **Lichter schützen** (darunter / darüber eine Gerade). |

Bei Arcsinh und GHS: **Invers** verwendet die inverse Kurve (staucht statt
zu strecken, z. B. um eine zu starke Streckung zurückzunehmen); **Keep star
colours** streckt die Helligkeit und erhält die Farbe jedes Pixels (aus: jede
Farbe für sich, satterer Himmel, blassere Sterne); **Points from the
background** setzt den Schwarzpunkt knapp unter den Hintergrund der dunkelsten
Farbe und bei GHS den Symmetriepunkt auf den Hintergrund.

**Anzeige** (über der Vorschau) ändert nur die Anzeige der Vorschau, nie das
gespeicherte Bild, wie die Anzeigemodi in Siril: **Linear (wie gespeichert)**,
**Logarithmus**, **Quadratwurzel**, **Quadriert**, **Asinh**, **AutoStretch**,
**Histogrammausgleich**. Nützlich, um in die Schatten zu schauen oder den
Hintergrund zu prüfen, ohne etwas zu ändern. Solange nicht Linear gewählt
ist, steht „view only" daneben.

**◐ Vergleichen** (gedrückt halten, oder B halten) zeigt den Ausgangspunkt:
gedreht, zugeschnitten und gestreckt, sonst nichts.

**Speichern** (rechts): **Größe** (volle Größe oder eine längste Seite wie 2048 px
für Foren), Dateiname und -typ, **JPEG-Qualität**. Die Zeile darunter nennt den
Speicherbedarf beim Speichern. FITS / XISF erhalten das fertige (gestreckte)
Bild als 32-Bit-Gleitkomma. **Einstellungen für das nächste Bild behalten** (links) gibt
mehreren Bildern denselben Look, z. B. den Teilen eines Panoramas.

| Taste | Aktion |
|---|---|
| Strg+O | Bild öffnen |
| Strg+S | Speichern (unter dem angezeigten Namen) |
| Strg+Umschalt+S | Speichern unter… |
| F | Vorschau einpassen |
| B (halten) | Mit dem Ausgangspunkt vergleichen |
| Strg+Z, Strg+Y | Rückgängig, wiederholen |

**Himmel und Vordergrund** (Himmel und Vordergrund; anfangs zugeklappt): Eine
Landschaft unter den Sternen braucht oft für beide Teile Verschiedenes, z. B.
weniger Rauschen und kleinere Sterne am Himmel, einen helleren, wärmeren
Vordergrund. Die Maske mit **✎ Malen** malen (linke Taste = Himmel, rechte
Taste = Vordergrund, Mausrad = Pinselgröße) oder von **◎ Himmel erkennen** finden
lassen (Himmel oben; mit dem Pinsel korrigieren; für ein Zirkular-Fisheye
**◯ Fisheye-Kreis**, Kapitel 17); **Ganzer Himmel**, **Umkehren** und
**Leeren** fangen neu an. Dann **Himmel** oder **Vordergrund** wählen und Licht,
Temperatur, Farbe und Detail einstellen. **Weiche Kante** lässt die beiden Looks
an der Grenze ineinander übergehen; **Maske zeigen** tönt den Himmel blau und
den Vordergrund orange (beim Malen immer). Die Maske liegt auf dem Bild
selbst und bleibt daher beim Drehen oder Zuschneiden an ihrem Platz. Die
gespeicherte Datei bekommt dieselben Looks; ein • hinter dem Namen des
Abschnitts zeigt, dass sie verwendet werden.

**↶ Rückgängig** und **↷ Wiederholen** (Strg+Z, Strg+Y oder Strg+Umschalt+Z) gehen durch
die Änderungen zurück und vor; ein Reglerzug oder ein Pinselstrich zählt als
ein Schritt, und der Tooltip der Taste sagt, was sie rückgängig macht.

**✂ Zuschnitt** schneidet im Bild zu: Das ganze Bild wird mit einem Rahmen
gezeigt; ziehen zeichnet einen neuen Rahmen, Ziehen an Kanten oder Ecken
passt ihn an, Ziehen im Inneren verschiebt ihn. **✂ Zuschnitt: fertig** zeigt das
Ergebnis. Die Formattasten und Zuschnittregler in **Drehen und beschneiden** setzen
denselben Rahmen.

**Vorgaben** (oben, unter dem Bild): einen Namen eingeben und **Speichern**
speichert den aktuellen Look: Streckung, Weißabgleich, Licht, Kurven, Farbe,
Detail und die Looks für Himmel und Vordergrund, aber nicht Drehen, Zuschnitt
und Exportgröße (die gehören zu einem Bild). **Vorgabe** wendet einen
gespeicherten Look auf das offene Bild an (Strg+Z macht es rückgängig);
**Löschen** löscht die Vorgabe mit dem eingegebenen Namen. Vorgaben liegen im
Einstellungsordner (`presets`).

## 17. Zeitraffer

*Neu in 0.4 (in Entwicklung).* Der Tab **Zeitraffer** macht aus einer
Bildfolge ein Video: eine Landschaft unter dem Nachthimmel, vom Stativ
aufgenommen, mit dem Himmel, der über einem ruhigen Vordergrund zieht. Ein
langsamer Zoom oder eine Fahrt wie auf einem Slider bringt Kamerabewegung.

**Projekte:** **⬇ Projekt speichern…** speichert die Bilder, die Looks, die
Maske (samt Fisheye-Kreis) und alle Videoeinstellungen in einer
`.tlproj`-Datei; **⇪ Projekt öffnen…** (Strg+O) holt alles zurück, auch wenn
der Ordner verschoben wurde. Strg+S speichert sofort ins offene Projekt.
„Start from the Post-processing tab's look“ wird nur als Schalter
gespeichert: Es gilt das Bild, das dann in Post-processing offen ist.

**Bilder:** Dateien oder einen Ordner hinzufügen; sie laufen in der
Reihenfolge der Dateinamen (Kameranummerierung). Ein Klick zeigt das Bild in
der Mitte; ◀ ▶ blättern. Bei Kamera-RAW-Bildern wählt **RAW über**, wie sie
gelesen werden: **eingebaut** (Standard) ist schnell und behandelt alle Bilder
gleich; **rawtherapee** übernimmt die Bearbeitungen aus den `.pp3`-Dateien
von RawTherapee neben den Bildern, braucht aber mehrere Sekunden pro Bild
(etwa zehnmal so lange); **auto** nimmt RawTherapee nur für Bilder mit einer
`.pp3`-Datei. Der Tab zeigt, wie viele Bilder eine haben. RawTherapee legt
für jedes Foto, das in seinem Dateibrowser geöffnet wurde, eine `.pp3` an;
sie bedeutet also nicht, dass das Foto bearbeitet wurde.

**Look:** dieselben Licht- und Farbregler wie in Post-processing (Belichtung,
Kontrast, Lichter, Mitteltöne, Schatten, Weiß, Schwarz, Temperatur, Tönung,
Sättigung, Dynamik), getrennt für **Ganzes Bild** (das ganze Bild), **Himmel**
(Himmel) und **Vordergrund**. Himmel und Vordergrund kommen zum
Look des ganzen Bildes hinzu, nur in ihrem Teil der Maske; • markiert
geänderte Teile. Das gewählte Bild zeigt den Look (**Look zeigen**; aus =
das Bild, wie es ist). Entflackern und Kamerabewegung sieht man nur im Video.

**Maske:** wie in Post-processing (Kapitel 16): **✎ Malen**,
**◎ Himmel erkennen**, **Ganzer Himmel**, **Umkehren**, **Leeren**, **Weiche Kante**,
**Maske zeigen**. Eine Maske gilt für alle Bilder, denn auf dem Stativ bewegt
sich der Vordergrund nicht.

Für einen Allsky-Zeitraffer mit einem nach oben gerichteten
Zirkular-Fisheye findet **◯ Fisheye-Kreis** den hellen Bildkreis und macht
den Himmel zu einer Scheibe in seiner Mitte; der Ring außen (Horizont, Bäume,
Lichter) ist Vordergrund. **Himmelskreis** legt fest, wie viel des Kreisradius
Himmel ist, und ein großer **Weiche Kante** (bis 25 %) macht die Grenze zu einem
radialen Verlauf. Den Rest mit dem Pinsel korrigieren, z. B. ein Teleskop oder
eine Person, die in den Himmel ragt.

**Video:**

| Einstellung | Was sie tut |
|---|---|
| **Deflicker** | Gleicht Helligkeits- und Farbsprünge zwischen Bildern aus (Belichtungsstufen, wechselndes Licht, der Weißabgleich der Kamera). **Leicht** vergleicht jedes Bild mit seinen Nachbarn (**Glättung**, in Bildern); **Stark** nimmt dreimal so viele. Langsame Änderungen wie die Dämmerung bleiben. **Auch die Farbe** gleicht jede Farbe für sich aus; **Bereichsweise** tut es auf einem groben Raster über das Bild, sodass großräumige Änderungen in einem Teil (Lichtverschmutzung, ein vorbeiziehendes Licht, Dämmerung auf einer Seite) dort ausgeglichen werden, wo sie entstehen. Beides funktioniert für normale Objektive und Fisheye. Das Messen liest jedes Bild ein weiteres Mal. |
| **Mit dem Look aus dem Tab Nachbearbeitung beginnen** | Jedes Bild bekommt zuerst die Einstellungen des in Post-processing geöffneten Bildes (Streckung, Weißabgleich, Licht, Kurven, Farbe, Detail; nicht dessen Drehen und Zuschnitt), dann den hier eingestellten Look. Werte der automatischen Streckung werden am ersten Bild gemessen und gelten für alle, sodass das Video nicht flackert. |
| **Slider-Fahrt** | Als liefe die Kamera auf einem motorisierten Slider: Das Video zeigt einen um so viele Prozent kleineren Ausschnitt (bis 20 %) und bewegt ihn über das Bild, **Links → rechts** (von links nach rechts), **Rechts → links**, **Nach oben** oder **Nach unten**. Die Bewegung ist fließend, nicht in ganzen Pixeln. |
| **Langsamer Zoom** | Zoomt langsam über das ganze Video hinein; mit der Slider-Fahrt kombinierbar. |
| **Sanft anfahren und anhalten** | Fahrt und Zoom laufen am Anfang sanft an und bremsen am Ende; aus: immer gleich schnell. Die Vorschau umrandet, wo das Video beginnt (hell) und endet (schwach). |

**Ausgabe:**

| Einstellung | Was sie tut |
|---|---|
| **Format** | **MP4** (H.264) und **WebM** (VP9) macht ffmpeg. Installieren (ffmpeg.org; Windows: `winget install ffmpeg`; Linux: Paketverwaltung; macOS: Homebrew) oder neben das Programm legen; der Tab zeigt, wo er es gefunden hat. **Animiertes GIF** und **Nummerierte Bilder** (nummerierte Bilder TIFF oder PNG 16 Bit, JPEG) brauchen nichts weiter. GIFs von Sternfeldern sind groß (etwa 1 MB pro Bild bei 720p; MP4 ist 5–10-mal kleiner), sie eignen sich also für kurze Clips. |
| **Bildrate** | 12–60 Bilder pro Sekunde. |
| **Auflösung** | Wie die Bilder, 4K, 1440p, 1080p oder 720p; nie vergrößert. Bis 1080p werden Kamera-RAWs in halber Größe gelesen, das ist schneller und immer noch scharf genug. |
| **Form** | Die Form des Videos; das Bild wird darauf zugeschnitten, die Vorschau umrandet den Zuschnitt. **Wie die Bilder** (kein Zuschnitt), **16:9**, **3:2** und **4:3** (richten sich nach der Ausrichtung der Bilder), **1:1 quadratisch** (der ganze Kreis eines Allsky-Fisheyes) oder **9:16 hochkant** (Handys). Bei 1080p: 1920 × 1080, 1620 × 1080, 1440 × 1080, 1080 × 1080 und 1080 × 1920. Mit Fisheye-Kreis warnt der Tab, wenn eine Form Himmel abschneidet. |
| **Qualität** | Für MP4, WebM und JPEG-Bilder: höher = besseres Bild, größere Datei. |
| **Vorwärts, dann zurück abspielen** | Am Ende die Bilder noch einmal rückwärts, sodass das Video nahtlos in Schleife läuft. |
| **Letztes Bild halten** | Lässt das letzte Bild einige Sekunden stehen. |

Unter den Einstellungen: wie viele Bilder, wie lange das Video läuft und seine
Größe in Pixeln. **▶ Video erstellen** arbeitet im Hintergrund, bis zu acht
Bilder gleichzeitig (weniger, wenn der Speicher knapp ist), mit Fortschrittsbalken (vergangene und verbleibende Zeit) und
**✖ Abbrechen** (nichts bleibt erhalten). Danach zeigt ein Bericht die Bilder,
Größe, Bildrate und Länge des Videos, die benötigte Zeit und die Datei mit
ihrer Größe, oder was schiefging. Zum Beispiel wurden 69 RAW-Bilder (24 MP)
mit Entflackern in etwa einer halben Minute auf einem Desktop-PC zu einem
1080p-MP4.
