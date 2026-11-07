# PhotoSuite App-Icon

`photosuite.png` ist das unveränderte, mit dem integrierten Imagegen-Werkzeug erzeugte Original mit Transparenz. Es liegt dauerhaft in den Projektquellen; der Paketbau benötigt keinen Zugriff auf Codex-Verzeichnisse und keine erneute Bildgenerierung.

`scripts/macos_app.py` erzeugt daraus mit macOS `sips` zehn PNG-Repräsentationen (16 bis 1024 Pixel), konvertiert das Iconset per `iconutil` in `Contents/Resources/PhotoSuite.icns` und registriert es über `CFBundleIconFile`. Das generierte ICNS gehört zum Build-Artefakt. Der PNG-Master bleibt unverändert.

## Generierungs-Prompt

Use case: logo-brand. Asset type: finished macOS app icon for PhotoSuite, a local photo and video archive viewer. Design ONE polished icon, not a contact sheet or mockup. A distinctive compact stack of three photographic slides, with the foremost slide containing an abstract landscape of two clean mountain silhouettes and a small sun; the overlapping slides subtly communicate an organized archive. Simple, premium, contemporary macOS visual language, precise geometry, tasteful softly dimensional materials and restrained highlights. Strong recognizable silhouette and minimal detail that remains clear at 32px. Balanced centered composition inside a softly rounded square icon tile, with generous macOS-style optical padding around the tile on a genuinely transparent canvas. Choose a cohesive sophisticated photographic color treatment with a dark foreground frame and luminous landscape accents. No text, no letters, no wordmark, no watermarks, no Apple Photos flower, no camera-brand logos, no surrounding desktop or presentation scenery. Square high-resolution production asset. The complete icon tile and its soft shadow must be fully visible without cropping.

Werkzeug: integriertes Imagegen; `transparent_background=true`.
