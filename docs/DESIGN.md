# PhotoSuite – Galeriedesign

## Ziel

Eine kompakte Fotoanwendung: Bilder dominieren, Bedienung und Dateinamen bleiben gut lesbar. Keine platzraubenden Dekorationszeilen. Deutsche Beschriftungen bleiben verständlich. Keine Cloud, Tracking- oder Kontofunktionen durch die Gestaltung suggerieren. Der UI-Technologie-Stack bleibt Rust mit egui/eframe.

## Gemeinsame Gestaltung

`src/theme.rs` ist die zentrale Quelle für Farben, Typografie, Widgetzustände und wiederverwendete Flächen. Das Galeriedesign setzt ausdrücklich `egui::Theme::Dark`; automatische macOS-Hell-/Dunkelwechsel dürfen nicht zwischen dunklen App-Flächen und hellen Standardstyles mischen. Keine Fachregeln oder Dateizugriffe ins Theme aufnehmen.

| Rolle | Wert |
| --- | --- |
| Bildfläche | `#13161c` |
| Werkzeugleiste, Seitenleiste, Dialoge | `#1b1f27` |
| Erhöhte Bedienelemente | `#242a34` |
| Dezente Kontur | `#313946` |
| Primärer Text | `#f2f5fa` |
| Sekundärer Text | `#d6dee9` |
| Akzent / Fokus | `#8bb5ff` |
| Auswahlfläche | `#2b4367` |

- Überschriften 20–22 Punkte, Fließtext 14, Buttons 13, Metadaten und Hilfstexte mindestens 12. Keine winzigen 10-/11-Punkte-Beschriftungen.
- Inhaltsränder 12, Seiten-/Werkzeugleisten 8, Dialoge 10 und Optionsgruppen 8. Basisabstände 8 horizontal und 4 vertikal; Fotowand 8 × 6. Keine zusätzlichen Abzeichen-/Bereichszeilen über dem Raster.
- Radius 5 für Widgets, 8 für Fotos, 6 für Optionsgruppen, 8 für Dialogfenster. Fotos nicht verzerren; kleine Rundungen dürfen nur die äußeren Ecken beschneiden.
- Dezente Konturen, keine dekorativen Verläufe oder schweren Schatten auf Fotokacheln. Akzent nur für Auswahl, Fokus und Hover, mit erkennbaren deaktivierten Zuständen.
- Der Kontrasttest sichert mindestens 7:1 für die verwendeten primären/sekundären Texte auf ihren vorgesehenen Flächen. Sekundärtext und Suchfeld-Hinweise erhalten eine explizite helle Farbe statt standardmäßig reduzierter Transparenz. Auch geöffnete Menüs und deaktivierte Elemente bleiben lesbar. Tooltips ergänzen Beschriftungen; Farbe allein darf einen Schreibmodus nicht erklären.

## Ansichten

- **Kopfbereich:** App-Icon und Name, kompakte umbrechende Aktionen mit Suche und Filter. Bibliothekspfad in eigener ruhiger Zeile mit vollständigem Tooltip; zusätzlich im Fenstertitel.
- **Seitenleiste:** abgesetzte Fläche, Gesamtansicht, absteigende aufklappbare Jahre, standardmäßig eingeklappt. Manuelles Aufklappen innerhalb der Sitzung respektieren. Monate als deutsche Namen darstellen; originale Gruppenschlüssel beibehalten. Kein zweiter fachlicher Filter in der UI.
- **Fotowand:** bildbetonte, gleich große Kacheln, fünf Größenstufen, 8 Punkte horizontaler Abstand und gleichmäßig gefüllte vollständige Zeilen. Letzte Zeile unverändert. Dateinamen mit mindestens 12 Punkten und gekürzt; Videoabzeichen auf dunkler Fläche. Hover-Kontur ohne Größenverschiebung. Beide Vorschauformen zentrieren.
- Allgemeine Bedienelemente 24 Punkte hoch, Navigationseinträge 26, Großansichtsbuttons 28. Größere Schrift nicht durch größere Ränder kompensieren.
- **Großansicht:** Dateiname oben, anschließend genau zwei feste Kontrollzeilen: Navigation und externes Öffnen, darunter Bild-/Videoaktionen und Status. Kein horizontaler Scroller. Gleiche Höhe bei Foto/Video, EXIF an/aus und laufendem Speichern. Beschriftungen bei Platzmangel kürzen; vollständige Tooltips. Foto horizontal zentriert auf dunkler Bildfläche.
- **Dialoge:** gemeinsame Optionsgruppen mit identischen Einrückungen. Lange Inhalte vertikal scrollbar, Pfade gekürzt mit Tooltip. EXIF-Schreibmodus ausdrücklich und kontrastreich beschriften.
- **Import:** standardmäßig eine kompakte Zeile pro Datei mit Entscheidung, Namen und relativem Ziel. Vollständige Pfade, Datum und Datumsquelle im Tooltip; Fehler kontrastreich markieren. „Alle Schritte anzeigen“ bietet die vollständigen Arbeitsschritte mit erhaltenen Zeilenumbrüchen und umbrochenen Pfaden. Kopieren folgt der gewählten Ansicht.
- **Fußzeile:** genau eine Zeile für Modus, Status und Fehler. Kürzen statt umbrechen; vollständige Meldung im Tooltip.
- Mindestfenster 360 × 400. Auf schmalen Fenstern bleiben Aktionen erreichbar. Kein abgeschnittener Text ohne Tooltip und kein horizontaler Scroller in der Großansicht.

## Pflege und Nachweis

