# PhotoSuite – verbindliche Projektregeln

## Produkt
- Rust-Desktop-Anwendung zum komfortablen Betrachten von Foto- und Videosammlungen.
- UI in Rust mit egui/eframe; verständliche deutsche Beschriftungen.
- Standardmäßig nur lesen. Explizite Ausnahme: bei aktiviertem Häkchen dürfen neue manuelle Drehbefehle JPEG-EXIF ändern. Keine automatische Massenänderung beim Öffnen/Scannen. JPEG-Pixel nicht neu komprimieren; Ohne dauerhafte Sicherung direkt in der Originaldatei aktualisieren (`-overwrite_original_in_place`); temporäre Arbeitsdatei darf intern verwendet werden. Bereits vorhandene fremde Backups nicht löschen. Andere Formate und symbolische Links nicht verändern. Keine Cache-Dateien in der Sammlung.
- Jahres-/Monatsstruktur respektieren; Jahre standardmäßig eingeklappt und manuell aufklappbar; ganze Jahre und vorhandene Monate auswählbar.
- Scrollbare, virtualisierte Fotowand; fünf Vorschaugrößenstufen. Vollständige Zeilen füllen die Breite mit gleichen Kachelgrößen und Abständen. Letzte Zeile behält Kachelgrößen.
- Häkchen „Quadratische Vorschau“: aktiviert zentrierter quadratischer Beschnitt ohne Verzerrung; deaktiviert vollständiges Bild im Seitenverhältnis innerhalb gleich großer Rasterzellen.
- Suche, Videofilter, Großansicht, Pfeiltastennavigation und Escape erhalten.
- EXIF-Orientierung automatisch berücksichtigen, einschließlich Spiegelungen. Manuelle Rotation zusätzlich zur EXIF-Ausrichtung; Original bleibt im Nur-Lesen-Modus unverändert. Schreibmodus und Sitzungskorrekturen klar kennzeichnen.
- Videowiedergabe auf macOS im eigenen nativen AVKit/AVFoundation-Playerfenster; externes Standardprogramm als zusätzliche Option. Fehlende Formate und Funktionen ehrlich dokumentieren.

## Architektur
- Clean Architecture und pragmatisches Domain-driven Design anwenden.
- `crates/domain`: fachliche Modelle und Value Objects; keine Abhängigkeit von Anwendung, UI oder Infrastruktur. Validierung und Invarianten durch Konstruktoren und private Felder sichern.
- `crates/application`: Anwendungsfälle, Sitzungszustände, Orchestrierung und technische Ports. Nur von der Domänen-Crate abhängig; keine GUI, Dateisystembibliotheken oder konkreten Adapter.
- `infrastructure`: Dateisystem- und Bilddecoderadapter.
- `presentation`: UI und Darstellungszustand; keine fachlichen Filter- oder Dateiscanregeln duplizieren.
- `main`: Einstiegspunkt und Verdrahtung. Abhängigkeiten zeigen nach innen; keine unnötigen Aggregate oder Services ohne fachlichen Nutzen.
- Verzeichnis- und Bildladen im Hintergrund; UI nicht durch Dateizugriff/Decodierung blockieren.
- Fehler sichtbar behandeln, symbolischen Verzeichnislinks nicht folgen.

## Qualität
- Änderungen vollständig implementieren und README bei verändertem Verhalten aktualisieren.
- `cargo fmt --check` und `cargo test` ausführen; relevante Regressionen für Fachregeln und Bildtransformationen testen.
- Manuelle UI-Prüfung nur als durchgeführt melden, wenn tatsächlich erfolgt.
- Einschränkungen wie HEIC/RAW, Videovorschauen, Cache-Speicher und fehlende Persistenz ausdrücklich dokumentieren.
- Konkrete Repository-, Decoder- und Öffnen-Adapter ausschließlich in `main` verdrahten und injizieren. UI und Bildlader dürfen `infrastructure` nicht direkt referenzieren.
- Adaptergrenzen verwenden frameworkunabhängige Daten; keine `egui`-/`image`-Typen in Anwendungs-Ports.

