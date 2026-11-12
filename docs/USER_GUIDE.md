# PhotoSuite

Rust-Desktop-Prototyp mit egui/eframe, Fotowand, nativen Videovorschauen und nachvollziehbarem Import.

## macOS-Installation

Im Projektverzeichnis:

```sh
make install
```

Das baut eine optimierte Release-Version, bündelt die Ressourcen und installiert **`/Applications/PhotoSuite.app`**. Danach im Finder unter **Programme → PhotoSuite** starten oder ins Dock ziehen. Eine laufende installierte PhotoSuite muss vor dem Aktualisieren geschlossen werden. Falls die Berechtigung für `/Applications` fehlt, das unten beschriebene `.pkg` verwenden oder in den eigenen Programme-Ordner installieren:

```sh
python3 scripts/macos_app.py install --destination "$HOME/Applications"
```

Ein fertiges macOS-Installationspaket für Doppelklick erzeugen:

```sh
make pkg
```

Ergebnisse:

- `target/macos/PhotoSuite.app`: vollständiges, verschiebbares App-Bundle.
- `target/macos/PhotoSuite-0.1.0.pkg`: Installer mit Ziel `/Applications`.

Mit `make app` nur das App-Bundle bauen. Ein bereits gebautes Paket ohne erneuten Build installieren bzw. prüfen:

```sh
python3 scripts/macos_app.py install --app target/macos/PhotoSuite.app
python3 scripts/macos_app.py verify --app target/macos/PhotoSuite.app
```

`make uninstall` entfernt ausschließlich die erkannte PhotoSuite-App aus `/Applications`. Fotoquellen, Bibliotheken und Preferences bleiben erhalten.

### Paketinhalt und Voraussetzungen

```text
PhotoSuite.app/Contents/
  Info.plist
  MacOS/photosuite
  Resources/PhotoSuite.icns
  Resources/exiftool/exiftool
  Resources/exiftool/lib/
  Resources/Licenses/
  Resources/README.md
```

Die native Rust-Anwendung, ExifTool samt benötigten Perl-Modulen und Lizenzhinweise sind im Paket. Rust, Cargo, Python und Homebrew werden auf dem Zielrechner nicht zum Starten gebraucht. Verwendet werden macOS-Systemframeworks und das vorhandene `/usr/bin/perl`. Für die aktuelle lokale macOS-Installation ist dieser Interpreter geprüft. Das Paket enthält keine Homebrew-Dylib-Verweise und verlangt keinen Homebrew-PATH beim Finder-Start. Beschädigte oder fehlende gebündelte Helfer werden nicht still durch ein extern installiertes Programm ersetzt.

Zum **Bauen** benötigt werden Rust/Cargo, die Apple-Entwicklungswerkzeuge, Python 3, Make und eine vollständige ExifTool-Perl-Distribution (auf diesem Rechner bereits installiert). Python steuert nur Paketbau und Installation, gehört nicht zum Laufzeitpfad. Ohne lokale Cargo-Abhängigkeiten zuerst `cargo fetch --locked` ausführen; Paketbau selbst arbeitet offline. Das Paket wird für die Architektur des Bau-Rechners erstellt, kein Universal-Binary.

Die lokale App erhält eine Ad-hoc-Signatur; der Installer ist nicht Developer-ID-signiert/notarisiert. Für öffentliche Verteilung wären ein entsprechendes Apple-Zertifikat und Notarisierung separat einzurichten. Die Installation ersetzt nur Pakete mit der erwarteten App-Kennung; bei einem Fehler während des Austauschs wird das vorherige Paket zurückgestellt.

## Entwicklung und Terminalversion

```sh
cargo run --release
cargo run --release -- /Pfad/zur/Sammlung
cargo install --path .
```

`cargo install` installiert bewusst nur den Terminal-Befehl; für die Finder-App **`make install`** verwenden. Im Entwicklungs-/Terminalbetrieb wird ExifTool im PATH benötigt. In der App werden die gebündelten Ressourcen relativ zum ausführbaren Programm gefunden; Preferences bleiben macOS-üblich außerhalb des App-Pakets unter `~/Library/Preferences`.

Für Videofunktionen werden direkt verlinkte AVFoundation-/AVKit-Systemframeworks verwendet, kein FFmpeg-Programm. Auf anderen Betriebssystemen sind native Videovorschau/-wiedergabe derzeit nicht verfügbar. Codecunterstützung richtet sich nach macOS. Fotoimport liest EXIF mit kamadak-exif, chrono prüft Daten und tempfile ermöglicht sichere Kopien.

## Betrachten

Mit **File → Open Library…** einen Ordner öffnen, Jahre aufklappen und ganze Jahre oder vorhandene Monate auswählen. **Edit → Find… (⌘F)** filtert Pfade/Dateinamen, **View → Videos Only** filtert Videos. Die Fotowand rendert nur sichtbare Zeilen. Fünf Vorschaugrößen unter **View → Thumbnail Size** nutzen die gesamte Breite; die letzte Zeile behält dieselbe Kachelgröße. „Quadratische Vorschau“ schaltet zwischen zentriertem Beschnitt und vollständigem Bild im Seitenverhältnis um. Die Jahresnavigation lässt sich ausblenden.

Ein Klick öffnet die Großansicht. Pfeiltasten wechseln Medien; Escape schließt sie. Videos zeigen ein Vorschaubild aus dem ersten verfügbaren Frame mit beachteter Track-Ausrichtung und maximal 640 × 640 Pixeln; Wiedergabe kann im eigenen nativen Player oder im Standardprogramm erfolgen. Unterstützte Bildformate: JPEG, PNG, WebP, TIFF, GIF, BMP. Erkannte Videos: MP4, MOV, M4V, MKV, AVI, WebM; Erkennung garantiert keinen verfügbaren Decoder.

## Import Schritt für Schritt

1. **File → Import…** öffnen, Quelle und Zielbibliothek wählen. Die Zielbibliothek muss existieren. Quelle und Ziel dürfen sich nicht überschneiden.
2. **Prüflauf** zeigt ohne Schreibzugriffe die geplanten Dateinamen und Kollisionsentscheidungen.
3. **Import starten** prüft alles erneut und kopiert die Dateien. Im Ziel entstehen `Jahr/Monat/img_JJJJMMTT_HHMMSS.ext`, zum Beispiel `2008/09/img_20080916_035811.jpg`.

