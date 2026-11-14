<div align="center">
  <img src="assets/app-icon/photosuite.png" alt="PhotoSuite App-Icon" width="96">
  <h1>PhotoSuite</h1>
  <p><strong>Deine Fotos. Deine Ordner. Deine Sammlung.</strong></p>
  <p>Eine lokale Foto- und Videogalerie mit Audio-Diary für macOS, geschrieben in Rust.</p>
</div>

PhotoSuite öffnet vorhandene Fotoordner als übersichtliche Galerie. Keine Cloud, kein Konto, keine neue Datenbank für deine Sammlung: Deine Struktur nach **Jahr und Monat** bleibt erhalten. Die Anwendung liest deine Originale standardmäßig nur und merkt sich die zuletzt geöffnete Bibliothek.

> **Projektstatus:** Umfangreicher Prototyp für macOS. Noch keine öffentlich signierte oder notarisierte Distribution; Videoformate hängen von den macOS-Codecs ab. Die Grenzen sind unten ausdrücklich beschrieben.

## Was du damit machen kannst

| Funktion | Verhalten |
| --- | --- |
| **Native Menüs** | PhotoSuite, File, Edit, View, Window und Help über muda; Aktionen und Tastenkürzel zentral verbunden. |
| **Fotowand** | Virtualisiertes Raster mit fünf Größenstufen, gleichmäßig gefüllten Zeilen und quadratischem Beschnitt oder vollständigem Seitenverhältnis. |
| **In der Sammlung navigieren** | Alles-Ansicht, aufklappbare Jahre, Monatsauswahl, Dateinamens-/Pfadsuche und Videofilter. |
| **Großansicht** | Zentrierte Fotos, Datum/Uhrzeit im Titel, drei klar getrennte Aktionsgruppen einschließlich Tags/Finder und eine zentrierte Hörleiste mit Play/Aufnahme nur für Kommentare zum aktuellen Foto; direkter Zugang zum Jahresverlauf. |
| **Videos ansehen** | Native Vorschauen, sechs zeitlich verteilte Standbilder und Wiedergabe über AVFoundation/AVKit. Kein externes FFmpeg erforderlich. |
| **Audio-Diary** | Pro Jahr ein zoomabhängiges Kachelraster mit Datum/Uhrzeit und eine durchgehende Hörleiste mit proportionalen Dateiabschnitten, globalem Schieber und nativer Audiowiedergabe. Keine Playlist-Ansicht. |
| **Ausrichtung korrigieren** | EXIF inklusive Spiegelungen; manuelles Drehen zunächst nur für die Sitzung. Optional JPEG-EXIF ohne Neukompression speichern. |
| **Kontrolliert importieren** | Prüflauf mit einem kompakten Ergebnis pro Datei, zuschaltbaren Arbeitsschritten, metadatenbasierten Zielnamen und Kollisionsprüfung. |
| **Kollisionsduplikate prüfen** | Vollständiger Pixelvergleich geeigneter Bilder, jeweils eine Gruppe mit ausdrücklicher Entscheidung. |
| **Tags und Alben** | Dateien mehreren kleingeschriebenen Tags zuordnen; Schreibvarianten werden zusammengeführt. Automatische Tags: commented für kommentierte Fotos, comment für Kommentaraufnahmen und diary für Tagebuchaufnahmen. Katalog im Speicher, Speicherung als YAML in der Bibliothek. |
| **Einstellungen behalten** | Anzeige, Filter, Tastenkürzel und Schreibmodus pro Bibliothek als YAML; nur der letzte Bibliothekspfad bleibt lokal. |

Die obere Buttonsammlung ist durch native Menüs ersetzt. Ein zusätzlicher Kopfbereich entfällt vollständig. **Edit → Find… (⌘F)** öffnet die Suche als Dialog; die Vorschaugröße steht unter **View → Thumbnail Size**, der Bibliothekspfad im Fenstertitel. **File** öffnet/aktualisiert/importiert, **Edit** bietet Bildbefehle, Textbearbeitung und Tags, **View** die Anzeigeoptionen. **PhotoSuite → Settings…** öffnet die Einstellungen.

Die Anwendung bleibt ein macOS-Prototyp: muda unterstützt weitere Systeme, aber die gesamte App ist damit noch nicht portiert. Windows-Menüanbindung ist vorbereitet und ungetestet; Linux/BSD benötigt für muda einen GTK-Fensterhost, den unser eframe-Host bisher nicht bietet. Es gibt keine Ersatz-Menüleiste.

## Das neue Galeriedesign

Dunkle Graphitflächen, helle kontrastreiche Texte und kompakte Abstände lassen viel Platz für Fotos. Das Galeriedesign bleibt auch bei hellem macOS-Systemdesign dunkel und verwendet dazu passende helle Textfarben. Eine schmale Jahres-/Monatsnavigation mit kompakten Zeilen, vollbreiten Einträgen und Scrollbalken am rechten Rand, klare Schriftgrößen und dezente blaue Auswahlzustände strukturieren die Sammlung. Jahre sind zunächst eingeklappt; darunter stehen ausschließlich die Monate. Der Jahresklick zeigt die Jahresfotowand. Audio ist über die obere Hörleiste und das Playerfenster erreichbar. Beim Öffnen wird zuerst nur die oberste Verzeichnisebene gelesen, sodass die Jahre früh verfügbar sind. Inhalte folgen jahresweise im Hintergrund; das Aufklappen eines Jahres wählt dessen Gesamtansicht und lädt es bevorzugt. Bereits geladene Jahre bleiben bedienbar. Einzelne laufende System-/Codecaufrufe können den Wechsel noch kurz verzögern. In der Großansicht stehen Navigation und Medienaktionen in zwei festen Zeilen; Fotos bleiben horizontal zentriert. Einstellungen und Import verwenden einheitliche Optionsgruppen.

**Screenshots folgen separat.** Die Bildaufnahme wurde zurückgestellt; diese README enthält deshalb keine Platzhalterbilder oder Mockups. Der vorbereitete Ablauf und die Quellen der isolierten Demobibliothek stehen unter [Screenshot-Dokumentation](docs/screenshots/README.md).

## Auf macOS installieren

Voraussetzungen zum Bauen: **Rust/Cargo**, die **Xcode Command Line Tools**, **Python 3** sowie **ExifTool** für die Paketierung, Video-/Audio-Datumsmetadaten und schreibende EXIF-Funktionen. Mit vorhandenem Homebrew lässt sich ExifTool über `brew install exiftool` installieren. Der Paketbau verwendet lokal verfügbare Cargo-Abhängigkeiten; bei einem frischen Checkout zuerst `cargo fetch` ausführen.

```sh
cargo fetch
make install
```

Das erstellt eine Release-Version und installiert **`/Applications/PhotoSuite.app`**. Danach im Finder unter **Programme → PhotoSuite** öffnen. Eine laufende PhotoSuite vor dem Aktualisieren schließen.

Weitere Möglichkeiten:

