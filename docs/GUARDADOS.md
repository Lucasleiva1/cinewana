# Guardados publicados de CINE WANA

Registro de cada versión publicada en GitHub Releases, para identificar rápido cuál es cuál.
El más reciente va arriba.

---

## 0.3.11 — La carpeta `peliculas nuevas`

| Dato | Valor |
|---|---|
| **Número de versión** | `0.3.11` |
| **Nombre del guardado** | CINE WANA v0.3.11 |
| **Fecha** | miércoles 26 de agosto de 2026 |
| **Hora** | 21:27:40 (hora de Buenos Aires, UTC-3) |
| **Hora UTC** | 2026-08-27 00:27:40 UTC |
| **Etiqueta** | `app-v0.3.11` |
| **Commit** | `cae7205e13fee541bfe1eb9323ef1ca54b3768bf` |
| **Marcada como Latest** | Sí |
| **Enlace** | https://github.com/Lucasleiva1/cinewana/releases/tag/app-v0.3.11 |

**Qué trae:** abrir la aplicación deja de escanear la biblioteca entera. Aparece la carpeta
`peliculas nuevas` al lado de `PELICULAS` y `SERIES`: lo que se deja ahí se procesa una vez y se muda
solo a `PELICULAS`. Lo que ya estaba terminado no se vuelve a tocar. Se cerró el bucle que
reconsultaba TMDB en cada arranque. Navegar ya no se traba durante un escaneo. Repaso completo
automático cada cinco días, en segundo plano y con aviso al terminar. Las series siguen siendo
manuales, con el botón de reescanear.

**Archivos publicados:**

| Archivo | Tamaño |
|---|---|
| `CINE.WANA_0.3.11_x64-setup.exe` | 5.431.627 bytes |
| `CINE.WANA_0.3.11_x64-setup.exe.sig` | 420 bytes |
| `latest.json` | 1.835 bytes |

**Huella del instalador (SHA256):**
`CE86883560BD06940B8C569975967D77603196E67AE9E60243A84D05A98605CC`

**Verificado después de publicar:** el enlace que consulta la aplicación
(`releases/latest/download/latest.json`) responde 200, sin BOM, con la versión `0.3.11` y las dos
plataformas. El instalador descargado desde GitHub tiene la misma huella que el firmado localmente.
La prueba de actualización desde una instalación anterior queda **pendiente** de hacerla el usuario.

---

## Anteriores

Las versiones publicadas antes de este registro están en el historial del repositorio y en
`docs/ROADMAP.md`. La última fue **0.3.10**, etiqueta `app-v0.3.10`, commit `82dd22c`.
