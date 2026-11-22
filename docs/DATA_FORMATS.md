# Dauerhafte Datenverträge

Stand der ersten eingefrorenen Basis: 22. November 2026. Diese Zusage beginnt bei den heute gültigen Formaten; frühere verworfene Prototypen sind nicht eingeschlossen.

## Versionsregister

| Format | Version | Maßgeblich | Referenzen |
| --- | --- | --- | --- |
| Medien-YAML | 3 | Tags, Kommentar-UUID/Audiopfad, Datumskorrektur, Transkript mit Segmenten und Quellenstand, Fehlermarker | `tests/fixtures/formats/media/v3/` |
| Bibliothekseinstellungen | 1 | Einstellungen und Tastenkürzel | `tests/fixtures/formats/settings/v1/` |
| Betriebszustand | 1 | Transkriptionsmodus running/paused/stopped | `tests/fixtures/formats/runtime/v1/` |
| Lokaler Bibliotheks-Locator | 1 | letzte Bibliothek, optionale Historie | `tests/fixtures/formats/locator/v1/` |
| Globale Transkriptionseinstellungen | 1 | Modell und Sprache | `tests/fixtures/formats/transcription/v1/` |
| Globale Geodateneinstellungen | 1 | Aktivierung und optionale SHA-256 | `tests/fixtures/formats/geodata/v1/` |

Originalmedien werden durch Formatupgrades niemals verändert. SQLite und heruntergeladene Modelle/Geo-Daten sind rekonstruierbar und keine zusätzliche maßgebliche Quelle. Ein Schemawechsel muss den Index aus den erhaltenen Quellen neu aufbauen können; ein inkompatibler Index darf nicht mit erfolgreichem leeren Bestand verwechselt werden.

## Unveränderliche Historie und Prüfungen

Die Fixtures sind synthetisch, enthalten keine echten Bibliotheksdaten und werden ausschließlich in temporäre Verzeichnisse kopiert. `baseline-2026-11-22.json` versiegelt die Referenzbytes mit SHA-256. Bestehende versionierte Fixtures und bestehende Baseline-Manifeste niemals ersetzen, normalisieren oder nach einem fehlschlagenden Test neu erzeugen. Zusätzliche Fälle sind erlaubt, bestehende müssen erhalten bleiben. Für neue Versionen neue Verzeichnisse und zusätzliche Manifeste anlegen.

`make check-formats` prüft die versiegelten Bytes und führt die `frozen_`-Kompatibilitätstests aus. Diese Tests gehören auch zur normalen Rust-Suite. Sie verwenden Produktionsleser; Medien, Settings und Runtime werden zusätzlich semantisch nach erneutem Schreiben verglichen. XML-/YAML-Whitespace ist kein Formatvertrag, die Referenzdateien selbst bleiben dennoch bytegenau unverändert. Bestehende Fehler-, Revisions-, Symlink- und Wiederaufbautests bleiben verbindlich.

## Verfahren für jede Erweiterung

1. Betroffenes Format und alle bisherigen Versionen im Register bestimmen. Feldbedeutung, Pfadidentität, Einheiten, optionale Werte und Defaultregeln dürfen nicht umgedeutet werden.
2. Bei unbekannten Feldern lehnen heutige Leser die Datei absichtlich ab. Deshalb benötigen auch additive persistierte Felder eine neue Version, sofern ältere Programme sie beim Schreiben verlieren könnten. Unbekannte Versionen/Felder ablehnen und Quelldatei erhalten. Kein stilles Zurücksetzen auf Defaults.
3. Neue Version mit minimalen und vollständig belegten Fixtures ergänzen. Für jeden historischen Ausgangsstand einen Test hinzufügen: alter Leserstand → Upgrade → aktueller Leser → Speichern → erneut Lesen. Alle essentiellen Werte und Beziehungen erhalten; wiederholtes Upgrade bleibt wirkungslos.
4. Vor Freigabe einen expliziten, versionierten Upgradeplan dokumentieren: Quell-/Zielversion, Defaults, Validierung, Konfliktbehandlung, Platzbedarf, Sicherung, Abbruch und Wiederaufnahme. Erst validieren und sichern, dann atomar und revisionsgeprüft veröffentlichen; Index erst anschließend aktualisieren. Ein Fehler darf den letzten gültigen Stand nicht zerstören.
5. Historische Fixtures müssen dauerhaft über den unterstützten Upgradepfad lesbar bleiben. Downgrades sind nicht zugesagt: ältere Programme müssen neuere Daten sicher ablehnen. Kein Upgradepfad ohne Tests und keinen Versionswechsel ohne neue Fixtures freigeben.