```sh
make app       # Nur target/macos/PhotoSuite.app bauen
make pkg       # Doppelklick-Installer unter target/macos/ erstellen
make uninstall # Installierte App entfernen; Fotos und Preferences bleiben erhalten
```

Installation ohne Schreibrechte auf `/Applications`:

```sh
python3 scripts/macos_app.py install --destination "$HOME/Applications"
```

Das App-Bundle enthält ExifTool, dessen Module, das App-Icon und Lizenzhinweise. Homebrew ist zur **Laufzeit des gebauten App-Bundles** nicht erforderlich. macOS-Systemframeworks und System-Perl werden verwendet. Aktuell wird für die Architektur des bauenden Macs gebaut; ein universelles Intel-/Apple-Silicon-Paket wird nicht automatisch erzeugt.

Es gibt noch keinen veröffentlichten Homebrew-Cask und keinen Mac-App-Store-Download. Der lokale Paketbau verwendet eine Ad-hoc-Signatur; sie ersetzt keine Developer-ID-Signatur oder Apple-Notarisierung.

## Erste Schritte

1. **File → Open Library…** wählen und eine Fotosammlung öffnen. Der aktive Pfad steht im Fenstertitel.
2. Ein zunächst eingeklapptes Jahr öffnen, einen Monat auswählen oder über das Suchfeld filtern.
3. Ein Foto anklicken. **← / →** wechseln das Medium, **Escape** schließt die Großansicht. **Leertaste** startet bei Videos den nativen Player.
4. Unter **View** Vorschaugröße, quadratischen Beschnitt und Seitenleiste einstellen.
5. **PhotoSuite → Settings…** beziehungsweise **⌘,** öffnet die Preferences. Beim nächsten Start wird die letzte Bibliothek automatisch wieder geöffnet, sofern diese Option aktiviert bleibt.

Die Einstellungen liegen als lesbare **`photosuite.yaml` direkt im Hauptverzeichnis der jeweiligen Bibliothek**. Gespeichert werden Vorschaugröße und -form, Seitenleiste, Videofilter, EXIF-Schreibmodus, Startoption und Tastenkürzel. Beim Bibliothekswechsel werden deren eigene Werte geladen. Nur der zuletzt geöffnete Bibliothekspfad bleibt lokal unter `~/Library/Preferences/de.photosuite.PhotoSuite.library.plist`. Er steht nicht in der YAML-Datei und reist nicht mit der Sammlung mit.

Vorhandene `de.photosuite.PhotoSuite.settings.plist` und das frühere App-Domain bleiben als Migrationsquellen erhalten. Fehlt einer Bibliothek die YAML-Datei, werden bisherige Einstellungen übernommen und beim Laden/Speichern im neuen Format abgelegt. Einstellungen werden im Hintergrund geschrieben; schnelle Bibliothekswechsel erhalten auch noch ausstehende Änderungen der vorherigen Bibliothek. Schreibfehler erscheinen in Fußzeile und Einstellungsdialog. Defekte, unbekannte oder verlinkte Einstellungsdateien werden nicht überschrieben. Auf schreibgeschützten Bibliotheken gelten Änderungen nur in der Sitzung. Medien bleiben unverändert; die Verwaltungsdatei ist die ausdrückliche Ausnahme zum Nur-Lesen-Modus.

„Originale: Nur Lesen“ in der Fußzeile bezieht sich auf die Mediendateien. `photosuite.yaml` wird als Verwaltungsdatei auch in diesem Modus gespeichert.

## Tags und virtuelle Alben

**Edit → Tags and Albums…** öffnet die Verwaltung. Einen Tag anlegen, Dateien zuordnen und bei Bedarf **„Album“** aktivieren: Alle Tags erscheinen links als umbrechende Wolke unter den Jahren; das Album-Flag bleibt in der Verwaltung erhalten. Tags können umbenannt und entfernt werden; die Mediendateien bleiben immer erhalten. Mehrere Tags je Datei sind möglich. In einem Album funktionieren Suche, Videofilter, Vorschaugrößen und Großansicht weiter. Audiozuordnungen sind ebenfalls unterstützt; ein Audioklick öffnet den bestehenden Jahresplayer. Beim Browsen im Bildbetrachter werden Audioeinträge übersprungen.

Eine Datei über **Rechtsklick → „Tags bearbeiten …“** markieren: das funktioniert in Fotowand und Diary-Raster. In der Großansicht gibt es zusätzlich **„Tags …“** oberhalb des Bildes. Häkchen ordnen die Datei einem Tag zu oder entfernen nur diese Zuordnung. In der Verwaltung zeigt **„Anzeigen“** auch nicht als Album markierte Tags als Filter. „Alle Erinnerungen“, ein Jahr/Monat oder „Filter aufheben“ führt zurück zur normalen Navigation.

Der vollständige Katalog liegt als **`photosuite-tags.yaml` im Hauptverzeichnis der Bibliothek**:

```yaml
version: 1
tags:
  Urlaub:
    - 2023/08/img_20230816_153000.jpg
    - 2023/08/img_20230816_153500.mp4
  Familie:
    - 2023/08/img_20230816_153000.jpg
albums:
  - Urlaub
```

Relative Pfade unterscheiden gleichnamige Dateien verschiedener Jahre/Monate und erlauben, die Bibliothek als Ganzes zu verschieben. Alle Zuordnungen werden einmal im Hintergrund geladen und vollständig im Speicher gehalten; Albumfilter lesen keine Dateien oder EXIF nach. Für noch nicht geladene Albumjahre wird die Bibliotheksladung priorisiert. Speichern läuft im Hintergrund; wartende Zwischenstände werden zum neuesten vollständigen Katalog zusammengefasst. Fehler erscheinen im Dialog und in der Fußzeile; während ungespeicherter Tagänderungen wird ein Bibliothekswechsel verhindert. Mit „Erneut speichern“ oder bewusstem „Nicht gespeicherte Änderungen verwerfen“ lässt sich der Zustand auflösen. Beim normalen Beenden wird der neueste angenommene Stand gespeichert.

Die App verschiebt, kopiert oder löscht für Tags keine Originale und schreibt keine Tags in EXIF. Gelöschte/extern umbenannte Dateien behalten ihre Zuordnungen als Verweise; fehlende oder noch nicht gescannte Dateien sind im Album nicht sichtbar. Keine automatische Suche nach beliebigen gleichnamigen Ersatzdateien. Manuelle YAML-Änderungen bei geschlossener App vornehmen oder danach **„Datei neu laden“** verwenden. Vorhandene externe Änderungen, ungültige YAML-Dateien und unbekannte Versionen werden nicht still überschrieben. Der erste Entwurf bietet Einzeldatei-Zuweisung, keine Mehrfachauswahl, automatische Tag-Erkennung oder manuelle Albumreihenfolge. Kataloggrenze: 32 MiB YAML; keine globale Sperre zwischen mehreren App-Instanzen. Fotos, Videos und Audio bleiben unverändert.

