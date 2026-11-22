# Oberfläche und Bedienung

Nur bei UI-/Theme-/Navigationsänderungen lesen. Fachabläufe bleiben in Application; siehe [AGENTS.md](../../AGENTS.md).

## Gemeinsames Design
- Farben, Schriften, Abstände und Maße zentral in src/theme.rs; native Farbwerte daraus ableiten. Kontrastreiche lesbare Texte in allen Zuständen, keine dunklen Texte auf dunklem Grund oder gelben Warntexte ohne ausreichenden Kontrast.
- Default-Kürzel für bestehende Medienaktionen zentral in `application::preferences::bindings` pflegen. Fehlende Overrides erben Defaults, explizites `None` deaktiviert; unbekannte Aktionskennungen ablehnen. Das aktuelle Settings-Format enthält ein Kürzel pro Aktion; zusätzliche Kürzel pro Aktion benötigen eine versionierte Formaterweiterung. Keine neue Konfigurationsoberfläche ohne Auftrag.
- Medienaktionen aus dem bestehenden Command-Katalog beziehen; dieselbe Aktion überall über den Dispatcher ausführen, Verfügbarkeit über `Command::available_for` bestimmen und gespeicherte Tastenkürzel verwenden, keine parallelen festen Tastenpfade.
- Gemeinsame Symbolbuttons mit Tooltip/Accessibility, 34 × 34 Punkte (`TRANSPORT_EDGE`), helle Symbole, Aufnahme rot. Rahmen/Hover/deaktivierte Farben zwischen Player und Recorder abgleichen, keine unzuverlässigen Unicode-Glyphen hinzufügen.
- Mindestfenster 360 × 400, lange Pfade kürzen mit Tooltip. Keine horizontalen Scroller in Betrachter-Steuerung. Hörleisten gleich hoch wie Buttons, mit gemeinsamen Rahmen.
- Gewöhnliche Dialoge über theme::dialog zentrieren. OK-Hinweise initial erst nach unsichtbarer Größenmessung fokussieren, Enter bestätigt fokussierten Button. Destruktive Dialoge haben Abbrechen als sichere Vorgabe. Viewer und bewusst angeheftetes Aktivitätspopup behalten eigenes Layout.
- Datumsfelder zentral fein dosieren: DATE_FIELD_DRAG_SPEED = 0.05, etwa 20 Pixel pro Einheit, Tastatureingabe erhalten. Mit echten Zeigerereignissen testen, nicht nur Konstanten vergleichen.

## Navigation und Raster
- Shift allein halten zeigt im Hauptfenster ein schwarzes, rein informatives Tastenkürzel-Overlay ohne Scrollbereich. Höhe inhaltsabhängig bis 75 % der Fensterhöhe, bei vielen Einträgen zweispaltig und entsprechend breiter bis zum Fensterrandabstand; bei sehr kleinen Fenstern Schrift gemeinsam passend skalieren. Loslassen blendet es aus. Bei Texteingabe oder zusätzlichen Tasten nicht anzeigen. Liste aus aktivierten Menüeinträgen und gültigen Betrachteraktionen ableiten; konfigurierte Kürzel anzeigen, Randpositionen berücksichtigen. Native Player besitzen eigene Fensterkontexte; deren Kürzel nicht im Hauptfenster als aktiv ausgeben.
- Native Hauptmenüs nur muda: PhotoSuite, File, Edit, View, Window, Help; keine Ersatz-Menüleiste. Suche/Zoom/EXIF-Modus/Pfad nicht als zusätzliche dauerhafte Kopfbuttonzeile. Settings via ⌘,, Find via ⌘F. Tastenkürzel während Texteingabe und Konflikten sicher behandeln.
- Baum: Alles wie Jahreszeile, Jahre standardmäßig eingeklappt. Klick wählt auch beim Zuklappen; im Jahr direkt Monate, keine Audiozweige. Überschriften Jahre/Tags/Orte hervorheben, vertikaler Abstand; Scroller am rechten Spaltenrand, kompakte Monatszeilen.
- Tags unter Jahren alphabetisch untereinander, alle bibliotheksweit bekannten Tags, im Zeitraum nicht verfügbare ausgegraut und deaktiviert, ausgewählte deutlich markieren, UND-Verknüpfung; Reset rechtsbündig neben Überschrift. Keine Tagfilterwolke über Raster, keine Alben.
- Ortsstruktur bei Zeitraumwechsel erhalten, nicht verfügbare Orte mit Anzahl 0 ausgegraut und deaktiviert. Orte ODER-verknüpft, Länder ausgeschrieben, falls vorhanden Bundesland/Ort hierarchisch, alphabetisch und mit rechtsbündigen Medienzahlen. Alle Ebenen ausklappbar, Länder standardmäßig zu; Länderbedienung wie Jahreszeilen, Reset rechtsbündig.
- Fotowand virtualisiert, fünf Zoomstufen, vollständige Zeilen gleichmäßig über Breite. Optional quadratischer zentrierter Beschnitt; sonst Bild unverzerrt vollständig. Spinner zentral auf ein Drittel der kürzeren Bildkante begrenzen.
- Foto/Video nach wirksamem Datum absteigend, Gleichstand Pfad aufsteigend, unbekannt zuletzt. Nur sichtbare Indizes sortieren, keine synchronen Metadatenaufrufe oder Mutation stabiler Medienindizes. Caption Datum/Uhrzeit statt Dateiname; kompakte gleich hohe Tag-/Videobadges, Globus bei GPS.
- Mehrfachauswahl Cmd/Shift; Kontextmenü Tags/Papierkorb für Auswahl, Finder/Datum fürs betroffene Medium. Nach Datumskorrektur Alles erhalten oder zum passenden Zeitraum gleicher Genauigkeit folgen, markieren/scrollen, passende Filter erhalten. Einzeländerungen laden nicht die ganze Bibliothek neu.
- Fußzeile einzeilig: Medien gesamt/Fotos/Audios/Videos, danach weniger beständige Zustände. Startseite letzte fünf andere Bibliotheken ohne Dateisystemprobe beim Rendern.