- Alle Bildbearbeitungsaktionen über den zentralen Command-Katalog und Dispatcher auslösen. Buttons und Tastenkürzel teilen dieselbe Ausführung. Kürzelkonflikte ablehnen und während Texteingabe bzw. Zuweisung keine Bearbeitungsbefehle auslösen.
- EXIF-Schreiben über injizierten Port im Hintergrund; Fehler erhalten Sitzungskorrektur, Erfolg invalidiert Bildcache und entfernt zusätzliche Sitzungsrotation.

- Werkzeugleiste kompakt und fensterbreitenabhängig: Ordneraktionen, Menüs und kompaktes Suchfeld (180 Punkte) mit Filter in derselben umbrechenden Buttonzeile. Anzeigeoptionen unter „Ansicht“, Speichern und Tastenkürzel unter „Einstellungen“. Lange Steuerelementzeilen umbrechen, Pfade kürzen mit Tooltip; Bibliotheksleiste ausblendbar. Mindestfenstergröße 360 × 400.

## Import und Videovorschauen
- Explizit gestarteter Import ist eine weitere Ausnahme zum Nur-Lesen-Modus: ausschließlich ins gewählte Ziel kopieren. Quelle niemals umbenennen, verschieben oder löschen. Quelle/Ziel dürfen sich nicht überschneiden.
- Foto-Aufnahmezeit ausschließlich aus EXIF DateTimeOriginal, bei Videos QuickTime CreateDate. Ungültiges/fehlendes Datum sichtbar melden; kein stiller Ersatz durch Dateisystemdatum. Keine implizite Zeitzonenverschiebung.
- Ziel: Jahr/Monat/img_JJJJMMTT_HHMMSS.ext; Originalformat erhalten, .jpeg zu .jpg normalisieren. Fotos bei Kollision bytegenau vergleichen; identische überspringen, unterschiedliche mit _001 usw. kopieren. Videos bei gleichem generiertem Zielnamen überspringen.
- Prüflauf ohne Schreibzugriffe und tatsächlicher Import mit erneuter Prüfung. Jede Phase, Quelle, Datum, Ziel, Kollisionsentscheidung, Fortschritt und Fehler im Protokoll sichtbar. Abbruch unterstützen, abgeschlossene Dateien behalten.
- Kopien temporär im Ziel schreiben, synchronisieren und bytegenau prüfen, dann ohne Überschreiben veröffentlichen. Keine symbolischen Zielordner akzeptieren. Temporäre Dateien bei regulärem Fehler/Abbruch entfernen.
- Importregeln im Anwendungsfall halten, Dateisystem/EXIF in injizierten Adaptern, UI separat. Tests mit temporären Verzeichnissen und echten Metadaten.
- Videovorschauen auf macOS mit verlinktem AVFoundation, ohne externes FFmpeg zur Laufzeit. Track-Ausrichtung beachten, Auflösung auf 640 begrenzen. Plattform-/Codecgrenzen sichtbar dokumentieren. Foto-EXIF direkt mit Rust-Bibliothek lesen; keine neue externe Anwendung dafür verlangen.