## Import und Umgang mit Originalen

Ein Import wird ausdrücklich gestartet und **kopiert** ausschließlich ins gewählte Ziel. Beim Import selbst wird die Quelle niemals gelöscht, verschoben oder umbenannt. Der separate Schritt **„Importquellen in den Papierkorb …“** ist erst nach einem tatsächlichen Import und ausdrücklicher Bestätigung verfügbar. Ein Prüflauf schreibt keine Medien. Das Protokoll zeigt standardmäßig einen Eintrag pro Datei mit Entscheidung und gekürztem Zielpfad. Tooltips zeigen vollständige Pfade, Datum und Datumsquelle. „Alle Schritte anzeigen“ blendet das ausführliche Protokoll ein; „Protokoll kopieren“ übernimmt jeweils die gewählte Ansicht. Bei Fotos/Videos werden fehlende Aufnahmedaten sichtbar gemeldet und nicht durch ein Dateisystemdatum ersetzt. Audio verwendet die unten beschriebene Priorität mit ausdrücklich gekennzeichneten Fallbacks. Bei Abbruch werden ausschließlich importereigene temporäre Zieldateien entfernt; fertige Kopien und alle Quellen bleiben erhalten.

```text
Fotos/
├── 2025/
│   └── 10/
│       └── img_20251016_153000.jpg
└── 2026/
    ├── 09/
    │   └── img_20260916_035811.jpg
    └── audio/
        ├── aud_20260115_143000.m4a
        └── aud_20260916_035811.wav
```

Fotos erhalten Datum und Uhrzeit aus **EXIF DateTimeOriginal**, Videos aus **QuickTime CreateDate**. Bei Foto- und Audiokollisionen wird bytegenau verglichen: identische Dateien werden übersprungen, verschiedene mit Nummernzusatz kopiert. Bei Videos genügt der generierte Zielname zum Überspringen.

Es gibt drei ausdrücklich freizugebende Schreibaktionen:

- **Import:** neue, verifizierte Kopien in die Zielbibliothek.
- **„Drehungen in JPEG speichern“:** neue manuelle Drehbefehle ändern EXIF im Original, ohne JPEG-Neukompression und ohne dauerhafte Sicherungsdatei. Keine Massenänderung beim Öffnen.
- **Duplikate in den Papierkorb:** ausschließlich nach Gruppenentscheidung und erneuter Prüfung. Keine endgültige Löschung; nicht sicher vergleichbare Bilder bleiben erhalten.

Details zu Prüflauf, Abbruch, Kollisionsnamen und Duplikatvergleich stehen im [Benutzerhandbuch](docs/USER_GUIDE.md).

### Importquellen nachträglich aufräumen

Nach einem tatsächlichen Import öffnet **„Importquellen in den Papierkorb …“** einen separaten Dialog. Jede protokollierte Quelle wird mit ihrem konkreten Importziel angezeigt: Importentscheidung, Quellpfad, Zielpfad und aktuelles Prüfergebnis. Quelle und Ziel müssen reguläre, nicht leere Dateien in getrennten Importordnern sein und vollständig bytegenau übereinstimmen. Symbolische Pfade, Hardlinks zwischen Quelle und Ziel, fehlende/abweichende Ziele und fehlgeschlagene Importe werden nicht freigegeben. Auch allein wegen des Videonamens übersprungene Dateien benötigen hier den vollen Bytevergleich.

Erst nach abgeschlossener Prüfung ist **„… freigegebene Quellen in den Papierkorb“** aktiv. Abbrechen/Schließen lässt alle noch nicht verschobenen Dateien erhalten. Vor jeder bestätigten Aktion werden Inhalte, Dateiidentität, Änderungsstand und Ordneridentität erneut geprüft; seit der Freigabe veränderte Quellen oder Ziele verhindern die Aktion. Es werden ausschließlich Quellen in den macOS-Papierkorb verschoben, niemals Zieldateien oder ganze Verzeichnisse, und es gibt keinen Rückfall auf endgültiges Löschen. Prüfläufe bieten diese Aktion nicht an. Der Dialog bezieht sich auf den aktuellen, noch im Importprotokoll vorhandenen Lauf und ist kein dauerhaftes Importarchiv.

Eine bereits laufende Papierkorbaktion kann nach Abbruch noch abgeschlossen werden; ihr Ergebnis bleibt sichtbar. Unquittierte Vorgänge werden als unklar gekennzeichnet und nicht blind wiederholt. Änderungen durch andere Programme im kurzen Zeitraum nach der letzten Prüfung sind nicht durch eine systemweite Dateisperre ausgeschlossen; der Papierkorb erhält die Wiederherstellbarkeit.

## Audio-Diary: ein Jahresverlauf

Die Hörleiste neben der Jahres-/Monatsüberschrift und das Fenster-Symbol öffnen die Audiowiedergabe. Im Player Tagebuch, Kommentare oder Beide wählen. Unter den Steuerelementen stehen alle passenden Dateien chronologisch mit Dateiname, Bezugsdatum und tatsächlichem Aufnahmedatum. Die Liste ist scrollbar; Doppelklick springt zur Aufnahme.

Der native Player bietet **eine zusammenhängende Hörleiste**, keine Playlist. Beim Schnellstart kommen Reihenfolge und Aufnahmezeit zunächst aus den Dateinamen; die Abschnitte sind zunächst gleich breit, ganz ohne Größenabfragen. Mit ersten Prüfergebnissen erhalten ungeprüfte Abschnitte das mittlere Gewicht der bereits geprüften Clips; diese vorläufigen Gewichte werden nicht als echte Sekunden ausgegeben. **Grau bedeutet ungeprüft, das bisherige Blau bedeutet erfolgreich geprüft, Rot kennzeichnet einen Prüfungsfehler.** Ein kompakter Fortschritt und Segment-Tooltips erläutern den Stand. Sobald ein Clip geprüft ist, werden Datum, Reihenfolge und Breite korrigiert. Erst nach vollständiger Prüfung belegt jede Datei exakt den Anteil ihrer Dauer an der Gesamtdauer. Eine Aufnahme von 10 Sekunden neben einer von 30 Sekunden erhält entsprechend ein Viertel der Leiste. Es gibt keine stillen Kalenderlücken und keine erzeugte Sammeldatei. Der Schieber kann über alle Dateien hinweg springen; der Player zeigt ausschließlich das aktuelle Aufnahmedatum und die dazugehörige Uhrzeit. Diese Uhrzeit entspricht dem Beginn der aktuellen Aufnahme plus der Position innerhalb dieser Datei, einschließlich Datumswechsel um Mitternacht. Beim Überschreiten einer Dateigrenze wird auf die Aufnahmezeit der nächsten Datei gewechselt. Datumsquellen sind per Tooltip erreichbar. Tooltips erläutern die Segmente. Leertaste spielt/pausiert, Pfeiltasten springen fünf Sekunden, Buttons 15 Sekunden; Lautstärke ist einstellbar.

