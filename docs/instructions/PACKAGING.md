# macOS-Paketierung und Installation

Nur bei Build-, Icon-, Installer- oder Distributionsänderungen lesen.

- `make app` baut das Bundle, `make install` installiert ausdrücklich nach `/Applications/PhotoSuite.app`, `make pkg` erstellt Installer. Cargo-Build/-Install-Hooks installieren niemals ungefragt ins System; cargo install bleibt Terminalinstallation.
- Bundle enthält Binary, ExifTool samt Modulen und erforderliche Lizenzen, keine Modelle/Benutzerbibliotheken/Preferences. macOS-Frameworks und System-Perl sind Voraussetzungen; keine Homebrew-Pfade oder externen Drittanbieter-Dylibs zur Laufzeit.
- Ressourcen relativ zu Contents/MacOS finden und in main injizieren. Im Bundle bei fehlenden Ressourcen sichtbar fehlschlagen statt PATH-Fallback; Entwicklungsmodus darf PATH verwenden. ExifTool-Benutzerkonfiguration für reproduzierbares Verhalten deaktivieren.
- Iconmaster assets/app-icon/photosuite.png im Repository mit Alpha; alle ICNS-Größen deterministisch erzeugen, CFBundleIconFile vor Codesign setzen. Dasselbe PNG als egui- und NSApplication-Icon auf Hauptthread nach AppKit-Initialisierung, damit cargo run das richtige Dock-Icon zeigt. Keine Änderungen am Dock anderer Apps oder Benutzer-Iconcache.
- Vor Installation laufende App/Bundle-ID prüfen, Symlinkziele ablehnen, im Zielfilesystem bereitstellen und bei Austauschfehler alte App wiederherstellen. Einstellungen/Bibliotheken bei Update/Deinstallation erhalten.
- Ad-hoc-Signatur und fehlende öffentliche Notarisierung ehrlich dokumentieren. Whisper/GGML-Lizenzen aus aufgelöster Cargo-Abhängigkeit sammeln; fehlende Texte nicht still ignorieren.
- Paketbau prüft Plist, Codesign, dynamische Verweise und ExifTool ohne Homebrew-PATH über --check-installation. Dieser Check darf keine Benutzerbibliothek öffnen.
- Tests nur in temporären Applications-/Benutzerverzeichnissen. `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_*.py'`; bei Iconänderungen zusätzlich make test-icon. Keine echte Systeminstallation in Standardtests.
- App-Paket nicht bei jeder kleinen Quell-/Dokumentationsänderung bauen. Paketbau bei explizitem Auftrag, Release oder Paketierungsänderung; erfolgreiche Prüfung nicht ohne Grund wiederholen.
