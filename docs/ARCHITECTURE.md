# Arquitectura de Thoriapp

Cómo está montado el programa **hoy**, no cómo debería estarlo. Distingue lo que el código
**garantiza** de lo que solo **pretende**; donde no hay garantía, se dice.

Para *dónde* está cada cosa se consulta `docs/CODE_MAP.md`. Para el estado de cada fase,
`STATUS.md`. Aquí no hay roadmap ni historial.

---

## 1. Las tres capas

```
┌─────────────────────────────────────────────────────────────┐
│ React + TypeScript (src/)                                   │
│   Presenta, pide, guarda estado de interfaz.                │
│   NO decide reglas de negocio, NO lee disco, NO re-evalúa.  │
└───────────────────────────┬─────────────────────────────────┘
                            │  invoke('comando', args)  ·  src/lib/api.ts
                            │  ← única frontera de datos
┌───────────────────────────▼─────────────────────────────────┐
│ Rust (src-tauri/src/)                                       │
│   main.rs  → 74 comandos #[tauri::command] registrados      │
│   Dominio  → engine, rules, benchmarks, evaluacion, profiles│
│   Infra    → hw, proceso_linux, steam*, net, covers, log*   │
└───────────────────────────┬─────────────────────────────────┘
                            │  fs / proc / lspci / xdg-open / red
                        Sistema, Steam, MangoHud
```

**Un solo binario.** No existe `lib.rs`: los 33 módulos de producción se declaran con `mod`
en `src-tauri/src/main.rs` (más 2 módulos de pruebas). Rust no expone biblioteca reutilizable;
toda la superficie pública hacia la interfaz son los comandos.

**La frontera tiene una sola puerta.** Las llamadas al backend salen únicamente de
`src/lib/api.ts`. Ningún componente ni hook invoca `invoke` directamente — verificado: no hay
`invoke(` fuera de `api.ts` y sus pruebas.

### Qué puede hacer cada capa (implementado)

| | Rust | React |
|---|---|---|
| Leer/escribir disco | Sí, es el único | No |
| Leer `/proc`, `lspci`, `localconfig.vdf` | Sí | No |
| Decidir si una comparación es válida | Sí | No |
| Calcular umbrales, márgenes, percentiles | Sí | No |
| Formatear para pantalla | Solo donde ya está decidido (§5) | Sí |
| Conservar estado de la ventana (vista, selección) | No | Sí |
### Intención arquitectónica, no garantía

- **«React no decide»** se cumple en el *cálculo*, pero **no en la formación de grupos de
  comparación**: la partición en «antes/después» se hace en `BenchmarksView.tsx` por índice.
  Es una decisión de dominio tomada en la interfaz. Está pendiente (fase 5.4.2).
- **«No dupliques lógica»** se cumple en casi todo, con una excepción abierta: la regla de
  identidad de juego existe dos veces — `coinciden_por_identidad` en `benchmarks.rs` y una
  comparación de `game_name` crudo en el comando `compare_benchmark_groups`. Divergen.

---

## 2. Flujos principales

### 2.1 Abrir la biblioteca y un juego

```
scan_library / scan_external_launchers        (launchers.rs, steam.rs)
  → api.scanLibrary / api.scanExternalLaunchers
  → useLibrary  → toGame(), summarizeLaunchers()      (hooks/useLibrary.ts)
  → LibraryView / GameSidebar / GameCard
  → al elegir: game_info, get_rules, get_requirements, steam_launch_options
  → useGameDetails → GameTabs → OptimizeTab / CompatTab
```

Sin persistencia: se recalcula en cada arranque y en `refresh_library`.

### 2.2 Analizar un comando y guardar un perfil

```
CommandBox (texto de Steam)
  → api.analyzeLaunchCommand → analyze_launch_command        (main.rs → launch_command.rs)
       parsea preservando argumentos, orden, comillas y wrappers
  → api.launchCommandStates / catalogoDeAjustes              (catalogo de ajustes)
  → el usuario cambia ajustes
  → api.serializeLaunchCommand / launchCommandChanges        (launch_command.rs)
  → api.saveProfile → save_profile → profiles.json
```

El **motor de comandos es no destructivo por contrato**: `launch_command.rs` distingue
segmentos conocidos, desconocidos y wrappers, y `ArgumentosConservados` los muestra. La
interfaz no reconstruye el comando.

### 2.3 Medir y comparar (el flujo más largo)