Die Wiedergabe enthält alle Audiodateien des gewählten Audiobereichs dieses Jahres, unabhängig vom Such- oder Videofilter. „Audio“ mischt Tagebuch und Kommentare, „Tagebuch“ spielt `aud_…`, „Kommentare“ spielt `comment_…`. Reihenfolge: ausgewähltes Datumsfeld, bei Gleichstand Dateipfad. Vor dem Öffnen werden ausschließlich die bereits bekannten Dateinamen ausgewertet; es gibt weder Dateigrößenabfragen noch Metadaten-/Daueranalysen der gesamten Liste. Zunächst lädt nur die gewählte Aufnahme. Erst wenn sie tatsächlich läuft, prüft ein Hintergrundauftrag die Dateien einzeln, beginnend mit der gewählten Aufnahme. Abschnittsbreiten und Reihenfolge dürfen sich dabei ändern; die laufende Datei und ihre lokale Hörposition bleiben erhalten, ohne sie neu zu laden. Unlesbare Aufnahmen bleiben mit sichtbarem Fehler in der Leiste. Es werden keine Audiodateien verschoben, umbenannt oder verändert. Fehlt ein gültiger Dateizeitstempel, steht vorläufig „Datum wird geprüft“; später gilt die übliche Metadatenpriorität.

Unter der Hörleiste erscheinen **fünf zeitlich passende Fotos** aus der geöffneten Bibliothek. Das größere mittlere Foto liegt am nächsten an der aktuellen Aufnahmezeit; links folgen seine zwei vorherigen, rechts seine zwei nächsten zeitlichen Nachbarn. Fehlende Nachbarn bleiben als leere Plätze sichtbar. Die Fotoleiste füllt gemeinsam die Breite der Audiospur. Die Bilder werden zentriert und unverzerrt beschnitten, sodass jede Kachel vollständig ausgefüllt ist. Unter jedem Foto stehen **sein Aufnahmedatum und seine Uhrzeit**; Datumsquelle und Pfad bleiben im Tooltip zugänglich. Ein Klick navigiert **im Hintergrund zur Jahresübersicht des Fotos**, scrollt zum Bild und markiert es mit einem blauen Rahmen. Ein Bildbetrachter wird dabei nicht geöffnet; der Audioplayer behält den Fokus und spielt weiter. Such- und Videofilter werden zurückgesetzt, damit das Foto sichtbar ist. Der zugehörige Jahreseintrag wird aufgeklappt.

Die Fotosuche beginnt erst, wenn Audio tatsächlich wiedergegeben wird. Datumsindex und Vorschauen werden im Hintergrund geladen und innerhalb des geöffneten Players wiederverwendet. Eine neue Auswahl erfolgt beim Clipwechsel, nach bewusstem Springen oder einer korrigierten Aufnahmezeit, nicht jede Sekunde. Fotozeiten stammen zuerst aus vollständigen Dateinamen (insbesondere `img_JJJJMMTT_HHMMSS`); nur nicht benannte Fotos werden separat über EXIF ergänzt. Ohne auswertbares Datum wird das Foto übersprungen. Diese Anzeigeausnahme ändert die strengeren Foto-Importregeln nicht. Bei großen Bibliotheken kann die erste Suche dauern; Audio läuft bereits weiter. Die Kandidatenliste entspricht dem Bibliotheksstand beim Öffnen des Players; nach Änderungen den Player erneut öffnen. Vorschauen berücksichtigen EXIF-Ausrichtung, verwenden macOS ImageIO und werden ausschließlich im Speicher gehalten.

### Datum und Audio-Import

Audio wird ohne Umwandlung als **`Jahr/audio/d/aud_JJJJMMTT_HHMMSS.ext`** kopiert. Datumsquellen werden in dieser Reihenfolge geprüft:

1. Eingebettete Aufnahmezeit: BWF/RIFF `DateTimeOriginal`, ID3 `RecordingTime`, XMP `DateTimeOriginal`.
2. Inhaltserstellungsdatum: QuickTime/Keys `CreationDate`, vollständiges RIFF `DateCreated` plus `TimeCreated`.
3. Container-/XMP-Erstellung: QuickTime `CreateDate`, XMP `CreateDate`.
4. **Gekennzeichneter Fallback:** Datum und Uhrzeit aus dem vollständigen Audio-Dateinamen: z. B. `2023-12-27_22-51-32`, `2023_12_27_22_51_32` oder `20231227225132`. Zwischen den Datums-/Zeitbestandteilen sind `-`, `_` oder keine Trennzeichen erlaubt. `aud_…`-Importnamen und getrennte numerische Kollisionszusätze bleiben unterstützt. Dieser Fallback erhält das gewählte Datum nach dem Kopieren.
5. **Gekennzeichneter Fallback:** Dateisystem-Erstellungsdatum in lokaler macOS-Zeit; niemals Änderungsdatum. Das ist kein nachgewiesenes Aufnahmedatum und kann etwa nach einem Export abweichen.

Nur vollständige, gültige Datum-/Uhrzeitwerte werden benutzt; Jahreszahlen und Datumswerte ohne Uhrzeit werden nicht still ergänzt. Bekannte Nullwerte wie `1904:01:01 00:00:00` werden verworfen. Enthaltene Zeitzonen werden protokolliert; Benennung und Sortierung verwenden den angegebenen lokalen Zeitwert ohne versteckte Verschiebung. Die Datumsquelle steht im Prüflauf, im Importprotokoll und in den Tooltips der Diary-Kacheln und des Players. Widersprüchliche Metadaten lassen sich nicht pauschal beweisen; die definierte Priorität wählt die plausibelste vorhandene Quelle.

Erkannt werden WAV/WAVE, MP3, M4A/M4B, AAC, AIFF/AIF/AIFC, CAF, FLAC, AC3/EAC3, AMR, AU/SND und MP2; tatsächliche Abspielbarkeit prüft AVFoundation. Reine Audio-MP4/MOV und andere bereits erkannte Mediencontainer werden über ihre Tracks als Audio klassifiziert. Enthält eine Datei Video, bleibt sie ein Video und wird nicht ins Diary importiert. Nicht aufgeführte Dateiendungen werden im ersten Entwurf noch nicht dynamisch aus allen macOS-Formaten ermittelt.

Audio erhält dieselben sicheren Kopier- und Kollisionsregeln wie Fotos: bytegleiche Dateien überspringen, verschiedene mit `_001` usw. erhalten. **Der Import löscht niemals Quelldateien.** Die separate Pixelduplikat-Bereinigung gilt nur für Fotos.

## Bekannte Grenzen

