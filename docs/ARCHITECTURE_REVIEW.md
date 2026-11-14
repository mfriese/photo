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

- **185 Rust-Tests** erfolgreich: 100 Desktop, 62 Anwendung, 23 Domäne.
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
AudioScope/AudioKind und die Präfixklassifikation liegen in der Domäne. Die Anwendung verwendet dieselbe Mitgliedschaft für Anzeige, Namen-Cache, Datumskatalog und Wiedergabestart. Kommentare behalten ihre logische Namenszeit bei der Dauerprüfung. UI-Ansichtsschlüssel sind von physischen Schreibpfaden getrennt; nur der vorhandene Importpfad erzeugt neue Dateien unter Jahr/audio. Der Dateisystemadapter liest alte/neue Audioordner ohne synchrones Container-Probing. Die Regressionen prüfen getrennte/gemischte Bereiche, unbekannte Namen, Legacy-Kompatibilität, tatsächliche Playerprojektionen, Kommentarzeit, Navigation und unveränderte Dateien. Keine automatische Migration. Die Aufnahmefunktion ist separat unten geprüft.

## Nachprüfung vom 12. November 2026: kompakte Galerie
Die Tagwolke bleibt Projektion des vorhandenen Katalogs, ohne zusätzliche Speicher-/Filterregeln. Alle Tags sind unabhängig vom Album-Flag erreichbar; Konfiguration liegt im nativen Edit-Menü. Captions teilen die namensbasierte Domänenregel und die neue begrenzte MediaDates-Sitzung; der injizierte Leseport darf keine Import-Schreibaktionen ausführen. Ein aktiver Worker, 32 Anfragen/32 Ergebnisse, 8192 Cacheeinträge, Generation/Abbruch und Audio-Priorität begrenzen Ressourcen. Cachetreffer und Ladeindikator sind ohne vollständige Frame-Scans abrufbar. Voller Kanal plant einen späteren Versuch statt still hängenbleibender Ladebeschriftungen. Alte Bibliotheksergebnisse werden verworfen und Fehler negativ gecacht. Der echte EXIF-Adapter wurde gegen unveränderte Testdateien geprüft.

Medienstatistiken sind eine inkrementelle append-only Projektion, keine erneute Bibliothekssuche pro Frame. Inline-Audio nutzt unverändert denselben Transport, nun neben der Überschrift ohne Mehr-Menü; zusätzliche Aktionen laufen über den bestehenden Menüport. UI-Regressionen prüfen Tagwolke, Datumsformatierung statt Dateiname, horizontale Audio-/Datumsanordnung und beständige Zähler links vor Statusmeldungen. Bekannte Grenze: momentan laufende System-Metadatenaufrufe können nach Abbruch noch auslaufen; es gibt keinen persistenten Datumsindex. Keine manuellen Screenshots oder Sichtprüfung behauptet.

## Nachprüfung: kompakte Vorschau-Badges
Dateizuordnungen werden ausschließlich aus TagCatalog/OrganizationSession im Speicher gelesen; der Renderer erhält Labels und Bildgeometrie. Keine neuen Dateizugriffe, Schemaänderungen oder Trefferflächen. Typ- und Tag-Badges teilen Theme-Farbe/Padding. Der Layouttest prüft die tatsächlichen Glyphengrenzen gegen den Hintergrund auf identische 3-Punkte-Ränder sowie Bildgrenzen und Überlauf an drei Kachelgrößen. Die Kommentar-Aufnahme und ihr Referenzmodell sind im getrennten Ablauf unten beschrieben.

