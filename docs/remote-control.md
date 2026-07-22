# Control remoto local de CINE WANA

## Estado de esta versión

Esta versión de prueba agrega un servidor HTTP/WebSocket autenticado dentro de la aplicación Windows y una interfaz móvil incluida en el instalador. No transmite video, no usa Internet y no expone las rutas físicas de la biblioteca.

El acceso inicial usa HTTP en la red local. El control funciona desde el navegador del teléfono, pero Chrome requiere un origen HTTPS confiable para habilitar el service worker y la instalación offline completa de una PWA. Esa capa de HTTPS local se incorporará después de validar el control por Wi‑Fi.

## Activar y vincular

1. Abrir CINE WANA en la computadora e iniciar sesión en una cuenta local.
2. Entrar en **Configuración → Control remoto**.
3. Presionar **Activar control remoto**.
4. Si Windows muestra el aviso del firewall, permitir acceso únicamente en redes privadas.
5. Presionar **Mostrar QR**.
6. Conectar el teléfono a la misma red Wi‑Fi y escanear el QR con la cámara.
7. Cuando aparezca la solicitud en CINE WANA, comprobar el nombre del dispositivo y presionar **Aprobar**.

Como alternativa al QR, se puede copiar la URL completa mostrada en la tarjeta y abrirla manualmente en el teléfono. El enlace temporal vence a los cinco minutos.

## Puerto y dirección

El puerto predeterminado es `47821`. Puede cambiarse antes de iniciar CINE WANA definiendo la variable de entorno `REMOTE_CONTROL_PORT` con un puerto entre 1024 y 65535. El servidor escucha en la red local y la pantalla muestra la dirección LAN detectada.

Si el teléfono no abre la página:

- comprobar que ambos dispositivos usan la misma Wi‑Fi;
- desactivar temporalmente el aislamiento de clientes o la red de invitados del router;
- permitir CINE WANA en el Firewall de Windows para redes privadas;
- comprobar que la dirección mostrada sigue siendo la dirección actual de la computadora;
- desactivar y volver a activar el control para detectar un cambio de IP.

## Seguridad y dispositivos

El QR contiene un desafío aleatorio temporal. Ningún comando se acepta antes de que la computadora apruebe el teléfono. Después de aprobar, el teléfono recibe una credencial propia; CINE WANA guarda únicamente su hash. Los mensajes tienen límite de tamaño, lista blanca de comandos y rate limit.

Para quitar acceso, abrir **Dispositivos vinculados** y presionar **Desvincular**. Una sesión revocada deja de aceptar comandos inmediatamente.

## Funciones disponibles

- reproducir o pausar;
- avanzar y retroceder 10 segundos;
- buscar una posición;
- volumen y mute;
- pantalla completa;
- seis controles reales de imagen y restablecimiento;
- catálogo táctil de películas, series agrupadas por temporada y Mi lista;
- orden diario de películas sincronizado con Inicio, manteniendo las agregadas recientemente en orden cronológico;
- episodios con miniatura disponible, descripción, ficha y reproducción en la computadora;
- búsqueda local;
- ficha de contenido;
- reproducir en la computadora;
- agregar o quitar de Mi lista y Favoritos;
- reconexión automática.

Los selectores finos de audio y subtítulos aparecen solamente cuando el reproductor interno pueda cambiar esas pistas realmente. En la versión actual permanecen ocultos porque esa capacidad todavía está pendiente en el reproductor de escritorio.

## Datos y archivos

Los archivos multimedia configurados se tratan siempre como solo lectura. El teléfono recibe identificadores opacos y versiones controladas de las portadas; nunca recibe una ruta física. Las credenciales de dispositivos se guardan en el directorio de datos de CINE WANA, fuera de la biblioteca multimedia.

## Prueba sin Internet

Después de vincular, se puede desconectar Internet manteniendo activa la red Wi‑Fi local. La página seguirá comunicándose con la computadora. En esta primera prueba HTTP, la carga inicial del shell móvil requiere que CINE WANA esté abierto; el modo offline completo depende del futuro HTTPS local confiable.

## Instalador de prueba

El instalador NSIS Windows x64 se genera con:

```powershell
corepack pnpm desktop:build
```

La salida queda en `target/release/bundle/nsis/`. El instalador incluye la compilación de `apps/remote/dist` como recurso interno de la aplicación.

## Limitaciones de la primera prueba

- host de escritorio únicamente Windows x64;
- sin transmisión de video al teléfono;
- sin aplicación Android nativa, APK, Bluetooth, nube ni Wi‑Fi Direct;
- instalación PWA offline completa pendiente de HTTPS local confiable;
- el firewall o la configuración del router pueden impedir conexiones entre dispositivos;
- selección fina de pistas de audio y subtítulos pendiente del reproductor interno.
