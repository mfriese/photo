# Vertiefter Architekturreview – 9. November 2026

## Ergebnis

Die überprüften Schichten folgen Clean Architecture: Desktop/Adapter → Anwendung → Domäne. Die Domäne und Anwendungs-Crate bleiben frei von GUI, Dateisystembibliotheken und unsafe; Cargo und die Architekturpolicy prüfen diese Richtung. Die Desktop-Crate enthält getrennte Präsentations- und Adaptermodule. Die fachlichen Aggregate (Zeitstempel, Medien, Diary-Zeitleiste, Tags), technischen Ports, Workflow-Sitzungen und Composition Root sind konsistent zugeordnet.

Die Nachprüfung hat konkrete Schwachstellen gefunden und behoben. Sie ist keine Zusicherung absoluter Fehlerfreiheit oder ein Ersatz für Tests auf jedem Laufwerk/Codec. Keine bestehenden Benutzer-Mediendateien wurden für den Review verändert oder entfernt.

## Korrekturen und Nachweise

| Befund | Korrektur | Regression/Evidenz |
| --- | --- | --- |
| Start las Bibliotheks-YAML im UI-Thread | `StartupPreferences` liest über den Port im Hintergrund; UI-Erstellung führt keine Preferences-I/O mehr aus | Blockierter Store bleibt nicht blockierend; verspätete Werte verdrängen keine manuell geöffnete Bibliothek |
| Nicht verfügbarer alter Pfad konnte expliziten Start verzögern | Startport erhält den expliziten Pfad vor dem Bibliothekslesen | Echte temporäre YAML-Konfiguration der gewählten Bibliothek trotz fehlender alter Bibliothek geladen |
| Tag-FIFO konnte acht volle Katalogkopien aufstauen | `OrganizationSession` und eigener Worker: eine aktive Operation, neuester wartender Schreibstand, neueste Ladeanfrage | 100 Änderungen bei blockiertem Schreiben ergeben nur ersten und letzten Schreibstand; Zwischenbibliothek wird nicht geladen |
| Namensnormalisierung war in Domäne, Anwendung und UI verteilt | Aggregat liefert kanonische Namen; Anwendung gibt strukturiertes `Change` zurück | Normalisierter Name wird unmittelbar korrekt zugeordnet; bestehende Album-/Rename-Invarianten getestet |
| Foto-Suchziele und manche Ergebnisströme waren unbeschränkt | Neueste-Ziel-Mailbox; 16 Fotoereignisse, 16 Diary-Datumsereignisse, 32 Duplikatereignisse | 10.000 Seekziele bei blockierter Metadatensuche liefern nur das neueste Ziel |
| Schnelle Producer konnten Poll-Schleifen lang belegen | Begrenzte Ereignisverarbeitung pro Poll | Bestehende Lifecycle-/Layouttests plus begrenzte Kanäle; keine Store-/Decoderarbeit unter UI-Sperren |
| Paarvergleich konnte quadratisch unbeschränkt wachsen | Geprüfte Rechnung und Limit von 100.000 Paaren pro Gruppe | Übergroße Gruppe scheitert vor erstem Dekodieren/Entfernen |
| Tag-Dateiprüfung lag zu früh vor aufwendigem Staging | Erwarteten Inhalt nach Schreiben/Sync unmittelbar vor Rename erneut prüfen | Während Vorbereitung extern geänderte Datei wird erhalten; eigene Tempdatei wird entfernt |
| Best-effort Persist konnte einen alten Hardlink-Namen hinterlassen | Gemeinsames `staging_fs`: Identität prüfen, Handle schließen, nur eigenen Alias entfernen | Veröffentlichte Datei bleibt bytegleich; fremder Ersatz unter altem Namen wird erhalten |
| Unterschiedliche Temp-Bereinigungen waren verteilt | Import und Metadata-Writer verwenden denselben überprüften Helfer | Fehler, Abbruch, fehlende/fremd ersetzte Pfade und eigene Stagingdateien getestet |
| Infrastruktur-Datei bündelte mehrere Verantwortungen | Separate Module für Sammlung, Bilder, Öffnen und EXIF-Schreiben | Bestehende Integrations-/Orientierungs-/Quellschutztests unverändert erfolgreich |
| Veraltete Regeln widersprachen aktuellem Kontextfoto-/Importverhalten | AGENTS und Architekturtext bereinigt | Dokumentation entspricht Hintergrundnavigation ohne Viewer/Fokuswechsel und getrennter Quellenbereinigung |

