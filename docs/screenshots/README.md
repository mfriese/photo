# Screenshots und Demobibliothek

## Aktueller Stand

Die Aufnahmen des neuen Galeriedesigns wurden auf Wunsch zurückgestellt. Es sind noch keine Screenshots erstellt und keine manuelle Sichtprüfung der laufenden App erfolgt. README enthält daher keine nicht vorhandenen Bildverweise.

## Echte Aufnahmen erstellen

1. `python3 scripts/create_demo_library.py` ausdrücklich ausführen. Das lädt Stockfotos nach `/tmp/PhotoSuite-Demo` und legt separate Demo-Preferences an. Netzwerk und ExifTool sind nur für dieses Dokumentationswerkzeug relevant. Vorhandene Fotos und Preferences werden nicht überschrieben.
2. Die aktuelle App bauen: `make app`.
3. Für eine isolierte Vorschau das gebaute Programm mit eigenem Preferences-Pfad starten:

```sh
PHOTOSUITE_PREFERENCES_PATH=/tmp/PhotoSuite-Demo/demo-preferences.plist \
  target/macos/PhotoSuite.app/Contents/MacOS/photosuite
```

4. Reale Fenster aufnehmen: `gallery.png` (Fotowand), `viewer.png` (Großansicht), `preferences.png` (Einstellungen). Erst nach Sichtprüfung hier ablegen und in README relativ einbinden.
5. Kein Mockup als Screenshot ausgeben. Keine privaten Fotos, Personen, persönlichen Dateipfade, anderen Apps oder Kontodaten aufnehmen.
6. GitHub-Verweise nach dem Einfügen überprüfen: `![Beschreibung](docs/screenshots/gallery.png)` aus der Repository-README; Groß-/Kleinschreibung beachten.

## Bildquellen

Demofotos: Unsplash, bezogen über [Lorem Picsum](https://picsum.photos/). Nutzung nach der [Unsplash-Lizenz](https://unsplash.com/license). Die heruntergeladenen Originale bleiben außerhalb des Repositories. Aufnahmedatum und Dateiname der Demo sind synthetische Testdaten, keine Aussagen über die tatsächliche Entstehung der Fotos.

| Demo-Datei | Fotografin/Fotograf | Original |
| --- | --- | --- |
| `2025/10/img_20251001_153000.jpg` | Alexey Topolyanskiy | [Unsplash](https://unsplash.com/photos/-oWyJoSqBRM) |
| `2025/10/img_20251002_153000.jpg` | Philippe Wuyts | [Unsplash](https://unsplash.com/photos/_h7aBovKia4) |
| `2025/10/img_20251003_153000.jpg` | Andrew Ridley | [Unsplash](https://unsplash.com/photos/Kt5hRENuotI) |
| `2025/10/img_20251004_153000.jpg` | Jared Erondu | [Unsplash](https://unsplash.com/photos/j4PaE7E2_Ws) |
| `2025/10/img_20251005_153000.jpg` | Wolfgang Lutz | [Unsplash](https://unsplash.com/photos/yOujaSETXlo) |
| `2026/06/img_20260606_153000.jpg` | Jordan McQueen | [Unsplash](https://unsplash.com/photos/93g2k8D1Mi8) |
| `2026/06/img_20260607_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/6J--NXulQCs) |
| `2026/06/img_20260608_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/Cm7oKel-X2Q) |
| `2026/06/img_20260609_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/I_9ILwtsl_k) |
| `2026/06/img_20260610_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/3MtiSMdnoCo) |
| `2026/08/img_20260811_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/IQ1kOQTJrOQ) |
| `2026/08/img_20260812_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/NYDo21ssGao) |
| `2026/08/img_20260813_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/gkT4FfgHO5o) |
| `2026/08/img_20260814_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/Ven2CV8IJ5A) |
| `2026/08/img_20260815_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/Ps2n0rShqaM) |
| `2026/09/img_20260916_153000.jpg` | Paul Jarvis | [Unsplash](https://unsplash.com/photos/P7Lh0usGcuk) |
| `2026/09/img_20260917_153000.jpg` | Jerry Adney | [Unsplash](https://unsplash.com/photos/_WiFMBRT7Aw) |
| `2026/09/img_20260918_153000.jpg` | Go Wild | [Unsplash](https://unsplash.com/photos/V0yAek6BgGk) |

Die Quelldateien werden ausschließlich zur Anzeige in der separaten Demobibliothek verwendet. PhotoSuite selbst lädt keine Bilder aus dem Internet. Für neue Bildquellen diese Liste gemeinsam mit den Screenshots aktualisieren.
