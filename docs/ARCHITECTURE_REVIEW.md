# Architekturreview – aktueller Stand

Stand: 17. November 2026. Dieser Bericht ersetzt die historische Sammlung früherer Reviewnotizen mit inzwischen überholten YAML-, Album- und Kompatibilitätsangaben.

## Umfang und Einordnung

Geprüft wurden die Cargo-Abhängigkeiten, Composition Root, Domäneninvarianten, Portverantwortlichkeiten, Verarbeitungsplanung, Geo-/Transkriptionsabläufe und Worker-Lebenszyklen. Die vollständige Testsuite prüft zusätzlich Import/Quellenschutz, Papierkorb, Kommentaraufnahmen, Tagkonsistenz, SQLite-Neuaufbau und native Schnittstellen anhand isolierter Fixtures. Der vertiefte Code-Review konzentrierte sich auf die zuletzt erweiterten Geo-, Filter- und Verarbeitungsabläufe; daraus folgt keine Garantie der Fehlerfreiheit jeder Zeile oder Plattform.

Die Schichten sind grundsätzlich korrekt getrennt:

- **Domain:** Media, LibraryFile, TagCatalog, Comment/RecordedAt, Recording/DiaryTimeline, Transcript/SourceVersion und Coordinates bilden Identitäten, Validierung und fachliche Regeln ab. Nur chrono als externe Abhängigkeit, kein unsafe.
- **Application:** Anwendungsfälle, Sitzungen, Planung, Abbruch und technische Ports. Nur die Domain als Abhängigkeit, keine SQL-, GUI-, Codec- oder Dateisystemimplementierung; kein unsafe.
- **Infrastruktur:** SQLite, YAML/plist, Dateisystem, EXIF, Decoder, Whisper und native macOS-Funktionen implementieren injizierte Ports.
- **Präsentation:** Rendern, Eingaben und Darstellungszustand in Rust/egui; native Menüs und Player über technische Ports. Konkrete Produktionsadapter werden in main verdrahtet.

DDD wird pragmatisch angewendet. Lesekataloge und Fortschritts-Snapshots sind Projektionen/DTOs und benötigen keine künstlichen Aggregate. Validierte Identitäten und fachliche Invarianten verbleiben in der Domain; Anwendungs- und Technikzustände werden nicht zu Domänenregeln umgedeutet.

## Korrigierte Befunde

1. **Verarbeitungsplanung im UI:** Inventar, Deduplizierung und Fotoziel eines Kommentartranskripts liegen nun in application/processing. Fremde/ungültige Pfade werden sichtbar abgelehnt. Die UI überspringt sie nicht still.
2. **Unvollständige Freigabepolitik:** Readiness schützt fehlende Bibliothek, unvollständiges Inventar/GPS, Indexpflege, noch nicht geladene Kommentarzuordnungen, Kommentarspeicherung und Audiopriorität. Auch ein erneutes Laden der Kommentarzuordnungen blockiert Inferenz. Gespeicherte Pause/Stopp-Zustände werden vor GPS-Start berücksichtigt.
3. **Zu breiter Geoport mit stiller Standardantwort:** LocationIndex ist ein eigener verpflichtender Leseport für die Navigation. Kein Fake- oder Produktionsadapter kann versehentlich einen nicht implementierten Index als erfolgreichen leeren Katalog melden.
4. **Doppelte Ortsauflösungsregeln:** PreparedSearch wird vom Ortspopup und der Bibliotheksprüfung verwendet. Bereitschaft, Cachehash, Suche, Abbruch und Veröffentlichung werden gemeinsam entschieden; die Quelle wird nur einmal pro Durchlauf vorbereitet.
5. **Verschluckter Datensatzfehler:** Ein fehlgeschlagener Geodatencheck wird als begrenzte data_warning angezeigt. Regulär nicht vorbereitete/deaktivierte Offline-Daten benötigen weiterhin keinen Download.
6. **Vermischte Fehlerklassen:** GPS-Zähler bilden eine vollständige Partition der geprüften Fotos. Fehler der Ortsabfrage/Veröffentlichung werden separat gezählt; gültige Koordinaten bleiben gültig.
7. **Veralteter Workerfehler:** Auch ein abgebrochener oder abgestürzter Ortslistenworker darf keine Fehlermeldung in die neue Bibliothek übertragen. Ein gescheiterter Refresh erhält den bestätigten Katalog und erlaubt Wiederholung.
8. **Mehrere lange Fehlermeldungen im kleinen Popup:** Die Kombination aus Fehlernavigation, Textnavigation und festen Controls konnte das Fenster überragen. Kleinere Textseiten auf kleinen Fenstern halten alle Inhalte seitenweise und die Steuerung erreichbar.