## Review: Audiokommentare
Domäneninvarianten schützen Referenzen, Zeitstempel, UUID und eindeutige Audiozuordnung. Native Capture, Dateioperationen und YAML bleiben außerhalb des Kerns. CommentSession steuert Workflow, Fehler/Wiederholen und asynchrone Lebenszyklen. main injiziert sämtliche Adapter einschließlich wiederverwendeter Audio-/Import-/Datumsports. Bezugszeit und tatsächliche Aufnahmezeit sind getrennt. Zugeordnete Kommentare werden über die Fotozeit in GalleryAudio, DiarySession und DiaryCatalog eingeordnet und gegen abweichende Audio-Metadaten gesperrt. Kein zusätzliches Duplikat-/Importregelwerk im Dialog.

Regressionen prüfen reale/logische Zeit, Indizes/Konflikte, Speicherfehler mit Retry ohne zweite Kopie, veraltete Bibliotheksladungen, Kollisionen, byteidentische Kopien, unveränderte Originale, YAML-Roundtrip/externe Änderungen/defekte Schemata/Symlinks und schmale Aufnahmeansichten. Nativer Test verwendet erzeugte Stille mit echter M4A-Kodierung, Finalisierung, Probe-Pause und Tempbereinigung, ohne Mikrofonzugriff. Verbleibend: reale Hardware/TCC-Interaktion nicht manuell geprüft; Absturz kann Tempdatei oder Audio ohne Zuordnung hinterlassen; kein globaler Mehrprozess-Lock; Pegelkurve statt samplegenauer Wellenform; Aufnahme nur macOS, kein Transkriptionsmodell integriert.

## Vertiefte Nachprüfung: Aufnahmezustände und Veröffentlichung
Die ursprüngliche Aufnahmeintegration hatte korrekte Cargo-Abhängigkeiten, aber öffentlich veränderbare Workflowfelder und Kollisionsorchestrierung im Dateiadapter. Beides wurde korrigiert: private Sitzungszustände mit lesenden Projektionen/validierten Aktionen; separater Anwendungsfall für Referenzprüfung, Namensplanung und Suffixauswahl; getrennte Ports für Katalogpersistenz und technische Medienoperationen. Ungültige Phasenbefehle werden vor dem nativen Aufruf abgewiesen. Discard aktualisiert Recorder und Sitzung gemeinsam. Nicht-Foto-Ziele werden vor Aufnahme abgelehnt, UUIDs fachlich normalisiert. Speicherworker halten die temporäre Aufnahme bei unerwartetem Sitzungsende bis zum Ende des Kopierauftrags am Leben.

Zusätzliche Regressionen prüfen diese Regeln ohne GUI, ungültige Referenzen vor jeglichem Adapterzugriff, Reihenfolge Validierung/Metadaten, Suffixauswahl in der Anwendung und case-unabhängige UUID-Identität. Gesamte Kernabhängigkeiten und Produktionspräsentation zusätzlich auf direkte Infrastruktur-/FS-/FFI-Zugriffe geprüft; Dateizugriffe in Integrationstests sind keine Produktionsabhängigkeiten. Dies ist keine formale Fehlerfreiheitsgarantie der Gesamtanwendung. Dokumentierte Grenzen (Mehrprozess-CAS, Absturzüberreste, macOS-Hardwaretest) bleiben bestehen.

Prüfabschluss dieser Nachprüfung: 198 Rust-Tests bestanden (104 Desktop, 67 Anwendung, 27 Domäne); Kern zusätzlich separat getestet. Cargo-Abhängigkeitsprüfung, Formatierung, Clippy ohne Warnungen, Workspace-Build sowie native Audio-, Video-, Menü- und Recorder-Tests bestanden. Kein Zugriff auf ein echtes Mikrofon und keine Benutzerfotos verändert.

## Aufnahmepegel und kompakter Transport
Darstellung (logarithmische Pegelskala, Zielmarken, gezeichnete Symbole, feste Transportzeile) von der vorhandenen Sitzungssteuerung getrennt gehalten. Mikrofonliste wird einmal pro Öffnen über den begrenzten Geräteworker aktualisiert. Native Stop-/Cancel-Guards berücksichtigen auch isRecordingPaused, damit eine Pause nicht die Finalisierung verhindert. Regressionen prüfen Pegelabbildung/Reserve, konstante Transportmaße, schmale 360×400-Dialoge und native Fortsetzen-/Stop-Abläufe mit simuliertem pausiertem Output ohne Mikrofonzugriff.

