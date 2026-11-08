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
| **Fotowand** | Virtualisiertes Raster mit fünf Größenstufen, gleichmäßig gefüllten Zeilen und quadratischem Beschnitt oder vollständigem Seitenverhältnis. |
| **In der Sammlung navigieren** | Aufklappbare Jahre, Monatsauswahl, Dateinamens-/Pfadsuche und Videofilter. |
| **Großansicht** | Zentrierte Fotos, Pfeiltastennavigation und zwei feste Kontrollzeilen oberhalb des Bildes. |
| **Videos ansehen** | Native Vorschauen, sechs zeitlich verteilte Standbilder und Wiedergabe über AVFoundation/AVKit. Kein externes FFmpeg erforderlich. |
| **Audio-Diary** | Pro Jahr ein zoomabhängiges Kachelraster mit Datum/Uhrzeit und eine durchgehende Hörleiste mit proportionalen Dateiabschnitten, globalem Schieber und nativer Audiowiedergabe. Keine Playlist-Ansicht. |
| **Ausrichtung korrigieren** | EXIF inklusive Spiegelungen; manuelles Drehen zunächst nur für die Sitzung. Optional JPEG-EXIF ohne Neukompression speichern. |
| **Kontrolliert importieren** | Prüflauf mit einem kompakten Ergebnis pro Datei, zuschaltbaren Arbeitsschritten, metadatenbasierten Zielnamen und Kollisionsprüfung. |
| **Kollisionsduplikate prüfen** | Vollständiger Pixelvergleich geeigneter Bilder, jeweils eine Gruppe mit ausdrücklicher Entscheidung. |
| **Einstellungen behalten** | Anzeige, Filter, Tastenkürzel, Schreibmodus und letzter Bibliothekspfad in macOS-Preferences. |

## Das neue Galeriedesign

Dunkle Graphitflächen, helle kontrastreiche Texte und kompakte Abstände lassen viel Platz für Fotos. Das Galeriedesign bleibt auch bei hellem macOS-Systemdesign dunkel und verwendet dazu passende helle Textfarben. Eine schmale Jahres-/Monatsnavigation, klare Schriftgrößen und dezente blaue Auswahlzustände strukturieren die Sammlung. Jahre sind zunächst eingeklappt; ein Klick zeigt ihre Monate. Beim Öffnen wird zuerst nur die oberste Verzeichnisebene gelesen, sodass die Jahre früh verfügbar sind. Inhalte folgen jahresweise im Hintergrund; das Aufklappen eines Jahres wählt dessen Gesamtansicht und lädt es bevorzugt. Bereits geladene Jahre bleiben bedienbar. Einzelne laufende System-/Codecaufrufe können den Wechsel noch kurz verzögern. In der Großansicht stehen Navigation und Medienaktionen in zwei festen Zeilen; Fotos bleiben horizontal zentriert. Einstellungen und Import verwenden einheitliche Optionsgruppen.

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

1. **„Ordner öffnen …“** wählen und eine Fotosammlung öffnen. Der aktive Pfad steht oben und im Fenstertitel.
2. Ein zunächst eingeklapptes Jahr öffnen, einen Monat auswählen oder über das Suchfeld filtern.
3. Ein Foto anklicken. **← / →** wechseln das Medium, **Escape** schließt die Großansicht. **Leertaste** startet bei Videos den nativen Player.
4. Unter **„Ansicht“** Vorschaugröße, quadratischen Beschnitt und Seitenleiste einstellen.
5. **„Einstellungen …“** beziehungsweise **⌘,** öffnet die Preferences. Beim nächsten Start wird die letzte Bibliothek automatisch wieder geöffnet, sofern diese Option aktiviert bleibt.

Die XML-Einstellungen liegen unter `~/Library/Preferences/de.photosuite.PhotoSuite.settings.plist`, getrennt vom macOS-eigenen App-Preferences-Domain. Vorhandene Einstellungen aus `de.photosuite.PhotoSuite.plist` werden beim ersten Start übernommen; die alte Datei bleibt erhalten. Das gemerkte Bibliotheksverzeichnis wird beim Start wieder geöffnet. macOS-Zusatzargumente, fehlgeschlagene Startpfade und Importe in andere Ziele ersetzen diese Auswahl nicht.

„Nur Lesen“ in der Fußzeile bedeutet: Das Betrachten und die Sitzungskorrekturen verändern deine Dateien nicht.

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
    └── diary/
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

Ein Jahr aufklappen und **„Diary“** auswählen. Der Eintrag ist auch ohne vorhandene Aufnahmen verfügbar; das physische Verzeichnis wird erst bei einem ausdrücklich gestarteten Import angelegt. Die Ansicht zeigt Audio-Kacheln mit einem stilisierten Audiovorschaubild, darunter ausschließlich Datum und Uhrzeit. Die fünf Zoomstufen steuern 10, 8, 6, 5 oder 4 Kacheln je Zeile, soweit die Fensterbreite reicht. Schmale Fenster reduzieren die Anzahl weiter. Datum und Reihenfolge erscheinen sofort aus gültigen Dateinamen. Nur Namen ohne vollständigen Zeitstempel benötigen eine Hintergrundabfrage. Die gründliche Prüfung im Player und beim Import verwendet weiterhin eingebettete Metadaten vor Dateinamen und dem gekennzeichneten Dateisystemdatum. Eine Aufnahme anklicken, um den Jahresplayer an ihrem Beginn zu öffnen, oder **„Jahresverlauf öffnen“** wählen.

