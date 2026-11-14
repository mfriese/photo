<div align="center">
  <img src="assets/app-icon/photosuite.png" alt="PhotoSuite App-Icon" width="96">
  <h1>PhotoSuite</h1>
  <p><strong>Deine Fotos. Deine Ordner. Deine Erinnerungen.</strong></p>
  <p>Lokale Foto- und Videogalerie mit Audio-Diary und gesprochenen Fotokommentaren für macOS.</p>
</div>

PhotoSuite macht deine nach **Jahr und Monat** geordnete Sammlung durchsuchbar – ohne Cloud oder Konto. Originalmedien bleiben standardmäßig unverändert; Tags, Kommentare und Einstellungen liegen als lesbare YAML-Dateien in deiner Bibliothek.

**Status:** Umfangreicher Rust-Prototyp für macOS. Noch keine öffentlich signierte oder notarisierte Distribution, kein App-Store-Download oder Homebrew-Cask. Screenshots folgen später.

## Funktionen

- **Fotowand:** virtualisiertes Raster, fünf Vorschaugrößen, optional quadratischer Beschnitt, Jahres-/Monatsnavigation und Suche.
- **Fotos und Videos:** Großansicht, EXIF-Ausrichtung, native Videowiedergabe und sechs Standbilder pro Video – ohne externes FFmpeg.
- **Audio-Diary:** gemeinsame Zeitachse mit Abschnitten proportional zur Aufnahmedauer, Hintergrundprüfung und fünf zeitlich passenden Fotos.
- **Gesprochene Kommentare:** direkt zum Foto aufnehmen, Mikrofon wählen, Pegel beobachten und vor dem Speichern vorhören.
- **Tags und virtuelle Alben:** mehrere Tags pro Datei, Tagwolken zum Filtern und Tags bereits beim Import. Neue Zuordnungen erscheinen automatisch.
- **Kontrollierter Import:** Prüflauf, kompakte Ergebnisse, datumsbasierte Namen, Kollisionsprüfung und separate Prüfung von Bildduplikaten.
- **Native Bedienung:** macOS-Menüs, Tastenkürzel und bibliotheksbezogene Einstellungen; die letzte Bibliothek wird beim Start wieder geöffnet.

Die Hörleiste folgt der Auswahl **Alles / Jahr / Monat**. Tagebucheinträge erscheinen zu ihrer Aufnahmezeit, Fotokommentare zur Zeit des zugehörigen Fotos. Im Player kannst du Tagebuch, Kommentare oder beides hören.

## Installieren und starten

Zum Bauen brauchst du **Rust/Cargo, Xcode Command Line Tools, Python 3, Make und ExifTool**. ExifTool lässt sich beispielsweise mit `brew install exiftool` installieren.

```sh
cargo fetch --locked
make install
```

Danach findest du **PhotoSuite.app unter Programme** (`/Applications`). Eine laufende PhotoSuite vor einem Update schließen. Das App-Bundle enthält ExifTool; Homebrew ist zur Laufzeit nicht erforderlich. Gebaut wird für die Architektur deines Macs.

| Befehl | Ergebnis |
| --- | --- |
| `make app` | App-Bundle unter `target/macos/` |
| `make pkg` | Doppelklick-Installer unter `target/macos/` |
| `make uninstall` | App entfernen; Bibliothek und Einstellungen bleiben erhalten |
| `cargo run --release` | Direkt aus dem Projekt starten |

Weitere Installationsmöglichkeiten stehen im [Benutzerhandbuch](docs/USER_GUIDE.md#macos-installation).

## Erste Schritte

1. **File → Open Library…** öffnet deine Sammlung. Der Pfad steht im Fenstertitel.
2. Links ein Jahr oder einen Monat wählen; **Edit → Find… (⌘F)** öffnet die Suche.
3. Ein Foto anklicken: **← / →** wechseln das Medium, **Escape** schließt die Großansicht. Bei Videos startet **Leertaste** die Wiedergabe.
4. **View** enthält Vorschaugröße und Beschnitt, **Edit → Tags and Albums…** die Tagverwaltung.
5. **File → Import…** importiert in die geöffnete Bibliothek; **PhotoSuite → Settings… (⌘,)** öffnet die Einstellungen.

In der Tagwolke zeigt keine Auswahl alle Medien; mehrere ausgewählte Tags werden mit **ODER** verknüpft. **Reset** hebt den Filter auf.

## Deine Dateien bleiben zugänglich

```text
Bibliothek/
├── 1980/09/img_19800916_035811.jpg
├── 2026/audio/diary/aud_20261114_120000.wav
├── 2026/audio/comments/com_20261114_130000.m4a
└── yaml/
    ├── settings.yaml
    ├── tags.yaml
    ├── tag-definitions.yaml
    ├── comments-index.yaml
    ├── 1980/09/img_19800916_035811.jpg.yaml
    └── 2026/11/com_20261114_130000.m4a.yaml
```

Dateibezogene YAMLs liegen einheitlich unter **`yaml/Jahr/Monat/<Dateiname.ext>.yaml`**. Sie enthalten Tags und gegebenenfalls Kommentarverweise. Die Gesamtindizes lassen sich daraus neu aufbauen. Nur der zuletzt geöffnete Bibliothekspfad bleibt lokal in den macOS-Preferences.

**Import kopiert und löscht keine Quelldateien.** Quellen können erst in einem getrennten, ausdrücklich bestätigten Schritt nach erneutem Bytevergleich in den Papierkorb verschoben werden. Auch Duplikatentfernung erfordert Bestätigung. Optionales Speichern manueller JPEG-Drehungen verändert EXIF im Original ohne Neukompression oder dauerhaftes Backup. Verwaltungs-YAMLs werden unabhängig vom Nur-Lesen-Modus der Medien gespeichert.

## Bekannte Grenzen

- **HEIC und Kamera-RAW fehlen derzeit.** Unterstützt sind JPEG, PNG, WebP, TIFF, GIF und BMP; Video-/Audioformate hängen von den macOS-Codecs ab.
- Die Anwendung ist noch nicht auf andere Betriebssysteme portiert. Transkription und umfassende Bildbearbeitung sind nicht enthalten.
- Audioabschnitte können während der Hintergrundprüfung ihre Breite ändern; lückenlose Übergänge sind nicht garantiert.
- Große Sammlungen werden schrittweise geladen. Cache-, Dateigrößen- und weitere Robustheitsgrenzen stehen im [Benutzerhandbuch](docs/USER_GUIDE.md) und in der [Architekturdokumentation](ARCHITECTURE.md).

## Entwicklung

Die UI verwendet **Rust mit egui/eframe**, native Menüs kommen von **muda**, Audio und Video von macOS-Systemframeworks. Clean Architecture trennt Domäne, Anwendungsfälle und Adapter; das Theme ist zentral verwaltet.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
make check-architecture
make test-native
cargo build --workspace
```

- [Benutzerhandbuch](docs/USER_GUIDE.md)
- [Architektur](ARCHITECTURE.md) und [Architekturreview](docs/ARCHITECTURE_REVIEW.md)
- [Designsystem](docs/DESIGN.md) und [Screenshot-Dokumentation](docs/screenshots/README.md)
- [Projektregeln](AGENTS.md)

**Lizenz:** [GNU GPL v3](LICENSE).