Abschluss: 200 Rust-Tests bestanden, Kern separat geprüft; Clippy, Formatierung, Workspace-Build, Cargo-Architekturprüfung und sämtliche nativen Tests grün. Reale Mikrofonaufnahme und manuelle Sichtprüfung nicht ausgeführt; Screenshots weiterhin zurückgestellt.

Langsamer Kurvenverlauf: begrenzte Messwerthistorie bleibt im Aufnahmeadapter; Live-Pegel unverändert bei 30 Hz, 960-Punkte-Verlauf über ungefähr 32 Sekunden. Der native Regressionstest prüft Erhalt aller Messwerte und Speicherlimit ohne Mikrofonzugriff.

## Aufnahme-Dialog: Dreizonenzeile und feine Kurve
Reine Präsentationsänderung ohne neue Fachregeln oder Dateizugriffe: Titel-X erhält denselben Schließschutz, Status und Mikrofon erhalten begrenzte getrennte Bereiche, Fotoformat wird über den bestehenden asynchronen Bildloader ermittelt. Geometrietests prüfen variable proportionale Fotobreite und physische Pixelstriche samt Mittellinie; schmale Dialogregression prüft Status/Mikrofon in derselben Zeile und das Entfernen der alten Textbuttons/Hinweiszeile.

Schließbestätigung: Layout bleibt Präsentation; automatisches Finalisieren vor Speichern liegt in CommentSession. Regressionen prüfen Abstände/nebeneinanderliegende Buttons sowie Speichern aus pausiertem Zustand mit wiederholbarem Zuordnungsfehler ohne zweite Kopie.

## Dichter Aufnahmeverlauf und Symbolabschluss
Footeraktionen verwenden unverändert request_save und bestehenden Schließschutz. Dropdown-Geometrie und Farb-/Pixelprojektion bleiben Präsentation; bounded Telemetrie enthält jetzt 960 statt 120 Messwerte, damit die 32-Sekunden-Zeitachse bei voller 30-Hz-Auflösung erhalten bleibt. Nativer Test prüft Erhalt jedes Messwerts/Speicherlimit; UI-Regressionen prüfen Peak-Erhalt beim Verdichten, lückenlose Pixelspalten und grün/gelb/rot. Keine neuen Medien-Schreibwege.

Abschluss der dichten Kurven-/Footeranpassung: 205 Rust-Tests, native Tests, Clippy, Formatierung, Workspace-Build und Architekturprüfung bestanden; keine echte Mikrofonaufnahme und keine manuelle Sichtprüfung durchgeführt.

## Audio-Cursor und abschnittsweises Ersetzen
Domänenwert AudioCursor sichert Positionsgrenzen; Sitzungsbefehle validieren Seek/Preview/Resume und sperren die Save-Übergänge. UI enthält keine Datei-/Schnittlogik. AVFoundation-Adapter finalisiert Abschnitte, separater comment_edit-Helfer erstellt den technischen Prefix-/Replacement-Export in neue eigene Tempdatei; Generationsschutz und bisheriger Arbeitsstand bleiben bis Erfolg erhalten. Fehler werden strukturiert gemeldet und stoppen automatisches Speichern.

Nativer Test benutzt zwei erzeugte Testtöne: tatsächliche Exportdauer, Frequenz des erhaltenen Präfixes und des Ersatzabschnitts, erhaltene Quelldatei sowie Rückfall bei ungültigem Abschnitt werden geprüft. Keine Mikrofonaufnahme im Test; Resume-Geräteöffnung wird durch einen Testadapter ersetzt. UI-Interaktionstest prüft Cursorwahl sowie Sperre während Aufnahme; Anwendungstests prüfen ungültige Positionen und fehlgeschlagenes Ersetzen ohne automatische Veröffentlichung.