Der Import selbst verschiebt, benennt oder löscht keine Quellen; ein separat bestätigter Aufräumschritt kann geprüfte Quellen anschließend in den Papierkorb verschieben. EXIF `DateTimeOriginal` liefert das Fotodatum, unverändert in der dort gespeicherten Ortszeit; keine automatische Zeitzonenverschiebung. Videos verwenden `QuickTime:CreateDate`, ebenfalls ohne Zeitzonenänderung. Fehlende oder ungültige Aufnahmedaten führen zu einem sichtbaren Fehler für die Datei; es gibt keinen stillen Rückgriff auf Dateisystemdatum oder Dateinamen. Nicht unterstützte Medien werden nicht aufgenommen. Unlesbare Scan-Einträge stoppen den Lauf vor dem Kopieren.

Die Dateiendung bleibt dem Originalformat entsprechend erhalten (JPEG-Endung wird zu `.jpg`). Bei Fotokollisionen werden die Dateien bytegenau verglichen: identische Dateien werden übersprungen, unterschiedliche erhalten `_001`, `_002` usw. Auch die nummerierten Ziele werden auf Identität geprüft. Unterschiedliche Metadaten zählen als unterschiedlicher Dateiinhalt. Bei Videos genügt derselbe generierte Zielname als Duplikat, ohne Inhaltsvergleich.

Über „Alle Schritte anzeigen“ zeigt das Detailprotokoll Quelle, gelesene Metadaten, Zielpfade, Kollisionen, Kopierfortschritt in Bytes, Verifikation, endgültige Übernahme und Fehler. Es lässt sich in der gewählten kompakten oder ausführlichen Ansicht in die Zwischenablage kopieren; lange Zeilen zeigen Details per Mauszeiger. Dateifortschritt und Zusammenfassung bleiben sichtbar. **Abbrechen** beendet den Lauf; abgeschlossene Kopien bleiben erhalten. Temporäre Kopien werden bei regulärem Abbruch/Fehler entfernt. Kopien werden im Zielverzeichnis zunächst temporär geschrieben, synchronisiert und bytegenau geprüft, dann ohne Überschreiben vorhandener Dateien veröffentlicht. Parallel entstandene Kollisionen werden als Fehler gemeldet. Ein Prüflauf reserviert Namen nur im Arbeitsspeicher; tatsächliche Verzeichnisänderungen werden beim Import erneut berücksichtigt.

Nach dem Import wird die Zielbibliothek neu geladen. Der Import ist eine ausdrücklich gestartete Schreiboperation unabhängig vom EXIF-Schreibmodus. Das Importprotokoll gilt für die Sitzung; es gibt keinen dauerhaften Importkatalog. Einstellungen und Tastenkürzel werden dauerhaft gespeichert. Kein globales Rollback: bereits abgeschlossene Importe bleiben bei Fehlern erhalten.

## Bildausrichtung und Befehle

EXIF-Orientierung einschließlich Spiegelungen wird in beiden Ansichten automatisch berücksichtigt. Bearbeitungsbuttons und Tastenkürzel verwenden denselben Command-Dispatcher. Unter **Edit → Keyboard Shortcuts…** stehen Zuweisen und Entfernen zur Verfügung. Standard: Alt+L, Alt+R, Alt+Backspace. Kürzelkonflikte werden abgelehnt; während Texteingabe/Zuweisung oder laufendem Schreibjob werden keine Bearbeitungskürzel ausgeführt.

Standardmäßig gelten manuelle Drehungen nur für die Sitzung. **Einstellungen → Drehungen in JPEG speichern** schreibt neue manuelle Drehungen als EXIF-Orientierung ohne Pixelneukompression. Aktivieren allein oder bloßes Öffnen/Scannen ändert keine Dateien. ExifTool aktualisiert das Original ohne dauerhafte Backup-Datei (`-overwrite_original_in_place`), intern mit temporärer Arbeitsdatei. Vorhandene Backups werden nicht gelöscht. Andere Formate und symbolische Dateien werden abgelehnt. Bei Fehlern bleibt die Sitzungskorrektur erhalten; Erfolg invalidiert den Cache. „Zur EXIF-Ausrichtung“ verwirft Sitzungskorrekturen und macht gespeicherte Änderungen nicht rückgängig.

## Architektur

Cargo-Workspace mit durch den Compiler getrennten Schichten:

- `crates/domain` (`photosuite-domain`): unveränderliche Medienmodelle, validierte Aufnahmezeit, Importbenennung und Command-Regeln. Nur Standardbibliothek und chrono; keine Adapter oder UI.
- `crates/application` (`photosuite-application`): Lade-/Filter-/Import-Anwendungsfälle, `EditingSession`, strukturierte Importabschlüsse und technische Ports. Hängt nur von der Domänen-Crate ab.
- Desktop-Crate: `infrastructure`, `import_fs`, `video`, `native/video.m` implementieren Adapter; `presentation`, `import_ui`, `command_bindings`, `image_loader` übernehmen UI/Darstellung. `main` verdrahtet die konkreten Adapter einschließlich des Scan-Repositories für den Import.

Domäne und Anwendung verbieten unsicheren Rust-Code. Ein Rückgriff auf Desktop-Module ist aus diesen Crates nicht möglich. Buttonbeschriftungen liegen außerhalb der Domäne. Fachlogik wird zusätzlich mit In-Memory-Adaptern ohne GUI und Dateisystem getestet. Architekturentscheidungen und Grenzen: `ARCHITECTURE.md`.

HEIC/RAW, Zoom/Pan und ein persistenter Thumbnail-Cache fehlen weiterhin. Ein installierbares macOS-App-Bundle und ein .pkg-Installer sind vorhanden. Der Galeriecache ist auf 256 MiB RGBA-Daten und 1024 Einträge begrenzt; das vollständige Importprotokoll wächst mit dem Lauf; Bildladeaufträge verwenden zwei Worker mit einer Queue von höchstens 32 wartenden Aufträgen. Diese Betriebsgrenzen sind unabhängig von der Schichtentrennung.

## Prüfen

```sh
cargo test --workspace
cargo fmt --all --check
# Kern ohne Plattformadapter:
cargo test -p photosuite-domain -p photosuite-application
```