- Bilder: JPEG, PNG, WebP, TIFF, GIF und BMP. **HEIC und Kamera-RAW werden derzeit nicht unterstützt.** Animierte Formate sind kein vollständiger Animationsplayer.
- Videos: Dateiendungen werden erkannt; tatsächlich dekodierbare Formate hängen von AVFoundation und installierten macOS-Codecs ab. Native Videovorschau, Player und Papierkorb sind auf macOS ausgelegt.
- Duplikatfreigabe: konservativer, vollständiger Pixelvergleich statischer JPEG-/PNG-Dateien; keine unscharfe Ähnlichkeitssuche. Farbprofilunterschiede und unklare Formate verhindern automatische Freigaben. Grenzen: 32 Megapixel bzw. 256 MiB pro Datei.
- Zwei Decoder, 32 wartende Aufträge und ein Galeriecache mit höchstens 256 MiB dekodierten RGBA-Daten bzw. 1024 Einträgen begrenzen die Bildhaltung. Rastervorschauen haben 256/512/1024 Pixel, die Großansicht maximal 4096 Pixel. Alte Cachetexturen werden freigegeben. Temporäre vollständige Dekodierungen und Grafiktreiber benötigen zusätzlichen Speicher; das vollständige Importprotokoll wächst mit dem jeweiligen Lauf.
- Audio-Diary ist ein erster macOS-Entwurf: das Vorschaubild ist ein Symbol und keine berechnete Wellenform; keine manuelle Datumskorrektur und keine garantierte Unterstützung jedes Systemformats. Die Startbreiten sind bewusst vorläufig und anfangs gleichmäßig, später an den Mittelwert der geprüften Clips angelehnt. Vor dem Start werden keine Größen aller Dateien abgefragt. Dauer und Datumsmetadaten werden anschließend einzeln im Hintergrund geprüft; einzelne Systemaufrufe sind nicht sofort abbrechbar. Die Wiedergabe verwendet einen AVPlayer für die jeweils aktuelle Originaldatei; beim automatischen Dateiwechel kann eine kurze Ladepause entstehen, lückenlose Wiedergabe ist nicht garantiert. Sekundensprünge über mehrere noch ungeprüfte Dateien können vorläufig ungenau sein. Geprüfte Breiten sind exakt proportional zur Dauer, ohne Mindestbreiten. Es gibt noch keinen persistenten Dauercache.
- Keine Bearbeitungs-Suite für Zuschnitt, Belichtung oder Farbkorrekturen. Nicht gespeicherte Drehungen sind Sitzungskorrekturen.
- Defekte Preferences werden sichtbar gemeldet und nicht überschrieben. Beim normalen Beenden werden ausstehende Einstellungen gespeichert; erzwungenes Beenden kann laufende Schreibvorgänge unterbrechen.

## Entwicklung und Tests

```sh
cargo run -- /Pfad/zur/Fotosammlung
make check-architecture
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
make test-native # Audio-/Videotransport mit kontrollierten Testmedien
make test-audio  # Nativer Schnellstart, Korrekturen und Wiedergabe mit stummen Testdateien
cargo build --workspace
python3 -m unittest discover -s scripts -p 'test_*.py'
```

Kern ohne native Oberfläche, Video und ExifTool testen:

```sh
cargo test -p photosuite-domain -p photosuite-application
```

Automatisierte Tests prüfen unter anderem Importregeln, echte EXIF-Daten, Drehungen, Duplikatentscheidungen, native Videoframes, Preferences sowie schmale Layouts und konstante Kontrollhöhen. Tests für EXIF-Schreiboperationen benötigen ExifTool; native Videotests benötigen macOS-Mediendekodierung. Screenshots und visuelle Prüfung ergänzen die Tests.

### Architektur

```text
Desktop (egui/eframe, Adapter, Composition Root)
                    ↓
       photosuite-application (Use Cases, Ports, Sitzungen)
                    ↓
       photosuite-domain (Modelle und fachliche Regeln)
```

Domäne und Anwendung kennen weder egui noch konkrete Dateisystem-/Bilddecoderadapter. Technische Implementierungen werden in `main` injiziert. Das Design liegt zentral in `src/theme.rs`.

- [Architektur und bewusste Vereinfachungen](ARCHITECTURE.md)
- [Designsystem und UI-Regeln](docs/DESIGN.md)
- [Ausführliches Benutzerhandbuch](docs/USER_GUIDE.md)
- [Verbindliche Projektregeln](AGENTS.md)

Build-Dateien unter `target/` gehören nicht ins Repository. README und Screenshots werden bei wesentlichen Änderungen an Oberfläche, Funktionen oder Installation gemeinsam aktualisiert.

Der vertiefte Architekturreview und die korrigierten Stellen sind unter [Architekturreview](docs/ARCHITECTURE_REVIEW.md) dokumentiert. Bibliotheks-/Videowechsel verwenden begrenzte Anwendungssitzungen; Importabschlüsse und Preferences-Schreibaufträge werden ebenfalls zentral in der Anwendung verwaltet.


### Import auf exFAT und eingeschränkten Dateisystemen
Importkopien werden zuerst als eindeutig benannte `.tmp`-Dateien im Zielordner geschrieben, synchronisiert und bytegenau geprüft. Unterstützt das Laufwerk die Veröffentlichung ohne Überschreiben nicht (z. B. exFAT), verwendet PhotoSuite automatisch eine kompatible Strategie: endgültigen Zielnamen exklusiv neu anlegen, geprüfte Daten hineinkopieren, synchronisieren und erneut vollständig mit der Quelle vergleichen. Vorhandene Dateien und symbolische Links werden nicht überschrieben; die Quelle bleibt unverändert. Das gilt für Fotos, Videos und Audio.

Der Ersatzweg benötigt zusätzliche Lese-/Schreibzeit und vorübergehend Platz für beide Kopien im Ziel. Bei regulärem Fehler oder Abbruch werden nur eigene unvollständige Dateien entfernt. Die endgültige Datei ist während dieses Ersatzwegs bereits sichtbar; bei Stromausfall oder erzwungenem Prozessende kann eine unvollständige Kopie zurückbleiben. Sie wird nicht als erfolgreicher Import protokolliert. Ein erneuter Import vergleicht Foto-/Audiokollisionen bytegenau; bei Videos gilt weiterhin die dokumentierte Namenskollisionsregel. Es gibt keine automatische Entfernung solcher Überreste.


### Schneller Audio- und Fotostart
Beim Öffnen eines Audioplayers haben dessen Start und die Prüfung der gewählten Aufnahme Vorrang: neue Galerie-Scans, normale Rasterdekodierungen und ergänzende Datumsabfragen warten kooperativ. Danach werden sie wieder freigegeben. Bereits laufende Systemaufrufe können nicht sofort angehalten werden. Die native Audio-Ladewarteschlange hat erhöhte Benutzerpriorität; präzise Daueranalysen laufen nach dem Wiedergabestart.

