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

Ordner öffnen, Jahre aufklappen und ganze Jahre oder vorhandene Monate auswählen. Suche filtert Pfade/Dateinamen, Filter bietet Videos. Die Fotowand rendert nur sichtbare Zeilen. Fünf Vorschaugrößen unter **Ansicht** nutzen die gesamte Breite; die letzte Zeile behält dieselbe Kachelgröße. „Quadratische Vorschau“ schaltet zwischen zentriertem Beschnitt und vollständigem Bild im Seitenverhältnis um. Die Jahresnavigation lässt sich ausblenden.

Ein Klick öffnet die Großansicht. Pfeiltasten wechseln Medien; Escape schließt sie. Videos zeigen ein Vorschaubild aus dem ersten verfügbaren Frame mit beachteter Track-Ausrichtung und maximal 640 × 640 Pixeln; Wiedergabe kann im eigenen nativen Player oder im Standardprogramm erfolgen. Unterstützte Bildformate: JPEG, PNG, WebP, TIFF, GIF, BMP. Erkannte Videos: MP4, MOV, M4V, MKV, AVI, WebM; Erkennung garantiert keinen verfügbaren Decoder.

## Import Schritt für Schritt

1. **Import …** öffnen, Quelle und Zielbibliothek wählen. Die Zielbibliothek muss existieren. Quelle und Ziel dürfen sich nicht überschneiden.
2. **Prüflauf** zeigt ohne Schreibzugriffe die geplanten Dateinamen und Kollisionsentscheidungen.
3. **Import starten** prüft alles erneut und kopiert die Dateien. Im Ziel entstehen `Jahr/Monat/img_JJJJMMTT_HHMMSS.ext`, zum Beispiel `2008/09/img_20080916_035811.jpg`.

Der Import selbst verschiebt, benennt oder löscht keine Quellen; ein separat bestätigter Aufräumschritt kann geprüfte Quellen anschließend in den Papierkorb verschieben. EXIF `DateTimeOriginal` liefert das Fotodatum, unverändert in der dort gespeicherten Ortszeit; keine automatische Zeitzonenverschiebung. Videos verwenden `QuickTime:CreateDate`, ebenfalls ohne Zeitzonenänderung. Fehlende oder ungültige Aufnahmedaten führen zu einem sichtbaren Fehler für die Datei; es gibt keinen stillen Rückgriff auf Dateisystemdatum oder Dateinamen. Nicht unterstützte Medien werden nicht aufgenommen. Unlesbare Scan-Einträge stoppen den Lauf vor dem Kopieren.

Die Dateiendung bleibt dem Originalformat entsprechend erhalten (JPEG-Endung wird zu `.jpg`). Bei Fotokollisionen werden die Dateien bytegenau verglichen: identische Dateien werden übersprungen, unterschiedliche erhalten `_001`, `_002` usw. Auch die nummerierten Ziele werden auf Identität geprüft. Unterschiedliche Metadaten zählen als unterschiedlicher Dateiinhalt. Bei Videos genügt derselbe generierte Zielname als Duplikat, ohne Inhaltsvergleich.

Über „Alle Schritte anzeigen“ zeigt das Detailprotokoll Quelle, gelesene Metadaten, Zielpfade, Kollisionen, Kopierfortschritt in Bytes, Verifikation, endgültige Übernahme und Fehler. Es lässt sich in der gewählten kompakten oder ausführlichen Ansicht in die Zwischenablage kopieren; lange Zeilen zeigen Details per Mauszeiger. Dateifortschritt und Zusammenfassung bleiben sichtbar. **Abbrechen** beendet den Lauf; abgeschlossene Kopien bleiben erhalten. Temporäre Kopien werden bei regulärem Abbruch/Fehler entfernt. Kopien werden im Zielverzeichnis zunächst temporär geschrieben, synchronisiert und bytegenau geprüft, dann ohne Überschreiben vorhandener Dateien veröffentlicht. Parallel entstandene Kollisionen werden als Fehler gemeldet. Ein Prüflauf reserviert Namen nur im Arbeitsspeicher; tatsächliche Verzeichnisänderungen werden beim Import erneut berücksichtigt.