Entfernen flüchtiger Aufnahme-/Pausentexte: reine Meldungs-/Darstellungsanpassung, keine Änderung an Befehlsfreigaben, Finalisierung, Fehlerbehandlung oder Speicherworkflow. Native Fehler-/Warnmeldungen werden weiterhin übertragen und angezeigt.

Pausierte Kurve: native Finalisierung erhält vorhandene Messwerte und den expliziten Fensterstart; Snapshot-Spanne bleibt konstant 32 Sekunden. Keine zusätzliche UI-Fachlogik oder Änderung am Audioinhalt. Nativer Regressionstest vergleicht Messwerte/Fensterstart vor und nach Finalisierung und pausiertem Ersatzexport.

## Getrennte Audioordner und unabhängige Kommentar-YAMLs
Neue Ziel-/Präfix-/Bereichsregeln in der Domäne, Veröffentlichungs- und Wiederaufbauworkflow in CommentSession, technische Codec-/Scan-/Dateioperationen in comments_fs. Bestehende verifizierte Veröffentlichung für zusätzliche Ordnertiefe wiederverwendet; keine ungeschützte Überschreib-/Rename-Alternative. Alle Adapterports einschließlich Datumsquelle, Kopierer und Arbeitspriorität in main injiziert.

Tests prüfen neue Pfade/Präfixe mit Legacy-Wiedergabe, byteidentischen Import, GUID und individuelle YAML ohne photo_taken_at, Wiederaufbau bei gelöschtem/defektem Index, Erhalt defekten Indexes, fehlende Einzeldateien als Rückfallfall, Vorrang vorhandener Einzeldateien, ignorierte AppleDouble-Begleiter, Abbruch, fremde Änderungen, beschädigte YAMLs und Symlinks. Rootindex wird erst nach vollständigem validiertem Scan und Revisionsprüfung ersetzt. Kein Test löscht oder verändert echte Benutzerdateien.

Grenzen: kein gemeinsamer Dateisystem-Commit für Audio und Einzel-YAML; nach Absturz davor kann Audio ohne Zuordnung bleiben. Einzel-YAMLs sind erst nach erfolgreichem Abschluss maßgeblich. Gesamtindex 32 MiB, Einzeldatei 64 KiB; Hintergrunddateiaufrufe nicht sofort abbrechbar; externe gleichzeitige Editoren bleiben durch Revisionsprüfung, nicht durch einen globalen Mehrprozess-Lock abgesichert. Fehlende Einzeldateien können aus dem Cache rekonstruiert werden, absichtliches Löschen erfordert künftig einen gemeinsamen Löschworkflow.

Baumauswahl-Hörleiste: Bereichsprojektion und vorgemerkter letzter Play-Auftrag liegen in DiarySession; UI rendert passenden Liveverlauf oder GalleryAudio-Entwurf. Regressionen prüfen Jahr/Monat/Audio als gemischte Ansicht, getrennte Kategorien, Kommentar zwischen zwei Diary-Dateien nach Bezugszeit und unveränderte Aufnahme-Reihenfolge nach Metadatenprüfung. Ein weiterer Test startet während alter Vorbereitung mehrfach neue Bereiche und prüft, dass ausschließlich die letzte Auswahl übernimmt. Native Transport-/Dateiports unverändert.

Abschluss Hörleisten-Navigation: 215 Rust-Tests (114 Desktop, 72 Anwendung, 29 Domäne), Kern separat, Clippy/Format/Build/Architekturprüfung und native Tests bestanden. Zusätzliche Mitgliedschaftsprüfung verwirft veraltete Live-Listen bei neu hinzugefügten oder neu zugeordneten Kommentaren.

