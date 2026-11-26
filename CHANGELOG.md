# Changelog

Cambios relevantes de Thoriapp. El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto usa [versionado semántico](https://semver.org/lang/es/).

## [1.2.0] — 2026-11-26

Trabajo acumulado desde `v1.1.0`: fases 5.1, 5.2 y 5.3.

### Añadido — Motor de comandos en Rust

- Motor propio que compone el comando final de lanzamiento: precedencia de envoltorios,
  marcador `%command%`, validación de argumentos y revisión del motor.
- **Conservación de argumentos desconocidos**: los argumentos que Thoriapp no interpreta se
  preservan literalmente y en su orden, en lugar de descartarse al reconstruir el comando.
- Vista previa del comando junto a su procedencia, para que se sepa qué se va a ejecutar y
  de dónde sale cada parte.

### Añadido — Perfiles y restauración

- Gestión de perfiles por juego con **restauración** fiel: guardar, aplicar y volver al estado
  anterior sin arrastrar cambios intermedios.
- Integridad de perfiles comprobada antes de aplicar; los datos que no se pueden leer se
  tratan como ausentes, nunca como un valor inventado.

### Añadido — Benchmarks e instantáneas de configuración

- Registro e importación de grabaciones de **MangoHud** (CSV y resumen), con ventana temporal.
- **Instantáneas de configuración** (`SnapshotConfig`): al guardar una prueba se congela la
  configuración con la que se hizo, incluida la procedencia de cada dato. A partir de ese
  momento la prueba es histórica: renombrar un perfil o cambiar Steam no la modifica.
- Comparación de pruebas con aislamiento por juego: historial, contadores y comparaciones no
  mezclan juegos distintos.
- `ejecucion_verificada` queda deliberadamente vacío: que Thoriapp tenga Proton configurado no
  demuestra que la prueba se hiciera con Proton.

### Añadido — Observador de ejecución (experimental, opcional)

- Detección de juegos en ejecución leyendo `/proc`, con agrupación por topología de procesos y
  correlación con el catálogo de lanzadores (Steam, Heroic, Heronite, Lutris).
- **Apagado por defecto**: si el usuario no lo enciende, no se lee `/proc` en absoluto.
- Sesiones de ejecución con identidad separada en **observada** y **comercial**; cuando no hay
  evidencia suficiente el resultado correcto es «identidad desconocida», no una suposición.
- Asociación de grabaciones a sesiones con niveles de confianza, y **asignación manual**
  disponible con prioridad sobre cualquier inferencia.

### Compatibilidad de lanzadores validada

Comprobada **en el equipo real** (CachyOS), no solo con fixtures:

| Lanzador | Juego probado | Resultado |
|---|---|---|
| Steam | Deskrawl, Palworld | Detección individual correcta |
| Heroic | Tattoo Tycoon | Detección individual correcta |
| Heronite | Eslabong | Detección individual correcta |
| Lutris (nativo) | Starsector | Nombre comercial correcto, estable 120 s |
| Lutris (Wine) | Star Valor | Detección individual correcta |

Simultaneidad validada en **Steam + Steam** (Palworld + Deskrawl) y **Lutris + Steam**
(Starsector + Deskrawl). Cierre de los seis juegos y reinicio de Thoriapp sin sesiones fantasma.

### Corregido

- **Producción y pruebas no ejecutaban el mismo pipeline**: la topología de producción solo
  contenía los procesos que ya habían pasado el prefiltro, mientras las pruebas recibían la
  topología completa. Corregido con un prefiltro único compartido.
- **Continuidad de sesión por identidad en vez de por instancia**: una coincidencia de nombre
  bastaba para adoptar procesos de otra ejecución. Ahora se exige pertenencia al grupo.
- **Duplicaciones de sesión** (Tattoo Tycoon, Eslabong, Palworld) y el bucle de Starsector que
  abría una sesión nueva en cada pasada.
- **Nombre observado de un proceso ajeno**: el grupo tomaba el nombre del primer miembro y no
  del representante, lo que llegó a mostrar `Heronite-x86_64` en tres juegos distintos.
- La **pista del frontend** ya no se usa como nombre observado: no es evidencia de identidad.
- `bajo_proton` solo marca Proton; `bajo_compatibilidad` cubre Wine y Proton.
- Unicidad de identificadores de sesión mediante sufijo, conservando el formato histórico.

### Seguridad

- **Dos vulnerabilidades corregidas en dependencias de Rust.** `cargo audit` sobre
  `src-tauri/Cargo.lock` detectó dos fallos con parche disponible, ambos transitivos de
  Tauri. Se aplicaron actualizaciones puntuales, sin actualización general:
  - `url` 2.5.8 → **2.5.9** (RUSTSEC-2026-0592): desbordamiento de pila en `Url::origin()`
    con URL `blob:` anidadas.
  - `cookie` 0.18.2 → **0.18.3** (RUSTSEC-2026-0594): los nonces de cookies privadas podían
    repetirse en procesos que hacen `fork`.
  - Arrastra a `form_urlencoded` 1.2.2 → 1.2.3, su dependencia directa.
- **La verificación de dependencias pasa a ejecutarse de forma reproducible**:
  `cargo audit` con versión fija (`0.23.0`) sobre `src-tauri/Cargo.lock`, y `npm audit
  --omit=dev` sobre las dependencias que se empaquetan. Ambas son bloqueantes.
- Quedan tres avisos de RustSec de categoría informativa, no bloqueantes y visibles en el
  registro: `chrono` y `proc-macro-error` sin mantener, y `glib` 0.18.5 con una implementación
  de iterador no sólida. Corregirlos exige subir Tauri y GTK, que es una actualización mayor
  fuera del alcance de esta entrega.

  **Limitaciones de este análisis.** Las comprobaciones se hicieron con las herramientas de
  RustSec sobre el lockfile; no son una auditoría manual del código de esas dependencias:

  - `glib`, `gtk`, `webkit2gtk` llegan **de forma transitiva** a través de `tauri` y `wry`.
    Thoriapp no las declara ni las invoca directamente, así que no se puede afirmar desde
    este repositorio qué partes exactas de la API usa la pila de WebKit en tiempo de
    ejecución. La revisión se limitó a comprobar que `glib::VariantStrIter` no aparece en el
    código de Thoriapp; **no** se ha auditado el código de GTK ni de WebKit.
  - `chrono` es dependencia directa y sí se usa (formato de fechas de benchmarks). El aviso
    es de falta de mantenimiento, no un fallo descrito, pero eso **no garantiza** que no
    vaya a aparecer uno: hoy no hay versión a la que actualizar.
  - `proc-macro-error` solo interviene en la compilación (`proc-macro`), no en el binario
    distribuido; aun así no se ha verificado qué macros generan el código que finalmente se
    enlaza.
  - El resultado depende de la base de datos de RustSec en el momento de ejecutar. Un aviso
    puede publicarse después y no aparecer hasta la siguiente ejecución.

  En consecuencia, el riesgo residual de estos tres avisos **no se ha medido**: se ha
  reducido la exposición donde estaba en esta entrega, pero determinar el riesgo real exige
  una revisión manual de las dependencias transitivas.

### Limitaciones conocidas

- **Detección simultánea de Eslabong (Heronite) y Tattoo Tycoon (Heroic): incompleta** en ambos
  órdenes de lanzamiento. La causa exacta no está demostrada y la incidencia queda aplazada;
  no bloquea la fase 5.4. Reabrirla exige capturar una pasada nueva con el observador activo:
  los diagnósticos existentes no incluyen ambas ejecuciones completas con sus catálogos.
- El observador es **experimental**. La fase 5.3 se cerró con esta limitación declarada.
- La detección es tan buena como el catálogo de lanzadores disponible: un juego que no esté en
  él y no aporte evidencia propia se queda como identidad desconocida.

### Versionado

Esta entrega es **1.2.0**, un cambio **menor**: añade funcionalidad (motor de comandos,
perfiles, benchmarks, observador) manteniendo la compatibilidad hacia atrás de datos y
comandos. Las correcciones de la fase 5.3 van dentro, sin justificar por sí solas un parche, y
ninguna funcionalidad existente se rompió de forma que exija una versión mayor.

Versión sincronizada en `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`,
`src-tauri/Cargo.lock` y `src-tauri/tauri.conf.json`. No se actualizó ninguna dependencia.

### Privacidad de los datos de ejemplo

Las rutas de los datos reales usados como fixtures se anonimizaron: el nombre de usuario se
sustituyó por `jugador` conservando la estructura completa de cada ruta y las relaciones entre
procesos. Los CSV de MangoHud incluidos no contienen rutas, usuarios ni identificadores
personales, solo modelo de hardware, kernel y métricas.