```
MangoHud escribe CSV + _summary.csv en disco
  → detect_mangohud_log_path / scan_new_mangohud_logs      (benchmarks.rs, log_watcher.rs)
  → analyze_mangohud_log / analyze_mangohud_log_command    (benchmarks.rs, mangohud.rs)
       métricas propias + ResumenOriginal como fuente separada (nunca decide)
  → save_benchmark → benchmarks.json  (con SnapshotConfig inmutable por prueba)

Comparación individual:
  compare_benchmarks_evaluada  → comparacion_entre → evaluar (evaluacion.rs)
  → ConfirmacionDeVistaPrevia
  → presentarEvaluacion (presentacionEvaluacion.ts) → UI
  → save_comparison_evaluada   → Rust recalcula, comprueba y escribe saved_comparisons.json

Comparación de grupos:
  compare_benchmark_groups → stats::compare_groups (bootstrap de medianas)
  → GroupComparisonCard. NO se persiste. Ver §6.
```

---

## 3. Fuentes de verdad por dominio

Regla general: **Rust decide, React muestra**. Donde la tabla dice «Rust», la interfaz no
puede re-derivar el dato.

| Dominio | Fuente de verdad | Vive en | ¿Solo lectura para React? |
|---|---|---|---|
| Comando de lanzamiento | `launch_command.rs` | memoria | Sí |
| Catálogo de ajustes y su efecto | `catalogo_lanzadores.rs`, `engine.rs`, `rules.rs` | memoria | Sí |
| Perfiles guardados | `profiles.rs` + `profiles.json` | disco | Sí |
| Métricas de una grabación | `benchmarks.rs` (`analyze_mangohud_log`) | `benchmarks.json` | Sí |
| Identidad de juego | `coinciden_por_identidad` (`benchmarks.rs`) | — | Sí |
| Evaluación de una comparación | `evaluacion.rs` | `saved_comparisons.json` | Sí |
| Estadística de grupos | `stats.rs` | **no se persiste** | Sí |
| Identidad y topología de sesión | `sessions.rs`, `proc` real | `sessions.json` | Sí |
| Hardware de la grabación | `mangohud.rs` (transcribe el CSV) | `benchmarks.json` | Sí |
| Hardware actual | `hw.rs` (`detect()`) | memoria | Sí |
| Presentación de una evaluación | `presentacionEvaluacion.ts` | memoria | **Es del frontend** |

**Dos bloques de hardware independientes, a propósito.** `hardware_grabacion` (lo que escribió
MangoHud) y `entorno` (lo detectado al analizar) no se rellenan mutuamente. Si difieren, la
interfaz avisa y no corrige nada.

**Procedencia.** Todo dato relevante viaja como `ConProcedencia<T>` + `Procedencia`: se sabe
si es declarado, inferido, observado o desconocido. Un hueco se queda en «no consta»; nunca se
rellena con el estado de hoy.

---

## 4. Contratos persistidos

Todos bajo `dirs::config_dir()/thoriapp/`. La versión es una constante en Rust; el frontend no
la interpreta.

| Fichero | Dueño | Constante de versión | Raíz | Notas |
|---|---|---|---|---|
| `benchmarks/benchmarks.json` | `benchmarks.rs` | `BENCHMARKS_SCHEMA_VERSION` | `BenchmarksData` | Historial por juego, máx. `MAX_BENCHMARKS_PER_GAME` |
| `benchmarks/saved_comparisons.json` | `benchmarks.rs` | `COMPARISONS_SCHEMA_VERSION` | `SavedComparisonsData` | Comparaciones evaluadas inmutables |
| `benchmarks/sessions.json` | `sessions_service.rs` | `SESSIONS_SCHEMA_VERSION` | servicio de sesiones | Observador; solo se escribe si está encendido |
| `profiles/profiles.json` | `profiles.rs` | `PROFILE_SCHEMA_VERSION` | lista de `Profile` | — |
| `watch_state.json` | `log_watcher_state.rs` | `MAX_REMEMBERED` | `WatchState` | Qué CSVs ya se procesaron |
| snapshot por prueba | `benchmarks.rs` | `SNAPSHOT_SCHEMA_VERSION` | `SnapshotConfig` | Inmutable; no se reconstruye |

**Garantías implementadas sobre estos ficheros:**

- **Escritura atómica**: `.tmp` + `rename` + `sync_dir`. Un corte deja el fichero anterior
  íntegro o el nuevo completo, no una mezcla.
- **Preservación de campos desconocidos**: `SavedComparisonsData` guarda el JSON original de
  cada comparación y los campos raíz que no entiende, y los reemite. Un campo de una versión
  futura no se pierde al reescribir.
- **Versión futura detectada**: `ErrorDatos::VersionFutura` bloquea la lectura en lugar de
  degradar. Los mensajes dicen qué **no** ha ocurrido.