## Prüfumfang

- **Domäne:** Zeitstempel/Kalender, Benennung/Datumsvorrang, Medienklassifizierung, Commands, Kollisionsgruppen, Diary-Proportionen und Tag-/Albuminvarianten. Private Felder/Konstruktoren schützen gültige Zustände.
- **Anwendung:** Sammlung, Start-/Bibliothekseinstellungen, Persistenzbereiche, Bearbeitung, Wiedergabe, Audio-/Fotokontext, Storyboard, Import, geführte Duplikatprüfung und explizite Quellenbereinigung. Geprüft wurden Zuständigkeiten, Generationen, Fehler-/Abbruchpfade, Quittierungen und Freigabe.
- **Adapter:** YAML/Plist, Versions-/Pfadprüfung, atomic replacement, verifizierte Kopien, Pixelvergleich, Tempbereinigung, Papierkorb, Bilddekodierung und macOS-Brücken. Keine konkreten Produktionsadapter werden versteckt in inneren Schichten erzeugt; main verdrahtet sie.
- **Präsentation:** Toolbar, Bibliotheksbaum, Fotowand, Viewer, Diary, Tagverwaltung, Import- und Optionsdialoge. UI hält Eingabe-/Darstellungszustand und stößt Anwendungsfälle an. Fachregeln und Dateioperationen liegen darunter.
- **Native Grenzen:** C-Header/Projektionen, Main-Thread-Bindung, kopierte Datenübergabe, begrenztes Laden, aktive Item-/Positionsidentität und Freigabe. Native Audio- und Videowieder­gabetests verwenden ausschließlich kontrollierte Testmedien.
- **Paketierung:** Vollständiges lokales Bundle, relative Ressourcenauflösung, Offline-/Locked-Build, Signatur-/Dylib-/ExifTool-Prüfung; keine echte Systeminstallation.

## Validierung dieses Standes

- **184 Rust-Tests** erfolgreich: 99 Desktop, 62 Anwendung, 23 Domäne.
- Kern zusätzlich unabhängig von nativer UI gebaut und getestet.
- `cargo fmt --all --check`, Clippy mit `-D warnings`, Workspace-Build und Cargo-basierte Architekturprüfung erfolgreich.
- **10 Python-Tests** für Architekturpolicy und Paketierung erfolgreich.
- Automatisierte native Audio-, Video- und Menütests: reale Wiedergabe, Steuerung und Freigabe mit Testmedien; `make test-native` führt sie reproduzierbar aus.
- Lokales macOS-Paket neu erstellt und geprüft, nicht im System installiert.
- Keine manuelle Sichtprüfung oder neuen Screenshots behauptet.

## Bewusste Grenzen

- Die äußeren Schichten liegen in einer gemeinsamen Desktop-Crate. Cargo sichert die inneren Grenzen; zwischen äußeren Modulen gelten zusätzlich geprüfte Konventionen. Eine Aufteilung in weitere Crates wäre ein eigener Aufwand, ohne derzeit erforderlichen fachlichen Nutzen.
- Manche Adapterfehler sind Strings; der Scanport enthält eine Fehleranzahl statt aller Einzelfehler. Das behindert später differenziertere Wiederholungs-/Fehleroberflächen, ist aber kein Rückwärtszugriff auf Adapter.
- OS-/Codecaufrufe sind nicht sofort abbrechbar. Normales Beenden kann auf laufendes I/O und den letzten Speicherstand warten. Unerwartete interne Worker-Paniken werden sichtbar, können aber einen Neustart erfordern.
- Dateirevisions-/Identitätsprüfungen ersetzen keine globale Dateisperre. Änderungen nach der letzten Prüfung können nicht über alle Prozesse atomar ausgeschlossen werden; andere App-Instanzen/Editoren sind nicht vollständig koordiniert.
- 256 MiB/1024 Galerieeinträge begrenzen den Cache, nicht den Gesamtprozess oder temporäre Originaldekodierungen/GPU-Treiberspeicher. Tagkatalog, aktiver/neuester Snapshot und Dateirevision benötigen zusätzlichen Speicher. Ausstehende Preferences-Bereiche hängen von der Anzahl noch nicht gespeicherter Bibliotheken ab.
- Vollständige Importprotokolle bleiben pro Lauf erhalten. Duplikatberichte sind innerhalb des Paarbudgets weiterhin quadratisch; das Limit verhindert unbegrenztes Wachstum und verändert nicht die Vergleichsgenauigkeit.
- Keine automatische Wiederverknüpfung extern umbenannter Tags-Dateipfade und kein persistenter Audio-Dauercache. Codec-/HEIC-/RAW- sowie lückenlose Wiedergabegrenzen bleiben dokumentiert.
- Die eingebettete Audioleiste verwendet dieselbe Wiedergabesitzung wie das separate Fenster. Fensteranzeige und Transportlebenszyklus sind getrennt; der Anwendungskern liefert den namensbasierten Jahresentwurf.