Nach dem Import wird die Zielbibliothek neu geladen. Der Import ist eine ausdrücklich gestartete Schreiboperation unabhängig vom EXIF-Schreibmodus. Das Importprotokoll gilt für die Sitzung; es gibt keinen dauerhaften Importkatalog. Einstellungen und Tastenkürzel werden dauerhaft gespeichert. Kein globales Rollback: bereits abgeschlossene Importe bleiben bei Fehlern erhalten.

## Bildausrichtung und Befehle

EXIF-Orientierung einschließlich Spiegelungen wird in beiden Ansichten automatisch berücksichtigt. Bearbeitungsbuttons und Tastenkürzel verwenden denselben Command-Dispatcher. Unter **Einstellungen → Befehle & Tastenkürzel** stehen Zuweisen und Entfernen zur Verfügung. Standard: Alt+L, Alt+R, Alt+Backspace. Kürzelkonflikte werden abgelehnt; während Texteingabe/Zuweisung oder laufendem Schreibjob werden keine Bearbeitungskürzel ausgeführt.

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

Einstellungen werden automatisch im Hintergrund als lesbare XML-Property-List gespeichert:

```text
~/Library/Preferences/de.photosuite.PhotoSuite.settings.plist
```

Die Key-Value-Datei enthält `preview_size` (0–4), `square_previews`, `show_library`, `videos_only`, `write_exif`, `reopen_library`, `last_library` und `shortcuts`. Die Tastenkürzel liegen als verschachtelte Key-Value-Einträge vor. Die Datei verwendet das macOS-Plist-Format, wird direkt durch die Anwendung verwaltet und benötigt keinen externen Dienst. Quellbibliotheken enthalten keine Konfigurationsdateien.

Die zuletzt geöffnete Bibliothek wird unmittelbar beim Öffnen gespeichert, unabhängig von Dauer oder Ergebnis des Scans, und beim nächsten Start automatisch geöffnet. Im Dialog lässt sich „Letzte Bibliothek wieder öffnen“ deaktivieren. Ein beim Start angegebener Bibliothekspfad hat Vorrang. Ist die Bibliothek nicht verfügbar, wird der Scanfehler angezeigt; die gemerkte Auswahl bleibt erhalten.

Auch der EXIF-Schreibmodus bleibt nach einem Neustart erhalten und ist weiterhin sichtbar gekennzeichnet. Manuelle Bilddrehungen ohne Speicherung bleiben dagegen reine Sitzungskorrekturen. Änderungen werden atomar über eine temporäre Preferences-Datei gespeichert; beim normalen Beenden werden ausstehende Änderungen abgearbeitet. Eine beschädigte oder unbekannte Dateiversion wird gemeldet und nicht automatisch überschrieben. In diesem Fall startet die Anwendung mit Standardwerten.

Die Preferences-Schnittstelle und Hintergrundspeicherung liegen in der Anwendungsschicht; ausschließlich der Desktop-Adapter kennt Plist und das Benutzerverzeichnis.

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

Die aktive Bibliothek steht mit ihrem Pfad direkt unter den Werkzeugaktionen und im Fenstertitel. Lange Pfade lassen sich per Tooltip vollständig lesen. Die Fußzeile zeigt Modus, Status und gegebenenfalls Speicherfehler in einer einzigen Zeile; gekürzte Meldungen sind ebenfalls per Tooltip lesbar. „Bereit“ bedeutet, dass die Anwendung auf eine Aktion wartet; „Nur Lesen“ kennzeichnet das Betrachten ohne Änderungen an Originaldateien. Expliziter Import und aktiviertes EXIF-Speichern bleiben die beschriebenen Ausnahmen.

In der Großansicht stehen Navigation und Bild-/Videoaktionen in zwei Zeilen mit konstanter Höhe. Kein horizontaler Scroller; schmale Fenster kürzen Beschriftungen mit vollständigem Tooltip. Fotos sind horizontal zentriert.