Die fünf zugehörigen Fotos werden zuerst ausschließlich aus gültigen Foto-Importnamen (`img_JJJJMMTT_HHMMSS…`) oder vollständigen Datum-/Uhrzeitnamen ausgewählt. Dafür wird keine Bilddatei geöffnet. Nur die fünf gewählten Vorschauen werden danach dekodiert. Fotos ohne auswertbaren Namen werden getrennt im Hintergrund über Metadaten ergänzt, ohne weitere Clip-/Suchwechsel aufzuhalten. Bei benannten Fotos ist der Dateiname für den Kontextindex maßgeblich; abweichende EXIF-Zeiten dieser Dateien werden dort nicht zusätzlich vollständig eingelesen. Foto-Import und seine strenge EXIF-Regel bleiben unverändert. Ohne einen einzigen auswertbaren Fotodateinamen muss die erste Kontextsuche weiterhin Metadaten lesen.

Jahres-/Monatsansichten verwenden zunächst Ordner und Endungen, ohne jedes MP4/MOV per Codec zu analysieren. Container im Diary sind vorläufige Audiokandidaten und werden beim Abspielen geprüft. Importprüfläufe verwenden weiterhin die vollständige Medienprüfung. Ein Fotoindex ist ein Snapshot der bereits geladenen Sammlung; nach weiteren Bibliotheksänderungen den Player erneut öffnen.


### Versteckte Dateien beim Import
Dateien wie `._aud_…wav` sind gewöhnlich macOS-AppleDouble-Begleitdateien für Zusatzmetadaten auf Dateisystemen wie exFAT. Sie sind von importereigenen `.photosuite-import-….tmp`-Dateien zu unterscheiden. PhotoSuite berücksichtigt `._`-Begleitdateien weder als Medien noch als Importquellen und löscht sie nicht automatisch. Andere versteckte, echte Medien bleiben unterstützt.

Eigene Stagingdateien werden nach regulären Fehlern, Abbruch und dem exFAT-Ersatzweg ausdrücklich und mit Identitätsprüfung entfernt. Schlägt diese Bereinigung fehl, nennt das Importprotokoll den vollständigen Temp-Pfad und gegebenenfalls den bereits vollständig geprüften Zielstand. Fehlgeschlagene Aufräumvorgänge werden nicht stillschweigend als erfolgreicher Import gemeldet. Nach Abstürzen oder erzwungenem Beenden können weiterhin Überreste bleiben; vorhandene fremde Dateien werden nicht automatisch bereinigt.


Die Startkonfiguration wird ebenfalls im Hintergrund gelesen; ein nicht verfügbares altes Laufwerk blockiert die Fenstererstellung nicht. Explizite Startpfade und manuell gewählte Bibliotheken haben Vorrang. Für Kollisionsduplikate gilt eine Grenze von 100.000 Paarvergleichen pro Gruppe; größere Gruppen werden ohne Dekodieren/Entfernen mit einem Fehler abgewiesen. Der aktuelle [Architekturreview](docs/ARCHITECTURE_REVIEW.md) dokumentiert Schichtentrennung, Ressourcenregeln, Korrekturen und verbleibende Grenzen.

### Kompakte Audioleiste
Jahres- und Monatsansichten zeigen eine kompakte Jahres-Audioleiste: Play/Pause und anklickbare/ziehbare Zeitachse direkt neben der Monats-/Jahresüberschrift, ohne zusätzliche Beschriftung oder Mehr-Menü. Monatsseiten enthalten ausschließlich den gewählten Monat. Graue Abschnitte werden im Hintergrund geprüft und anschließend proportional zur echten Dauer blau angezeigt. Während einer laufenden Vorbereitung kann keine weitere Jahresvorbereitung gestartet werden.

**Window → Audio Player** zeigt denselben laufenden Player ohne Neustart. Das Schließen dieses Fensters lässt die Wiedergabe weiterlaufen. ▶ startet die Wiedergabe und wechselt dabei zu ⏸ zum Pausieren; ein separater Stop-Button entfällt. Jahres-/Monatswechsel unterbrechen die Wiedergabe nicht, ein Bibliothekswechsel stoppt sie. Die Anwendung hält den Zeitachsenentwurf und die Auswahl im Anwendungskern; die Oberfläche steuert denselben nativen Transport über einen frameworkunabhängigen Port.

### Getrennte Ablage, gemeinsame Audioansichten
Neue Tagebuchimporte landen direkt unter **`Jahr/audio/d/aud_JJJJMMTT_HHMMSS.ext`**. Kommentare liegen unter **`Jahr/audio/c/com_JJJJMMTT_HHMMSS.m4a`**, ihre einzelnen Zuordnungsdateien unter **`Jahr/audio/y`**. **Audio** öffnet alle Aufnahmen; darunter gibt es die virtuellen Filter **Tagebuch** und **Kommentare**; die physische Trennung ist für die gemeinsame Wiedergabe transparent. Ein Klick auf Audio wählt die gemeinsame Ansicht. Jahres-/Monats-Hörleisten spielen alle Kategorien; Kategorieansichten spielen nur ihre Art. Die Abschnitte werden weiterhin im Hintergrund auf ihre echte Dauer korrigiert.

Unbekannte Audionamen bleiben in der kombinierten Playeransicht sichtbar. Audiokommentare werden nach dem Bezugsdatum ihres verknüpften Fotos eingeordnet; das tatsächliche Aufnahmedatum bleibt getrennt erhalten. Alte `Jahr/diary`-Ordner und bereits vorhandene Kategorieunterordner bleiben ohne Umbenennen/Verschieben lesbar. Neue Importe bleiben Tagebuchaufnahmen; Audiokommentare lassen sich direkt am Foto aufnehmen; es gibt keine automatische Migration. Die Kollisionsprüfung bezieht sich auf das jeweilige Importziel; ältere Bestände werden nicht automatisch zusammengeführt oder dedupliziert.

### Jahresnavigation
Ein Jahr aufklappen: zuerst **Audio**, darunter direkt die Monate ohne zusätzliche Zwischenebene. Der Jahresklick zeigt alle Fotos/Videos des Jahres; ein Monat beschränkt die Fotowand auf diesen Monat. **Audio** zeigt alle Jahresaufnahmen und klappt die Filter **Tagebuch** und **Kommentare** aus. Ein zusätzlicher „Ganzes Jahr“- oder „Alle Aufnahmen“-Eintrag entfällt.

### Kompakte Galerie und Tagwolke
Links stehen die Jahre zuerst; darunter zeigt **Tags** alle vorhandenen Tags als umbrechende Wolke. Ein Klick filtert die Sammlung, erneutes Anklicken hebt den Tagfilter auf. Die Verwaltungsaktion liegt unter **Edit → Tags and Albums…**. „Mediathek“, „Alle Erinnerungen“ und der frühere Verwaltungseinstieg sind aus der Seitenleiste entfernt.

Fotokacheln zeigen **DD.MM.YYYY** und darunter **HH:MM:SS**. Vollständige Aufnahmedateinamen liefern die Angaben sofort; bei anderen Namen werden die Metadaten eines sichtbaren Eintrags im Hintergrund ergänzt. Dateiname und Datumsquelle bleiben im Tooltip. Ohne belegte Aufnahmezeit steht dort „Datum unbekannt“. Die Originale bleiben unverändert.