## Durch den Compiler gesicherte Grenzen
- Cargo-Workspace mit `photosuite-domain`, `photosuite-application` und äußerer Desktop-Crate. Richtung: Desktop → Anwendung → Domäne; Desktop darf Domänenmodelle verwenden. Keine Rückabhängigkeiten hinzufügen.
- Technische Repository-/Decoder-/Öffnen-/Schreiben-Ports gehören in die Anwendung, nicht ins fachliche Modell. Port-Signaturen bleiben frameworkunabhängig.
- Command-Identitäten und Drehregeln gehören in die Domäne; deutsche Buttontexte in die Präsentation.
- Aufnahmezeit und Importbenennung sind Domänenregeln. Importablauf und Kollisionsorchestrierung bleiben in der Anwendung; Kopieren/Metadatenzugriff in Adaptern.
- Drehzustand, Speicherserialisierung und Verhalten bei Speicherfehlern in `EditingSession`, nicht in UI-Callbacks implementieren. UI übernimmt Darstellung und Cacheinvalidierung.
- Importabschluss mit strukturiertem Ergebnis melden, UI darf Erfolg oder Statistik nicht aus Protokolltext parsen. Abbruch nicht als Dateifehler zählen.
- `cargo test --workspace` und `cargo fmt --all --check` prüfen. Kern separat mit `cargo test -p photosuite-domain -p photosuite-application` ohne native UI/Video/ExifTool testen.

## Persistente Einstellungen
- macOS-Einstellungen in `~/Library/Preferences/de.photosuite.PhotoSuite.plist` als lesbare XML-Key-Value-Property-List speichern, niemals in Fotoquellen.
- Anzeigeoptionen, Medienfilter, EXIF-Schreibmodus, Tastenkürzel und zuletzt geöffnete Bibliothek persistieren. Pfad bereits beim Öffnen speichern, unabhängig vom Scanabschluss. Automatisches Wiederöffnen standardmäßig aktiv und abschaltbar; Startargument hat Vorrang. macOS-Finder-Starttoken nicht als Pfad behandeln.
- Preferences-Port und Hintergrundspeicherung in Anwendung, Dateiformat/Benutzerpfad ausschließlich im Desktop-Adapter. Konkreten Store in main injizieren.
- Änderungen automatisch im Hintergrund, atomar und beim normalen Beenden vollständig speichern. Defekte/unbekannte Konfiguration sichtbar melden und erhalten.
- Einstellungsdialog über „Einstellungen …“ und ⌘, erreichbar. Darstellung, Start, Bildänderungen und Bedienung als einheitlich eingerückte Gruppen; alle Konfigurationsfenster mit konsistenten Abständen und begrenzten/scrollbaren Abmessungen.

- EXIF-Schreibmodus als klar beschriftete Kennzeichnung mit weißem Text auf dunklem Hintergrund zeigen; keinen gelben Hinweistext verwenden.
- Video-Großansicht: sechs zeitlich gleichmäßig verteilte Standbilder mit tatsächlichen Zeitangaben als responsives Raster zeigen. Sequenziell im Hintergrund laden, Einzelfehler sichtbar behandeln. Beim Wechsel alte Texturen und Aufträge freigeben; zeitliche Auswahl/Orchestrierung in Anwendung, native Dekodierung hinter Port, Raster in Präsentation.

- Videowiedergabe über zentralen Command „Video abspielen“, standardmäßig Leertaste; Tastenkürzel persistieren. Bei Texteingabe und offenen Konfigurationsdialogen keine Wiedergabebefehle auslösen. Tastenwiederholung darf den Player nicht mehrfach öffnen.
- Native Player-UI ausschließlich auf dem Hauptthread öffnen, Video asynchron laden. Leertaste pausiert/startet, Escape beendet Fenster (bzw. Vollbild). Beim Schließen oder Ersetzen Wiedergabe anhalten und AVPlayerItem freigeben; Codecfehler sichtbar anzeigen.

