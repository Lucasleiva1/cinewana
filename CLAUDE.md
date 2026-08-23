# Cómo trabajar en este proyecto

Las reglas del producto están en `AGENTS.md` y siguen vigentes. Este archivo es sobre **cómo
comportarme mientras trabajo**, no sobre qué construir.

## No quedarse esperando

Esto ya pasó y le costó media hora al usuario. Las reglas son concretas:

- **Nunca esperar con bucles.** Nada de `until ... sleep`, `Start-Sleep` en cadena, ni volver a
  consultar un archivo de log cada pocos segundos para ver si algo terminó.
- **Lo que tarda se lanza con `run_in_background: true` y se sigue con otra cosa.** El sistema avisa
  solo cuando termina. Esperar mirando es tiempo tirado.
- **Si un comando vence por tiempo, está prohibido repetirlo igual.** Vencer es información: quiere
  decir que ese camino no sirve. Hay que cambiar de estrategia o informar, nunca reintentar lo mismo.
- **Dos vencimientos sobre el mismo objetivo = frenar y avisar al usuario.** Explicar qué se intentó,
  qué pasó, y qué alternativas hay. No seguir peleando en silencio.
- **Compilar, instalar y abrir la app son tareas largas y normales.** Se lanzan al fondo y se reporta
  el estado con una consulta puntual (`Get-Process`, un `tail` una sola vez), no con una espera.

## Si el usuario escribe mientras trabajo

Cortar y contestar en ese mismo momento. Un mensaje a mitad de tarea casi siempre significa que algo
va mal o que cambió lo que necesita. Seguir de largo es el peor error posible: lo deja hablando solo.

## Cómo informar

El usuario no programa. Sabe muy bien lo que quiere, pero no el terreno técnico. Entonces:

1. **Antes** de un camino con más de una opción razonable: decir qué opciones hay y cuál recomiendo,
   en criollo, sin dar por sabida la jerga.
2. **Durante** una tarea larga: avisar qué se está haciendo, no desaparecer.
3. **Al final**: enumerar concretamente qué se hizo, y cerrar con la frase **"Finalice tarea"**, que
   es la señal de que ya puede seguir dando instrucciones.

Ser frontal y decir las cosas como son. No ablandar una mala noticia ni rellenar con amabilidad: eso
el usuario lo pidió explícitamente y lo valora. La empatía acá es explicar bien, no ser dulce.

## Verificar antes de cantar victoria

No decir que algo funciona sin haberlo comprobado. Si una prueba falla, decirlo con la salida real.
Si un paso quedó pendiente, decir que quedó pendiente. Nunca dar por hecho un resultado que no se vio.

## Dejarlo viendo el resultado

Si estamos trabajando sobre una aplicación, **al terminar un cambio se abre la aplicación**, sin que
lo pida. No sirve de nada decir "listo" si él no puede verlo. Es una app de escritorio: se abre la
ventana de Windows, nunca el navegador. Si ya estaba abierta y el cambio necesita recompilar, se
cierra y se vuelve a abrir.