Die kompakte Audioleiste liegt direkt neben der Seitenüberschrift. **Window → Audio Player** öffnet den laufenden separaten Player; **View → Play Audio for Current View** startet den Audiobereich der aktuellen Ansicht. Die Fußzeile zeigt links **Medien / Photos / Audios / Videos**, danach Modus, Filter, Status und Fehler. Die Zahlen beziehen sich während des Bibliotheksscans auf bereits geladene Medien; bei knapper Breite werden große Zahlen verkürzt, genaue Werte stehen im Tooltip.

### Tags direkt auf Vorschauen
Fotos und Videos zeigen zugeordnete Tags als kompakte dunkle Badges unten im Bild. Die Videokennung verwendet denselben Stil mit gleichmäßigem knappen Textrand. Badges umbrechen innerhalb der Kachel; bei vielen Tags zeigt +N weitere Zuordnungen an, alle Namen stehen im Bild-Tooltip. Bildklicks, Kontextaktionen und Datumsbeschriftungen bleiben erhalten.

### Audiokommentar zu einem Foto
Rechtsklick auf ein Foto → **Audiokommentar …**, oder **Aufnahme …** in der Großansicht. Mikrofon wählen, **Aufnehmen**, optional pausieren, **Aufnahme beenden**, probehören und speichern. Das Mikrofon wird erst nach deinem Klick aktiviert. Laufende Audiowiedergabe pausiert beim Aufnahmestart.

Neue Kommentare liegen als M4A/AAC im Audioordner des tatsächlichen Aufnahmejahrs. Die einzelne YAML unter `Jahr/audio/y` verknüpft den Kommentar über bibliotheksrelative Pfade mit dem Foto und enthält den tatsächlichen Aufnahmestart mit Zeitzone. `photosuite-comments.yaml` ist ein rekonstruierbarer Startindex. Im Jahresplayer erscheinen sie zur Fotozeit, auch wenn das Foto viele Jahre älter ist. Fotos bleiben unverändert. Kommentare sind am Foto erneut anhörbar.

Erster Entwurf für macOS: maximal eine Stunde je Aufnahme, Gerätewahl nur für die Sitzung. Pegel/Audiokurve zeigen die Amplitudenhüllkurve. Bei Speicherfehlern bleiben veröffentlichte Audiodateien erhalten; Zuordnung kann erneut gespeichert werden. Nach einem Absturz sind temporäre oder noch unzugeordnete Aufnahmen möglich. Einzelne extern umbenannte Bilder werden nicht automatisch neu zugeordnet. Transkription ist noch nicht enthalten. Automatisierte Tests greifen nicht auf dein Mikrofon zu; eine Aufnahme mit realem Eingabegerät wurde nicht manuell geprüft. Screenshots folgen wie vereinbart später.

Die Audioaufnahme folgt denselben Architekturgrenzen wie die übrigen Anwendungsfälle: gekapselter Sitzungszustand, unabhängig testbare Veröffentlichungsregeln und getrennte Ports für Aufnahme, Medienoperationen und YAML-Verwaltung. UI und native Aufnahmeadapter entscheiden nicht über Kollisionsbenennung oder Fotozuordnungen.

Die Aufnahmesteuerung bleibt im Dialog fest sichtbar: roter Kreis startet bzw. setzt fort, zwei Balken pausieren, das Quadrat beendet die Aufnahme. Eingabegeräte werden beim Öffnen der Auswahl aktualisiert. Gelb (−18 dBFS) bis Rot (−6 dBFS) kennzeichnet den empfohlenen Orientierungsbereich; oberhalb Rot verbleiben 6 dB Reserve bis 0 dBFS. Die App regelt die Mikrofonverstärkung nicht automatisch.

Der Pegelverlauf läuft langsam über ungefähr 32 Sekunden (ein Achtel der ursprünglichen Geschwindigkeit); der Live-Pegel reagiert weiterhin schnell. Alle 30 Messungen je Sekunde bleiben im 960-Punkte-Verlauf erhalten; beim Anpassen an schmale Displays bleiben kurze Ausschläge durch Maximumsbildung sichtbar.

Der Aufnahmedialog besitzt eine Titelzeile mit X. Aufnahmebuttons, Status und rechtsbündige Mikrofonauswahl teilen eine feste Zeile. Die langsam laufende Kurve zeigt feine, auf die Displayauflösung abgestimmte Striche, dezente Pegelgrenzen und eine vertikale Mittellinie.

Beim Schließen einer ungespeicherten Aufnahme bietet der Dialog **Speichern** oder **Verwerfen**. Speichern beendet bei Bedarf zunächst die Aufnahme; erst nach erfolgreichem Speichern schließt der Dialog.

Unten rechts im Aufnahmedialog: X zum Abbrechen mit Sicherheitsabfrage, Häkchen zum Abschließen/Speichern. Die Mikrofonauswahl ist rechtsbündig und gleich hoch wie die Aufnahmebuttons. Die dichte Kurve färbt Ausschläge unter der gelben Grenze grün, bis zur roten Grenze gelb und darüber rot.

### Vorhören und ab einer Markierung neu aufnehmen
Links neben Aufnahme steht jetzt Play. Pausieren, dann in der Kurve klicken oder die helle Markierung ziehen: Play spielt ab dort; der rote Aufnahmekreis erhält den bisherigen Anfang und ersetzt alles ab dieser Stelle durch die neue Aufnahme. Ohne Verschieben wird am Ende weiter aufgenommen. Bei Pause bleibt der zuvor sichtbare 32-Sekunden-Ausschnitt unverändert stehen; er wird nicht auf die gesamte Breite gestreckt. Nur die aktuelle ungespeicherte Aufnahme wird bearbeitet; gespeicherte Dateien bleiben unverändert. AAC-Abschnitte werden bei Bedarf neu kodiert; Abschluss kann kurz dauern.

Aufnahme und Pause blenden keine kurzzeitigen Vorbereitungstexte mehr ein; Fehlermeldungen bleiben sichtbar.

### Foto-Kommentarablage
```text
2008/09/img_20080916_035811.jpg
2008/audio/y/09/img_20080916_035811.jpg.yaml
2026/audio/c/com_20261113_143000.m4a
2027/audio/c/com_20270102_100000.m4a
```
Ein Foto-YAML (Version 2) enthält alle zugehörigen Kommentare als GUID → Audiopfad. Aufnahmedatum und Uhrzeit stehen ausschließlich im Audiodateinamen. Kommentare bleiben in der Baumauswahl und Jahreswiedergabe im tatsächlichen Aufnahmejahr; es gibt kein Umsortieren zum Fotojahr.

