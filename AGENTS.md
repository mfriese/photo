# PhotoSuite – Arbeitsregeln für neue und laufende Chats

## Sofort orientieren
- Rust-Desktop-App für Fotos, Videos, Audio-Diaries und Fotokommentare. UI: egui/eframe, native macOS-Menüs/Player. Deutsche, verständliche Oberfläche.
- Diese Datei zuerst lesen. Danach nur die unten zur Aufgabe passenden Anleitungen und betroffenen Quellmodule lesen. Nicht standardmäßig die gesamte Architektur-/Reviewdokumentation oder den Chatverlauf aufarbeiten.
- Repository und Arbeitsbaum prüfen (`pwd`, `git status --short`). Vorhandene Änderungen erhalten; nichts ohne Auftrag zurücksetzen, committen oder installieren. Bei abweichenden Arbeitsverzeichnissen das tatsächliche Repository verifizieren.
- Aktuellen Code und gezielte Tests als Implementierungsstand prüfen. Anleitungen beschreiben Anforderungen, keine pauschale Garantie ihrer vollständigen Umsetzung.

## Fokussiert arbeiten
- Den konkreten Auftrag vollständig lösen. Keine zusätzlichen Features, umfassenden Refactorings, Abhängigkeitsupdates oder projektweiten Aufräumaktionen ohne sachlichen Bezug.
- Vor dem Editieren den betroffenen Ablauf und seine Schichtengrenzen kurz bestimmen. Nur nach benötigten Symbolen suchen (`rg`); kleine Dateiausschnitte lesen, unabhängige Prüfungen bündeln, Ausgaben begrenzen.
- Architektur schrittweise verbessern: Bei jeder Aufgabe kleine, sachlich zugehörige Teile glätten, Verantwortlichkeiten und DDD-Schichtengrenzen klären und bestehendes Verhalten durch passende Tests erhalten. Keine unabhängigen Großumbauten daraus ableiten.
- Bei jeder Änderung die Architektur **im betroffenen Ablauf** über die relevanten Schichten prüfen und lokale Verstöße korrigieren. Keine vollständige Projektprüfung für jede kleine UI-Änderung. Größere unabhängige Befunde mit Auswirkung benennen und separat behandeln.
- Bestehende Abstraktionen und gemeinsame Regeln wiederverwenden. Neue Ports/Module nur bei konkreter Verantwortung oder echter Test-/Plattformgrenze. Keine Schichten, Wrapper oder Aggregate allein um eines Musters willen.
- Keine Delegation/Subagenten ohne ausdrücklichen Auftrag. Keine wiederholten Vollprüfungen oder Paketbauten nach unverändertem Code.
- Abschluss knapp: Ergebnis, tatsächlich ausgeführte Prüfungen und relevante Einschränkungen. Manuelle Sichtprüfung nur behaupten, wenn durchgeführt.

## Architektur und DDD
- Abhängigkeiten: Desktop → Application → Domain. Desktop darf Domain verwenden; keine Rückabhängigkeiten.
- `crates/domain`: fachliche Identitäten, Value Objects, Invarianten, Benennung, Datums-/Filterregeln. Nur chrono als externe Abhängigkeit; kein I/O, SQL, GUI, serde oder unsafe.
- `crates/application`: Anwendungsfälle, Sitzungen, Workflow, Abbruch und frameworkfreie technische Ports. Nur Domain als Abhängigkeit; keine konkreten Adapter, Dateisystem-/SQL-/GUI-Implementierungen oder unsafe.
- Äußere Desktop-Adapter: Dateisystem, YAML/plist, SQLite, EXIF, Decoder, Whisper, FFI/macOS. Produktionsadapter ausschließlich in `main` verdrahten und injizieren; keine versteckte Erzeugung konkreter Adapter in UI oder anderen Adaptern.
- Präsentation rendert, sammelt Eingaben und hält Darstellungszustand. Keine mehrstufigen Fachabläufe, Speicherregeln oder Textanalyse von Logs als Workflowsteuerung. Gemeinsame Bearbeitungsaktionen über Command-Katalog/Dispatcher.
- Validierte Konstruktoren/private Felder für Invarianten; technische Snapshots/Lesemodelle dürfen einfache DTOs bleiben. Fachliche Regeln nicht zwischen Rust und nativer UI duplizieren.
- Hintergrundarbeit: begrenzte Worker/Kanäle, definierter Abschluss/Fehler/Abbruch, Root und Generation gegen veraltete Ergebnisse, Ressourcenfreigabe auch bei Kanalfehlern. Keine blockierenden Scans/Decodierung im UI-Thread.