Integrationstests brauchen ExifTool, native Videotests auf macOS Zugriff auf System-Videodienste. Die kleine MP4-Testdatei liegt im Repository; FFmpeg war nur zu ihrer einmaligen Erzeugung nötig, nicht zum Testen oder zur Laufzeit. Tests prüfen u.a. Metadaten-/Datumsnamen, Kollisionen, Wiederholungen, unveränderte Quellen, Prüflauf ohne Schreibzugriff, Abbruch, Nichtüberschreiben und native MP4-Vorschauen. Projektregeln stehen in `AGENTS.md`.


## Einstellungen und macOS Preferences

**Einstellungen …** oder **⌘,** öffnet den gegliederten Optionsdialog. Darstellung, Startverhalten, Bildänderungen und Bedienung verwenden einheitliche Gruppen und Abstände. Ansicht und Filter bleiben als Schnellzugriff verfügbar; Tastenkürzel und Importdialog verwenden dasselbe Abstands- und Gruppenschema.

Alle dauerhaft unterstützten Einstellungen außer dem Bibliothekspfad werden direkt im Bibliotheks-Hauptverzeichnis gespeichert:

```yaml
# PhotoSuite – Einstellungen dieser Bibliothek
version: 1
preview_size: 2
square_previews: true
show_library: true
videos_only: false
write_exif: false
reopen_library: true
shortcuts: {}
```

Die Datei heißt **`photosuite.yaml`**. Vorschaugröße (0–4), Darstellung, Videofilter, EXIF-Schreibmodus, Startoption und Tastenkürzel gehören damit zur jeweiligen Sammlung. Angepasste Tastenkürzel werden als verschachtelte Key-Value-Einträge gespeichert; die App schreibt eine gut lesbare YAML-Datei. Manuelle Änderungen bei geschlossener App vornehmen: Kommentare/Formatierung können beim nächsten Speichern vereinheitlicht werden. Unbekannte Schlüssel oder Versionen, ungültige Typen sowie Dateien über 64 KiB werden gemeldet und nicht überschrieben. Die Datei enthält keinen absoluten Bibliothekspfad.

Nur die zuletzt gewählte Bibliothek bleibt lokal:

```text
~/Library/Preferences/de.photosuite.PhotoSuite.library.plist
```

Vorhandene `de.photosuite.PhotoSuite.settings.plist` bzw. `de.photosuite.PhotoSuite.plist` bleiben unverändert als Migrationsquellen erhalten. Fehlt `photosuite.yaml`, dienen die bisherigen Werte als Ausgangspunkt. Nach erfolgreichem Laden werden Änderungen im Hintergrund über eine synchronisierte temporäre Datei und Rename gespeichert, auch auf exFAT. Beim normalen Beenden werden ausstehende Schreibaufträge abgearbeitet. Ein Bibliothekswechsel erhält den letzten Stand der vorherigen Bibliothek und lädt neue Werte im Hintergrund; veraltete Ergebnisse werden nicht übernommen.

Die Startoption „Letzte Bibliothek wieder öffnen“ wird aus der YAML-Datei der zuletzt gemerkten Bibliothek gelesen. Ein beim Start angegebener Pfad hat Vorrang. Ist die Bibliothek nicht verfügbar, bleibt der lokal gespeicherte Pfad erhalten und ein Fehler wird angezeigt. Ohne geladene Bibliothek gelten neue Anzeigeänderungen nur für die Sitzung. Bei schreibgeschützter Bibliothek bleiben Änderungen ebenfalls vorläufig und ein Speicherfehler erscheint in Fußzeile und Optionsdialog. Die Anwendung legt kein nicht vorhandenes Bibliotheksverzeichnis an. Defekte Einstellungen werden erhalten; zum Reparieren die Datei bei geschlossener App korrigieren oder bewusst umbenennen.

Diese Speicherung ändert nur Verwaltungsdaten, keine Fotos, Videos oder Audioaufnahmen. Manuelle ungespeicherte Bilddrehungen bleiben Sitzungskorrekturen. Schnittstellen, Routing und Hintergrundaufträge liegen in der Anwendungsschicht; YAML, Plist, Dateizugriffe und atomischer Austausch bleiben im äußeren Adapter. Änderungen in derselben noch wartenden Bibliothek werden zusammengefasst; unterschiedliche Bibliotheken behalten getrennte ausstehende Snapshots. Zeitgleiche Änderungen durch andere App-Instanzen/Editoren sind nicht über eine globale Sperre koordiniert.

Die Suche ist als kompaktes Feld direkt in die obere Buttonzeile integriert. Auf schmalen Fenstern bricht die Zeile um. Der aktive EXIF-Schreibmodus wird kontrastreich mit weißem Text auf dunklem Hintergrund als „EXIF speichern: an“ gekennzeichnet.

## Videoübersicht mit sechs Standbildern

Die Großansicht von Videos zeigt sechs Standbilder mit Zeitangaben. Es wird jeweils die Mitte eines von sechs gleich langen Zeitabschnitten angefordert, wodurch der exakte Video-Endpunkt vermieden wird. Angezeigt wird die tatsächliche Zeit des dekodierten Frames. Bei sehr kurzen oder bildarmen Clips können mehrere Stichproben dasselbe Videobild ergeben.

Die Ansicht verwendet drei Spalten bei breiten, zwei bei mittleren und eine bei schmalen Fenstern. Die Bilder werden nacheinander im Hintergrund geladen und einzeln angezeigt; Track-Ausrichtung und Seitenverhältnis bleiben erhalten. Fehler einzelner Frames werden separat dargestellt; „Vorschau laden“ wiederholt den Versuch. Die Wiedergabe bleibt über „Im Standardprogramm öffnen“ erreichbar. Die Übersicht hält maximal sechs Texturen für das aktuelle Video; beim Wechsel wird der alte Auftrag verworfen. Eine bereits laufende native Frame-Dekodierung kann noch zu Ende laufen.

Videodauer und Frame-Dekodierung kommen weiterhin aus macOS AVFoundation, ohne externe FFmpeg-Abhängigkeit. Der Anwendungsfall und die zeitliche Auswahl sind unabhängig von UI und Plattformadapter getestet. Eine zusätzliche Testdatei mit wechselndem Bildinhalt prüft sechs unterschiedliche Frames und korrekte Zeitreihenfolge.


## Video mit Leertaste abspielen