## Geführte Kollisionsduplikate
- Auf Importseite jeweils nur eine Kollisionsgruppe prüfen und nach ausdrücklicher Entscheidung automatisch fortsetzen; Abbrechen unterstützen. Gruppen enthalten denselben gültigen img-Zeitstempel im selben Ordner mit beliebigen Anhängen, einschließlich Buchstaben; alle Varianten paarweise vergleichen.
- Vollständige Bildpixel mit image dekodieren, EXIF ausrichten, 16-Bit-Kanäle erhalten. Keine Hash-/Thumbnail-/Ähnlichkeitsentscheidung für Entfernen. Farbraum-/Profilunterschiede, Mehrbildformate und unklare Daten konservativ behandeln und erhalten.
- Neue ausdrückliche Ausnahme zum Nur-Lesen-Modus: ausschließlich nach Gruppenentscheidung bestätigte Pixelduplikate in der Zielbibliothek in den macOS-Papierkorb verschieben. Keeper pro pixelidentischer Teilgruppe erhalten, Metadatenunterschiede offenlegen, niemals endgültig löschen. Quelle des Imports bleibt unverändert.
- Vor Papierkorbaktion erneut Pixel und Dateistand prüfen; symbolische Pfade und Ziele außerhalb der Bibliothek ablehnen. Keine echte Benutzerdatei in Tests entfernen.
- Gruppierung in Domäne; Paarvergleichsablauf/Entfernungsplan in Anwendung; Dekodierung/Papierkorb in injizierten Adaptern; UI separat. Tests auch für EXIF-Drehung, 16-Bit-Unterschiede, Metadaten und Farbprofile durchführen.

## Verbindliche Architekturpflege der gesamten Anwendung
- Bei jeder funktionalen Erweiterung und bei Architekturaufträgen die betroffenen Abläufe über alle Schichten prüfen: Domänenregel, Anwendungsfall/Sitzungszustand, Port, Adapter, UI und Composition Root. Neue Module und wesentliche Entscheidungen in `ARCHITECTURE.md` nachführen. Veraltete Regeln in dieser Datei korrigieren, nicht durch widersprechende Anhänge ersetzen.
- Ziel ist eine zum Funktionsumfang passende, nachvollziehbare Architektur. Keine „perfekte“ Architektur behaupten; bewusst verbleibende Vereinfachungen, Ressourcenlimits und Plattformgrenzen mit Begründung dokumentieren.
- Fachliche Entscheidungen, Dateipläne und Workflow-Übergänge gehören in die inneren Schichten. UI-Callbacks dürfen Aktionen anstoßen und Eingaben/Darstellungszustände halten, aber keine mehrstufigen Arbeitsabläufe steuern. Duplikatgruppen werden durch `ReviewSession` gesteuert.
- Alle Produktionsadapter in `main` verdrahten. Auch Adapter untereinander dürfen konkrete Implementierungen nicht versteckt erzeugen; Fotodecoder erhalten den Video-Frame-Port injiziert. Testadapter dürfen ausschließlich innerhalb von Tests zusammengestellt werden.
- Präsentation in überschaubare Ansichten gliedern (`presentation/toolbar`, `library`, `wall`, `viewer`). Einheitliche Optionsgruppen wiederverwenden. Fachlogik nicht zur bloßen Dateiverkürzung in UI-Hilfsfunktionen verschieben.
- Hintergrundaufträge brauchen einen definierten Lebenszyklus: nicht blockierender Start, Fortschritt/Ergebnis, Fehler, Abbruch, Freigabe und Umgang mit veralteten Ergebnissen. Ein unterbrochener Kanal darf keinen dauerhaften Ladezustand erzeugen.
- Bilddekodierung über einen begrenzten Worker-Pool ausführen: zwei aktive Decoder und höchstens 32 wartende Bildaufträge. Bei voller Warteschlange UI nicht blockieren; Anfragen später erneut versuchen. Cache-Generationen verhindern, dass alte Aufträge neu geladene Bilder überschreiben.
- Beim Abbrechen abgeschlossene Dateischritte weiter quittieren und Bibliotheksansicht aktualisieren; keine nächste Gruppe starten. Bereits laufende Betriebssystemoperationen sind nicht sofort abbrechbar. Weder Fehler noch abgeschlossene Dateibewegungen stillschweigend verwerfen.
- FFI strikt in Plattformadaptern halten. Domäne/Anwendung bleiben frei von unsafe. Native Video-Decodierung, Player-UI und Papierkorbzugriff in getrennten Dateien halten; gemeinsamer C-Header dokumentiert Thread- und Speicherbesitz.
- Änderungen an Abhängigkeiten, Schreiboperationen und Nebenläufigkeit durch sinnvolle Regressionstests absichern. Keine Tests, die nur Quelltext oder Implementierungsdetails nachbauen. Keine echten Fotos verändern oder in den Papierkorb verschieben.
- Architekturprüfung abschließen mit `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` und `cargo build --workspace`. Kern separat testbar halten. Native Wiedergabe bei Änderungen am Player zusätzlich über `tests/native_playback.m` prüfen. Manuelle Sichtprüfung und automatisierte native/UI-Tests unterscheiden.
- Bekannte Ressourcen- und Fehlergrenzen offen halten: Decoder-/GPU-Cache, Protokollgröße, mögliche Wartezeiten beim Beenden und noch untypisierte Adapterfehler. Solche Grenzen sind kein Anlass für zusätzliche Schichten ohne fachlichen Nutzen, müssen bei relevanten Erweiterungen aber erneut bewertet werden.