Bereichsplayer-Symbol: Domänenwert AudioViewScope trennt Monatsansichten von Audioverzeichnissen, Timeline hält Monatsidentität. Alle Auswahl-/Startwege verwenden dieselbe Mitgliedschaft inklusive Foto-Bezugszeit von Kommentaren; embedded/window-Modus gehört in den gemeinsamen Anwendungsfall. Regressionen prüfen Monatsgrenzen, Kommentar mit anderem Aufnahmejahr, Jahresrückkehr, Moduswahl und Schutz gegen Übernahme einer fremden Monats-Timeline. Native Fensteröffnung für eine passende aktive Sitzung bleibt ShowWindow ohne erneutes Öffnen.

Native Playerbuttons: reine Symbol-/Layoutänderung mit bestehendem Transport; neue Foto-Aufnahmeaktion als sitzungsgebundener Intent über den frameworkfreien AudioPlayer-Port. Native Tests prüfen Zentrierung bei 440/740/1600 Punkten, gleiche Buttonmaße, Play/Pause-Bildwechsel, Record-Intent für den Mittelpunkt, deaktivierte/stale Anfragen und pausierte/ausgeblendete Wiedergabe. Präsentationstest prüft korrekten Aufnahmezielindex ohne Galeriewechsel oder Mikrofonstart und Ablehnen fremder Pfade.

Die Zentrierung ist über native CenterX-/Größenconstraints abgesichert; alle vier Buttons bleiben 40×32 Punkte groß. 219 Rust-Tests bestanden; die Aufnahmeaktion wurde mit nativen Testdaten und ohne Mikrofonzugriff geprüft.

Sichtbarkeitskorrektur Playercontrols: bisherige positive bottomAnchor-Konstante positionierte die Gruppe unterhalb des Fensters. Korrigiert auf −22. Der native Test prüft nun vollständige tatsächliche Buttonbounds im Contentbereich und unter den Fotos, zusätzlich zur horizontalen Zentrierung und gleichen Größen. Reine Präsentationskorrektur ohne Änderungen an Domäne, Transport oder Daten.

- Transport-Stilangleichung: ausschließlich native Darstellung (34-Punkte-Geometrie und feste kontrastreiche Symbolfarben); Anwendungsfälle, Ports und Domänenregeln bleiben unverändert. Native Layoutprüfung berücksichtigt echte Buttonbounds auch bei schmalem Fenster.

Theme-/Navigations-/Tag-Abgleich geprüft: zentrale Darstellungstokens ohne Abhängigkeit des Kerns; native Palette aus Rust-Theme generiert. Normalisierung und Kommentar-Tag-Projektion in der Domäne, Ladevorgang/serialisierter Speichervorgang und verzögerter Refresh in Anwendungssitzungen, UI vermittelt nur Katalog-/Menüereignisse. Wartende Daten je Bibliothek begrenzt; Ladefehler können keine Zuordnungen löschen. Medien bleiben unverändert. Regressionen prüfen Unicode-Kleinschreibung, verlustfreies Zusammenführen, alle Tagaktionen, Bibliothekswechsel, idempotenten Tagabgleich samt Speicherfehler/Wiederholung, Refresh-Lebenszyklus, Alles-Navigation, deaktivierte Headercontrols und tatsächlich gerenderte native Buttonfarben einschließlich Rahmen/Hover/Klick.

Großansicht nachgeprüft: Foto-Kommentarauswahl wird von CommentSession aus dem indizierten Domänenkatalog abgeleitet; Wiedergabe bleibt im vorhandenen DiarySession-/AudioPlayer-Ablauf. UI enthält weder Dateifilterregeln noch Medienprobes. Mitgliedervergleich verhindert die Übernahme einer fremden Foto-Timeline, separater Jahrescache vermeidet Vollaufbau beim Durchblättern desselben Jahres. Gezeichnete Symbole nutzen gemeinsame Theme-Buttons; Bildbefehle bleiben zentral dispatcht. Tests prüfen fotoexakte Zuordnungen inklusive fehlender Kommentare/fremder Bibliothek sowie die echte Großansicht nach dem UI-Größenpass bei 360/600/1280 Punkten und konstante Zweizeilenhöhe.