Die optionale Detailansicht des Importprotokolls stellt jeden Schritt als eigenen Absatz dar, erhält Zeilenumbrüche und bricht lange Pfade passend zur Fensterbreite um. Die Standardansicht zeigt ein kompaktes Ergebnis pro Datei.

## Audio-Diary

Audio wird im bestehenden Import mitkopiert: `Jahr/diary/aud_JJJJMMTT_HHMMSS.ext`. Beim Import bleiben Quellen unverändert; nur der separate bestätigte Aufräumschritt kann sie danach in den Papierkorb verschieben. Zum Anhören ein Jahr aufklappen, „Diary“ wählen und eine Datei anklicken. Der Jahresplayer enthält alle Aufnahmen dieses Diary-Ordners chronologisch auf einer einzigen Hörleiste. Diese startet mit Dateinamen und Dateigrößen als vorläufiger Grundlage; nach der Prüfung sind die Abschnitte proportional zur echten Dauer. Der Schieber navigiert auch über Dateigrenzen hinweg; es wird keine Sammeldatei exportiert und keine Playlist dargestellt.

Eingebettete Aufnahmezeit vor Inhalts-/Containerdatum; ohne diese Daten zuerst den vollständigen Zeitstempel im Dateinamen (z. B. `2023-12-27_22-51-32`, mit `-`, `_` oder ohne Trenner; einschließlich aud-Importnamen), zuletzt das Dateisystem-Erstellungsdatum als gekennzeichneten Fallback verwenden. Datum und Quelle im Prüflauf kontrollieren. Bei Fotos/Videos bleiben die strikten bisherigen Regeln unverändert. Audio-Kollisionen werden bytegenau verglichen. Formatliste, Einschränkungen und genaue Priorität stehen in der aktuellen Repository-README.

Das Importprotokoll ist standardmäßig kompakt: ein Ergebnis pro Datei, vollständige Pfade und Datumsquelle im Tooltip. „Alle Schritte anzeigen“ öffnet das detaillierte, mehrzeilige Protokoll. „Protokoll kopieren“ kopiert jeweils die gewählte Ansicht.

Diary zeigt jetzt Kacheln statt einer Dateinamenliste. Die gewählte Vorschaugröße steuert 10/8/6/5/4 Spalten auf ausreichend breiten Fenstern. Unter dem Audiosymbol stehen Datum und Uhrzeit; vollständiger Dateiname und Datumsquelle erscheinen im Tooltip. Der Player zeigt das laufende Aufnahmedatum und die Aufnahmeuhrzeit, einschließlich des Fortschritts innerhalb der aktuellen Datei. Vorhandene Preferences werden in die separate Datei `de.photosuite.PhotoSuite.settings.plist` übernommen; das alte App-Domain bleibt erhalten. Importe in andere Ziele ändern die ausgewählte Fotobibliothek nicht mehr.

### Fotos zum Gehörten

Unter der Audioleiste erscheinen fünf Fotos aus der geöffneten Sammlung. Das größere mittlere liegt zeitlich am nächsten an der Aufnahmezeit beim Start, Clipwechsel oder letzten Sprung. Links stehen zwei frühere, rechts zwei spätere Nachbarn. Ohne Nachbar bleibt ein Platz leer. Die Auswahl folgt nicht jeder Sekunde. Nach einem Sprung wird kurz gewartet, damit Ziehen am Schieber keine Suchflut erzeugt.