## Datenquellen und Rekonstruktion

Originalmedien bleiben bei Prüfung, Wiedergabe und Transkription unverändert. Ausdrückliche separate Schreibaktionen bleiben Import, bestätigter Papierkorb und aktiviertes EXIF-Schreiben.

Individuelle Medien-YAMLs Version 3 unter yaml/Jahr/Monat halten nicht rekonstruierbare Tags, Kommentarzuordnungen und bestätigte Transkripte/Fehlversuche. Einstellungen bleiben texteditierbare YAML/plist-Dateien; yaml/runtime.yaml enthält typisierten Betriebszustand.

yaml/library.sqlite (Schema 2) ist ein rekonstruierbarer Index/Cache: Tags, Kommentarprojektion, Transkriptionsjobs, Audiolängen und Geoergebnisse. Keine großen Transkripte oder Geo-Zwischenkopien in YAML. GeoNames cities500.zip und die globale places.sqlite bleiben appweit getrennt. Foto-GPS und Ortscache sind aus Originalbild und vorbereiteten Quelldaten neu berechenbar.

Alte Gesamt-YAMLs und SQLite-Schemata werden nicht migriert. Der ausdrücklich gestartete vollständige Neuaufbau verwendet aktuelle Einzelquellen und veröffentlicht nur einen vollständig aufgebauten, geprüften Index bei unverändertem Zielstand. Tests sichern Datenbankverlust, beschädigte Indizes, konkurrierendes Schreiben, Abbruch, Symlinks und unveränderte Quellen ab.

## Ergänzte Regressionen

- Inventar mit alten Fotozielen neuer Kommentare, doppelten Medien und fremden Pfaden.
- Freigabematrix mit unbekanntem Betriebsmodus, Pause/Stopp, unvollständigem Bestand, nicht geladenen Kommentaren und exklusiver Arbeit.
- Gemeinsamer Ortscache: gleicher Hash wird wiederverwendet, neuer Hash erzwingt Auflösung, Abbruch verhindert Veröffentlichung.
- Defekte Datenquelle und fehlschlagende Ortsabfrage bei weiterhin korrekt gezählten gültigen GPS-Daten.
- Abgestürzter alter Worker bei Bibliothekswechsel; fehlgeschlagener Refresh und erfolgreicher Retry ohne Verlust des bestätigten RAM-Katalogs.
- Mehrere lange Unicode-Fehler einschließlich Geodatenwarnung in sämtlichen Popup-Reitern und Betriebszuständen ab 360 × 400.

## Prüfungen und Grenzen

Pflichtprüfungen: cargo fmt --all --check, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace, cargo build --workspace, make check-architecture, make test-native und die Python-Scripttests. Die drei ausdrücklich optionalen Integrationsprüfungen mit externem Whisper-Modell beziehungsweise echten Quelldaten bleiben ohne deren Voraussetzungen ausgenommen. Kein Modell wird dafür automatisch geladen; kein echtes Mikrofon und keine Benutzerbibliothek werden verändert.

Verbleibende Grenzen sind bewusst dokumentiert: Mehrere YAML-Dateien und SQLite bilden keine gemeinsame Dateisystemtransaktion. Revisionsprüfungen und prozessinterne Sperren ersetzen keine systemweite Sperre gegen fremde Programme. Größe/mtime sind keine kryptografische Identitätsprüfung; bereits laufende Betriebssystemaufrufe sind nicht sofort abbrechbar. GeoNames liefert den nächsten Ort samt Codes, keine zuverlässige administrative Polygonzuordnung. Native Funktionen sind wesentlich auf macOS ausgerichtet. Signierung, Notarisierung, manuelle Sichtprüfung und reale Großsammlungs-/Langzeittests bleiben separate Release-Prüfungen.