Aufnahmebutton neben Play: reine Präsentationsänderung. Der gemeinsame Transport liefert einen Record-Intent an den bestehenden Dialogaufruf zurück. Vollständiger UI-Layouttest prüft die Nachbarschaft von Play/Record, die gemeinsame vertikale Mitte samt Spur/Jahresbutton und die Trennlinie bei schmalen und breiten Fenstern.

Drei Aktionsgruppen und Finder-Reveal: vorhandenen technischen Port um eine explizite Auswahlfunktion erweitert, Systemverhalten ausschließlich im injizierten Außenadapter. Kein Öffnen als stiller Ersatz für Reveal. UI-Integrationstest klickt den echten Finder-Button über einen Testadapter und prüft die exakte Datei; Öffnen wäre ein Testfehler. Geometrieprüfung bestätigt drei gleich hohe Gruppen, Tags/Finder untereinander und zentrierten Audiotransport bei 360/600/1280 Punkten. Adaptertest schützt absolute und unverändert als einzelnes Argument übergebene Pfade, ohne Finder zu starten.

## Fotozentrierte Kommentarquelle ohne Altformat-Unterstützung
Quelle ist genau ein Version-2-YAML je Foto; Gesamtindex Version 3 ist abgeleitet. Comment validiert Foto-Metadatenpfad und Audionamensdatum, Media verändert seine physische Gruppe nicht. Physische Gruppen bleiben unverändert; die logische Wiedergabezuordnung folgt dem Foto-Bezugsdatum. Der explizite Foto-Wiedergabefilter enthält sämtliche zugeordneten Kommentare, auch aus verschiedenen Aufnahmejahren, mit derselben Foto-Bezugszeit. Der Commit-Anwendungsfall in der Anwendungsschicht gibt den vollständigen gespeicherten Katalog zurück, damit Quelle und Cache auch bei veraltetem RAM-Stand übereinstimmen. RebuildBase enthält nur Revision/Warnung; keine Zuordnungen zur Rückgewinnung. Neue Quelldateien werden niemals aus dem Cache erzeugt.
Veraltete Cache-Versionen werden lediglich erkannt und nach roher Sicherung aus neuen Fotoquellen ersetzt; kein alter Referenzdecoder und keine Migration. Der konkret gemeldete Version-2-Fehler blockiert deshalb den Aufnahmedialog nicht mehr. Regressionen prüfen Mehrjahres-Zuordnungen, physische Baum-/Monatsfilterung, explizite Foto-Timeline, neuen Index, gemeinsames Foto-YAML bei Mehrfachaufnahme, fehlende Quellen ohne Wiederherstellung, alte Quellen ohne Übernahme, defekte neue Quellen ohne Indexaustausch, CAS-Konflikte, Abbruch und Symlinks. Bild-/Audiodateien bleiben unverändert.

## Vertiefter Review nach Umstellung auf Foto-YAMLs
Geprüft wurden Schichtabhängigkeiten, Domänenkonstruktoren, Kommentar-Publikation/Indexierung, UI-Aufrufketten, Composition Root und Lebenszyklen der Sammlung, Audio-/Kommentar- und Tag-Worker. Die Compilergrenzen und die bestehende Trennung von Import, Bereinigung und Pixelduplikaten bleiben erhalten.