## Betrachter und Recorder
- F schaltet die Großansicht bildschirmfüllend; Escape verlässt zuerst Vollbild. Pfeile links/rechts blättern, außer bei Texteingabe. Audiowiedergabe bei Fotos erhalten; Videos im Vollbild zunächst als Sechserraster zeigen; erst die konfigurierte Aktion „Video abspielen“ (standardmäßig Leertaste) startet den nativen Player und pausiert vorher laufendes Audio.
- Bild horizontal zentrieren, sämtliche Controls darüber. Datum/Uhrzeit neben Fenstertitel, keine 1/1- oder EXIF-Statuszeile. Drei dezente Gruppen: Navigation/Aktionen, Tags/Finder untereinander, Audio. Schmal: Audiogruppe als nächste volle Zeile.
- Audio oben Play/Kommentarspur/Jahresfenster; darunter Record unter Play, gleich hohe Kurve, Pegel, rechts Mikrofon und Zugang zum separaten Recorder. Kein Kommentar: Controls/Spur deutlich deaktivieren.
- Recorder: Titelzeile mit X, Geräte beim Öffnen der rechtsbündigen Combo aktualisieren. Play/Record-Pause/Stop links, Status dazwischen. Keine kurz flackernden Vorbereitungs-/Pausenmeldungen. Foto neben Beschreibung in voller Höhe, X/Häkchen zum Verwerfen/Speichern; Rückfrage mit Abstand und zentrierten Buttons Speichern/Verwerfen.
- Kurve im Pausezustand nicht strecken, langsamer Bildlauf, dichte physisch schmale Pegellinien, vertikale Mittellinie. Unter −18 dBFS grün, bis −6 gelb, darüber rot mit etwas Headroom; dezente horizontale Grenzlinien. Keine zusätzliche Zielbereich-Legende.

## Verarbeitungspopup und Settings
- Popup nur durch erneuten Klick auf Fortschrittsbutton schließen; interne Aktionen, Außenklick und Escape schließen nicht. Symbol Dokument/Spinner/Pause nach Zustand.
- Acht großzügige Kacheln, Titel links, Zähler direkt rechts davon auch eingeklappt, Status rechts. Fertige automatisch kompakt einklappen, aktive geöffnet, Fehler sichtbar; manuelles Öffnen erhalten. Wiederholen rechts unten im geöffneten Inhalt. Beschreibung/Ressourcen klein einzeilig unter Balken mit Tooltip.
- Reiter Übersicht/Fehler/Live-Text, kein separater Details-Reiter. Bei geringer Höhe Kachelliste scrollen, Gesamtsteuerung erreichbar: Pause↔Play, daneben Reload; Fehler-Retry gleich hoch. Lange Fehler/Texte seitenweise zugänglich, Dateiüberschriften im FIFO-Log fett.
- Settings: Allgemein, Datenbank, Transkription, Geodaten; globale Vorbereitung auch ohne Bibliothek. Speicherpfade sichtbar mit Tooltip/Finderzugang. Einheitlich eingerückte optische Gruppen.
- Import: Quelle und Ziel nebeneinander als Gruppen, darunter kompakte gleich hohe Tags in eigener Gruppe, keine mittige Trennlinie; Duplikatsuche separat im File-Menü.
- Bibliotheksstart wartet sichtbar auf langsame Datenträger, versucht erneut, bietet Abbrechen/andere Bibliothek und endet nach Zeitlimit leer mit „Bibliothek aktuell nicht erreichbar.“ Keine UI-blockierenden Zugriffe.

## Ressourcen und Prüfung
- Bilddecoder begrenzen (zwei aktiv, 32 wartend), Galeriekacheln 256/512/1024, Großbild maximal 4096, Galerie-RGBA-Cache 256 MiB/1024 Einträge; Großansicht mit getrenntem Budget von 128 MiB/1024 Einträgen, damit Navigation keine Rastervorschaubilder verdrängt. Keine egui-Aufrufe unter Cache-Lock; evizierte Texturen freigeben. Scans zuerst Jahre, dann priorisiertes gewähltes Jahr, begrenzte Batches.
- Relevante Layout-/Interaktionstests mit künstlichen Daten; kleine Fenster, lange Texte und deaktivierte Zustände beachten. Screenshots derzeit ausgesetzt; keine echte Bibliothek zur Sichtprüfung öffnen.
