# Validación real en el equipo (CachyOS)

Procedimiento para comprobar el observador de sesiones con datos reales. Separado de
`sesiones-de-ejecucion.md`, que describe **cómo funciona** el observador; aquí se describe
**cómo se comprueba**.

## Qué demuestra y qué no

Las pruebas automáticas usan fixtures: reproducen topologías y catálogos capturados, y
verifican el pipeline de producción, pero **no demuestran que la detección funcione en el
equipo real**. Eso solo lo dice este protocolo, y solo con datos reales.

No mezclar ambos planos al informar: un fixture en verde nunca es «validado en el equipo», y
un paso de este protocolo en rojo no se tapa con las pruebas automáticas.

## Estado de la fase 5.3

La fase está **cerrada** con una limitación declarada. El observador es **experimental y
opcional, apagado por defecto**: no se lee `/proc` si el usuario no lo pide. Apagado por
defecto es lo correcto, no una carencia.

La limitación conocida (Heroic + Heronite simultáneos) se describe en
`sesiones-de-ejecucion.md` y en `STATUS.md`. Este protocolo **no la reabre**: su investigación
está aplazada y exige una captura nueva con el observador activo.

## Antes de empezar

Comprobar que el observador está **encendido** (interruptor de la interfaz). Si está apagado,
el protocolo no mide nada: los pasos siguientes asumen que se está leyendo `/proc`.

Ejecutar los pasos **en este orden**; no dar por bueno un paso porque el anterior pasó.

### Paso 1 — Referencia sin juegos

Con todo cerrado, encender el observador y esperar una pasada. Se espera «no corre nada».

Si dice que hay un juego, es un **falso positivo**: anotar qué enseñó y no seguir. Los pasos
siguientes no aportan nada mientras la referencia esté contaminada.

### Paso 2 — Steam (un juego que no sea Deskrawl ni Palworld)

1. Lanzar desde Steam con Proton.
2. En menos de una pasada de sondeo (5 s), la pantalla debe nombrar **ese** juego.
3. Comprobar que el nombre mostrado es el comercial de la biblioteca, no `GameThread`,
   `pv-adverb`, `srt-bwrap` ni el `.exe`.
4. Cerrar el juego. La sesión **no** debe cerrarse en la primera pasada sin verlo: pasa a
   pérdida temporal y termina tras unas pasadas.
5. Volver a abrir el mismo juego: debe salir **sesión nueva** (otra ejecución), no
   reutilizar la anterior.

### Paso 3 — Lutris (el caso difícil)

Se elige un juego añadido a mano: `directory` vacía y `service`/`service_id` a NULL en
`pga.db`, con runner conocido. Es el caso real que motivó el prefijo de Wine.

1. Lanzarlo. Debe identificarse por el **prefijo** declarado en el `.yml`, no por carpeta.
2. Si hay dos juegos de Lutris que comparten prefijo, **no** debe elegir uno al azar:
   identidad desconocida es el resultado correcto.
3. Repetir cierre y reapertura como en el paso 2.

### Paso 4 — Heronite (`eslabong`)

1. Lanzarlo. El catálogo guarda el directorio **base** y el proceso declara `…/pfx`: debe
   reconocerse igualmente.
2. Comprobar que el nombre que aparece es `eslabong`, no `umu-run` ni `umu.exe`.

### Paso 5 — Dos juegos a la vez

Lanzar dos juegos distintos (dos lanzadores, si se puede) y comprobar que la pantalla nombra
**los dos**.

Antes se veía solo uno: es la regresión que se corrigió al dejar de saltar el escaneo cuando
«ya hay una sesión abierta».

Casos ya validados en el equipo: **Steam + Steam** (Palworld + Deskrawl) y **Lutris + Steam**
(Starsector + Deskrawl). **Heroic + Heronite** es la limitación conocida, no un caso a
resolver aquí.

### Paso 6 — Coste y caché

Con el observador encendido y sin juegos, mirar los contadores de `mod medida`
(`LECTURAS`, `ACIERTOS`, `ULTIMA_LECTURA`).

Lo esperado: muchas pasadas y muy pocas lecturas reales del catálogo (una cada 5 min, salvo
invalidación). Si se lee en cada pasada, la caché no está haciendo su trabajo: anotarlo como
defecto.

## Qué anotar de cada paso

- Qué juego, qué lanzador, y el **árbol de procesos real** si falla. El diagnóstico de
  procesos ya vuelca el paso; usar `crate::diagnostico_procesos`.
- Si el nombre mostrado era el comercial o el del ejecutable.
- Si la sesión se cerró antes de tiempo o se quedó abierta de más.
- Diferenciar siempre **simulado** (fixture) de **real** (este protocolo).

## Alcance de lo validado

La compatibilidad **individual** está validada en el equipo en Steam, Heroic, Heronite y
Lutris (tanto nativo como Wine). La simultaneidad está validada en Steam + Steam y en
Lutris + Steam. El estado concreto de cada juego y cada comprobación vive en `STATUS.md`; no
se replica aquí para que no quede desactualizado en dos sitios.