Der native Player bietet **eine zusammenhängende Hörleiste**, keine Playlist. Beim Schnellstart kommen Reihenfolge und Aufnahmezeit zunächst aus den Dateinamen; die Abschnitte sind zunächst gleich breit, ganz ohne Größenabfragen. Mit ersten Prüfergebnissen erhalten ungeprüfte Abschnitte das mittlere Gewicht der bereits geprüften Clips; diese vorläufigen Gewichte werden nicht als echte Sekunden ausgegeben. **Grau bedeutet ungeprüft, das bisherige Blau bedeutet erfolgreich geprüft, Rot kennzeichnet einen Prüfungsfehler.** Ein kompakter Fortschritt und Segment-Tooltips erläutern den Stand. Sobald ein Clip geprüft ist, werden Datum, Reihenfolge und Breite korrigiert. Erst nach vollständiger Prüfung belegt jede Datei exakt den Anteil ihrer Dauer an der Gesamtdauer. Eine Aufnahme von 10 Sekunden neben einer von 30 Sekunden erhält entsprechend ein Viertel der Leiste. Es gibt keine stillen Kalenderlücken und keine erzeugte Sammeldatei. Der Schieber kann über alle Dateien hinweg springen; der Player zeigt ausschließlich das aktuelle Aufnahmedatum und die dazugehörige Uhrzeit. Diese Uhrzeit entspricht dem Beginn der aktuellen Aufnahme plus der Position innerhalb dieser Datei, einschließlich Datumswechsel um Mitternacht. Beim Überschreiten einer Dateigrenze wird auf die Aufnahmezeit der nächsten Datei gewechselt. Datumsquellen sind per Tooltip erreichbar. Tooltips erläutern die Segmente. Leertaste spielt/pausiert, Pfeiltasten springen fünf Sekunden, Buttons 15 Sekunden; Lautstärke ist einstellbar.

Die Wiedergabe enthält alle Audiodateien im `diary` dieses Jahres, unabhängig vom Such- oder Videofilter. Reihenfolge: ausgewähltes Datumsfeld, bei Gleichstand Dateipfad. Vor dem Öffnen werden ausschließlich die bereits bekannten Dateinamen ausgewertet; es gibt weder Dateigrößenabfragen noch Metadaten-/Daueranalysen der gesamten Liste. Zunächst lädt nur die gewählte Aufnahme. Erst wenn sie tatsächlich läuft, prüft ein Hintergrundauftrag die Dateien einzeln, beginnend mit der gewählten Aufnahme. Abschnittsbreiten und Reihenfolge dürfen sich dabei ändern; die laufende Datei und ihre lokale Hörposition bleiben erhalten, ohne sie neu zu laden. Unlesbare Aufnahmen bleiben mit sichtbarem Fehler in der Leiste. Es werden keine Audiodateien verschoben, umbenannt oder verändert. Fehlt ein gültiger Dateizeitstempel, steht vorläufig „Datum wird geprüft“; später gilt die übliche Metadatenpriorität.

Unter der Hörleiste erscheinen **fünf zeitlich passende Fotos** aus der geöffneten Bibliothek. Das größere mittlere Foto liegt am nächsten an der aktuellen Aufnahmezeit; links folgen seine zwei vorherigen, rechts seine zwei nächsten zeitlichen Nachbarn. Fehlende Nachbarn bleiben als leere Plätze sichtbar. Die Fotoleiste füllt gemeinsam die Breite der Audiospur. Die Bilder werden zentriert und unverzerrt beschnitten, sodass jede Kachel vollständig ausgefüllt ist. Unter jedem Foto stehen **sein Aufnahmedatum und seine Uhrzeit**; Datumsquelle und Pfad bleiben im Tooltip zugänglich. Ein Klick navigiert **im Hintergrund zur Jahresübersicht des Fotos**, scrollt zum Bild und markiert es mit einem blauen Rahmen. Ein Bildbetrachter wird dabei nicht geöffnet; der Audioplayer behält den Fokus und spielt weiter. Such- und Videofilter werden zurückgesetzt, damit das Foto sichtbar ist. Der zugehörige Jahreseintrag wird aufgeklappt.

Die Fotosuche beginnt erst, wenn Audio tatsächlich wiedergegeben wird. Datumsindex und Vorschauen werden im Hintergrund geladen und innerhalb des geöffneten Players wiederverwendet. Eine neue Auswahl erfolgt beim Clipwechsel, nach bewusstem Springen oder einer korrigierten Aufnahmezeit, nicht jede Sekunde. Fotozeiten stammen zuerst aus vollständigen Dateinamen (insbesondere `img_JJJJMMTT_HHMMSS`); nur nicht benannte Fotos werden separat über EXIF ergänzt. Ohne auswertbares Datum wird das Foto übersprungen. Diese Anzeigeausnahme ändert die strengeren Foto-Importregeln nicht. Bei großen Bibliotheken kann die erste Suche dauern; Audio läuft bereits weiter. Die Kandidatenliste entspricht dem Bibliotheksstand beim Öffnen des Players; nach Änderungen den Player erneut öffnen. Vorschauen berücksichtigen EXIF-Ausrichtung, verwenden macOS ImageIO und werden ausschließlich im Speicher gehalten.

### Datum und Audio-Import

Audio wird ohne Umwandlung als **`Jahr/diary/aud_JJJJMMTT_HHMMSS.ext`** kopiert. Datumsquellen werden in dieser Reihenfolge geprüft:

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