`photosuite-comments.yaml` ist der ableitbare Startindex (Version 3, nach Fotos gruppiert). Speichern aktualisiert erst das Foto-YAML revisionsgeprüft, dann den Index. **File → Refresh** und der Neuaufbau unter Settings lesen ausschließlich die neuen Foto-YAMLs. Keine Kompatibilität/Migration für die bisherigen audio-benannten YAMLs oder alten Zuordnungen. Veraltete Index-Versionen werden automatisch als Cache verworfen und aus den neuen Quellen rekonstruiert; ohne neue Quellen bleibt der neue Index leer. Rohe alte Indexbytes werden vor Austausch gesichert. Defekte neue Quellen oder Abbruch lassen den bestehenden Index erhalten. Medien bleiben unverändert.

Die Hörleiste folgt der Auswahl im Jahresbaum. Jahr, Monat und **Audio** zeigen Tagebuch und Kommentare gemeinsam in zeitlicher Reihenfolge. Nur **Tagebuch** bzw. **Kommentare** filtern die Leiste. Kommentare stehen bei ihrem eigenen Aufnahmedatum und ihrer Aufnahmezeit. Laufendes Audio bleibt bei Navigation erhalten; Play in einem anderen Audiobereich startet die neue Auswahl.

Rechts an jeder Hörleiste öffnet ein Fenster-Symbol den passenden Player. **Jahr/Audio** spielt das gesamte Jahr, **Monat** ausschließlich den gewählten Monat; **Tagebuch/Kommentare** begrenzt auf die gewählte Kategorie. Kommentare zählen zum Jahr und Monat ihres verknüpften Fotos. Ein bereits passender laufender Player wird ohne Neustart und Positionsverlust eingeblendet.

Der Bereichsplayer hat zentrierte Symbolbuttons für 15 Sekunden zurück, Play/Pause, Kommentar aufnehmen und 15 Sekunden vorwärts. Aufnahme öffnet den Dialog für das mittlere hervorgehobene Foto; die Wiedergabe pausiert dabei. Das Mikrofon startet erst nach Aufnehmen im Dialog.

Die Symbolbuttons im Bereichsplayer bleiben vollständig im sichtbaren Fensterbereich; der vertikale Layoutfehler der vorherigen Fassung ist korrigiert.

Die Wiedergabesteuerung im Jahresverlauf nutzt kompakte quadratische Buttons wie der Aufnahmedialog: helle Symbole und einen roten Aufnahmekreis.

Aufnahme- und Wiedergabebuttons teilen zentrale Theme-Farben einschließlich Rahmen und Interaktionszuständen. Audioleisten ohne Aufnahmen sind deutlich ausgegraut. **File → Refresh** prüft zusätzlich den Kommentarindex anhand der Foto-YAMLs und gleicht anschließend die automatischen Tags ab; der vollständige Neuaufbau unter Settings tut dasselbe. Originalmedien bleiben unberührt.

### Architektur und Robustheit
Domäne und Anwendung sind getrennte Rust-Crates ohne GUI-, Dateisystem- oder native Abhängigkeiten im Kern. Kommentarabläufe liegen zentral in der Anwendung; YAML-Adapter übernehmen ausschließlich Datenzugriff. Hintergrundaufträge verwerfen veraltete Ergebnisse auch beim erneuten Laden desselben Ordners. Der Kommentar-Neuaufbau durchsucht nur Foto-Metadatenverzeichnisse und ersetzt den Index bei fehlerhaften oder verlinkten Quellen nicht. Die automatisierten Tests decken diese Fehlerfälle ohne Zugriff auf echte Benutzerfotos oder Mikrofone ab. Verbleibende Grenzen sind in ARCHITECTURE.md beschrieben.

Beim Wechsel auf eine Seite mit Hörleiste werden die Audiodauern sofort im Hintergrund geladen. Die Spur zeigt danach bereits vor Play die richtigen Größenverhältnisse. Gemessene Dauern werden für den Playerstart wiederverwendet; eine Seite zu öffnen startet kein Audio. Andere laufende Wiedergabe bleibt erhalten. Ein begrenzter Speicher-Cache hält die Dauerwerte bis zum Neuladen der Bibliothek.

Die Tagwolke bietet automatisch `commented` (Fotos mit Audiokommentar), `comment` (Kommentaraufnahmen) und `diary` (Tagebucheinträge). Aufnahme, Laden, Import und Aktualisieren gleichen diese Tags ab. Noch nicht eingelesene Jahre verlieren während eines laufenden oder fehlerhaften Scans keine vorhandenen Audiozuordnungen; andere Tags bleiben erhalten.

### Jahresplayer und Kategorien
Die obere Hörleiste ist der Audiozugang für alle Übersichten. Im Playerfenster filtern Tagebuch / Kommentare / Beide die sichtbare Zeitachse und Wiedergabe. Bleibt die aktuelle Aufnahme enthalten, werden Position und Wiedergabe erhalten. Filter verändern keine Dateien. Die vollständige Quellliste wird unabhängig weiter geprüft; die Kategoriezuordnung kommt aus der Domäne über den Plattformadapter. Die bildbezogene Kommentarleiste der Großansicht bleibt auf das Foto beschränkt.

### Bezugsdatum und Zeitraum des Players
Tagebuchaufnahmen verwenden ihr eigenes Datum. Kommentare verwenden Datum und Uhrzeit des verknüpften Fotos: Ein 2026 aufgenommener Kommentar zu einem Foto von September 1980 erscheint in 1980 beziehungsweise 1980/09. Audio und YAML werden dabei nicht verschoben oder verändert. Das tatsächliche Aufnahmedatum bleibt in der Dateiliste separat sichtbar.

Der geöffnete Player folgt der Navigation in der Fotowand. Sein Kopf zeigt Jahr oder Jahr/Monat; darüber hinaus zeigt er über der Zeitachse die laufende Bezugszeit. Kategorienfilter und Pausenzustand bleiben beim Zeitraumwechsel erhalten. Unter den Transportbuttons folgen Trennlinie und chronologische Tabelle: Dateiname, Bezugsdatum, Aufnahmedatum. Sie verwendet dieselbe gefilterte Reihenfolge wie Zeitachse und Wiedergabe; Doppelklick springt zum Dateianfang. Leere Bereiche bleiben klar erkennbar und deaktiviert.

### Bibliotheksweite Audiowiedergabe unter Alles
„Alles“ verwendet einen expliziten bibliotheksweiten Bereich (`AudioViewScope` ohne Jahresgrenze, `DiaryTimeline::for_library`). Keine Auswahl eines Ersatzjahres. Tagebuchaufnahmen werden nach ihrem eigenen Datum, zugeordnete Kommentare nach Foto-Bezugsdatum einsortiert. Hörleiste, separates Fenster und Dateiliste teilen dieselbe Auswahl; Tagebuch/Kommentare/Beide bleiben als Kategorienfilter verfügbar. Der Playerkopf zeigt „Alles“. Jahres-/Monatsfilter bleiben davon getrennt. Während der Bibliotheksscan läuft, ergänzen neu geladene Aufnahmen den Bereich; noch nicht eingelesene Dateien werden nicht als bereits verfügbar ausgegeben. Bestehende Medien bleiben unverändert.