## macOS-Paketierung und Installation
- App-Installation über `make install` bzw. `scripts/macos_app.py install` als vollständige `/Applications/PhotoSuite.app`. Cargo-Build/-Install-Hooks dürfen niemals ungefragt ins System installieren; `cargo install` bleibt Terminalinstallation.
- App-Bundle enthält Rust-Binary, ExifTool samt benötigten Modulen und Lizenzhinweise. macOS-Systemframeworks und System-Perl sind explizite OS-Voraussetzungen. Keine Homebrew-Pfade oder externen Drittanbieter-Dylibs im Laufzeitpaket voraussetzen.
- Gebündelte Hilfsprogramme relativ zu `Contents/MacOS` finden und in main injizieren; im App-Bundle bei fehlenden Ressourcen sichtbar fehlschlagen, nicht auf externes PATH ausweichen. Entwicklungsmodus darf PATH verwenden. Benutzer-ExifTool-Konfiguration für reproduzierbare Metadatenaufrufe deaktivieren.
- Paketbau im Projektzielverzeichnis; Systeminstallation erst bei explizitem Installationsaufruf. Vor Austausch laufende App und Bundle-ID prüfen, Symlinkziele ablehnen, im Ziel-Dateisystem bereitstellen und vorherige App bei Austauschfehler zurückstellen.
- `make pkg` erzeugt einen macOS-Installer für `/Applications`. Lokale Ad-hoc-Signatur und fehlende öffentliche Notarisierung ehrlich dokumentieren.
- Preferences und Fotobibliotheken gehören nicht in das Bundle und bleiben bei Update/Deinstallation erhalten.
- Paket nach Bau auf Plist, Codesign, dynamische Verweise und ExifTool ohne Homebrew-PATH prüfen. Installer-Tests nur in temporären Verzeichnissen; keine echte Installation in automatischen Standardtests verändern.

- App-Icon-Master unter `assets/app-icon/photosuite.png` im Projekt speichern, unverändert und mit Alpha. Paketbau erzeugt alle macOS-Icongrößen und ICNS deterministisch per Systemwerkzeugen; CFBundleIconFile setzen und Icon vor Codesign einbinden. Build darf keine Codex-/Generator-Cachepfade benötigen.

- Großansicht: alle Aktionen oberhalb des Inhalts in zwei festen Zeilen: Navigation oben, Bild-/Videoaktionen und Modus darunter. Kein horizontaler Scroller. Gleichmäßige Buttonmaße, einzeilige bei Bedarf gekürzte Beschriftungen mit Tooltips; Höhe bei Medien-/Speicherzustandswechsel konstant halten. Fotos horizontal zentrieren. Video-Vorschau neu laden ebenfalls oberhalb des Rasters.