- **Cerrojos entre comandos**: `CERROJO_BENCHMARKS` → `CERROJO_COMPARACIONES`, siempre en ese
  orden. Evitan pisarse entre sí dentro del proceso.
- **Conflicto de esquema en Lectura ≠ escritura**: cargar nunca escribe.

**Lo que no está garantizado:** los cerrojos son de proceso (`Mutex`). Dos instancias de la app
sobre el mismo `config_dir` no están coordinadas — deuda de fase 9.

---

## 5. Límites entre capas

1. **React no recalcula umbrales, márgenes ni veredictos.** El modelo único de presentación es
   `presentacionEvaluacion.ts`; `quienFavorece` sale de `estado`, nunca de una comparación de
   números hecha en la interfaz.
2. **Rust no formatea para la interfaz**, con dos excepciones ya decididas y con deuda
   registrada: `BenchmarkComparison.conclusion` y `GroupComparison.conclusion` son texto ya
   redactado en español. Impide traducir (fase 8) y no es el patrón del resto de contratos,
   que usa códigos semánticos.
3. **Lo ausente no se convierte en cero ni en valor por defecto.** Donde el contrato lo
   incumple se señala; un ejemplo abierto es `stats.rs`, que devuelve mediana `0.0` para un
   grupo vacío.
4. **Errores de lectura se clasifican en Rust** (`ErrorDatos`, `EstadoDeLaEvaluacionLeida`) y
   viajan ya clasificados; el frontend no distingue «no hay» de «no se pudo leer» adivinando.
5. **La persistencia no se reconstruye desde el estado actual.** Un snapshot histórico es
   inmutable; volver a pulsar «guardar» no lo refresca.

---

## 6. Las tres superficies de confianza

No son intercambiables y no afirman lo mismo. La interfaz las muestra juntas; conviene no
confundirlas al leerla.

| Superficie | Qué calcula | Qué **sí** afirma | Qué **no** afirma |
|---|---|---|---|
| Grabación suelta (descriptiva) | Métricas de un CSV | Cómo fue esa partida | Nada comparativo |
| Comparación individual evaluada (`evaluacion.rs`) | Dos grabaciones, cinco dimensiones separables | Si el cambio está dentro del margen **operativo** | Reproducibilidad entre partidas |
| Comparación de grupos (`stats.rs`) | Mediana de n por lado, bootstrap, IC 95 % | Si el intervalo de las medianas observadas supera un suelo por métrica | **Metodología validada**, comparabilidad, ni que el grupo sea homogéneo |

**Sobre la comparación de grupos, explícitamente: no está metodológicamente validada.** Lo que
hoy está implementado es un bootstrap de la diferencia de medianas con semilla fija y un suelo
por métrica; no hay criterio de elegibilidad del percentil 1 % (aplazado a 5.4.3), no se
comprueba hardware, software ni variable evaluada, y la partición de grupos la decide el
frontend. El intervalo describe la dispersión de las grabaciones que entraron, no la
variabilidad entre partidas del mismo experimento.

**Ruido no es margen.** `noise_percent` mide variación **dentro** de una grabación y alimenta
el margen de la comparación individual (`combined_noise`). El margen de grupos es un **suelo
fijo por métrica**, independiente de ese ruido. Son dos cantidades distintas y no deben
presentarse como la misma.

---

## 7. Observador de sesiones

Experimental, **apagado por defecto**, opcional. Apagado, no lee `/proc`. Los benchmarks no
dependen de él. Arquitectura, decisiones y garantías en `docs/sesiones-de-ejecucion.md`;
aquí solo su posición:

- Es el **único** componente que lee `/proc` de forma continua.
- Corre como servicio con estado compartido (`sessions_service.rs`), sondeando a intervalos
  fijos según su estado.
- Agrupa por **instancia de proceso**, no por nombre; la pista de juego que envía el frontend
  no es evidencia de identidad.
- Escribe `sessions.json` periódicamente y **también cuando los bytes no cambian**, lo que
  actualiza el `mtime`. Deuda conocida y registrada en `STATUS.md`.

---

## 8. Dónde NO mirar

- `src-tauri/target/`, `dist/`, `node_modules/`, `.vite-cache/` — generados.
- `tests/fixtures/mangohud/` — grabaciones reales; **no se modifican ni se borran**.
- `src-tauri/src/fixtures_procesos.rs` — topologías de `/proc` de prueba, no código de
  producción.
- `thoriapp-benchmark-prototipo-v2.html` — prototipo suelto, no forma parte del build.