Audio startet ohne vorgeschaltete Fotosuche. Erst während der Wiedergabe werden die Fotozeiten im Hintergrund gelesen und kleine Vorschauen nachgeladen. Die erste Suche kann bei großen Sammlungen dauern; danach wird der Index im geöffneten Player wiederverwendet. Fehlende oder defekte Fotos beeinträchtigen Audio nicht. EXIF DateTimeOriginal hat Vorrang; fehlt es, wird ein vorhandener img-Importzeitstempel verwendet, niemals das Dateisystemdatum. Die Kacheln werden zentriert und unverzerrt beschnitten, damit keine zusätzlichen schwarzen Balken entstehen. Unter jedem Foto stehen sein Aufnahmedatum und seine Uhrzeit; Quelle und Pfad bleiben im Tooltip. Ein Klick auf ein geladenes Bild oder seine Beschriftung navigiert im Hintergrund zum gesamten Jahr dieses Fotos. Die Fotowand scrollt zum Bild und hebt es mit einem blauen Rahmen hervor; das Jahr wird in der Seitenleiste aufgeklappt. Such- und Videofilter werden dafür zurückgesetzt. Der Bildbetrachter bleibt geschlossen, der Audioplayer behält den Fokus und Audio läuft weiter. Bei einem Foto außerhalb der Jahresstruktur wird stattdessen die gesamte Sammlung angezeigt. Bibliotheksänderungen werden nach erneutem Öffnen des Players berücksichtigt. Es werden keine Fotos oder Cache-Dateien geschrieben.

### Schneller Start und Prüfzustände

Der Player öffnet nach den Größenabfragen aller Aufnahmen mit einer vorläufigen Leiste und lädt zuerst nur die gewählte Aufnahme. **Grau:** noch ungeprüft; Datum aus dem Dateinamen und Breite nach Dateigröße. **Blau:** Dauer und Datum geprüft, Breite nach Dauer korrigiert. **Rot:** Prüfung fehlgeschlagen; der Eintrag bleibt sichtbar und der Tooltip nennt den Fehler. Eine zusätzliche Fortschrittszeile benennt diese Zustände.

Erst während tatsächlicher Wiedergabe beginnt die gründliche Prüfung, Aufnahme für Aufnahme. Reihenfolge und Breiten dürfen sich ändern; die laufende Datei bleibt an ihrer lokalen Hörposition. Eingebettete Aufnahmezeiten haben bei der Prüfung wieder Vorrang vor Dateinamen. Bei korrigierter Aufnahmezeit werden auch die Kontextfotos aktualisiert. Abbrechen beendet die Prüfung, nicht die bereits laufende Wiedergabe; ungeprüfte Abschnitte bleiben vorläufig. Keine Datei wird verschoben oder umbenannt. Bei langsamen Laufwerken können die anfänglichen Größenabfragen dauern, beim Dateiwechel kann eine kurze Ladepause entstehen. Sekunden-Sprünge über mehrere noch ungeprüfte Clips sind zunächst nur näherungsweise möglich.

## Importquellen in den Papierkorb

1. Einen tatsächlichen Import abschließen oder abbrechen; abgeschlossene Dateiberichte bleiben verfügbar. Ein reiner Prüflauf genügt nicht.
2. **„Importquellen in den Papierkorb …“** anklicken. Der neue Dialog verwendet die gespeicherten Quell-/Zielpfade dieses Imports, auch wenn danach andere Ordner ausgewählt wurden.
3. Die Zeilen prüfen: „Freigegeben“ erfordert ein vorhandenes, vollständig byteidentisches Ziel. Fehlerhafte, nur geplante, fehlende oder abweichende Dateien bleiben erhalten. Lange Pfade sind im Tooltip vollständig lesbar.
4. **„… freigegebene Quellen in den Papierkorb“** ausdrücklich bestätigen oder abbrechen/schließen. Vor jedem Verschieben werden die Inhalte und die bei der Freigabe erfassten Dateistände erneut geprüft. Änderungen seit der Freigabe sperren die Quelle.
5. Ergebnisse kontrollieren. „Im Papierkorb“ bestätigt die Aktion; „Nicht verschoben“ nennt den Grund. Ein unklarer Status erfordert erneutes Prüfen von Quelle und Papierkorb. Bereits verschobene Quellen bleiben bei Abbruch im Papierkorb; keine Zieldatei wird gelöscht.

Es gibt keine automatische Quellenlöschung, keine endgültige Löschung als Ersatz bei Papierkorbfehlern und kein Entfernen ganzer Quellordner. Die Funktion ist auf den aktuellen Importlauf beschränkt; nach einem neuen Lauf bzw. Neustart gibt es noch kein dauerhaftes Aufräumarchiv.