- Aktive Bibliothek mit gekürztem Pfad und vollständigem Tooltip prominent unter den Werkzeugaktionen zeigen und in den Fenstertitel übernehmen. Fußzeile strikt einzeilig halten: Modus, Status und Fehler nebeneinander, bei Platzmangel kürzen und vollständigen Text im Tooltip anbieten.

- Importprotokoll: jeden Schritt als eigenen Absatz anzeigen, eingebettete Zeilenumbrüche erhalten und lange Pfade umbrechen. Protokolltexte nicht auf eine gekürzte Rasterzeile beschränken.

## Verbindliches Galeriedesign und Dokumentationspflege
- Zentrales Designsystem in `src/theme.rs`; Farben, Schriftgrößen, Widgetzustände und gemeinsame Flächen dort pflegen. `docs/DESIGN.md` beschreibt das verbindliche Erscheinungsbild. Keine Fachlogik und keine Dateizugriffe im Theme.
- Kompakte Graphitflächen, helle kontrastreiche Texte, zurückhaltender blauer Akzent. Wenig Randabstand, flache Bedienelemente, keine zusätzlichen Galerie-Badgezeilen. Hilfs-/Datei-/Statustexte mindestens 12 Punkte. Primär- und Sekundärtexte sowie Widgetzustände auf mindestens 7:1 Kontrast prüfen; deaktivierte Texte nicht bis zur Unlesbarkeit abdunkeln. Dezente Fotokachelrundungen, klare Typografiehierarchie und sichtbarer Fokus. Aktive Schreibmodi zusätzlich textlich kennzeichnen.
- Bei Änderungen das gesamte betroffene Layout mit dem echten Theme prüfen, einschließlich 360-Punkte-Fenstern, langer Pfade und Foto-/Videowechsel. Kontrast- und Layouttests erhalten.
- README.md ist die GitHub-Einstiegsseite: Nutzen, aktueller Projektstatus, echte Screenshots, Funktionen, Installation, Bedienung, Grenzen und Entwicklungsbefehle verständlich aktuell halten. Technische Detailanleitungen nach `docs/USER_GUIDE.md`, Architektur nach `ARCHITECTURE.md`, Designregeln nach `docs/DESIGN.md` auslagern.
- README.md **einschließlich Screenshots regelmäßig aktualisieren, sobald entsprechend umfangreiche Änderungen an Oberfläche, Funktionen, Installation oder Verhalten erfolgt sind**. Betroffene Screenshots im selben Änderungsstand erneuern; keine veralteten Ansichten neben neu beschriebenem Verhalten belassen. Kleine unsichtbare Korrekturen erfordern keine identischen Neuaufnahmen.
- Screenshots als echte Aufnahmen der laufenden Anwendung unter `docs/screenshots/` versionieren. In Markdown ausschließlich relative Repository-Pfade mit exakter Schreibweise verwenden, damit die Darstellung auf GitHub funktioniert. Keine lokalen absoluten Pfade oder Build-Verzeichnisse als Bildquelle in der GitHub-README.
- Für Dokumentationsaufnahmen ausschließlich separate Demofotos mit nachvollziehbaren Bildrechten und isolierte Preferences benutzen. Keine privaten Sammlungen oder persönlichen Dateipfade veröffentlichen. Aufnahmeablauf und Bildquellen unter `docs/screenshots/README.md` dokumentieren; heruntergeladene Demobibliotheken bleiben außerhalb des versionierten Quellcodes.

- Dunkle Galeriefarben müssen an eine explizite egui-ThemePreference::Dark gebunden bleiben. Nur Visuals/Style zu setzen reicht nicht: ein später gemeldetes helles macOS-Systemtheme darf keine schwarzen Standardtexte auf dunkle eigene Flächen bringen. Regressionen mit RawInput.system_theme und tatsächlich gerenderten Textfarben prüfen (u. a. Jahreszahlen, starke Überschriften, Menüs, Buttons und Suchhinweise), nicht nur Farbkonstanten vergleichen.