Aktuell existiert kein Formatwechsel und daher keine künstliche Migration. Der heutige Leser bildet den Identitätspfad der ersten Basis. Die erste neue Formatversion muss vor ihrer Freigabe den ersten echten Upgrader ergänzen. Globale Preference-Fixtures prüfen derzeit Lesen und Quellschutz; ihre Schreib-/Upgradepfade müssen bei der ersten Formatänderung zusätzlich mit historischen Fixtures abgesichert werden.

## SQLite nach einer Quellformatmigration

Nach einer Migration maßgeblicher YAML-Daten wird SQLite aus den migrierten Quellen neu aufgebaut, nicht parallel fachlich migriert. Sonst entstehen zwei konkurrierende fachliche Upgradepfade. Den Neubau getrennt vorbereiten, Vollständigkeit und Beziehungen prüfen und erst dann veröffentlichen. Quellenrevisionen gegen Änderungen während des Aufbaus prüfen. Bei Abbruch/Fehler den bisherigen Index und alle Quellen erhalten; den alten Index nach erfolgreicher Quellmigration nicht als aktuellen Stand ausgeben. Wiedereinstieg muss den erforderlichen Neubau erkennen und fortsetzen/neu starten können. Veröffentlichung und Wiederaufnahme sind bei der ersten echten Migration durch Fehler-/Abbruchtests abzusichern.

Eine reine Änderung am SQLite-Schema ohne Quellformat-/Bedeutungsänderung darf einen separaten technischen, transaktionalen Schemaschritt erhalten. Dieser benötigt eigene Tests und darf niemals essentielle Daten ausschließlich in SQLite voraussetzen. Falls das Schema nicht sicher überführt werden kann, Quellen erhalten und einen kontrollierten Neubau anbieten. Das gilt ebenso für den separat rekonstruierbaren GeoNames-Index.

### Entscheidung je Tabelle

Bei jeder SQLite-Änderung betroffene Tabellen und ihre Indizes, Fremdschlüssel und abhängigen Projektionen explizit bestimmen. Kleine rein technische Änderungen (z. B. zusätzlicher Index oder eine aus vorhandenen Werten eindeutig befüllbare Spalte) dürfen gezielt per versioniertem SQL-Schritt migriert werden. Nur abgeleitete Tabellen mit geänderter Berechnungslogik dürfen gezielt neu aufgebaut werden, sofern ihre Abhängigkeiten ebenfalls konsistent aktualisiert werden. Eine fachliche Quellformatmigration löst weiterhin den vollständigen Bibliotheksindex-Neuaufbau aus.

Eine gemeinsame Datenbank-Schemaversion ordnet die technischen Schritte; nicht vorsorglich voneinander unabhängige Versionszähler je Tabelle einführen. Tabellenbezogene Schritte in einer definierten Reihenfolge und transaktional veröffentlichen. Fixtures der alten Schemastände, Schema-/Inhaltstests sowie Fehler-/Rollbacktests vor Freigabe ergänzen. Unbeteiligte Tabellen dürfen ihre Daten behalten; bestätigte, nicht rekonstruierbare Benutzerdaten dürfen niemals in solche Migrationen verlagert werden. Nach Abbruch entweder alter konsistenter Stand oder vollständig neuer Stand, kein Mischzustand.
