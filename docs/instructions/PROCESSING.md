# Audio, Queue, Transkription und Geo

Nur bei Änderungen an diesen Abläufen lesen. Persistenzregeln: [PERSISTENCE.md](PERSISTENCE.md); Darstellung: [UI.md](UI.md).

## Schnellstart und serielle Wartung
- Erste Hörleistenanzeige beim Start/Bereichswechsel ausschließlich aus SQLite-Audioprojektion, ohne Dateigrößen-/Dauer-/EXIF-Runde. Fehlender Cache wird als unvorbereitet gezeigt; keine zweite unabhängige Vollprüfung im Player starten.
- application/processing gibt genau einen Wartungsschritt frei: Audiobestand → Dauern → Tag-/Kommentarprojektionen → Jahresblöcke → GPS/Orte → Modellprüfung/Transkriptionsplanung/Inferenz. Alle acht bestehenden UI-Schritte auch nach Abschluss erhalten. Keine parallele Wartung/Inferenz; aktuelle Aufnahme/Wiedergabe priorisieren.
- Queue bei Programm-/Bibliotheksstart automatisch aktivieren. Neue Benutzerpause in dieser Sitzung erhalten, auch vor erstem Inventar; typisierten Laufzustand vor Arbeit in runtime.yaml speichern. Wiederholungen bündeln, Worker vor Folgeschritt vollständig abholen, Generation/Root prüfen.
- Audiocache nur Lookup für vorhandenen Bestand, keine maßgebliche Dateiliste. Fehlende/geänderte Dauern ergänzen, bei Tageswechsel auch vorhandene prüfen und verschwundene Einträge entfernen. Jahresblöcke unter Alles als rekonstruierbare Summen speichern.
- SQLite-Start ohne erreichbare Audioquellen muss möglich sein; verzögerte Pflege, Pause/Retry, kein dauerhafter Ladezustand bei Kanalabbruch und seriellen Übergang bis Inferenz testen.

## Audio und Kontextfotos
- Diary nach Aufnahmezeit, Kommentare nach wirksamem Datum des zugehörigen Fotos einsortieren/filtern. Oberer Player und Jahresfenster teilen Zeitraum Alles/Jahr/Monat, eine Wiedergabesitzung; Navigation erhält Pause und Kategorie. Foto-Player spielt nur explizit zugeordnete Kommentare. Keine physischen Audio-Dateibewegungen für Wiedergabe.
- Jahresfenster: Tagebuch/Kommentare/Beide, Datum/Zeit der aktuellen Bezugsposition; Tabelle Dateiname, Bezugsdatum, Aufnahmedatum aufsteigend. Unter Alles pro Jahr proportionalen Block zeigen. Leere Auswahl pausiert und deaktiviert Controls.
- Native Wiedergabe lädt aktive Originaldatei; Layoutkorrektur erhält AVPlayerItem und lokale Position anhand Pfadidentität. Echte Dauer und vorläufiges Gewicht getrennt, keine Gewichte als Sekunden ausgeben. Gemessene Abschnitte proportional zu Dauer.
- Fünf Kontextfotos zuerst aus Dateinamenindex, auch vor Play bei bereiter Position. Mitte nächster Nachbar, links Vorgänger/rechts Nachfolger; keine Duplikate an Rändern. Nur Clipwechsel/beruhigten Seek aktualisieren, keine sekündliche Suche. Unbekannte Namen begrenzt nachladen.
- Höchstens zwei Kontextdecoder, Kante 384, zehn PNGs im RAM. Klick navigiert ohne Fokuswechsel im Hintergrund zum Foto und hebt es hervor, öffnet keinen Betrachter. Native Ansicht meldet nur Slot/Sitzung/Anfrage.

## Aufnahme
- Technischer Recorder/Export außen; CommentSession steuert Aufnahme/Pause/Vorhören/Ersetzen/Speichern. Ab Cursor nur ungespeicherten temporären Rest ersetzen; alten Stand bis erfolgreichem Export behalten.
- Kommentarname nach tatsächlichem Aufnahmestart, Foto-YAML verweist per GUID/Audiopfad. Erst Audio exklusiv bytegeprüft veröffentlichen, dann YAML revisionsgeprüft, dann SQLite. Indexfehler erlauben Retry ohne zweite Audiokopie.
- Wechsel zum nächsten Foto während aktiver Inline-Aufnahme: bisherige erfolgreich speichern, erst dann neue Aufnahme zum neuen Foto; kein stiller Verlust bei Fehler. Mikrofon/Geräte-/Berechtigungsfehler sichtbar.

## Whisper und Lebenszyklus
- whisper-rs als Cargo-Abhängigkeit; keine vendorten Whisper/GGML-Quellen, Bibliothekspatches oder eigene Engine-Buildlogik. Modelle global und ausdrücklich vorbereitet, keine Build-/Startdownloads. Fest versionierter Katalog, Größe/SHA-256 und Sprachkompatibilität prüfen; Settings erst nach bestätigter Vorbereitung committen.
- Ein Enginekontext unter exklusivem Mutex; begrenzte PCM-Fenster (5 Minuten), Text 2 MiB/50000 Abschnitte. Callbacks synchron geliehen, Panic-/Abbruchschutz an FFI, keine entkommenden Zeiger.
- Transkripte im jeweiligen Medien-YAML, Fotokommentare im YAML des Fotos, Schlüssel relativer Audio-/Videopfad. Quellgröße/mtime vor/nach Inferenz prüfen, gültige Texte bei Modellwechsel erhalten. Fremde YAML unverändert lassen und nur betroffenen Auftrag fehlschlagen lassen; andere Arbeit fortsetzen.
- Normales Beenden: neue Arbeit sperren, kooperativ abbrechen, begrenzte Kanäle während Warten leeren, alle Queue-Threads joinen, erst danach Whisper-/Metal-Kontext und Power-Leases freigeben. Ergebnisereignis allein ist kein Threadende. Shutdown idempotent, keine Leaks/Assertionsabschaltung/Zeitlimit-Freigabe.
- RAII verhindert automatischen Idle-Systemschlaf nur während aktiver Arbeit; Display-Sleep zulassen, manuellen Schlaf/Zuklappen nicht als verhindert darstellen. Aufnahme hat Priorität. Live-Log begrenzt auf letzte 16 Segmente plus Dateiüberschriften, Generationen schützen.

## Offline-Geo
- GeoNames cities500 global neben appweiten Ressourcen vorbereiten, ZIP/SQLite-Bereitschaft prüfen, nur ausdrücklich herunterladen/reparieren. Netzwerk/ZIP/SQL hinter Ports, keine Render-I/O oder Tokens nötig.
- GPS aus Originalen, Ortsdaten aus vorbereiteten GeoNames rekonstruieren; nur SQLite-Cache, keine zusätzlichen Geo-YAMLs. Gültige Koordinaten, fehlende Daten, Prüf-/Auflösungsfehler getrennt behandeln.
- Standortpflege vor Inferenz, automatische Fortsetzung nach GPS sicherstellen. Ortsindex als eigener Leseport; Fehler nicht als erfolgreicher leerer Katalog verbergen. Alte Kataloge bei Refreshfehler behalten.
- Offline-/temporäre Tests für Modelltransport, Integritätsfehler, Retry, Rootwechsel, Indexverlust und Queuefortsetzung. Echte Inferenz/Metal-/Netztests nur ausdrücklich und mit vorhandenen Modellen/Projektfixture, keine Downloads großer Modelle in Standardtests.