Korrigierte Befunde:
1. Ablaufsteuerung im YAML-Adapter: load_catalog/commit_comment/Neuaufbau in application/comments/index zusammengeführt; technischer Port klassifiziert nur Cache und verarbeitet Dateien. Vollständige Quellenbestätigung vor Indexaktualisierung wird geprüft.
2. Veraltete Ergebnisse beim Reload desselben Root: Generationen schützen erfolgreiche Resultate und Fehlermeldungen; Ladeabbruch und serieller Nachfolgeauftrag gelten auch bei gleichem Pfad.
3. Übersprungene Quellen durch symlinkte Vorfahren: yaml/Jahr-Vorfahren müssen reguläre Verzeichnisse sein; Fehler verhindern den Indexaustausch. Der Scan traversiert nur Metadatenzweige.
4. Unzureichende Pfad-/Dateiinvarianten: Comment sichert eigenes Aufnahmejahr/audio/comments und com_-Zeitstempel. Media übernimmt nur passende Referenzzeiten. Vorhandene Ziele müssen Dateien sein, deren Vorfahren Verzeichnisse.

Neue Regressionen prüfen den geblockten Reload mit zwei nacheinander freigegebenen Leseaufträgen, Abbruch unmittelbar nach Quellenscan, unbestätigte Portantwort vor Indexspeicherung, reale temporäre Symlinks auf drei Vorfahrenebenen, Verzeichnisse mit Medienendung und widersprüchliche Domänenreferenzen. Keine Benutzerbibliothek oder echtes Mikrofon wird verwendet. Bekannte Interprozess-Rennbereiche, nicht sofort abbrechbare OS-Aufrufe und Stringfehler bleiben ausdrücklich dokumentiert.

## Frühe Dauern und automatische Tags
Neue Vorbereitung liegt vollständig in der Anwendung, ohne Player-Port. Ein aktiver und ein neuester wartender Auftrag, begrenzter Ergebniskanal, Cachegeneration bei Reload, 8192 positive FIFO-Einträge und explizite Fehlerbehandlung sichern den Lebenszyklus. Datumsschritte nutzen später die vorhandene Probe, Domänen-Batchvalidierung verändert nur Gewichte/Dauern. Tests prüfen den echten Seitenwechsel ohne Play/Datums-I/O, spätere Playerübernahme der Gewichte, wiederverwendete Probes, schnelle Bereichswechsel, Cache-Reset während blockiertem Probeaufruf, Vorrang/Abbruch und Fehlerende eines Workers.

Automatische Tagregeln sind in der Domäne. OrganizationSession aggregiert nur neue Audiomitgliedschaften und weiß, ob ein Scan vollständig erfolgreich war. UI liefert Batches/Abschluss und Katalogereignisse, keine eigenen Dateityp- oder Zuordnungsregeln. Regressionen prüfen commented vs. comment, diary, fortbestehende unbekannte Jahresdateien während Teilladen, Bereinigung nach Vollscan, Erhalt benutzerdefinierter Tags und Persistenz über den vorhandenen Store. Keine Originalmedien oder Benutzer-Mikrofone werden in Tests verändert/verwendet.

### Jahresplayer und Kategorien
Die obere Hörleiste ist der Audiozugang für alle Übersichten. Im Playerfenster filtern Tagebuch / Kommentare / Beide die sichtbare Zeitachse und Wiedergabe. Bleibt die aktuelle Aufnahme enthalten, werden Position und Wiedergabe erhalten. Filter verändern keine Dateien. Die vollständige Quellliste wird unabhängig weiter geprüft; die Kategoriezuordnung kommt aus der Domäne über den Plattformadapter. Die bildbezogene Kommentarleiste der Großansicht bleibt auf das Foto beschränkt.

### Bezugsdatum und Zeitraum des Players
Tagebuchaufnahmen verwenden ihr eigenes Datum. Kommentare verwenden Datum und Uhrzeit des verknüpften Fotos: Ein 2026 aufgenommener Kommentar zu einem Foto von September 1980 erscheint in 1980 beziehungsweise 1980/09. Audio und YAML werden dabei nicht verschoben oder verändert. Das tatsächliche Aufnahmedatum bleibt in der Dateiliste separat sichtbar.

