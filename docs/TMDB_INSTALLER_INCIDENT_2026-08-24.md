# Incidente: instalador de CINE WANA sin credencial TMDB

Fecha de detección: 24 de agosto de 2026  
Versión afectada: 0.3.10  
Estado: reparado localmente y protegido contra repetición

## Qué ocurrió

La instalación de Windows de CINE WANA 0.3.10 mostraba que TMDB no estaba configurado al intentar buscar una película o serie. La credencial seguía presente en el archivo raíz `.env`, pero el ejecutable instalado se había generado sin incorporarla al binario.

El error de empaquetado fue permitir que se produjera un instalador de versión `release` aunque el proceso de Rust no tuviera `TMDB_READ_ACCESS_TOKEN` ni `TMDB_API_KEY`. El código podía detectar la ausencia durante el uso, pero la compilación no se detenía. Por eso el instalador parecía correcto y la falla recién se veía al buscar información en TMDB.

## Impacto

- Las películas y series agregadas se conservaron.
- Las fichas que necesitaban identificación, sinopsis, reparto o imágenes de TMDB no podían completar la búsqueda.
- Los archivos originales de la biblioteca no fueron modificados.
- La recuperación consiste en instalar un ejecutable compilado correctamente y volver a ejecutar la búsqueda o actualización de fichas pendientes.

## Reparación aplicada

1. Se comprobó que la API key local seguía presente y tenía el formato esperado.
2. Se reconstruyó CINE WANA mediante el comando canónico `npm.cmd run desktop:build`, que carga el `.env` antes de invocar Tauri.
3. Se verificó localmente, sin imprimir la clave, que el nuevo binario 0.3.10 contiene la credencial compilada.
4. Se generó el instalador versionado `CINE WANA_0.3.10_x64-setup.exe`.

## Protección permanente

`apps/desktop/src-tauri/build.rs` ahora bloquea toda compilación `release` que no reciba al menos una de estas variables:

- `TMDB_READ_ACCESS_TOKEN`
- `TMDB_API_KEY`

También declara ambas variables con `cargo:rerun-if-env-changed`, para que Cargo vuelva a compilar cuando cambie la configuración y no reutilice silenciosamente un binario anterior.

## Lista obligatoria antes de entregar otro instalador

1. Confirmar proyecto, raíz Git, remoto `origin`, nombre de producto y versión.
2. Confirmar que el `.env` raíz existe y que una credencial TMDB no está vacía, sin mostrar su valor.
3. Compilar sólo con `npm.cmd run desktop:build`.
4. Exigir que la validación de `build.rs` termine correctamente.
5. Comprobar que el nombre del instalador incluye la misma versión que Tauri y el ejecutable.
6. Verificar en CINE WANA una búsqueda real de película y otra de serie antes de entregar o publicar.
7. No limpiar ni borrar `.env`, cachés de metadatos ni contenido de la biblioteca durante esta comprobación.

Este control es parte del proceso de entrega: un instalador que no supere la validación TMDB no debe distribuirse.