Ein Video in der Großansicht auswählen und **Leertaste** oder **„Video abspielen“** drücken. Ein eigenes PhotoSuite-Playerfenster nutzt AVFoundation mit den nativen AVKit-Steuerelementen (Ton, Zeitposition und Vollbild). Es startet automatisch; **Leertaste** pausiert bzw. setzt fort. **Escape** schließt das Fenster oder verlässt zunächst Vollbild. Beim Schließen oder Ersetzen eines Videos werden Wiedergabe und Medienzugriff beendet. Der Sechs-Bilder-Überblick bleibt verfügbar.

„Video abspielen“ steht im zentralen Befehlsdialog; das Tastenkürzel ist änderbar und wird mit den Preferences gespeichert. Bestehende Preferences erhalten beim Laden die Leertaste als Vorgabe für den neuen Befehl. Texteingaben und Einstellungsdialoge unterdrücken die PhotoSuite-Wiedergabekürzel. Nicht von macOS unterstützte Dateien/Codecs führen zu einer sichtbaren Fehlermeldung im Player. „Im Standardprogramm öffnen“ bleibt als weitere Option erhalten. Andere Plattformen melden, dass die native Wiedergabe nur unter macOS verfügbar ist. Kein externes FFmpeg erforderlich.

Der native automatisierte Wiedergabetest öffnet kurz ein Fenster und prüft tatsächlich fortschreitende Wiedergabe, Pause/Fortsetzen und Schließen inklusive Freigabe. Er wird zusätzlich zu `cargo test --workspace` bei Bedarf separat ausgeführt:

```sh
clang -fobjc-arc -Wno-deprecated-declarations tests/native_playback.m -framework AVFoundation -framework AVKit -framework AppKit -framework Foundation -framework CoreMedia -framework CoreGraphics -o /tmp/photosuite-playback-test
/tmp/photosuite-playback-test "$PWD/tests/fixtures/storyboard.mp4"
```


## Geführte Duplikatsuche für Kollisionsnamen

Im Importdialog die **Zielbibliothek** wählen und **Duplikatsuche starten** drücken. Eine Importquelle ist dafür nicht erforderlich. Geprüft werden Foto-Dateigruppen im selben Ordner mit dem Basisnamen `img_JJJJMMTT_HHMMSS` und beliebigen Anhängen dahinter, z. B. `_001`, `_abc`, `XYZ` oder ` (1)`. Groß-/Kleinschreibung und verschiedene Bildendungen sind berücksichtigt. Eine Gruppe muss mindestens zwei Varianten und mindestens einen Anhang enthalten; auch Gruppen ohne suffixfreies Original werden berücksichtigt. Videos sind ausgeschlossen. Unbekannte Namensschemata werden nicht geraten und nicht gruppiert.

Es wird **eine Gruppe vollständig verglichen**, und zwar jedes Dateipaar, auch Varianten untereinander. Danach wartet die Anwendung auf eine Entscheidung. Pixelidentische Teilgruppen erhalten jeweils eine Auswahl „Diese Datei behalten“; verschiedene oder unklar vergleichbare Bilder bleiben eigenständig. Vorschaubilder, Dateinamen und alle Paarvergleiche stehen zur Kontrolle bereit.

- **Alle behalten und weiter:** unverändert zur nächsten Gruppe wechseln.
- **Duplikate in Papierkorb & weiter:** nur die bestätigten pixelidentischen Varianten der ausgewählten Keeper in den macOS-Papierkorb verschieben. Vor jedem Verschieben erfolgt eine neue vollständige Pixelprüfung sowie Prüfung auf Änderungen. Kein endgültiges Löschen. Metadaten können abweichen; erhalten bleiben die Metadaten der gewählten Datei.
- **Abbrechen:** keine weitere Gruppe bearbeiten; bereits abgeschlossene Papierkorb-Aktionen bleiben über macOS wiederherstellbar. Ein bereits gestarteter Betriebssystemaufruf lässt sich nicht rückgängig abbrechen.

Nach einer Entscheidung geht es automatisch zur nächsten Gruppe. Teilfehler werden im kopierbaren Entscheidungsprotokoll festgehalten; betroffene Dateien bleiben erhalten. Die Importsteuerung bleibt während einer laufenden Duplikatprüfung gesperrt.

Der Vergleich verwendet die vorhandene Rust-Bibliothek **image**, keine Hashentscheidung und keine verkleinerten Vorschaubilder. JPEG- und statische PNG-Dateien werden vollständig dekodiert, nach EXIF-Orientierung ausgerichtet und in RGBA mit 16 Bit pro Kanal pixelgenau verglichen. Unterschiede bei ICC-/EXIF-Farbräumen, Farbtiefe und PNG-Farbmetadaten führen konservativ zu „nicht sicher“. JPEG-Neukompression, veränderte Auflösung oder auch nur geringe Pixelunterschiede werden nicht als exakte Duplikate freigegeben. Es erfolgt kein unscharfer Ähnlichkeitsabgleich.

Mehrbildformate, animierte PNGs, JPEGs mit MPF-Kennung, andere Bildformate, defekte Dateien und Bilder über 32 Megapixel bzw. Dateien über 256 MiB bleiben ohne automatische Freigabe erhalten. Die Anzeige erklärt den Grund. Diese Grenzen vermeiden, dass nur ein Einzelbild oder eine reduzierte Darstellung zum Entfernen einer Originaldatei führt. Große Gruppen brauchen wegen des vollständigen Paarvergleichs entsprechend Zeit; dekodiert werden jeweils höchstens zwei Bilder gleichzeitig im Prüfauftrag.

Gruppierungsregeln liegen in der Domäne, Vergleichsablauf und Entscheidungen in der Anwendung. Pixeldecoder und macOS-Papierkorb sind injizierte Infrastrukturadapter; die Importseite ist ausschließlich für Darstellung und Benutzerauswahl zuständig.


## Architekturpflege

Die gesamte Anwendung ist in `ARCHITECTURE.md` nach Bereichen geprüft; `AGENTS.md` schreibt diese Pflege für künftige Erweiterungen verbindlich vor. Fachliche Workflows bleiben in unabhängig testbaren Anwendungssitzungen. Die Duplikatprüfung nutzt `ReviewSession`; die Desktop-Ansichten sind nach Toolbar, Bibliothek, Wand und Viewer gegliedert. Native Video-, Player- und Dateisystemadapter sind getrennt.

