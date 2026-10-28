# Fertige Create-Mods für Horizon

Zum direkten Importieren, ohne selbst zu bauen:

| Datei | Inhalt |
| --- | --- |
| `create.hmod` | Blöcke, Rezepte, Texte (Inhaltsmod) |
| `create_kinetics.hmod` | Rotation, Drehzahl und Belastung (Server-Mod, braucht `create.hmod`) |
| `create-horizon-mods.zip` | beide Dateien zusammen |

1. Beide `.hmod`-Dateien in `horizon-mods/` neben `options.txt` legen
   (iPhone/iPad: in der Dateien-App unter „Horizon“ → `Documents/horizon-mods/`).
2. In `horizon-mods.txt` eintragen (oder im Mods-Menü einschalten):
   ```
   mod.create:true
   mod.create_kinetics:true
   ```
3. Spiel bzw. Server neu starten. `/create help` zeigt die Befehle.

Für einen Server gehören beide Dateien auf den Server. Spieler bekommen die Blöcke vom Server.

Prüfsummen stehen in `SHA256SUMS.txt`. Die Dateien sind unsigniert und wurden aus
`../content` und `../kinetics` mit `cargo horizon-mod build` gebaut.
