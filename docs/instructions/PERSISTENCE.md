# Persistenz und rekonstruierbare Indizes

Nur bei Änderungen an Speichern, Laden, Tags, Konfiguration oder Datenbank lesen. Übergeordnete Regeln: [AGENTS.md](../../AGENTS.md).

## Quellen und Identitäten
- Maßgebliche Quellen: vorhandene Originalmedien und aktuelle Einzel-Medien-YAMLs Version 3 mit `file`. YAML enthält essentielle Benutzerinformationen: Tags, Foto-Kommentarverweise mit GUID/Audiopfad, Transkripte/Versuchsmarker und optionale `capture_date`-Korrektur. Keine aus Dateinamen/EXIF rekonstruierbaren Datumsduplikate.
- Alle Medien-YAMLs unter `yaml/Jahr/Monat/<vollständiger Dateiname inklusive Endung>.yaml`; keine audio-/comments-/diary-Unterordner dort. Auch Scans erhalten ihre YAML im zugeordneten Monat. Unbekannte Datumsanteile gemäß gemeinsamem `metadata_path` behandeln.
- Identität ist der vollständige normalisierte relative Medienpfad, nicht Dateiname oder Jahr/Monat allein. Gemeinsamen Locator `media_records_fs::path` benutzen. Fremde Standard-YAML nicht umhängen: eigene stabile `.path-<Kennung>.yaml` verwenden und Besitz stets prüfen. Vorhandene eigene Zusatzadresse beibehalten.
- Keine Symlinks, Traversal, fremden Besitzer oder defekten/unbekannten Quellen überschreiben. Einzel-YAMLs auf 16 MiB begrenzen, Struktur-/Alias-/Expansionsschutz erhalten; selbst erzeugte gültige Transkripte müssen wieder lesbar sein.
- Revisionsgeprüft und atomar zuerst Quelle veröffentlichen, danach Index transaktional aktualisieren. Fehler nach Quellcommit als reparierbare Indexfehler melden. Angenommene Schreibstände beim normalen Beenden abschließen; externe Änderungen nicht still überschreiben.

## SQLite
- `yaml/library.sqlite`: lesbare typisierte Tabellen für Medien, Tagzuordnungen, Kommentare, Audiolängen, Jahresblöcke, Geo und Transkriptionsaufträge. Keine maßgeblichen Nur-in-SQLite-Benutzerdaten; kein generischer Key-Value-Store. Betriebszustand typisiert in `yaml/runtime.yaml`.
- Datenbankverlust muss aus Medien/Einzel-YAMLs rekonstruierbar sein; Dauern/GPS/Orte erneut berechnen, bestätigte Texte aus YAML übernehmen. Keine Cache-Kaskaden, GPS-YAMLs oder großen Gesamt-YAMLs hinzufügen.
- Alte große YAML-Listen/Tagdefinitionen weder lesen, migrieren, schreiben noch löschen. Nur aktuelles Format; keine Alben oder Albumflags.
- Neuaufbau hinter Anwendungsport, abbrechbar, Quellen validieren, Konflikte/Scanfehler melden. Erst vollständigen bestätigten Stand veröffentlichen; Fehler, Abbruch oder konkurrierende Quellenänderung erhalten den bisherigen Index. Ungültigen abgeleiteten Index nur ausdrücklich und nach Sicherung ersetzen.
- Normales Öffnen liest Tag-/Kommentar-/Audioprojektionen gesammelt aus SQLite; keine Quell-Vollscans oder Schreibreservierungen für Lesepfade. Wiederverwendete readonly Verbindungen, kurze deferred Snapshots, keine Abfrage/Schemaprüfung pro Medium. Datenbankersatz invalidiert Verbindungen; Schreibtransaktionen kurz und gebündelt.

## Tags
- Namen durch Domänenmodell stets trimmen/normalisieren und lowercase behandeln; relative Pfade validieren, Mitgliedschaften eindeutig. Keine eigenständigen leeren Tags; Namen und Anzahlen aus Zuordnungen ableiten. Gesamten Katalog im RAM halten.
- Automatisch `commented` für Fotos mit Kommentar, `comment` für Kommentar-Audio und `diary` für Tagebuch-Audio abgleichen. Benutzer-Tags bewahren; unvollständige/fehlgeschlagene Scans dürfen gültige Zuordnungen nicht entfernen.
- Import-Tags je betroffenem Medium in Einzel-YAML veröffentlichen, danach SQLite/RAM und sichtbare Optionen aktualisieren. OrganizationSession steuert serielles Laden/Speichern, neueste Anfragen bündeln; Fehler erhalten den RAM-Stand. Ungespeicherte Änderungen vor Bibliothekswechsel behandeln.

## Einstellungen
- Bibliothekskonfiguration lesbar in `yaml/settings.yaml`, ohne Bibliothekspfade. Lokaler Locator `~/Library/Preferences/de.photosuite.PhotoSuite.library.plist`: Version 1, `last_library`, optional `recent_libraries` mit höchstens fünf anderen Pfaden, neueste zuerst, ohne Duplikate. Alte Version-1-Dateien ohne Array bleiben lesbar; keine weiteren Legacy-Stores.
- Pfad sofort beim Öffnen speichern, nicht erst nach Scan. Automatisches Wiederöffnen standardmäßig an; expliziter Startpfad hat Vorrang. Späte Start-/Bibliotheksantworten dürfen manuelle Auswahl oder neue Einstellungen nicht überschreiben.
- Whisper-Modell/Sprache appweit in `de.photosuite.PhotoSuite.transcription.plist`, Gewichte unter `~/Library/Application Support/PhotoSuite/Whisper/models/`; GeoNames-Vorbereitung ebenfalls global, nicht bibliotheksgebunden. Globale Ressourcen beim Bibliothekswechsel wiederverwenden.
- Konfiguration im Hintergrund atomar speichern, höchstens neuesten wartenden Snapshot je Bereich halten. Keine Store-Aufrufe unter Mailbox-Lock, keine Werte alter Bibliothek vor Load in neue schreiben. Defekte Dateien/Schreibschutz sichtbar erhalten; Bibliotheks-Settings auf 64 KiB begrenzen.

## Relevante Regressionen
- YAML-Roundtrip/Besitzkonflikt/Symlinks/externe Änderung, Tags nach Import und SQLite-Verlust, Kommentar-/Transkriptzuordnung, Abbruch beim Neuaufbau und unveränderte Quellen.
- Preferences mit temporären Benutzerwurzeln: Locator-Abwärtskompatibilität, MRU, verzögerte Antworten, Bibliothekswechsel und letzter gespeicherter Stand.