## Nachprüfung: native Menüs
Die neue Menüführung bleibt eine Präsentationsaufgabe. `desktop_menu` trennt frameworkunabhängige Einträge/Aktionen vom injizierten muda-Adapter; `presentation/menu` delegiert fachliche Aktionen an bestehende Sitzungen und den Bild-Dispatcher. Keine GUI-Abhängigkeit im Anwendungskern. Native und egui-Befehlskürzel sind im Produktivbetrieb nicht gleichzeitig aktiv. Aktuelle Verfügbarkeit wird vor jedem Aktivierungsereignis erneut geprüft. Der Kanal ist auf 64 Ereignisse begrenzt, und Änderungen wecken die UI. Native Textmenüs bedienen den egui-Textfokus über dessen eigene Schnittstellen. macOS-AppKit-Menüs einschließlich realer Aktivierung und Shortcutprojektion wurden automatisiert geprüft. Windows und Linux bleiben ausdrücklich unvalidierte bzw. noch fehlende Host-Portierungen; keine Menü-Fallbacks.

## Nachprüfung: flache Audioablage
AudioScope/AudioKind und die Präfixklassifikation liegen in der Domäne. Die Anwendung verwendet dieselbe Mitgliedschaft für Anzeige, Namen-Cache, Datumskatalog und Wiedergabestart. Kommentare behalten ihre logische Namenszeit bei der Dauerprüfung. UI-Ansichtsschlüssel sind von physischen Schreibpfaden getrennt; nur der vorhandene Importpfad erzeugt neue Dateien unter Jahr/audio. Der Dateisystemadapter liest alte/neue Audioordner ohne synchrones Container-Probing. Die Regressionen prüfen getrennte/gemischte Bereiche, unbekannte Namen, Legacy-Kompatibilität, tatsächliche Playerprojektionen, Kommentarzeit, Navigation und unveränderte Dateien. Keine automatische Migration oder Aufnahmefunktion enthalten.

## Nachprüfung vom 12. November 2026: kompakte Galerie
Die Tagwolke bleibt Projektion des vorhandenen Katalogs, ohne zusätzliche Speicher-/Filterregeln. Alle Tags sind unabhängig vom Album-Flag erreichbar; Konfiguration liegt im nativen Edit-Menü. Captions teilen die namensbasierte Domänenregel und die neue begrenzte MediaDates-Sitzung; der injizierte Leseport darf keine Import-Schreibaktionen ausführen. Ein aktiver Worker, 32 Anfragen/32 Ergebnisse, 8192 Cacheeinträge, Generation/Abbruch und Audio-Priorität begrenzen Ressourcen. Cachetreffer und Ladeindikator sind ohne vollständige Frame-Scans abrufbar. Voller Kanal plant einen späteren Versuch statt still hängenbleibender Ladebeschriftungen. Alte Bibliotheksergebnisse werden verworfen und Fehler negativ gecacht. Der echte EXIF-Adapter wurde gegen unveränderte Testdateien geprüft.

Medienstatistiken sind eine inkrementelle append-only Projektion, keine erneute Bibliothekssuche pro Frame. Inline-Audio nutzt unverändert denselben Transport, nun neben der Überschrift ohne Mehr-Menü; zusätzliche Aktionen laufen über den bestehenden Menüport. UI-Regressionen prüfen Tagwolke, Datumsformatierung statt Dateiname, horizontale Audio-/Datumsanordnung und beständige Zähler links vor Statusmeldungen. Bekannte Grenze: momentan laufende System-Metadatenaufrufe können nach Abbruch noch auslaufen; es gibt keinen persistenten Datumsindex. Keine manuellen Screenshots oder Sichtprüfung behauptet.
