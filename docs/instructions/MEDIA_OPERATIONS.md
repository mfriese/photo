# Sichere Dateioperationen

Nur für Import, Duplikate, Papierkorb, Datum, EXIF und physisches Einsortieren lesen. Zusätzlich bei Metadatenänderungen: [Persistenz](PERSISTENCE.md).

## Import und Duplikate
- Ziel ist immer die aktuell geöffnete Bibliothek; ohne sie kein Import. Quelle und Ziel dürfen sich nicht überschneiden. Import kopiert, benennt/löscht/verschiebt niemals Quellen. Prüflauf ohne Schreibzugriffe.
- Aufnahmezeit: Foto EXIF DateTimeOriginal, Video QuickTime CreateDate; Audio gültige eingebettete Aufnahmezeit vor Containerzeit, danach vollständiger Dateinamenzeitstempel. Unterstützte Trenner und Kalender zentral validieren. Letzter Importfallback: älteres gültiges Dateierstellungs-/Inhaltsänderungsdatum, ausdrücklich kennzeichnen; keine implizite Zeitzonenverschiebung.
- Foto/Video nach Jahr/Monat/img_JJJJMMTT_HHMMSS.ext; optional Fotos nach Jahr/scans ohne weitere Unterordner. Originalformat erhalten, jpeg → jpg. Diary nach Jahr/audio/diary/aud_…; Kommentaraudio separat Jahr/audio/comments/com_… nach Aufnahmezeit.
- Importkollision: Fotos bytegenau identisch überspringen, unterschiedliche mit _001 usw.; Videos bei gleichem generiertem Zielnamen überspringen. Audioentscheidungen über bestehenden Anwendungsfall, keine UI-Sonderregeln.
- Eigene eindeutige temporäre Ziele schreiben, synchronisieren und vollständig prüfen; niemals überschreibendes Rename zur Veröffentlichung. exFAT-Fallback bei Unsupported: create_new, geprüfte Kopie, fsync, erneuter Bytevergleich. Nur eigene unvollständige Dateien nach Handle-/Identitätsprüfung entfernen; Fehler sichtbar melden. Kein Symlinkziel; Absturzüberreste und zusätzlicher Platzbedarf ehrlich behandeln.
- Pro Datei kompakter strukturierter Bericht mit Quelle, Datum/Quelle, Ziel und Entscheidung; optimal grün, Fallback gelb, Fehler rot plus Text. Abbruch behält bestätigte Teilerfolge. Keine Erfolgsableitung aus Logtext.
- Quellenbereinigung ist ein separater bestätigter Ablauf: jede Quelle und veröffentlichtes Ziel erneut prüfen und bytevergleichen, Ergebnis zeigen, erst dann Quelle in macOS-Papierkorb. Nie Teil des normalen Imports.
- Duplikatsuche eigener File-Menüdialog, jeweils eine Kollisionsgruppe mit Varianten beliebiger Suffixe. Volle ausgerichtete Bildpixel vergleichen, 16-Bit erhalten; Hash/Thumbnail/Ähnlichkeit reicht nicht zur Entfernung. Farbprofil-/Mehrbildunsicherheit konservativ behalten. Vor bestätigtem Papierkorb erneut Dateistand/Pixel prüfen, Keeper erhalten; über 100000 Paarvergleiche vor Vergleich ablehnen.
- AppleDouble `._`-Begleitdateien ignorieren, nicht automatisch löschen; andere versteckte Medien nicht pauschal ausschließen. Eigene Stagingdateien von fremden Begleitdateien unterscheiden.

## Datum und optionales Verschieben
- Datumskorrektur für Foto/Video über zentralen Command, keine Audio-Aufnahmezeit ändern. Validiertes lokales Datum/Uhrzeit als `capture_date` im Medien-YAML; Standard ist ausschließlich logische Korrektur ohne Medien-/EXIF-/Pfadänderung.
- Korrektur steuert Raster, Zeitraum, Player-Kontext und Kommentar-Bezugsdatum. Kommentaraufnahmezeit und Audiodateipfad bleiben erhalten. `capture_date_overrides` ist nur rekonstruierbarer SQL-Marker.
- Option „Datei auch verschieben und umbenennen“ standardmäßig aus. Aktiviert exakt Jahr/Monat/img_Zeitstempel.ext nach Domänenregel. Jede Medien-/YAML-/Indexkollision blockiert den ganzen Vorgang; kein Überschreiben, kein Suffix, keine Duplikatentscheidung. Benutzer kann Datum/Uhrzeit ändern.
- Transfer hinter Repository-Port, nur gleiches Dateisystem: exklusives Ziel, bytegeprüfte Kopie, erhaltene Änderungszeit, dauerhaftes Journal und Recovery, gemeinsames Metadaten-Schreibgate. Quelle erst nach geprüfter Veröffentlichung entfernen; Fehler zurückabwickeln. Keine Übertragung aller Dateisystem-Zusatzattribute garantieren.
- Medien-YAML, Tags, Transkripte und Kommentarverweise umhängen, SQL-Projektion reparieren. Verspätete Transkription darf alte YAML nicht neu anlegen. RAM-Auswahl/Rotation/Orte und laufende Scans generationensicher umstellen. Nur leere Quellverzeichnisse entfernen, nie fremde Dateien.

## Papierkorb und EXIF
- Rasterlöschung nur nach Bestätigung in macOS-Papierkorb, nie endgültig löschen. Nur bestätigte Erfolge inkrementell entfernen; Fehler behalten. Kein Bibliotheksreload oder Audio-Vollscan, Tags/Orte/Filter erhalten; Viewerindizes per Dateipfad remappen, späte Scanbatches mit Tombstones abweisen.
- EXIF-Orientierung einschließlich Spiegelungen lesen. Manuelle Drehung grundsätzlich Sitzungskorrektur; nur mit aktivierter Schreiboption JPEG-Orientierung ändern, ohne Pixelneukompression oder dauerhafte neue Backups (`-overwrite_original_in_place`). Keine fremden Backups entfernen, keine anderen Formate/Symlinks ändern. Fehler erhalten Sitzungskorrektur, Erfolg invalidiert alle Bildcachevarianten.

## Relevante Regressionen
- Temporäre Medien für Quellschutz, exFAT-Fallback, Kollision, Abbruch, externe Ersetzung, Symlinks und Teilerfolg.
- Verschieben: Fehler und Prozessabbruch an dauerhaften Grenzen, Rückabwicklung ohne vorhandene YAML, identischer YAML-Pfad, spätes Transkript, Kommentar-Bezugsdatum, Tags/Orte/Transkripte und Neuaufbau nach SQLite-Verlust.
- Papierkorb nur mit Fake/temporären Fixtures; keine echte Bibliothek für Sichtprüfung starten.