## Unverzichtbarer Datenschutz und Datenbestand
- **Keine echte Benutzerbibliothek für Tests verwenden, verändern oder automatisch öffnen.** Ausschließlich RAM-/temporäre Fixtures; keine echten Mikrofone, Papierkorb- oder Preferences-Aktionen in Standardtests.
- Originalmedien grundsätzlich nur lesen. Nur ausdrücklich ausgelöste Import-, Aufnahme-, EXIF-, Papierkorb- und Verschiebeaktionen dürfen nach ihren jeweiligen Regeln schreiben. Import selbst löscht niemals Quellen.
- Keine fremden Dateien überschreiben/löschen, keine Symlinks verfolgen, keine Fehler still als leeren erfolgreichen Datenbestand behandeln. Bestätigte Teilerfolge erhalten und melden.
- Einzel-Medien-YAMLs halten essentielle, nicht aus Originalen rekonstruierbare Benutzerdaten. SQLite ist rekonstruierbare Projektion/Cache. Keine redundanten maßgeblichen Datenquellen oder Cache-Kaskaden; rekonstruierbare EXIF-/GPS-Daten nicht zusätzlich in YAML speichern.
- Nur aktuelles vereinbartes Format unterstützen; keine Legacy-Migrationen/Fallback-Stores hinzufügen. Ausnahme: optionales `recent_libraries` bleibt kompatibel zur lokalen Locator-Plist Version 1.

## Passende Anleitung gezielt lesen
| Aufgabe | Verbindliche Detailregeln |
| --- | --- |
| YAML, SQLite, Tags, Preferences, Neuaufbau | [Persistenz](docs/instructions/PERSISTENCE.md) |
| Import, Papierkorb, Datum, physisches Verschieben | [Dateioperationen](docs/instructions/MEDIA_OPERATIONS.md) |
| Queue, Audio, Aufnahme, Whisper, Geo, Worker | [Medienverarbeitung](docs/instructions/PROCESSING.md) |
| Raster, Baum, Dialoge, Theme, native Controls | [Oberfläche](docs/instructions/UI.md) |
| App-Bundle, Icon, Installation, Distribution | [Paketierung](docs/instructions/PACKAGING.md) |

Bei übergreifenden Änderungen mehrere passende Anleitungen lesen. `ARCHITECTURE.md` und `docs/ARCHITECTURE_REVIEW.md` sind vertiefende Entwurfs-/Reviewreferenzen, keine Pflichtlektüre für jede Aufgabe. Bei Widersprüchen gelten diese Arbeitsregeln und die aktuellen Detailanleitungen vor historischen Beschreibungen; explizite Benutzeraufträge haben Vorrang.

## Prüfungen nach Risiko
- **Nur Dokumentation:** Links, Widersprüche und `git diff --check` prüfen. Keine Rust-/nativen Tests und keinen App-Build allein wegen Prosa starten.
- **Kleine lokale UI-/Theme-Änderung:** Formatierung und vorhandene relevante UI-/Layouttests; keine Tests bloßer Konstanten oder Quelltextspiegelungen. Build/Codeprüfung nur soweit zur Änderung erforderlich.
- **Fachlogik/Anwendungsfall:** relevante Modul-/Crate-Tests; sinnvolle Regression für verändertes Verhalten. Bei Schicht-/Abhängigkeitsänderungen zusätzlich `make check-architecture`.
- **Speichern, Löschen, Verschieben, SQLite-Neuaufbau, Nebenläufigkeit oder Shutdown:** gezielte Fehler-/Abbruch-/Quellschutztests und vollständige Rust-Suite sowie Clippy. Betroffene native Tests bei Änderungen an nativen Verträgen/Playern/FFI ergänzen.
- **Release oder ausdrücklich vollständiger Review:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace`, `make check-architecture`; relevante native Prüfungen und Pakettests. Paketbau nur bei Release-/Paketauftrag oder geänderter Paketierung.
- Kern separat testbar halten: `cargo test -p photosuite-domain -p photosuite-application`. Native Prüfungen: `make test-audio`, `make test-video`, `make test-recorder`, `make test-menu`, `make test-icon` bzw. `make test-native`.
- Fehler erst verstehen, dann gezielt erneut prüfen. Bereits erfolgreiche Prüfungen nur bei neuen Änderungen oder offenen Risiken wiederholen. Umfang und ausgelassene relevante Prüfungen ehrlich nennen.

## Dokumentation aktuell halten
- Regeln am passenden bestehenden Ort korrigieren, keine chronologischen Anhänge oder widersprüchlichen Doppelregeln anhäufen. AGENTS.md bleibt ein kompakter Einstieg; Details nur in der passenden Anleitung.
- README bleibt kurz und für GitHub-Besucher verständlich. Bei merklichen Funktions-/Bedienungsänderungen aktualisieren; ARCHITECTURE.md nur bei wesentlichen Strukturentscheidungen.
- README samt Screenshots regelmäßig nach größeren sichtbaren Änderungen aktualisieren; Screenshots mit relativen GitHub-tauglichen Pfaden und ausschließlich Demodaten. **Screenshots sind derzeit auf Benutzerwunsch ausgesetzt** und erst nach neuer Freigabe zu erstellen.
- Neue Chats sollen Aufgabe, betroffene Anleitung, Code und Tests benötigen, nicht die gesamte Entwicklungsgeschichte. Offene wesentliche Einschränkungen am zuständigen Dokument festhalten, keine unnötige zweite Statusdatenbank anlegen.