Der geöffnete Player folgt der Navigation in der Fotowand. Sein Kopf zeigt Jahr oder Jahr/Monat; darüber hinaus zeigt er über der Zeitachse die laufende Bezugszeit. Kategorienfilter und Pausenzustand bleiben beim Zeitraumwechsel erhalten. Unter den Transportbuttons folgen Trennlinie und chronologische Tabelle: Dateiname, Bezugsdatum, Aufnahmedatum. Sie verwendet dieselbe gefilterte Reihenfolge wie Zeitachse und Wiedergabe; Doppelklick springt zum Dateianfang. Leere Bereiche bleiben klar erkennbar und deaktiviert.

### Bibliotheksweite Audiowiedergabe unter Alles
„Alles“ verwendet einen expliziten bibliotheksweiten Bereich (`AudioViewScope` ohne Jahresgrenze, `DiaryTimeline::for_library`). Keine Auswahl eines Ersatzjahres. Tagebuchaufnahmen werden nach ihrem eigenen Datum, zugeordnete Kommentare nach Foto-Bezugsdatum einsortiert. Hörleiste, separates Fenster und Dateiliste teilen dieselbe Auswahl; Tagebuch/Kommentare/Beide bleiben als Kategorienfilter verfügbar. Der Playerkopf zeigt „Alles“. Jahres-/Monatsfilter bleiben davon getrennt. Während der Bibliotheksscan läuft, ergänzen neu geladene Aufnahmen den Bereich; noch nicht eingelesene Dateien werden nicht als bereits verfügbar ausgegeben. Bestehende Medien bleiben unverändert.

### Kontextfotos auch vor Play
Beim Öffnen und erneuten Einblenden des Players, Clipwechsel und Seek werden die fünf Kontextfotos zur bereiten Bezugsposition geladen, auch bei Pause und vor der ersten Wiedergabe. Die Audio-Bereitschaft und tatsächlicher Wiedergabestart sind getrennte Zustände: Bereitstehen gibt die Hintergrundpriorität für Fotos frei; gründliche Audio-Metadatenprüfung wartet weiterhin auf tatsächliche Wiedergabe. Keine sekündlichen Neuanfragen. Unbekannte Bezugszeiten und leere Filter entfernen alte Fotos und deaktivieren deren Aktionen. Zwischen Zeitraum-/Kategorienzeile und genauer Datumszeile liegen mindestens zwölf Punkte Abstand. Native Regressionen prüfen Öffnen ohne Play, Seek, verspätete Ergebnisse, unbekannte Zeiten, leere Auswahl und Geometrie.

### Zentrale YAML-Ablage
Alle bibliotheksbezogenen YAML-Dateien liegen ausschließlich unter `yaml/`: `yaml/settings.yaml`, `yaml/tags.yaml`, `yaml/comments-index.yaml`. Dateibezogene Quellen spiegeln den gesamten Medienpfad mit angehängtem `.yaml`, etwa `1980/09/img_19800916_035811.jpg` → `yaml/1980/09/img_19800916_035811.jpg.yaml`. Interne Foto-/Audioverweise bleiben relativ zur Bibliothekswurzel, nicht zum YAML-Verzeichnis. Audio bleibt unter Aufnahmejahr/audio/diary beziehungsweise audio/comments.

Kein Einlesen, Migrieren oder automatisches Entfernen alter Ablageorte. Den zuletzt geöffneten Bibliothekspfad weiterhin lokal in macOS-Preferences halten. Leser erzeugen keine Verzeichnisse; Schreiboperationen legen nötige YAML-Unterordner nach Prüfung an. Gemeinsame Verzeichnisprüfung in metadata_fs; Symlinks und nicht reguläre Vorfahren ablehnen. Medienbaum und Medienscans lassen den reservierten yaml-Zweig aus. Kommentar-Neuaufbau liest ausschließlich Foto-YAMLs unter yaml/<Jahr>, überspringt Konfigurationen/Index/Recovery-Dateien und traversiert keine Medienzweige. Beschädigte Quellen ersetzen keinen gültigen Index; Sicherungen eines beschädigten Index bleiben ebenfalls unter yaml/.