Bildaufträge laufen über einen begrenzten Pool (zwei Decoder, 32 wartende Aufträge); veraltete Ergebnisse dürfen nach Neuladen keinen neueren Cacheeintrag überschreiben. Zusätzlich begrenzen 256 MiB RGBA-Daten und 1024 Einträge den Galeriecache; alte Texturen werden freigegeben. Die Großansicht verwendet höchstens 4096 Pixel. Temporäre Originaldekodierungen und Grafiktreiberressourcen sind zusätzlich zu berücksichtigen. Die Architekturprüfung dokumentiert diese und weitere verbleibende Betriebsgrenzen ausdrücklich.


Paketierungsprüfungen: `python3 -m unittest discover -s scripts -p 'test_*.py'` prüft Installation ausschließlich in temporären Verzeichnissen. `verify` kontrolliert Plist, Signatur, dynamische Abhängigkeiten und den gebündelten ExifTool-Aufruf mit minimalem System-PATH. `PhotoSuite.app/Contents/MacOS/photosuite --check-installation` prüft die Hilfsprogrammauflösung ohne Öffnen einer Bibliothek oder Schreiben von Preferences.


### App-Icon

Das Original liegt unter `assets/app-icon/photosuite.png`, inklusive Transparenz. Jeder App-Build erzeugt daraus automatisch das vollständige macOS-Iconset von 16 bis 1024 Pixeln und bindet `PhotoSuite.icns` über die Info.plist ein. Das `.pkg` übernimmt dieses App-Paket einschließlich Icon. Design und Generierungs-Prompt sind unter `assets/app-icon/README.md` dokumentiert.

Die Großansicht verwendet eine einheitliche Werkzeugleiste oberhalb des Bildinhalts: Navigation, Positionsanzeige und externes Öffnen oben, Bild-/Videoaktionen in einer zweiten Zeile. Alle Buttons bleiben einzeilig und die Leiste behält ihre Höhe. Auch das Neuladen der Videoübersicht befindet sich oberhalb des Rasters; darunter gibt es keine Aktionsbuttons.

Die aktive Bibliothek steht mit ihrem Pfad im Fenstertitel. Es gibt keinen zusätzlichen Kopfbereich. Die Fußzeile zeigt Modus, Status und gegebenenfalls Speicherfehler in einer einzigen Zeile; gekürzte Meldungen sind ebenfalls per Tooltip lesbar. „Bereit“ bedeutet, dass die Anwendung auf eine Aktion wartet; „Nur Lesen“ kennzeichnet das Betrachten ohne Änderungen an Originaldateien. Expliziter Import und aktiviertes EXIF-Speichern bleiben die beschriebenen Ausnahmen.

In der Großansicht stehen Navigation und Bild-/Videoaktionen in zwei Zeilen mit konstanter Höhe. Kein horizontaler Scroller; schmale Fenster kürzen Beschriftungen mit vollständigem Tooltip. Fotos sind horizontal zentriert.

Die optionale Detailansicht des Importprotokolls stellt jeden Schritt als eigenen Absatz dar, erhält Zeilenumbrüche und bricht lange Pfade passend zur Fensterbreite um. Die Standardansicht zeigt ein kompaktes Ergebnis pro Datei.

## Audio-Diary

Audio wird im bestehenden Import mitkopiert: `Jahr/audio/aud_JJJJMMTT_HHMMSS.ext`. Beim Import bleiben Quellen unverändert; nur der separate bestätigte Aufräumschritt kann sie danach in den Papierkorb verschieben. Zum Anhören ein Jahr aufklappen, „Audio“ für alle Aufnahmen oder darunter „Tagebuch“ beziehungsweise „Fotokommentare“ wählen und eine Datei anklicken. Der Jahresplayer enthält alle Aufnahmen des gewählten Audiobereichs chronologisch auf einer einzigen Hörleiste. Diese startet ausschließlich mit Dateinamen und zunächst gleichen vorläufigen Gewichten; nach der Prüfung sind die Abschnitte proportional zur echten Dauer. Der Schieber navigiert auch über Dateigrenzen hinweg; es wird keine Sammeldatei exportiert und keine Playlist dargestellt.

Eingebettete Aufnahmezeit vor Inhalts-/Containerdatum; ohne diese Daten zuerst den vollständigen Zeitstempel im Dateinamen (z. B. `2023-12-27_22-51-32`, mit `-`, `_` oder ohne Trenner; einschließlich aud-Importnamen), zuletzt das Dateisystem-Erstellungsdatum als gekennzeichneten Fallback verwenden. Datum und Quelle im Prüflauf kontrollieren. Bei Fotos/Videos bleiben die strikten bisherigen Regeln unverändert. Audio-Kollisionen werden bytegenau verglichen. Formatliste, Einschränkungen und genaue Priorität stehen in der aktuellen Repository-README.

Das Importprotokoll ist standardmäßig kompakt: ein Ergebnis pro Datei, vollständige Pfade und Datumsquelle im Tooltip. „Alle Schritte anzeigen“ öffnet das detaillierte, mehrzeilige Protokoll. „Protokoll kopieren“ kopiert jeweils die gewählte Ansicht.

Diary zeigt jetzt Kacheln statt einer Dateinamenliste. Die gewählte Vorschaugröße steuert 10/8/6/5/4 Spalten auf ausreichend breiten Fenstern. Unter dem Audiosymbol stehen Datum und Uhrzeit; vollständiger Dateiname und Datumsquelle erscheinen im Tooltip. Der Player zeigt das laufende Aufnahmedatum und die Aufnahmeuhrzeit, einschließlich des Fortschritts innerhalb der aktuellen Datei. Vorhandene Preferences dienen als Migrationsquelle für `photosuite.yaml` in der jeweiligen Bibliothek; alte Preferences-Dateien bleiben erhalten. Importe in andere Ziele ändern die ausgewählte Fotobibliothek nicht mehr.

### Fotos zum Gehörten

Unter der Audioleiste erscheinen fünf Fotos aus der geöffneten Sammlung. Das größere mittlere liegt zeitlich am nächsten an der Aufnahmezeit beim Start, Clipwechsel oder letzten Sprung. Links stehen zwei frühere, rechts zwei spätere Nachbarn. Ohne Nachbar bleibt ein Platz leer. Die Auswahl folgt nicht jeder Sekunde. Nach einem Sprung wird kurz gewartet, damit Ziehen am Schieber keine Suchflut erzeugt.

