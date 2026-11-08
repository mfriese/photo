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

Dunkle Graphitflächen, helle kontrastreiche Texte und kompakte Abstände lassen viel Platz für Fotos. Das Galeriedesign bleibt auch bei hellem macOS-Systemdesign dunkel und verwendet dazu passende helle Textfarben. Eine schmale Jahres-/Monatsnavigation, klare Schriftgrößen und dezente blaue Auswahlzustände strukturieren die Sammlung. Jahre sind zunächst eingeklappt; ein Klick zeigt ihre Monate. In der Großansicht stehen Navigation und Medienaktionen in zwei festen Zeilen; Fotos bleiben horizontal zentriert. Einstellungen und Import verwenden einheitliche Optionsgruppen.

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

Ein Import wird ausdrücklich gestartet und **kopiert** ausschließlich ins gewählte Ziel. Die Quelle wird niemals gelöscht, verschoben oder umbenannt. Ein Prüflauf schreibt keine Medien. Das Protokoll zeigt standardmäßig einen Eintrag pro Datei mit Entscheidung und gekürztem Zielpfad. Tooltips zeigen vollständige Pfade, Datum und Datumsquelle. „Alle Schritte anzeigen“ blendet das ausführliche Protokoll ein; „Protokoll kopieren“ übernimmt jeweils die gewählte Ansicht. Bei Fotos/Videos werden fehlende Aufnahmedaten sichtbar gemeldet und nicht durch ein Dateisystemdatum ersetzt. Audio verwendet die unten beschriebene Priorität mit ausdrücklich gekennzeichneten Fallbacks. Bei Abbruch werden ausschließlich importereigene temporäre Zieldateien entfernt; fertige Kopien und alle Quellen bleiben erhalten.

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

## Audio-Diary: ein Jahresverlauf

Ein Jahr aufklappen und **„Diary“** auswählen. Der Eintrag ist auch ohne vorhandene Aufnahmen verfügbar; das physische Verzeichnis wird erst bei einem ausdrücklich gestarteten Import angelegt. Die Ansicht zeigt Audio-Kacheln mit einem stilisierten Audiovorschaubild, darunter ausschließlich Datum und Uhrzeit. Die fünf Zoomstufen steuern 10, 8, 6, 5 oder 4 Kacheln je Zeile, soweit die Fensterbreite reicht. Schmale Fenster reduzieren die Anzahl weiter. Datum und Reihenfolge werden im Hintergrund aus den eingebetteten Metadaten ermittelt; fehlen diese, folgt zuerst der Zeitstempel im Dateinamen und zuletzt das gekennzeichnete Dateisystemdatum. Nach Importen wird dieselbe Auswahl erneut angewandt, damit ein neues Kopierdatum den Importnamen nicht verdrängt. Eine Aufnahme anklicken, um den Jahresplayer an ihrem Beginn zu öffnen, oder **„Jahresverlauf öffnen“** wählen.

Der native Player bietet **eine zusammenhängende Hörleiste**, keine Playlist. Jede Datei belegt exakt den Anteil ihrer Dauer an der Gesamtdauer. Eine Aufnahme von 10 Sekunden neben einer von 30 Sekunden erhält entsprechend ein Viertel der Leiste. Es gibt keine stillen Kalenderlücken und keine erzeugte Sammeldatei. Der Schieber kann über alle Dateien hinweg springen; der Player zeigt ausschließlich das aktuelle Aufnahmedatum und die dazugehörige Uhrzeit. Diese Uhrzeit entspricht dem Beginn der aktuellen Aufnahme plus der Position innerhalb dieser Datei, einschließlich Datumswechsel um Mitternacht. Beim Überschreiten einer Dateigrenze wird auf die Aufnahmezeit der nächsten Datei gewechselt. Datumsquellen sind per Tooltip erreichbar. Tooltips erläutern die Segmente. Leertaste spielt/pausiert, Pfeiltasten springen fünf Sekunden, Buttons 15 Sekunden; Lautstärke ist einstellbar.

Die Wiedergabe enthält alle Audiodateien im `diary` dieses Jahres, unabhängig vom Such- oder Videofilter. Reihenfolge: ausgewähltes Datumsfeld, bei Gleichstand Dateipfad. Der gesamte Verlauf wird vor dem Öffnen im Hintergrund vorbereitet; kann eine Aufnahme nicht gelesen werden, wird der Fehler angezeigt und keine unvollständige Leiste stillschweigend abgespielt.

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
- Zwei Decoder und maximal 32 wartende Bildaufträge begrenzen die Arbeitsqueue. Bild-/GPU-Cache und Importprotokoll haben noch kein globales Speicherlimit; sehr große Sammlungen oder Protokolle können viel Speicher benötigen.
- Audio-Diary ist ein erster macOS-Entwurf: das Vorschaubild ist ein Symbol und keine berechnete Wellenform; keine manuelle Datumskorrektur und keine garantierte Unterstützung jedes Systemformats. Eine große Jahreskomposition kann beim Vorbereiten Zeit und Speicher benötigen; einzelne laufende Systemaufrufe sind nicht sofort abbrechbar. Die native Leiste verwendet angegebene Dauern, ohne Mindestbreiten für sehr kurze Segmente.
- Keine Bearbeitungs-Suite für Zuschnitt, Belichtung oder Farbkorrekturen. Nicht gespeicherte Drehungen sind Sitzungskorrekturen.
- Defekte Preferences werden sichtbar gemeldet und nicht überschrieben. Beim normalen Beenden werden ausstehende Einstellungen gespeichert; erzwungenes Beenden kann laufende Schreibvorgänge unterbrechen.

## Entwicklung und Tests

```sh
cargo run -- /Pfad/zur/Fotosammlung
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
make test-audio  # Native Jahreskomposition und Wiedergabe mit stummen Testdateien
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
