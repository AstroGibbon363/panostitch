# Panorama Stitcher: Benutzerhandbuch

*Andere Sprachen: [English](manual_en.md) · [Česky](manual_cs.md)*

Ein Desktop-Programm, das überlappende Bilder zu einem Panorama oder Mosaik
zusammenfügt: JPEG, TIFF, PNG, Kamera-RAW und astronomische FITS-Bilder. Jeder
Schritt läuft erst, wenn Sie seine Schaltfläche drücken, und jedes Ergebnis
lässt sich vor dem Weitermachen prüfen und korrigieren. Ein zweiter Reiter
stackt Sternaufnahmen zu einem rauscharmen Nachthimmel oder zu Strichspuren
([Sterne stacken](#13-sterne-stacken)).

Die Bedienelemente des Programms sind englisch beschriftet. In diesem Handbuch
stehen sie deshalb **fett auf Englisch**, genau so, wie sie auf dem Bildschirm
erscheinen.

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
```

Hinweise:

- Das Programm nutzt automatisch alle Prozessorkerne.
- Das Fenster öffnet sich in ein bis zwei Sekunden; vorher muss nichts
  vorbereitet werden.
- Erscheint kein Fenster, starten Sie das Programm im Terminal: Dort stehen
  die Fehlermeldungen.
- Die Schalter für das Aussehen (Dark, Modern, Simple) stehen in `ui.toml` im
  Einstellungsordner: `~/.config/panostitch` unter Linux,
  `~/Library/Application Support/panostitch` unter macOS,
  `%APPDATA%\panostitch` unter Windows.

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

Die Schalter oben rechts ändern das Aussehen: **Dark** (dunkel), **Modern**
(runder, luftiger) und **Simple** (einfach). Der einfache Modus blendet
die Detaileinstellungen aus (Merkmalsuche und Zuordnung, Überblendung,
Streckung, Verschieben) und behält, was die meisten Panoramen brauchen: die
Bilder, eine Vorgabe, **Run all steps**, die Projektion mit Brennweite,
Gieren (Yaw), Neigen (Pitch) und Rollen (Roll), die Belichtung, **Even out**,
Zuschnitt, Vorschau und Speichern. Ausgeblendete Einstellungen behalten die
Werte der Vorgabe. Das Programm merkt sich die Schalter.

---

## 4. Schnellstart

**Foto-Panorama:**

1. Lassen Sie **half size** eingeschaltet. Das ist schneller und reicht zum
   Prüfen des Ergebnisses völlig aus.
2. Wählen Sie ein passendes **Preset** (siehe [Voreinstellungen](#50-voreinstellungen)).
3. Tippen Sie den Ordner mit den Bildern in das Pfadfeld und drücken Sie
   **Enter**, oder klicken Sie auf **Add…** und wählen Sie die Bilder aus.
   **Die Reihenfolge spielt keine Rolle.**
4. Klicken Sie auf **Run all steps** und warten Sie, bis die Statuszeile
   *Done* meldet.
5. Wählen Sie rechts eine **Projektion**: **cylindrical** für eine einzelne
   Reihe, **spherical** für mehrere Reihen oder Aufnahmen, die weit nach oben
   oder unten reichen. Ist das Ergebnis zu dunkel, klicken Sie auf **Auto
   exposure**; ist der Horizont gebogen, passen Sie **Pitch** an. Mit **Auto
   crop** schneiden Sie die ausgefransten Ränder ab.
6. Klicken Sie auf **Save panorama…**. Wenn Sie später weiterarbeiten
   möchten, klicken Sie auch auf **Save project…**.

**Deep-Sky-Mosaik aus FITS-Dateien:**

1. Wählen Sie die Voreinstellung **deep-sky mosaic**.
2. Laden Sie die FITS-Kacheln (gestackt oder Einzelbilder). Sie werden zur
   Anzeige automatisch gestreckt.
3. Klicken Sie auf **Run all steps** und dann auf **Auto stretch**, damit die
   Streckung zum ganzen Mosaik passt. Bei Bedarf feinjustieren.
4. Speichern Sie als **`.fits`**, um die linearen Daten weiterzuverarbeiten,
   oder als **`.tif`**, um das gestreckte Bild so zu erhalten, wie Sie es sehen.

---

## 5. Schritt für Schritt

**Werte eintippen:** Neben jedem Schieberegler steht ein Zahlenfeld. Klicken
Sie hinein, tippen Sie einen Wert und drücken Sie **Enter**. Der Wert wird auf
Schrittweite und Bereich des Reglers gerundet.

### 5.0 Voreinstellungen

Das Menü **Preset** (linke Spalte, unter *RAW via*) setzt alle Werte für
Zuordnung und Zusammensetzung für eine Motivart auf einmal. Jeder Wert lässt
sich danach weiter ändern.

| Voreinstellung | Für | Wichtigste Einstellungen |
|---|---|---|
| **day landscape** | Tageslicht-Panoramen, Kamera auf der Stelle gedreht | leichtes Entrauschen, Homographie, zylindrische Projektion |
| **astro / night landscape** | Nachthimmel, Milchstraße über einer Landschaft | starkes Entrauschen, lockerere Zuordnung, sphärische Projektion, +1 EV (dann **Auto exposure**), höhere Eckenschwelle (weniger falsche Ecken im Rauschen) |
| **deep-sky mosaic** | Teleskop-Mosaike von Sternfeldern, z. B. FITS | wenig Entrauschen (Sterne sind klein), Ähnlichkeitsmodell (Verschiebung + Bildfeldrotation), keine Verfeinerung, geradlinige Projektion; **Auto stretch** verwenden |
| **macro / microscope** | Objektträger oder Tisch unter einer festen Kamera verschoben | Verschiebungsmodell (Bilder verschieben sich nur), keine Verfeinerung, geradlinige Projektion |
| **scan / flat document** | Karten, Gemälde, Dokumente in Teilen fotografiert | affines Modell, keine Verfeinerung, geradlinige Projektion |

### 5.1 Bilder (Images)

| Element | Funktion |
|---|---|
| **Add…** | Öffnet einen Dateidialog. Mehrere Dateien sind möglich (Strg/Umschalt + Klick). |
| **Pfadfeld** | Pfad zu einer Datei oder einem **Ordner** eintippen und Enter drücken. Ein Ordner lädt alle Bilder darin, nach Namen sortiert. Dateien namens `panorama…` (frühere Ergebnisse) werden übersprungen. |
| **Bildliste** | Ein Klick wählt ein Bild aus und zeigt es an. **Strg + Klick** fügt ein Bild der Auswahl hinzu oder entfernt es, **Umschalt + Klick** wählt einen Bereich, **Strg+A** (Maus über der Liste) wählt alles. Mit dem Mausrad blättern. |
| **Remove** | Entfernt die ausgewählten Bilder. Die **Entf**-Taste tut dasselbe, wenn die Maus über der Liste ist. |
| **Clear all** | Entfernt alle Bilder. |
| **▲ / ▼** | Verschiebt die ausgewählten Bilder in der Liste nach oben oder unten. Nur die Paar-Modi *sequential* und *grid* achten auf die Reihenfolge. |
| **Reference** | Das Bild, um das herum die anderen platziert werden; es bleibt unverzerrt. **auto (centre)** wählt das Bild in der Mitte der Gruppe, was meist das gleichmäßigste Ergebnis gibt. |
| **RAW via** | Welcher RAW-Konverter verwendet wird, siehe [RAW-Dateien](#8-raw-dateien). Gilt für danach hinzugefügte Bilder. |
| **half size** | Lädt RAW-Dateien und FITS-Dateien von Farbkameras in halber Auflösung: etwa 4× schneller, ¼ des Speichers. Gilt für danach hinzugefügte Bilder. Beim Speichern mit einer **Output size** über 50 % werden RAW-Dateien erneut in voller Größe gelesen. |

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
| **Detection size (px)** | 2000 | Das Bild wird vor der Suche auf diese Größe (längere Seite) verkleinert. Größer ist genauer, aber langsamer. |
| **Max keypoints** | 5000 | Obergrenze pro Bild. Die Punkte werden gleichmäßig über das ganze Bild verteilt. |
| **Corner threshold** | 0,001 | Mindeststärke einer Ecke. Niedriger findet auch schwache Ecken (nützlich bei Himmel oder Nebel), höher nur kräftige. |
| **Denoise (σ px)** | 1,5 | Glättet vor der Suche das Sensorrauschen. Für **Nachtaufnahmen** und verrauschte Hoch-ISO-Bilder erhöhen (2–3); für Sternfelder und saubere Tageslichtbilder mit feinen Details verringern (0,5–1). |

Klicken Sie auf **Detect features**. Die Ansicht zeigt das aktuelle Bild mit
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
| **all pairs** | Standard. Jedes Bild wird mit jedem anderen probiert, daher **spielt die Reihenfolge keine Rolle**, und mehrere Reihen funktionieren automatisch. Bei vielen Bildern langsamer (74 Fotos ≈ 35 s). |
| **sequential** | Eine Reihe, Bilder in Aufnahmereihenfolge. Jedes Bild wird nur mit dem nächsten in der Liste verglichen. Am schnellsten. |
| **grid** | Mehrere Reihen in bekannter Reihenfolge: **Grid columns** Bilder pro Reihe, Reihe für Reihe. Probiert werden die Nachbarn rechts, unten und diagonal. Schalten Sie **snake** ein, wenn jede zweite Reihe in Gegenrichtung aufgenommen wurde. |

**Einstellungen:**

| Element | Standard | Bedeutung |
|---|---|---|
| **Grid columns** | 5 | Bilder pro Reihe (nur im Modus grid). |
| **Ratio test** | 0,8 | Um wie viel besser die beste Zuordnung als die zweitbeste sein muss. Niedriger ist strenger (weniger, aber verlässlichere Zuordnungen); höher ergibt mehr Zuordnungen. |
| **Max distance** | 0,35 | Wie unterschiedlich zwei Punkte aussehen dürfen, um noch zugeordnet zu werden. |
| **RANSAC tol. (px)** | 5,0 | Wie weit (in Pixeln) eine Zuordnung vom Modell abweichen darf und noch als Inlier zählt. |
| **Min. inliers** | 20 | Mindestanzahl an Inliern, damit ein Paar als echte Überlappung gilt. |
| **Model** | homography | Wie ein Bild auf ein anderes abgebildet wird, siehe unten. |
| **refine** | an | Globale Verfeinerung, siehe unten. |

**Wann ein Paar als echte Überlappung gilt:** Es braucht mindestens *Min.
inliers* Inlier, eine klare Mehrheit seiner Zuordnungen muss übereinstimmen,
und die Transformation muss plausibel sein (sie darf das Bild weder umklappen
noch extrem verzerren). So werden falsche Zuordnungen zwischen Bildern
aussortiert, die sich gar nicht überlappen.

**Modelle:**

| Modell | Wann verwenden |
|---|---|
| **homography** | Normale Panoramen: Kamera auf der Stelle gedreht. Standard. |
| **affine** | Scans oder Fotos eines flachen Objekts aus großer Entfernung. |
| **similarity** | Nur Verschiebung, Drehung und gleichmäßige Skalierung, z. B. Teleskop-Mosaike mit Bildfeldrotation. Stabil auch bei wenigen Zuordnungen. |
| **translation** | Nur Verschiebung, z. B. Scanner oder Mikroskoptisch. Am robustesten, am wenigsten flexibel. |

**Platzierung und globale Verfeinerung (refine):** Die Bilder werden über ihre
stärksten Paare verbunden, ausgehend vom Referenzbild. Mit eingeschaltetem
**refine** (nur beim Modell homography) werden danach alle Paare gemeinsam als
eine sich drehende Kamera gelöst. Kleine Fehler verteilen sich so
gleichmäßig, statt sich aufzusummieren, und die Schleifen zwischen den Reihen
schließen sich; das beseitigt die meisten „Geisterbilder“. Außerdem wird die
tonnenförmige Verzeichnung des Objektivs geschätzt (stark bei Weitwinkeln) und
beim Rendern korrigiert. Paare, die nicht zur gemeinsamen Lösung passen, werden
**automatisch verworfen**, da es meist falsche Zuordnungen oder bewegte Dinge
sind. Die Statuszeile meldet, wie viele verworfen wurden, und den **typischen
Paarfehler** (1–3 px ist gut).

**Schaltflächen:**

- **Match**: ordnet die im Menü Pairs gewählten Paare zu (parallel),
  platziert und verfeinert dann.
- **Re-match pair**: ordnet nur das angezeigte Paar mit den aktuellen
  Einstellungen neu zu.
- **Discard pair**: verwendet das angezeigte Paar nicht mehr (für eine
  sichtbar falsche Zuordnung); die Platzierung wird ohne es neu berechnet.
- **Run all steps**: Merkmale, Zuordnung, Platzierung und Vorschau in einem.

Die Ansicht **Pair matches** zeigt zwei Bilder nebeneinander:

- **grüne Linien**: Inlier (richtige Zuordnungen)
- **rote Linien**: Ausreißer. Mit dem Schalter **outliers** ausblenden.

Das Menü **Pair** listet die angenommenen Paare mit ihrer Inlier-Anzahl auf.
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
| **Focal (× auto)** | Die Brennweite wird aus den Zuordnungen geschätzt. Dieser Regler skaliert sie. Wirkt das Panorama seitlich gestaucht oder gedehnt, probieren Sie Werte zwischen 0,8 und 1,2. |
| **Yaw (°)** | Dreht das ganze Panorama nach links oder rechts: bestimmt, welche Richtung in der Mitte liegt. Nützlich bei 360°-Panoramen (z. B. damit das Zentrum der Milchstraße in der Mitte statt am Rand liegt). |
| **Pitch (°)** | Neigt das ganze Panorama nach oben oder unten. Damit **begradigen Sie einen gebogenen Horizont** (etwa wenn die Kamera nach unten zeigte). |
| **Roll (°)** | Dreht das ganze Panorama. Damit richten Sie einen schiefen Horizont aus. |
| *Feineinstellung* | Yaw, Pitch und Roll haben je einen Regler, ein Zahlenfeld und **◀ ▶**. Die Regler für Pitch und Roll sind nahe 0° fein (wo Korrekturen meist liegen) und weiter außen grob. Langsames Ziehen am Zahlenfeld ergibt 0,02° pro Pixel; per Doppelklick lässt sich der Wert eintippen. **◀ ▶** gehen in Schritten von 0,1° (mit Umschalt 1°). |
| **Reset focal / yaw / pitch / roll** | Setzt diese vier Regler zurück. |

Der Titel der Ansicht zeigt Panoramagröße, Brennweite in Pixeln, Bildwinkel des
Referenzbilds und den typischen Ausrichtungsfehler.

Diese Elemente aktualisieren die Vorschau sofort (sobald eine Vorschau
existiert).

### 5.5 Schritt 4: Zusammensetzen (Compose)

| Element | Bedeutung |
|---|---|
| **Blend** | **smart** (Standard): Mehrband-Überblendung mit Geisterbild-Entfernung. Feine Details stammen an jeder Stelle aus einem einzigen Bild, sodass keine Doppelbilder von Menschen, Lichtern oder nahen Objekten entstehen. Helligkeitsunterschiede werden über eine breite Zone überblendet, sodass keine harten Übergänge entstehen. Wo sich überlappende Bilder widersprechen (etwas hat sich bewegt), wird der ganze Bereich aus einem Bild übernommen. **feather**: einfaches weiches Mischen; zeigt Geisterbilder, wo sich etwas bewegt hat. **seam**: jedes Pixel aus dem zentralsten Bild; keine Geisterbilder, aber harte Übergänge. **average**: einfacher Mittelwert; hauptsächlich zum Prüfen der Ausrichtung. |
| **Preview size (px)** | Größe der Vorschau (längere Seite). Größer ist schärfer, aber langsamer. Beim Hineinzoomen werden Details ohnehin nachberechnet (siehe [In der Ansicht navigieren](#6-in-der-ansicht-navigieren)). |
| **Exposure (EV)** | Hellt das ganze Panorama auf oder dunkelt es ab, von −3 bis +8 EV (+1 EV = doppelt so hell). Dunkle Astro-Panoramen brauchen oft +1,5 bis +4. |
| **Brightness (sel.)** | Helligkeitsfaktor der **ausgewählten** Bilder. |
| **show unlinked images** | Zeigt auch nicht verbundene Bilder in einer Reihe unter dem Panorama, zum Platzieren von Hand. |
| **Auto exposure** | Setzt **Exposure** anhand der aktuellen Vorschau: typische Helligkeit auf Mittelgrau, ohne mehr als die hellsten 0,5 % (Sterne, Lampen) zu beschneiden. |
| **Even out** | Berechnet Helligkeitsfaktoren je Bild, damit die Überlappungen zusammenpassen. |
| **Reset moves** | Macht alle manuellen Verschiebungen rückgängig. |
| **Auto crop** | Beschneidet das Panorama auf das größte Rechteck, das vollständig von Bildern bedeckt ist, sodass die schwarzen, ausgefransten Ränder verschwinden. Die Vorschau zeigt dann das beschnittene Ergebnis, und **Save panorama…** speichert es. Die Statuszeile nennt, wie viel vom Panorama erhalten bleibt. Richten Sie zuerst den Horizont aus (Pitch, Roll); von einem gebogenen Panorama bleibt nur ein schmaler Streifen. |
| **Uncrop** | Hebt den Beschnitt auf. Der Beschnitt wird auch automatisch aufgehoben, wenn Sie etwas ändern, das die Bilder verschiebt (Projektion, Brennweite, Yaw, Pitch, Roll, Verschiebungen, Zuordnung). |
| **Nudge ← ↑ ↓ →** | Verschiebt das aktuelle Bild um ein Vorschau-Pixel (siehe unten). |

In der Ansicht **Panorama** wird der Umriss jedes Bildes gezeichnet und
nummeriert:

- **orange**: das aktuelle Bild
- **hellblau**: weitere ausgewählte Bilder
- **cyan**: das Referenzbild
- **weiß**: die übrigen Bilder
- **gestrichelte gelbe Linien**: die zur Platzierung verwendeten Verbindungen

Mit dem Schalter **overlays** in der Werkzeugleiste blenden Sie diese aus und
sehen das Panorama selbst.

**Ein Klick in einen Umriss** wählt das Bild aus (funktioniert auch mit
ausgeblendeten Überlagerungen).

**Bilder von Hand verschieben:** **Strg** gedrückt halten und ein Bild ziehen,
oder die Pfeiltasten bzw. die **Nudge**-Schaltflächen verwenden. Das Bild
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
| **Black point** | Alles mit dieser oder geringerer Helligkeit wird schwarz. Knapp unter den Himmelshintergrund setzen (linker Rand des Hauptgipfels im Histogramm). |
| **Midtones** | Hellt die Mitteltöne auf (kleinere Werte) oder dunkelt sie ab (größere Werte), ohne zu beschneiden. 0,5 ist neutral. Deep-Sky-Bilder brauchen typischerweise 0,005–0,05. |
| **White point** | Alles mit dieser oder höherer Helligkeit wird weiß. |
| **Auto stretch** | Setzt alle drei Werte anhand des aktuellen Histogramms: Schwarzpunkt knapp unter dem Himmelshintergrund, Mitteltöne so, dass der Hintergrund bei etwa 25 % Grau landet (dasselbe Prinzip wie die automatische Bildschirmstreckung in PixInsight). |
| **Reset stretch** | Zurück zu keiner Streckung (0 / 0,5 / 1). |

- Die Streckung ändert nur, wie Bilder **angezeigt** werden und wie das
  Panorama als TIFF, PNG oder JPEG gespeichert wird. Die Zuordnung arbeitet
  immer mit den Originaldaten.
- **FITS-Dateien** werden beim Laden automatisch zur Anzeige gestreckt,
  basierend auf allen geladenen FITS-Bildern zusammen.
- Änderungen der Streckung wirken sofort; nichts muss neu berechnet werden.
- **Exposure** wird vor der Streckung angewendet.

### 5.7 Rückgängig, Wiederherstellen und Projekte

**Rückgängig / Wiederherstellen:** **Undo** und **Redo** in der Werkzeugleiste
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
  reine Anzeigedinge (Zoom, gewählte Ansicht, Überlagerungen), *RAW via* und
  *half size* sowie das Speichern von Dateien.

**Projekte:** **Save project…** (oben in der linken Spalte) schreibt eine
Projektdatei mit der Endung **`.panoproj`**. **Open project…** lädt sie wieder.
Sie können den Pfad einer `.panoproj`-Datei auch in das Pfadfeld tippen und
Enter drücken oder das Programm damit starten:
`panostitch mein_pano.panoproj`.

Ein Projekt speichert:

- die Bilder (vollständiger Pfad und Pfad relativ zur Projektdatei)
- alle Einstellungen: Regler, Menüs, Schalter, einschließlich der Streckung
- die Zuordnungsergebnisse, sodass nichts neu zugeordnet werden muss
- manuelle Verschiebungen, Helligkeitsfaktoren, das Referenzbild und den
  Beschnitt
- *RAW via* und *half size*, damit die Bilder wieder genauso geladen werden

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
| Zoom zurücksetzen | | `F`, `0` oder `Pos1` | **Fit** |
| Bild auswählen (Ansicht Panorama) | in seinen Umriss klicken | | |
| Umrisse, Nummern, Verbindungen ein/aus | | | Schalter **overlays** |
| Bild verschieben (Ansicht Panorama) | **Strg + Ziehen** | Pfeiltasten (`Umschalt` = 10 Schritte) | **← ↑ ↓ →** |
| Rückgängig | | `Strg+Z` | **Undo** |
| Wiederherstellen | | `Strg+Y` oder `Strg+Umschalt+Z` | **Redo** |

Die Schaltflächen **Image + keypoints**, **Pair matches** und **Panorama**
wechseln zwischen den drei Ansichten.

**Weitere Tastenkürzel** (zeigt der Tooltip einer Schaltfläche):

| Tasten | Panorama stitcher | Star stacking |
|---|---|---|
| `Strg+Enter` | **Run all steps** | **Start** |
| `P` / `Strg+P` | `P`: **Preview panorama** | `Strg+P`: **Quick preview** |
| `Strg+S` | Projekt speichern (in die offene Projektdatei sofort; beim ersten Mal mit Namensabfrage) | ebenso |
| `Strg+Umschalt+S` | **Save panorama…** | |
| `Strg+O` | **Open project…** | ebenso |
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
| **rectilinear** (geradlinig) | ein normales Foto: gerade Linien bleiben gerade | schmale Panoramen (bis ~100°), Architektur, Teleskop-Mosaike | mit wachsendem Bildwinkel werden die Ränder extrem gedehnt |
| **cylindrical** (zylindrisch) | waagerechte Linien biegen sich, senkrechte bleiben senkrecht | die meisten waagerechten Panoramen | nicht für hohe Aufnahmen (oben und unten gedehnt) |
| **spherical** (sphärisch, equirektangular) | wie zylindrisch, aber vertikal weniger gedehnt | sehr breite oder 360°-Panoramen, Panorama-Viewer | gerade Linien krümmen sich |
| **mercator** | zwischen zylindrisch und sphärisch | breite Panoramen mit etwas Höhe | Dehnung nahe dem oberen und unteren Rand |
| **stereographic** (stereografisch) | fischaugenähnlich, Formen bleiben rund | sehr große Bildwinkel, kreative Aufnahmen | starke Krümmung |
| **transverse_cylindrical** (transversal zylindrisch) | zylindrisch, auf die Seite gelegt: senkrechte Linien biegen sich, waagerechte bleiben gerade | **senkrechte Panoramen** (Horizont bis Zenit), der aufgerichtete Milchstraßenbogen | nicht für breite waagerechte Schwenks (sie laufen über den oberen und unteren Rand) |
| **transverse_mercator** (transversal Mercator) | wie transversal zylindrisch, seitlich weniger gedehnt | senkrechte Panoramen mit etwas Breite | wie oben |
| **fisheye** (Fischauge, abstandstreu) | rundes Bild, Abstand zur Mitte proportional zum Winkel | **Ganzhimmelansichten**: mit Pitch so neigen, dass der Zenit in der Mitte liegt | Ränder gestaucht |
| **fisheye_equisolid** (Fischauge, flächentreu) | rundes Bild, das Flächenverhältnisse erhält | Ganzhimmelansichten, so wie die meisten Fischaugenobjektive abbilden | Ränder stärker gestaucht |
| **little_planet** (kleiner Planet) | der Boden in der Mitte, der Himmel als Ring darum | 360°-Panoramen mit Boden, kreativ | das Panorama muss weit nach unten reichen |
| **tunnel** | der Himmel in der Mitte, der Horizont darum | 360°-Nachthimmel, Sternspuren, Bäume von unten | das Panorama muss weit nach oben reichen |
| **orthographic** (orthografisch) | eine Kugel von außen gesehen | kreativ; zeigt eine Hälfte der Kugel | die Ränder sind stark gestaucht |
| **hammer** (Hammer–Aitoff) | ovale Karte der ganzen Kugel mit richtigen Flächenverhältnissen | Panoramen der ganzen Kugel als ein Bild, Ganzhimmel-Sternkarten | Formen am Rand verzerrt |
| **mollweide** | ovale flächentreue Karte, an den Polen flacher | wie Hammer; die klassische Ganzhimmelkarte der Astronomie | Formen am Rand verzerrt |

Sieht der Horizont wie ein Lächeln oder ein Stirnrunzeln aus, passen Sie
**Pitch** an. Ist er schief, passen Sie **Roll** an. **Yaw** dreht die Ansicht
seitlich. Bei Fischauge, kleinem Planeten und Tunnel bestimmt Pitch, was in
der Mitte liegt.

---

## 8. RAW-Dateien

Unterstützt: **Canon CR2 und CR3, Nikon NEF und NRW, Sony ARW und SRF, DNG,
Fuji RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW** und weitere.
Die genaue Kameraunterstützung hängt vom Decoder ab; der eingebaute kennt die
meisten Kameras der letzten zwei Jahrzehnte, LibRaw (falls installiert) etwa
1250 Kameras.

**RAW via** wählt den Decoder:

| Option | Decoder | Eigenschaften |
|---|---|---|
| **auto** | der erste, der funktioniert, in der Reihenfolge unten; hat ein Foto eine `.pp3`-Datei, zuerst RawTherapee | empfohlen |
| **built-in** | der eigene Decoder des Programms | immer verfügbar, schnell; neutrale Umwandlung mit den Farben und dem Weißabgleich der Kamera, ohne automatisches Aufhellen, sodass alle Bilder dieselbe Belichtung behalten |
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
**built-in** und korrigieren Sie die Farben anschließend.

---

## 9. FITS-Dateien (Astronomie)

Dateien mit der Endung **`.fits`, `.fit` oder `.fts`** werden direkt gelesen,
ohne zusätzliche Software. Unterstützt:

| Inhalt | Beispiele | Wie es gelesen wird |
|---|---|---|
| **8-, 16-, 32- und 64-Bit-Ganzzahlen, 32- und 64-Bit-Gleitkomma** | Kameraaufnahmen, Stacking-Ergebnisse | auf 0…1 skaliert (16 Bit vorzeichenlos: 0…65535) |
| **Farbe (3 Ebenen)** | gestackte Ergebnisse aus Siril, PixInsight, DSS | als RGB |
| **Mono** | Monochromkameras, einzelne Filter | als Grau |
| **Rohdaten von Farbkameras (Bayer)** | Bilder von ZWO, ToupTek, QHY … mit Header `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | Debayering; mit **half size** ein Farbpixel pro 2×2-Zelle (schnell, sauber), sonst Interpolation in voller Größe |
| **Zeilenreihenfolge** | `ROWORDER` = `TOP-DOWN` oder `BOTTOM-UP` (FITS-Standard) | das Bild wird richtig herum angezeigt |

Nicht unterstützt: **kachelkomprimierte Dateien (`.fz`)**. Entpacken Sie diese
zuerst, z. B. mit `funpack`. Weitere Teile einer Datei (Tabellen,
ICC-Profile) werden ignoriert; verwendet wird das erste Bild.

**Hinweise zu Deep-Sky-Mosaiken:**

- Verwenden Sie die Voreinstellung **deep-sky mosaic**. Teleskop-Kacheln
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

Klicken Sie auf **Save panorama…** und wählen Sie einen Dateinamen. Der
Dateityp richtet sich nach der Endung:

| Endung | Ergebnis |
|---|---|
| `.tif` / `.tiff` | 16 Bit pro Kanal, am besten für die Weiterbearbeitung; gespeichert **wie angezeigt** (mit Belichtung und Streckung) |
| `.png` | 16 Bit pro Kanal; wie angezeigt |
| `.jpg` | 8 Bit pro Kanal, kleinere Datei; wie angezeigt |
| `.fits` / `.fit` / `.fts` | 32-Bit-Gleitkomma, drei Farbebenen; **linear** (Belichtung und Helligkeitsfaktoren angewendet, aber **keine Streckung** und kein Beschneiden), für Astronomie-Software |

Sind alle geladenen Bilder FITS, endet der vorgeschlagene Dateiname auf
`.fits`.

**Output size** (über der Speichern-Schaltfläche) legt die Größe des
gespeicherten Panoramas fest, in % der vollen Kameraauflösung; die Zeile
darunter zeigt die Größe in Pixeln (nach einer Vorschau). ¼, ½, full und 2×
sind Schnellwahlen. 100 % behält alle Details; mit **half size** geladene
RAW-Dateien werden beim Speichern erneut in voller Größe gelesen, das braucht
mehr Zeit und Speicher. 50 % oder 25 % ergeben schnell eine kleinere Datei,
über 100 % wird vergrößert (ohne neue Details).

Darunter zeigt eine Schätzung, wie viel Speicher das Speichern braucht und wie
groß die Datei werden kann, neben dem freien Platz. Passt es nicht, wird sie
orange (⚠); der Tooltip sagt, was zu tun ist. Das Speichern lehnt dann mit
demselben Rat ab, statt den Speicher zu füllen, wodurch der ganze Computer
nicht mehr reagieren würde. Das Laden hält bei fast vollem Speicher früher an
und meldet es.

Panoramen, die breiter oder höher als 60 000 px würden, werden abgelehnt. Das
bedeutet fast immer, dass ein Paar falsch zugeordnet ist oder die Projektion
ungeeignet ist; probieren Sie cylindrical oder spherical.

---

## 11. Fehlerbehebung

| Problem | Lösungsansatz |
|---|---|
| **Kein Fenster erscheint** | Unter Windows nennt eine Meldung den Grund; unter Linux und macOS das Programm im Terminal starten und die Fehlermeldung dort lesen. Das Fenster braucht OpenGL 2.0 oder neuer: Grafiktreiber aktualisieren (bei alten PCs den des Grafikchip-Herstellers). |
| **Das Programm wurde unerwartet beendet** | Beim nächsten Start meldet es die Statuszeile. Die Details stehen in `crash.log` im Einstellungsordner (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`); bitte beim Melden des Problems anhängen. |
| **Bedienelemente am Rand abgeschnitten** | Den Rand zwischen Seitenleiste und Bild breiter ziehen. |
| **„Busy – wait…“** | Ein Schritt läuft noch; warten Sie, bis sich die Statuszeile ändert. |
| **Ein Bild wird beim Laden übersprungen** | Die Statuszeile nennt Datei und Grund. Bei RAW einen anderen Konverter unter **RAW via** probieren. `.fz`-FITS-Dateien zuerst entpacken. |
| **Panorama ist fast schwarz** | Fotos: **Auto exposure** klicken oder **Exposure (EV)** erhöhen (Nachtaufnahmen brauchen oft +1,5 bis +4), oder die Fotos in RawTherapee bearbeiten (deren `.pp3`-Dateien werden dann verwendet). FITS und andere lineare Daten: **Auto stretch** klicken. |
| **FITS-Bilder sind nach dem Laden schwarz oder weiß** | Die automatische Streckung beim Laden beruht auf allen FITS-Bildern; ein stark über- oder unterbelichtetes Bild kann sie verfälschen. Solche Bilder entfernen, dann **Auto stretch** klicken (oder **Reset stretch** und von Hand einstellen). |
| **FITS-Bilder wirken grün** | Rohdaten von Farbkameras sind nicht weißabgeglichen. Zuerst stacken und farbkalibrieren, oder die Farben des Ergebnisses in Astronomie-Software korrigieren. |
| **Wenige gelbe Punkte** | **Corner threshold** senken; **Max keypoints** erhöhen; **Detection size** erhöhen. |
| **Punkte vor allem auf Gras, Himmel oder Rauschen** | **Denoise** auf 2–3 erhöhen. |
| **Bilder mit UNLINKED markiert** | Ansehen: Überlappen sie wirklich mit einem anderen Bild? **Ratio test** (0,85–0,9) und **Max distance** (0,4) erhöhen, **Min. inliers** senken, **Denoise** erhöhen, dann erneut **Match**. Klappt es weiterhin nicht, **show unlinked images** einschalten und die Bilder mit **Strg + Ziehen** platzieren. |
| **Wolken, Wasser oder andere bewegte Dinge** | Sie verändern sich zwischen den Aufnahmen und lassen sich daher nicht zuverlässig zuordnen (Bilder nur mit Wolken bleiben meist unverbunden). Solche Teile schnell nacheinander aufnehmen oder von Hand platzieren. |
| **Ein falsches Paar wird verwendet** (Linien in alle Richtungen, Bilder am falschen Platz) | Im Menü **Pair** auswählen und **Discard pair** klicken. Mit eingeschaltetem **refine** werden die meisten solchen Paare automatisch verworfen. |
| **Geisterbilder (doppelte Objekte)** | **refine** eingeschaltet lassen und **Blend → smart** (Standard) verwenden. Verbleibende Probleme stammen von Dingen, die sich zwischen den Aufnahmen verändert haben (z. B. eine Taschenlampe, die nur auf einem Foto leuchtet), oder von **Parallaxe**: Nahe Objekte verschieben sich gegenüber dem Hintergrund, wenn die Kamera nicht um ihr Objektiv (den „Nodalpunkt“) gedreht wird. |
| **Sichtbare Helligkeitssprünge** | **Even out**, dann einzelne Bilder mit **Brightness (sel.)** feinjustieren. |
| **Gebogener Horizont** | **Pitch** anpassen (und **Roll**, wenn er schief ist). |
| **Oben oder unten stark gedehnt** | Projektion **spherical** verwenden; die Statuszeile schlägt sie vor, wenn das Panorama weit nach oben oder unten reicht. |
| **Panorama gestaucht oder gedehnt** | **Focal (× auto)** anpassen. |
| **Riesiges, verzerrtes Panorama** | Ein falsches Paar (Paare prüfen), oder **spherical** / **cylindrical** statt rectilinear verwenden. |
| **Langsam oder Speicher reicht nicht** | Vor dem Laden **half size** einschalten; **Preview size** senken. Bilder in voller Auflösung brauchen etwa 12 Byte pro Pixel, rund 300 MB für ein 24-MP-Bild. Bei vielen Bildern dauert die Zuordnung *all pairs* länger; *grid* ist schneller, wenn Sie die Anordnung kennen. |
| **Auto crop lässt nur einen schmalen Streifen übrig** | Das Panorama ist gebogen. Zuerst mit **Pitch** und **Roll** begradigen (und eine passende Projektion wählen), dann erneut **Auto crop**. |
| **Ein Projekt öffnet sich ohne einige Bilder** | Die Statuszeile listet die fehlenden Dateien auf. Sie wurden verschoben oder umbenannt, und nicht zusammen mit der Projektdatei. Zurücklegen oder erneut hinzufügen und zuordnen. |
| **Rückgängig tut nichts** | Rückgängig wirkt auf abgeschlossene Änderungen; nach der letzten Änderung kurz warten. Während ein Schritt läuft (*Busy*), wartet es. |
| **Alles ist langsam** | Vor dem Laden **half size** einschalten (RAW und FITS von Farbkameras); **Preview size** verringern. Im Stacking-Tab **Half size** unter RAW conversion. |

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
- Bei langen Serien das Ergebnis zuerst mit eingeschaltetem **half size**
  prüfen.
- Wenn Sie Bilder ordnerweise laden, speichern Sie das Ergebnis nicht in den
  Ordner der Quellbilder. Dateien namens `panorama…` werden übersprungen,
  andere Namen aber nicht.

---

## 13. Sterne stacken

Der zweite Reiter, **Star stacking (landscape / trails)**, vereint viele
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
Motiv mit einem Klick ein: **Milky Way landscape** (Vordergrund eingefroren,
Himmelsregion automatisch erkannt, Verzeichnungskorrektur,
Lichtverschmutzung), **Star trails** (Lücken gefüllt) und **Deep sky
(telescope)** (das ganze Bild ist Himmel, Hintergrundextraktion,
Farbkalibrierung, automatische Streckung). Jede Einstellung lässt sich
weiterhin ändern. Während eines Laufs zeigt ein Fortschrittsbalken unter der
Ansicht den Schritt und die geschätzte Restzeit.

### 13.1 Schnellstart

1. Markieren Sie **Light frames**, klicken Sie auf **Add…** und wählen Sie die
   Himmelsaufnahmen (oder tippen Sie einen Datei- oder Ordnerpfad in das
   Pfadfeld und drücken Sie Enter).
2. Markieren Sie **Sky region**, wählen Sie **Boundary line** und ziehen Sie in
   der Basisbild-Ansicht eine Linie entlang des Horizonts, oder klicken Sie
   auf **Detect sky** und korrigieren Sie die Maske mit dem Pinsel. Sterne
   werden nur im Himmel gesucht.
3. Klicken Sie auf **Quick preview**: ein kleines, schnelles Ergebnis aus
   höchstens 10 Aufnahmen, um die Einstellungen zu prüfen. Es wird nichts
   gespeichert.
4. Prüfen Sie die Zeile **Save as** unter der Eingabeliste: Dateinamen
   eintippen oder einen vorgeschlagenen Namen in der Liste daneben wählen, in
   der zweiten Liste den Dateityp, mit **Folder…** den Ordner (Vorgabe:
   `stacked.tif` neben der ersten Himmelsaufnahme). Existiert die Datei schon,
   sagt die Zeile das; **next free name** hängt eine Nummer an. Darunter zeigt
   eine Schätzung, wie viel Platz die Aufnahmen beim Stacken brauchen (nach
   Anzahl und Größe) und ob sie im Speicher oder auf der Festplatte liegen;
   reicht der Platz nicht, wird sie orange (⚠). Dann klicken Sie auf
   **Start**.
5. Passen Sie rechts die Nachbearbeitung an; das Ergebnis ändert sich sofort.
   **Save result…** speichert es erneut mit den neuen Einstellungen.

### 13.2 Eingabedateien

| Eintrag | Was aufnehmen |
|---|---|
| **Light frames** (*Star images* im einfachen Modus) | Die Himmelsaufnahmen: gleiche Brennweite, Blende und Belichtung. RAW ist am besten; TIFF, FITS oder XISF nur, wenn die Daten noch linear (unbearbeitet) sind. **Unify exposure** gleicht Aufnahmen unter verschiedenen Bedingungen an. |
| **Base frame** (*Base image* im einfachen Modus) | Das Bild, an dem die anderen ausgerichtet werden. Leer = das mittlere Light-Bild. Ein anderes wählen Sie in der Auswahlliste neben **Base frame**, oder wählen Sie ein Light-Bild in dessen Liste und klicken Sie auf **Use as base**. |
| **Dark frames** (*Noise images* im einfachen Modus) | Dunkelbilder (Darks): Objektivdeckel drauf, gleiche ISO und Belichtungszeit, direkt vor oder nach den Himmelsaufnahmen. Entfernen thermisches Rauschen und Hotpixel. |
| **Bias frames** (*Bias images* im einfachen Modus) | Kürzeste Belichtung, Objektivdeckel drauf, gleiche ISO. |
| **Flat field frames** (*Vignetting images* im einfachen Modus) | Flats: eine gleichmäßig beleuchtete weiße Fläche, gleiche Brennweite und Blende. Korrigieren dunklere Ecken und Staubschatten. |
| **Flat-dark frames** (*Flat-dark images* im einfachen Modus) | Darks für die Flats: gleiche ISO und Belichtungszeit wie die Vignettierungsbilder. |
| **Foreground image** | Freiwillig: eine Aufnahme des Bodens vom selben Stativstandort – zur blauen Stunde, lang belichtet oder mit niedriger ISO. Sie ersetzt unterhalb der Himmelsregion den (verrauschten, dunklen) gestackten Vordergrund. **Match brightness** passt sie an die Helligkeit des gestackten Vordergrunds an; **Exposure (EV)** macht sie von dort aus heller oder dunkler. Braucht eine Himmelsregion. |
| **Output** | Wird in der Zeile **Save as** gesetzt: Name, Typ und Ordner. `.tif` / `.png` (16 Bit), `.jpg` (8 Bit) oder `.fits` / `.xisf` (32-Bit-Gleitkomma, linear, zur Weiterverarbeitung in Siril, PixInsight usw.). |

Alle Kalibrierungsbilder sind freiwillig. Bei hoher ISO helfen schon wenige
Dunkelbilder viel.

**Die Dateilisten:** Jede Gruppe zeigt ihre Dateien in einer Auswahlliste
neben ihrem Namen, sodass auch Hunderte von Light-Bildern kein Scrollen
brauchen. Wählen Sie dort eine Datei, um sie in der Vorschau zu sehen;
**Remove** und **Use as base** wirken auf diese Datei, **Clear** leert die
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
Lauf schon vor dem Start mit einer Meldung an: **Half size** einschalten (ein
Viertel des Platzes) oder die Ausgabe auf einem Laufwerk mit mehr freiem
Platz speichern. Auch Master (Dark, Bias, Flat) werden nicht gespeichert.

### 13.3 Einstellungen

| Einstellung | Wirkung |
|---|---|
| **Composition** | **Align stars**: Die Aufnahmen werden an den Sternen ausgerichtet und vereint. **Accumulation** mittelt sie; **Select best pixels** (Vorgabe) lässt Ausreißer weg (Flugzeuge, Satelliten); **Freeze ground** richtet nur den Himmel aus, sodass Himmel und Vordergrund scharf bleiben (braucht eine Himmelsregion); **Align only** schreibt nur die ausgerichteten Aufnahmen. **Star trails**: keine Ausrichtung, der Himmel behält den hellsten Wert jedes Pixels. **Trail fade**: **Fade in** (die Spuren beginnen schwach, Sequators Bewegungseffekt), **Fade out** oder **Both ends** (kometenartig an beiden Enden). **Fill gaps between frames**: Die kurzen Pausen des Kamera-Intervalls werden entlang der gemessenen Sternbewegung gefüllt, sodass die Spuren durchgehende Linien statt Striche sind. |
| **Sky region** | Wo der Himmel ist. **All sky**: das ganze Bild ist Himmel (kein Vordergrund, z. B. Teleskopaufnahmen). **Boundary line**: eine Linie entlang des Horizonts ziehen. **Gradient**: die Linie mit weichem Übergang. **Irregular mask**: den Himmel mit der linken Maustaste malen, mit der rechten radieren; **Brush size** (Schieberegler oder Mausrad). **Detect sky** malt eine erste Maske aus Helligkeit und Sternen – ungefähr, also prüfen und mit dem Pinsel korrigieren. **Mask feather** macht den Rand der Maske weich, nur zum Himmel hin, sodass der Vordergrund nichts vom Himmel abbekommt (keine Säume am Horizont). **Show detected stars** kreist die Sterne ein, die der letzte Lauf in der gezeigten Aufnahme gefunden hat. |
| **Sigma clipping** | Wie streng Ausreißer entfernt werden (unten / oben × σ). Üblich sind 2–3; kleinere Werte entfernen mehr, lassen aber mehr Rauschen. Braucht mindestens 4 Aufnahmen, am besten 10 und mehr. |
| **Frame rejection** | Lässt Aufnahmen weg, die deutlich schlechter als die typische sind: Wolken oder Tau (schwächere Sterne), wenige Sterne, unscharfe oder längliche Sterne (Fokus, Wind, angestoßenes Stativ). |
| **RAW conversion** | Weißabgleich (Kamera, Bilddurchschnitt, Tageslicht), Demosaicing (Vorgabe AHD; DHT oder AAHD haben bei hoher ISO oft weniger Farbrauschen) und **half size** (4× weniger Pixel, viel schneller). |
| **Reduce dynamic noises** | Findet und entfernt Hotpixel auch ohne Dunkelbilder. |
| **Reduce distor. effects** | Korrigiert Objektivverzeichnung, die in den Ecken langer Serien kurze Spuren hinterlässt: **Lens**, **Complex** (Ultraweitwinkel, Refraktion am Horizont) oder **Auto**. |
| **Merge 4 pixels** | Addiert 2×2 Pixel: heller und rauschärmer, halbe Breite und Höhe. |
| **Time-lapse** | Stackt jeweils N aufeinanderfolgende Aufnahmen zu einer; ergibt eine nummerierte Folge rauscharmer Bilder in einem Ordner neben der Ausgabe. |

### 13.4 Die Ansichten

- **Base image**: das Bild, an dem ausgerichtet wird; hier wird die
  Himmelsregion eingezeichnet. Ein Klick auf eine Datei in der Eingabeliste
  zeigt sie an.
- **Result**: der Stack nach der Nachbearbeitung. Ziehen verschiebt, das
  Mausrad zoomt, **Fit** zeigt alles.
- **Frame report**: ein Diagramm aller Messwerte pro Aufnahme, jeweils durch
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
Einstellungen, **Save result…** speichert erneut.

| Einstellung | Wirkung |
|---|---|
| **Background extraction** | Zieht ein glattes Modell des Himmelshintergrunds ab (Verläufe durch Lichtverschmutzung oder Mondlicht). |
| **Color calibration** | Macht den Himmelshintergrund neutral grau und den durchschnittlichen Stern weiß. |
| **Reduce colour noise** | Glättet nur die Farben und entfernt die bunten Pünktchen von Aufnahmen mit hoher ISO. |
| **Light pollution** | **Deep sky**: entfernt ein glattes Glühen. **Uneven**: folgt dem Glühen Kachel für Kachel (Lichtglocken von Städten im Weitwinkel). **Strength** bestimmt, wie viel entfernt wird. Zuerst die Vignettierung korrigieren (Vignettierungsbilder), sonst wird es schlimmer. |
| **Enhance star light** | Hebt die Sterne über den Himmelshintergrund, ohne dessen Rauschen zu verstärken. |
| **Auto brightness** | Bringt den Himmel auf eine angenehme Helligkeit, ohne die hellsten Sterne zu beschneiden. |
| **High dynamic range** | Komprimiert helle Sterne und Lichter, statt sie zu beschneiden. |
| **Tone** | **sRGB curve** (wie eine Kamera), **Stretch** (Schwarzpunkt, Mitteltöne, Weißpunkt; automatisch oder von Hand) oder **Linear** (keine Kurve). |
| **Color space** | sRGB oder Adobe RGB für TIFF- und JPEG-Ausgabe. |
| **Linear (for processing)** | Schaltet alles ab und speichert ein lineares 32-Bit-FITS: das, was Siril oder PixInsight erwarten. |
| **Use in the Stitcher** | Fügt das gespeicherte Ergebnis als ein Teilbild eines Panoramas in den Reiter Panorama stitcher ein (siehe 13.7). |

### 13.6 Projekte

**Save project…** schreibt eine `.stackproj`-Datei mit den Dateilisten, der
Himmelsregion und allen Einstellungen. **Open project…** (oder `panostitch
nacht.stackproj`) lädt sie wieder. Dateien werden über ihren
vollständigen Pfad gefunden, oder relativ zur Projektdatei, wenn der Ordner
verschoben wurde.

### 13.7 Panoramen aus gestackten Teilbildern

Ein rauscharmes Milchstraßen-Panorama: Nehmen Sie an jeder Position des
Panoramas mehrere Aufnahmen auf (mit Überlappung zwischen den Positionen),
dann:

1. Stacken Sie die Aufnahmen der ersten Position (Start) und klicken Sie auf
   **Use in the Stitcher**.
2. Ersetzen Sie die Himmelsaufnahmen durch die der nächsten Position, geben
   Sie der Ausgabe einen neuen Namen, Start, wieder **Use in the Stitcher**;
   und so weiter.
3. Im Reiter Panorama stitcher fügt **Run all steps** die gestackten
   Teilbilder zusammen.

Speichern Sie jedes Teilbild als `.tif`; dieselbe Nachbearbeitung für alle
Teilbilder hält Helligkeit und Farben gleich.

### 13.8 Tipps

- Nehmen Sie mindestens 10–20 Himmelsaufnahmen mit gleichen Einstellungen
  auf, direkt danach 10–20 Dunkelbilder, ohne die Kamera zu bewegen.
- Setzen Sie die Himmelsregion sorgfältig: Bäume und Gebäude, die als Himmel
  zählen, verschmiert die Ausrichtung. **Detect sky** ist ein schneller
  Anfang, nicht das letzte Wort.
- Sehen Sie nach einer Quick preview in den Frame report: Das Diagramm zeigt
  sofort, welche Aufnahmen schlechter sind (Wolken, Wind, Tau).
- Nehmen Sie **Freeze ground**, wenn der Vordergrund wichtig ist, sonst
  **Select best pixels**.
- Zur Weiterverarbeitung in anderen Programmen als `.fits` speichern und
  **Linear (for processing)** einschalten.
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
| `*.panoproj`, `*.stackproj` | Ihre Projektdateien (wo immer Sie sie speichern). |
| `ui.toml` im Einstellungsordner | Die Schalter für das Aussehen (siehe [Programm starten](#2-programm-starten)). |
| `docs/manual_de.md` | Dieses Handbuch (Deutsch). |
| `docs/manual_en.md`, `docs/manual_cs.md` | Das Handbuch auf Englisch und Tschechisch. |

Quellcode (um das Programm selbst zu bauen):

| Pfad | Inhalt |
|---|---|
| `Cargo.toml` | Das Projekt und die verwendeten Bibliotheken. |
| `src/image.rs`, `src/io/` | Bilder; Laden und Speichern (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Zusammenfügen: Merkmale, Zuordnung, Platzierung, Verfeinerung, Projektionen, Überblendung. |
| `src/stacker/` | Sterne stacken. |
| `src/gui/` | Das Fenster: beide Tabs und die Dateidialoge. |
| `src/bin/` | Die beiden Programme, `panostitch` und `panostack`. |
| `tests/` | Tests: Stacken mit künstlichen Nachtszenen, Zusammenfügen, Dateiformate und beide Tabs des Fensters. Ausführen mit `cargo test --release`. |
| `examples/scripted.rs` | Das Zusammenfügen aus eigenem Rust-Code verwenden. |