Bei wesentlichen sichtbaren Änderungen README und betroffene Screenshots im selben Änderungsstand aktualisieren. Das gilt ebenso bei neuen Funktionen, geänderter Installation oder geändertem Sicherheits-/Schreibverhalten. Kleine unsichtbare Korrekturen benötigen keine identischen neuen Screenshots.

Screenshots sind echte Aufnahmen der aktuellen Anwendung, keine Mockups. Nur separate Demodaten und isolierte Preferences verwenden; keine privaten Fotos, persönlichen Verzeichnisse oder Zugangsdaten veröffentlichen. Bildquellen dokumentieren. Relative Markdown-Pfade mit exakter Groß-/Kleinschreibung verwenden, damit GitHub sie auf Branches und Tags auflösen kann. Screenshots gehören unter `docs/screenshots/`, Demo-Originale und Build-Dateien nicht ins Git-Repository.

Automatisierte Layout-/Kontrasttests ergänzen die Sichtprüfung. Ein Regressionstest sendet helle und dunkle Systemtheme-Ereignisse und prüft die tatsächlich gerenderten Textfarben von Jahreszahlen, Überschriften, Menüs, Buttons und Eingabehinweisen. Eine manuelle Sichtprüfung nur als durchgeführt angeben, wenn die laufende App tatsächlich betrachtet wurde. Reproduktion unter [screenshots/README.md](screenshots/README.md).

## Audio-Diary

Unter jedem eingeklappten Jahr einen „Diary“-Eintrag anbieten. Audio-Kacheln analog zur Fotowand: fünf Zoomstufen mit 10/8/6/5/4 Zielspalten, bei Platzmangel weniger; vollständige Zeilen füllen die Breite. Stilisiertes Audio-Symbol vektoriell zeichnen, keine Wellenformanalyse behaupten. Unter jeder Kachel ausschließlich Datum und darunter Uhrzeit in mindestens 12 Punkten; Dateiname und Datumsquelle nur im Tooltip. Metadaten im Hintergrund lesen und bei Aktualisierung neu bestimmen.

Der native Audioplayer verwendet eine einzelne segmentierte Jahresleiste mit heller Positionsmarke. Segmentbreiten beim Schnellstart vorläufig nach Dateigrößen, danach pro geprüftem Clip nach echter Dauer korrigieren; keine gleich breiten Titel und keine Playlist. Ungeprüft gedämpft grau, geprüft in den bisherigen Blautönen, Prüfungsfehler rot. Zustand zusätzlich textlich in kompakter Fortschrittszeile und Tooltip benennen. Vorläufige Aufnahmezeiten aus Dateinamen verwenden, unbekannte ausdrücklich als „Datum wird geprüft“ kennzeichnen. Sichtbare Zeitangaben ausschließlich aktuelles Aufnahmedatum und Aufnahmeuhrzeit, berechnet aus Dateibeginn plus lokaler Dateiposition. Dateiquellen bleiben im Tooltip zugänglich. Keine zusätzlichen Dateinamen oder verstrichenen Gesamtdauern in der Hauptanzeige.

Buttons benutzen klare Textbeschriftungen statt Unicode-Dekorationen wie Drehsymbolen oder Musiknoten. Grafische Symbole als Vektor zeichnen. Tests prüfen echte UI-Texte auf Glyphenabdeckung.

Unter der nativen Audio-Jahresleiste stehen fünf kleine Fotoplätze mit gleichmäßigen Abständen über exakt die Spurbreite. Mitte 28 % breiter und höher als Nachbarn, zusätzlich dezenter blauer Rahmen. Seitenbilder bleiben kleiner. Alle fünf Bilder werden zentriert und unverzerrt so beschnitten, dass sie die Kachel vollständig ohne zusätzliche Balken ausfüllen. Unter jedem Foto stehen sein Aufnahmedatum und darunter seine Uhrzeit in heller 12-Punkte-Schrift. Datumsquelle/Pfad zusätzlich im Tooltip; keine generische Erfolgszeile. Fehlende Nachbarn und Fehler erhalten klare Platzhalter, Lade-/Fehlerhinweise eine kompakte Statuszeile. Ein Klick auf ein geladenes Bild oder seine Beschriftung navigiert die Fotowand im Hintergrund zum gesamten Jahr, zentriert das Foto möglichst im sichtbaren Bereich und markiert es dauerhaft mit einem blauen 3-Punkte-Rahmen. Such-/Videofilter zurücksetzen, damit es sichtbar ist. Keine Großansicht öffnen und keinen Fokuswechsel anfordern; Audio läuft weiter. Bei App-Deaktivierung wird dieses ausgeblendet. Während einer neuen Suche sind veraltete Fotos nicht anklickbar. Fotosuche erst nach Wiedergabestart, Wechsel bei Clipwechsel oder bewusstem Sprung; die laufende Uhrzeit bewegt die Fotoauswahl nicht sekündlich.

## Importquellen aufräumen

Separater Button „Importquellen in den Papierkorb …“ nach einem tatsächlichen Import. Eigener scrollbarer Dialog mit einer klaren Zeile/Gruppe je Datei: Importentscheidung, Quelle, Ziel und Prüfergebnis. Lange Pfade kürzen und im Tooltip vollständig zeigen. Freigegebene und gesperrte Zeilen ausdrücklich beschriften; keine reine Farbcodierung. Bestätigung erst nach vollständiger Prüfung und mit exakter Anzahl freigegebener Quellen. Abbruch und Schließen bieten; Abschluss-, Fehler- und unklare Zustände sichtbar erhalten. Keine vorausgewählte automatische Quellenlöschung.