Audio startet ohne vorgeschaltete Fotosuche. Erst während der Wiedergabe werden die Fotozeiten im Hintergrund gelesen und kleine Vorschauen nachgeladen. Die erste Suche kann bei großen Sammlungen dauern; danach wird der Index im geöffneten Player wiederverwendet. Fehlende oder defekte Fotos beeinträchtigen Audio nicht. Vollständige Foto-Dateinamen (insbesondere img-Importnamen) liefern zuerst den Zeitstempel, ohne die Bilddateien zu öffnen. Nur Namen ohne auswertbares Datum werden getrennt über EXIF ergänzt; das Dateisystemdatum wird dafür nie verwendet. Die Kacheln werden zentriert und unverzerrt beschnitten, damit keine zusätzlichen schwarzen Balken entstehen. Unter jedem Foto stehen sein Aufnahmedatum und seine Uhrzeit; Quelle und Pfad bleiben im Tooltip. Ein Klick auf ein geladenes Bild oder seine Beschriftung navigiert im Hintergrund zum gesamten Jahr dieses Fotos. Die Fotowand scrollt zum Bild und hebt es mit einem blauen Rahmen hervor; das Jahr wird in der Seitenleiste aufgeklappt. Such- und Videofilter werden dafür zurückgesetzt. Der Bildbetrachter bleibt geschlossen, der Audioplayer behält den Fokus und Audio läuft weiter. Bei einem Foto außerhalb der Jahresstruktur wird stattdessen die gesamte Sammlung angezeigt. Bibliotheksänderungen werden nach erneutem Öffnen des Players berücksichtigt. Es werden keine Fotos oder Cache-Dateien geschrieben.

### Schneller Start und Prüfzustände

Der Player öffnet ohne Größenabfragen aller Aufnahmen mit einer vorläufigen Leiste und lädt zuerst nur die gewählte Aufnahme. **Grau:** noch ungeprüft; Datum aus dem Dateinamen und zunächst gleiche Abschnittsbreiten; später erhalten ungeprüfte Abschnitte das mittlere Gewicht geprüfter Clips. **Blau:** Dauer und Datum geprüft, Breite nach Dauer korrigiert. **Rot:** Prüfung fehlgeschlagen; der Eintrag bleibt sichtbar und der Tooltip nennt den Fehler. Eine zusätzliche Fortschrittszeile benennt diese Zustände.

Erst während tatsächlicher Wiedergabe beginnt die gründliche Prüfung, Aufnahme für Aufnahme. Reihenfolge und Breiten dürfen sich ändern; die laufende Datei bleibt an ihrer lokalen Hörposition. Eingebettete Aufnahmezeiten haben bei der Prüfung wieder Vorrang vor Dateinamen. Bei korrigierter Aufnahmezeit werden auch die Kontextfotos aktualisiert. Abbrechen beendet die Prüfung, nicht die bereits laufende Wiedergabe; ungeprüfte Abschnitte bleiben vorläufig. Keine Datei wird verschoben oder umbenannt. Beim Dateiwechsel kann eine kurze Ladepause entstehen. Sekunden-Sprünge über mehrere noch ungeprüfte Clips sind zunächst nur näherungsweise möglich.

## Importquellen in den Papierkorb

1. Einen tatsächlichen Import abschließen oder abbrechen; abgeschlossene Dateiberichte bleiben verfügbar. Ein reiner Prüflauf genügt nicht.
2. **„Importquellen in den Papierkorb …“** anklicken. Der neue Dialog verwendet die gespeicherten Quell-/Zielpfade dieses Imports, auch wenn danach andere Ordner ausgewählt wurden.
3. Die Zeilen prüfen: „Freigegeben“ erfordert ein vorhandenes, vollständig byteidentisches Ziel. Fehlerhafte, nur geplante, fehlende oder abweichende Dateien bleiben erhalten. Lange Pfade sind im Tooltip vollständig lesbar.
4. **„… freigegebene Quellen in den Papierkorb“** ausdrücklich bestätigen oder abbrechen/schließen. Vor jedem Verschieben werden die Inhalte und die bei der Freigabe erfassten Dateistände erneut geprüft. Änderungen seit der Freigabe sperren die Quelle.
5. Ergebnisse kontrollieren. „Im Papierkorb“ bestätigt die Aktion; „Nicht verschoben“ nennt den Grund. Ein unklarer Status erfordert erneutes Prüfen von Quelle und Papierkorb. Bereits verschobene Quellen bleiben bei Abbruch im Papierkorb; keine Zieldatei wird gelöscht.

Es gibt keine automatische Quellenlöschung, keine endgültige Löschung als Ersatz bei Papierkorbfehlern und kein Entfernen ganzer Quellordner. Die Funktion ist auf den aktuellen Importlauf beschränkt; nach einem neuen Lauf bzw. Neustart gibt es noch kein dauerhaftes Aufräumarchiv.


### Import auf exFAT und eingeschränkten Dateisystemen
Importkopien werden zuerst als eindeutig benannte `.tmp`-Dateien im Zielordner geschrieben, synchronisiert und bytegenau geprüft. Unterstützt das Laufwerk die Veröffentlichung ohne Überschreiben nicht (z. B. exFAT), verwendet PhotoSuite automatisch eine kompatible Strategie: endgültigen Zielnamen exklusiv neu anlegen, geprüfte Daten hineinkopieren, synchronisieren und erneut vollständig mit der Quelle vergleichen. Vorhandene Dateien und symbolische Links werden nicht überschrieben; die Quelle bleibt unverändert. Das gilt für Fotos, Videos und Audio.

Der Ersatzweg benötigt zusätzliche Lese-/Schreibzeit und vorübergehend Platz für beide Kopien im Ziel. Bei regulärem Fehler oder Abbruch werden nur eigene unvollständige Dateien entfernt. Die endgültige Datei ist während dieses Ersatzwegs bereits sichtbar; bei Stromausfall oder erzwungenem Prozessende kann eine unvollständige Kopie zurückbleiben. Sie wird nicht als erfolgreicher Import protokolliert. Ein erneuter Import vergleicht Foto-/Audiokollisionen bytegenau; bei Videos gilt weiterhin die dokumentierte Namenskollisionsregel. Es gibt keine automatische Entfernung solcher Überreste.


