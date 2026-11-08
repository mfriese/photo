# Vertieftes Architekturreview – 08.11.2026

Geprüft wurden Domänenmodelle und Invarianten, Cargo-Abhängigkeiten, Command-Verarbeitung, Import und Quellschutz, Duplikatfreigabe, EXIF-Speicherung, Preferences, Bibliotheksscan, Bild-/Videoladen, Audio-Schnellstart, FFI-Lebenszyklen und die zugehörige Präsentation. Ziel war die Korrektur konkreter Entwurfs- und Betriebsprobleme, nicht zusätzliche Schichten ohne Nutzen.

| Befund | Korrektur | Nachweis |
|---|---|---|
| Schnelle Bibliothekswechsel konnten parallele Vollscans hinterlassen | CollectionSession mit einem aktiven Scan, neuester wartender Anfrage und kooperativem Repository-Abbruch | Blockierter Scan, schnelle Wechsel, veraltetes Ergebnis, Workerfehler und Wiederanlauf getestet |
| Videoübersichten konnten bei schnellen Wechseln Decoderaufträge ansammeln | StoryboardSession begrenzt aktive Arbeit und verwirft alte Ergebnisse | Blockierte Dauerabfrage, übersprungene Zwischenanfrage und ausschließlich aktuelle Frames getestet |
| Importabschluss und Refresh gehörten teilweise zur UI | ImportSession übernimmt Zustand, Abbruch, strukturierte Quittierung und Refresh | Bestätigte Kopie vor simuliertem Worker-Absturz wird genau einmal quittiert; Prüflauf und Abbruch getestet |
| Unbegrenzter Import-Ereignisrückstau und fehlender Job-Drop-Abbruch | Kanal mit 128 Plätzen, kooperativer Abbruch beim Freigeben | Import- und Sitzungstests einschließlich unveränderter Quellen |
| Widersprüchliche Bildmaße konnten UI-/Decoderfehler verursachen | Private Felder und validierender DecodedImage-Konstruktor | Nullmaße, falsche Pixelzahl und Größenüberlauf abgewiesen |
| Galerie hielt volle Bildauflösungen und wachsendes Cachevolumen | Separate 256/512/1024-Pixel-Rastervorschauen und 4096-Pixel-Großansicht; 256-MiB-/1024-Einträge-Budget, Freigabe alter Texturen | Größen-/Orientierungstest mit unverändertem Original, Cachebudget, Generationen und echter egui-Texturfreigabe getestet |
| Preferences konnten bei langsamem Store Snapshots ansammeln | Ein ersetzbarer wartender Snapshot; kein Storeaufruf unter Sperre | 1000 Änderungen während blockiertem Schreiben führen zum letzten Stand und vollständigem Shutdown-Flush |
| Persistente Command-IDs lagen in der UI; Foto-Dateinamenregeln waren doppelt | Command.id in der Domäne; gemeinsamer CaptureTime-Dateinamenparser | Bestehende Kürzel-, Kollisions- und Datumsregressionen bestehen |
| Kernabhängigkeiten waren nur durch Konvention gegen zusätzliche Adapterbibliotheken geschützt | make check-architecture mit Cargo-Metadaten und getesteter Freigabeliste | Zulässige Richtung akzeptiert, GUI-/Adapterabhängigkeiten und fehlende Kernpakete abgewiesen |

Die Produktionsverdrahtung bleibt in main. Fachliche Auswahl, Benennung, zeitliche Reihenfolge und Kollisionsentscheidungen liegen innen; Dateisystem, Decoder, Plist, EXIF und macOS-Fenstersteuerung außen. Duplikatentscheidungen beruhen weiterhin auf vollständigen Pixeln, nicht auf den neuen begrenzten Darstellungsvorschauen. Neue Cachevarianten werden nach erfolgreichem EXIF-Schreiben gemeinsam invalidiert. Originaldateien werden durch dieses Review nicht verändert.

## Prüfstand beim Abschluss

- 104 Rust-Tests im Workspace erfolgreich, Kern zusätzlich separat geprüft.
- cargo fmt --all --check, Clippy mit -D warnings und Workspace-Build erfolgreich.
- 10 Python-Tests für Architekturpolicy und Paketierung erfolgreich; keine echte Systeminstallation.
- Cargo-Metadatenprüfung sowie native Audio-/Videoregressionen erfolgreich.
- Keine manuelle Sichtprüfung oder neue Screenshots behauptet.
- Auf Wunsch des Nutzers zum Abschluss gebracht; nach diesen Reviewänderungen wurde noch kein neues Installationspaket gebaut.

## Verbleibende technische Grenzen

UI und Infrastruktur teilen weiterhin eine Desktop-Crate; Cargo sichert die inneren Grenzen, die äußere Modultrennung wird reviewt. Manche Adapter liefern Stringfehler und der Scanport eine Fehleranzahl statt vollständiger Einzelreports. Ein laufender OS-/Decoderaufruf ist nicht sofort abbrechbar. Der Cachebudgetwert begrenzt gespeicherte Galerie-RGBA-Daten, nicht temporäre vollständige Dekodierungen oder den gesamten Grafiktreiberspeicher. Die Großansicht ist auf 4096 Pixel begrenzt; Originale und Pixelduplikatvergleich bleiben voll aufgelöst. Vollständige Importprotokolle wachsen mit dem Lauf. Dateiprüfungen ersetzen keine systemweite Sperre gegen gleichzeitige Fremdänderungen. Audio- und Formatgrenzen bleiben in README dokumentiert. Aus diesen bewusst benannten Grenzen wird keine Behauptung absoluter Fehlerfreiheit abgeleitet.


## Nachprüfung: Schnellstart und Audio-Priorität (8. November 2026)
Der betroffene Ablauf wurde erneut durch Domäne, Anwendung, Ports, Adapter, native Wiedergabe und Composition Root geprüft. Zeitstempelberechnung und vorläufige Gewichte sind Domänenregeln. Sitzungen steuern Namensindex, Nachladen, Generationen und Priorität. Scheduler-Abhängigkeiten wurden aus den Metadatenports herausgehalten und als expliziter Anwendungsdienst injiziert. Galerieklassifizierung und gründliche Importvalidierung sind getrennte lesende Adapterpfade. Foto-Metadaten für unbekannte Namen haben einen begrenzten Kanal mit explizitem Ende und Fehlerbehandlung. Die native Dauerkorrektur verändert weder PlayerItem noch lokale Position.

Zusätzliche Regressionen prüfen 10.000 benannte Fotos bei blockierter optionaler EXIF-Suche, unmittelbare Diary-Daten ohne Metadatenzugriff, Audio-Priorität samt Freigabe, Galerieklassifizierung ohne Codecprobes und stabile vorläufige Gewichte. Verbleibende Grenzen: kein harter Echtzeit-Scheduler, nicht sofort abbrechbare OS-Aufrufe, dateinamenbasierte Kontextdaten statt vollständigem EXIF-Abgleich benannter Fotos, Snapshot der bereits geladenen Sammlung und kein persistenter Dauercache. Die ursprünglichen Prüfergebnisse oben sind historisch; den aktuellen Testumfang nennt der zugehörige Änderungsbericht.