### Dateinamen zuerst, Details später
Jahres-/Monatsansichten verwenden zuerst die vorhandene Ordnerstruktur und Endungen; die Diary-Kacheln zeigen sofort Datum und Uhrzeit aus vollständigen Dateinamen. Nur nicht auswertbare Namen werden im Hintergrund geprüft. MP4/MOV im Diary erscheinen zunächst als Audiokandidaten; die tatsächlichen Tracks werden später geprüft. Der Import selbst bleibt gründlich und prüft die tatsächlichen Medien und Metadaten.

Der Audioplayer startet aus den bereits bekannten Namen, ohne für jede Aufnahme die Dateigröße abzufragen. Seine Startphase und die erste Clipprüfung haben Vorrang vor zusätzlichen Galerie-Scans und Rastervorschauen. Sobald die gewählte Aufnahme spielt und geprüft wurde, laufen diese Hintergrundaufträge weiter. Genauere Audiodaten und Breiten folgen anschließend; die Aufnahme läuft dabei an ihrer lokalen Position weiter.

Auch die fünf zugehörigen Fotos werden zuerst nur anhand ihrer Dateinamen gefunden. Deren Dateinamen-Zeitstempel sind im Kontextindex maßgeblich; widersprechende EXIF-Daten benannter Fotos werden dafür nicht zusätzlich eingelesen. Nur die ausgewählten Vorschaubilder werden dekodiert. Fotos ohne gültiges Namensdatum werden separat über Metadaten ergänzt und bremsen keine spätere Clipauswahl. Ohne passende Dateinamen ist eine erste Metadatensuche weiterhin nötig. Als Grundlage dienen die beim Playerstart bereits geladenen Fotos der Sammlung; für später hinzugekommene Fotos den Player erneut öffnen.


## Tags und Alben

1. Bibliothek öffnen und links „Tags & Alben …“ wählen. Namen eingeben und „Tag anlegen“ anklicken.
2. In Fotowand/Diary eine Datei rechts anklicken und „Tags bearbeiten …“ wählen; in der Großansicht alternativ „Tags …“. Häkchen setzen oder entfernen. Ein neu angelegter Tag wird in dieser Dateiansicht direkt der Datei zugewiesen.
3. „Album“ bei einem Tag aktivieren: Die linke Navigation zeigt ihn als virtuelles Album. „Anzeigen“ filtert auch Tags, die nicht als Album markiert sind. Eine Datei darf mehreren Tags angehören.
4. Zum Umbenennen oder Entfernen „Alle Tags verwalten“ wählen, einen Tagnamen auswählen und die passende Aktion benutzen. Tagentfernen entfernt ausschließlich die Zuordnungen und gegebenenfalls die Albumkennzeichnung.
5. „Alle Erinnerungen“, Jahres-/Monatsauswahl oder „Filter aufheben“ beendet den Tagfilter. Suche und Videofilter gelten auch im Album. Audio erscheint als Audiokachel und öffnet beim Anklicken den normalen Jahresplayer; die Bildnavigation überspringt Audio.

Alle Tags und Albumkennzeichnungen stehen in `photosuite-tags.yaml` direkt im Bibliotheksverzeichnis. Unter `tags` steht je Tag eine YAML-Liste relativer Dateipfade (zum Beispiel `2023/08/img_20230816_153000.jpg`). Unter `albums` stehen die als Album gekennzeichneten Tagnamen als Liste. Relative Pfade sind absichtlich eindeutiger als bloße Dateinamen; absolute Pfade und `..` sind unzulässig. Die Datei ist unabhängig von `photosuite.yaml` für Programmeinstellungen.

Der vollständige Katalog bleibt im Speicher, bis die Bibliothek gewechselt oder die Datei ausdrücklich neu geladen wird. Filterung benötigt keine Bild-/Metadatenzugriffe. Änderungen werden sofort im Speicher wirksam und anschließend seriell im Hintergrund gespeichert; wartende Zwischenstände werden jeweils durch den neuesten vollständigen Stand ersetzt. Auf Speichervorgänge achten: Schreibschutz oder externe YAML-Änderungen führen zu einem sichtbaren Fehler, die lokale Änderung bleibt vorläufig. Ein Bibliothekswechsel wird bis zur Speicherung oder bewusstem Verwerfen verhindert. „Erneut speichern“ versucht es erneut; bei einem echten externen Konflikt zunächst die Datei/Änderungen prüfen, dann gegebenenfalls lokale Änderungen verwerfen und neu laden. Bei Beenden trotz Speicherfehler gehen nur ungespeicherte Sitzungsänderungen verloren. Bereits veröffentlichte Daten bleiben erhalten.

Unbekannte/fehlerhafte Kataloge werden nicht durch leere ersetzt. Extern entfernte/umbenannte Medien bleiben in der Zuordnung stehen; sie werden nicht allein anhand eines gleichen Namens ersetzt. Noch nicht gescannte Albumjahre werden bevorzugt geladen. Die Sammlung wird für Alben nicht umgebaut; keine Foto-/Audio-/Video-Datei wird kopiert, verschoben, umbenannt oder gelöscht. Der erste Entwurf unterstützt einzelne Dateizuordnungen; keine Mehrfachauswahl, intelligente Alben oder freie Albumreihenfolge. YAML-Dateien über 32 MiB werden erhalten und als zu groß gemeldet. Manuelle Kommentare/Formatierung können beim Speichern vereinheitlicht werden.


### Ressourcen und Programmstart
Die Startkonfiguration wird im Hintergrund geladen; die Oberfläche bleibt währenddessen bedienbar. Anzeige-/Filtereinstellungen werden erst nach abgeschlossener Konfigurationsabfrage freigegeben, damit verspätete Werte keine Benutzereingaben verdrängen. Ein inzwischen manuell geöffneter Ordner wird nicht durch das alte Startresultat ersetzt.

Sehr große Kollisionsgruppen werden vor dem Vergleichen mit einer Fehlermeldung abgewiesen, wenn mehr als 100.000 Paarvergleiche nötig wären. In diesem Fall wird keine Datei entfernt. Die Grenze schützt Laufzeit/Speicher; sie ersetzt keinen weniger genauen Vergleich. Native Wiedergabetests laufen reproduzierbar über `make test-native` mit mitgelieferten/stummen Testmedien; Video öffnet dafür kurz ein eigenes Testfenster.

### Kompakte Audioleiste
Jahres- und Monatsansichten zeigen eine kompakte Jahres-Audioleiste: Play/Pause und anklickbare/ziehbare Zeitachse direkt neben der Seitenüberschrift. Der Start bevorzugt eine Aufnahme des ausgewählten Monats. Graue Abschnitte werden im Hintergrund geprüft und anschließend proportional zur echten Dauer blau angezeigt. Während einer laufenden Vorbereitung kann keine weitere Jahresvorbereitung gestartet werden.

**Window → Audio Player** zeigt denselben laufenden Player ohne Neustart. Das Schließen dieses Fensters lässt die Wiedergabe weiterlaufen. ▶ startet die Wiedergabe und wechselt dabei zu ⏸ zum Pausieren; ein separater Stop-Button entfällt. Jahres-/Monatswechsel unterbrechen die Wiedergabe nicht, ein Bibliothekswechsel stoppt sie. Die Anwendung hält den Zeitachsenentwurf und die Auswahl im Anwendungskern; die Oberfläche steuert denselben nativen Transport über einen frameworkunabhängigen Port.

## Native Menüleiste
PhotoSuite verwendet ausschließlich native muda-Menüs: **PhotoSuite**, **File**, **Edit**, **View**, **Window**, **Help**. Auf macOS stehen sie am oberen Bildschirmrand. Ein zusätzlicher Kopfbereich entfällt. Edit → Find… (⌘F) öffnet den Suchdialog; View → Thumbnail Size wählt die Vorschaugröße.

- **File:** Open Library… (⌘O), Refresh Library (⌘R), Import… (⇧⌘I), Close Window.
- **PhotoSuite:** About PhotoSuite, Settings… (⌘,), Services, Hide und Quit PhotoSuite (⌘Q).
- **Edit:** Undo/Redo und Cut/Copy/Paste/Select All für fokussierte Texteingaben; Rotate Left/Right, Reset to EXIF Orientation und Play Video für die geöffnete Großansicht; Tags and Albums…, Keyboard Shortcuts… und Find… (⌘F).
- **View:** Show Library Sidebar, Square Thumbnails, Videos Only und Thumbnail Size mit fünf Stufen. Häkchen folgen dem aktuellen Zustand.
- **Window:** Minimize, Zoom, Enter Full Screen, Bring All to Front.
- **Help:** PhotoSuite Help mit integrierter Kurzanleitung.

Die Bildbefehle sind während Texteingabe, Dialogen und EXIF-Schreibaufträgen deaktiviert. Native Menü-Kürzel und Buttons führen dieselben Bildbefehle aus. Anwendungs- und Textkürzel sind dafür reserviert; Konflikte werden gemeldet. Bereits gespeicherte konfliktbehaftete Kürzel werden nicht still geändert; bei Ladefehler bleiben die gültigen Standardbindungen aktiv.

### Suche ohne dauerhafte Suchzeile
**Edit → Find… (⌘F)** öffnet ein separates Suchfenster. Eingaben filtern die Fotowand direkt. Schließen oder Escape erhält den Filter; „Filter zurücksetzen“ entfernt ihn. Ein aktiver Such-/Videofilter bleibt in der einzeiligen Fußzeile sichtbar. Der Bibliothekspfad steht nur im Fenstertitel, EXIF-Schreiben unter **PhotoSuite → Settings…**, die Vorschaugröße unter **View → Thumbnail Size**.

### Ein Ordner, drei Audioansichten
Alle neuen Audioimporte werden als Tagebuch unter `Jahr/audio/aud_….ext` gespeichert. Fotokommentare mit `comment_….ext` können im selben Ordner liegen; die Aufnahmefunktion kommt später. Die Kategorien unter Audio sind Filter und erzeugen keine Unterordner. Audio mischt beide Arten chronologisch; Tagebuch und Fotokommentare spielen nur ihren Bereich. Der Suchfilter begrenzt die sichtbaren Kacheln, nicht die Hörleiste des gewählten Bereichs. Unbekannte Namen erscheinen nur in der gemeinsamen Audioansicht.

Bestehende `Jahr/diary`-Ordner sowie bereits angelegte Audio-Kategorieordner werden mitgelesen und nicht verändert. Bei alten Namen ohne Präfix liefert deren bisheriger Kategorieordner die Zuordnung. Explizite Präfixe gehen vor. Alte Bestände werden nicht automatisch migriert oder mit neuen Importzielen dedupliziert. Bei `comment_` bleibt der gültige Zeitstempel im Namen als Fotozeit maßgeblich, während Dauer/Abspielbarkeit nachgeladen werden.

### Jahr und Audio aufklappen
Ein aufgeklapptes Jahr zeigt zuerst Audio und darunter direkt alle Monate auf derselben Ebene. Der Jahresklick wählt die Jahresfotowand. Audio öffnet die gemeinsame Jahres-Höransicht und klappt Tagebuch/Fotokommentare auf oder zu. Kontextfotos aus der Audiowiedergabe öffnen bei Bedarf das betreffende Jahr und markieren das Bild in der Fotowand. Keine zusätzliche Photos-Ebene.

### Tagwolke, Aufnahmedaten und Audioüberschrift
Die linke Spalte beginnt mit den Jahren. Unter ihnen zeigt Tags alle Tags als Wolke; kurze Namen teilen eine Zeile, längere werden umgebrochen beziehungsweise bei Platzmangel mit Tooltip gekürzt. Anklicken wählt einen Tag, erneutes Anklicken entfernt den Filter. Verwalten und Konfigurieren über Edit → Tags and Albums…; bestehende Kontextaktionen an einzelnen Medien bleiben verfügbar.

Unter Fotos stehen Datum und Uhrzeit statt des Dateinamens. Für korrekt benannte Dateien sofort, für andere nach kurzer Hintergrundprüfung. Ohne gültige Metadaten wird kein Datum geraten. Name, Pfad, Datumsquelle und Fehler finden sich im Tooltip. Neben der Datums-/Monats-/Jahresüberschrift steht nur noch Play/Pause und die Audioachse. Den separaten Player öffnet Window → Audio Player; einen anderen aktuellen Audiobereich startet View → Play Audio for Current View.

In der einzeiligen Fußzeile stehen zuerst geladene Medien gesamt, Photos, Audios und Videos; rechts folgen Schreibmodus, Filter, aktuelle Meldung und Fehler. Große Zahlen können auf schmalen Fenstern abgekürzt sein; der Tooltip zeigt die genauen Werte. Während der Sammlungssuche können die Zahlen noch wachsen.
