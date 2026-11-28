# Mapa de código de Thoriapp

Índice de navegación. **No es documentación de comportamiento**: dice *dónde* está cada cosa
para no explorar el repositorio entero antes de empezar.

El **código es la fuente de verdad**. Si una entrada no coincide con el código, gana el código:
verifica y corrige el mapa en el mismo cambio.

---

## 0. Cómo usar este mapa

**Orden de consulta, del más barato al más caro:**

1. **¿Conoces la ruta o el símbolo?** Acceso directo. No abras este fichero.
2. **¿No sabes dónde está?** `## 1. Mapa de una ojeada` → una fila. Luego **solo la ficha**
   relevante de `## 2`.
3. **¿Necesitas responsabilidades, flujos o contratos?** `docs/ARCHITECTURE.md`.
4. **¿Necesitas el estado o por qué se decidió así?** `STATUS.md` **por búsqueda de sección**,
   nunca entero (≈205 KB: el fichero más caro del repositorio).
5. **PI Lens y RTK** cuando ahorren de verdad: `symbol_search` para saltar al símbolo,
   `module_report` antes de leer un módulo de más de 1000 líneas, `rtk read` para rangos.

**Localizar una ficha sin leer el fichero entero:**

```bash
rg -n '^## ' docs/CODE_MAP.md             # índice de secciones
rg -A 14 '^## Comparación de grupos' docs/CODE_MAP.md
```

**Convenciones.** Las fichas usan `ruta::símbolo` y **no llevan números de línea** a propósito:
un número desplazado miente en silencio; un símbolo renombrado falla a la vista. Los comandos
IPC se citan por su nombre registrado, sin el prefijo del módulo.

**Mantenimiento.** Actualiza la ficha en el mismo cambio que renombre, mueva o elimine una
ruta, símbolo o test que figure aquí, o que añada una funcionalidad. **No reproduzcas estado
ni decisiones**: eso vive en `STATUS.md`. Los comandos IPC tienen su propio índice
(`docs/IPC_MAP.md`); aquí solo se cita el que identifica la ficha. Recuentos verificados por
última vez sobre `85d640c`.

---

## 1. Mapa de una ojeada

| Necesito tocar… | Empieza en |
|---|---|
| Comando de lanzamiento (parsear, serializar, validar) | `launch_command.rs` |
| Catálogo de ajustes y qué hace cada uno | `catalogo_lanzadores.rs`, `engine.rs`, `rules.rs` |
| Bibliotecas, lanzadores externos, carátulas | `launchers.rs`, `steam.rs`, `covers.rs` |
| Perfiles guardados | `profiles.rs` |
| Métricas de una grabación MangoHud | `benchmarks.rs`, `mangohud.rs`, `log.rs` |
| Comparación individual evaluada | `evaluacion.rs`, `presentacionEvaluacion.ts` |
| Comparación de grupos | `stats.rs`, `GroupComparisonCard.tsx` |
| Snapshot de configuración por prueba | `benchmarks.rs` (`SnapshotConfig`) |
| Diagnóstico de shaders | `shaders.rs`, `lib/shaders.ts` |
| Hardware, rankings de CPU/GPU | `hw.rs`, `cpu_rank.rs`, `gpu_rank.rs` |
| Anticheat, ProtonDB, requisitos | `anticheat.rs`, `steam.rs`, `requirements.rs` |
| Observador de sesiones | `sessions_service.rs`, `sessions.rs` |
| Persistencia, versiones y cerrojos | `benchmarks.rs`, `profiles.rs` |
| Diagnóstico y entorno de ejecución | `diagnostico_procesos.rs`, `preflight.rs` |

---

## 2. Fichas por funcionalidad

Cada ficha: **Rust → IPC → contrato TS → cliente → hook/UI → tests**.

---

## Motor de comandos

Motor no destructivo: preserva argumentos desconocidos, su orden, comillas y wrappers.

- **Rust**: `src-tauri/src/launch_command.rs` — `analyze`, `serialize`, `validate`,
  `MotivoRechazo`, `LANZADORES_PERMITIDOS`, `REGLAS_WRAPPER`, `CARACTERES_PROHIBIDOS`.
  `src-tauri/src/preflight.rs` — `KNOWN_WRAPPERS`, `find_binary`.
- **IPC**: `analyze_launch_command`, `serialize_launch_command`, `validate_launch_command`,
  `launch_command_states`, `launch_command_changes`, `launch_command_remove_segment`,
  `escape_launch_value`, `known_wrappers`. En `main.rs`.
- **Contrato TS**: `src/lib/types.ts::OrigenComando`. Tipos de análisis en `src/lib/api.ts`.
- **Cliente**: `api.analyzeLaunchCommand`, `launchCommandStates`, `launchCommandChanges`,
  `launchCommandRemoveSegment`, `serializeLaunchCommand`, `validateLaunchCommand`,
  `knownWrappers`.
- **Hook/UI**: `src/hooks/useComando.ts::usarComando` → `src/components/CommandBox.tsx`
  (`ArgumentosConservados.tsx`, `SectionNav.tsx`).
- **Lógica pura TS**: `src/lib/command.ts`, `src/lib/motorComando.ts`,
  `src/lib/comandoManual.ts`, `src/lib/secuenciaComando.ts`.
- **Tests**: `launch_command.rs::tests` · `preflight.rs::tests` ·
  `components/CommandBox.test.tsx`, `CommandBoxSteam.test.tsx`,
  `ArgumentosConservados.test.tsx` · `hooks/useComando.test.tsx` · `lib/command.test.ts`,
  `motorComando.test.ts`, `comandoManual.test.ts`, `secuenciaComando.test.ts`.

---

## Catálogo de ajustes y recomendaciones

- **Rust**: `src-tauri/src/catalogo_lanzadores.rs`, `src-tauri/src/engine.rs` (`SIGNATURES`),
  `src-tauri/src/rules.rs`, `src-tauri/src/requirements.rs`.
- **IPC**: `catalogo_de_ajustes`, `detect_engine`, `get_engine_recommendations`,
  `get_engine_recommendations_for_gpu`, `get_community_recommendations`, `get_rules`,
  `get_requirements`. En `main.rs`.
- **Contrato TS**: `src/lib/types.ts::Param`, `GameRules`, `GameEngine`,
  `CommunityRecommendation`.
- **Cliente**: `api.catalogoDeAjustes`, `detectEngine`, `getEngineRecommendations`,
  `getCommunityRecommendations`, `getRules`, `getRequirements`.
- **Hook/UI**: `src/hooks/useCatalogoDeAjustes.ts` → `src/components/OptimizeTab.tsx`,
  `TarjetaAjuste.tsx`, `OptimizationSummary.tsx`, `RequirementsCard.tsx`, `RankingCard.tsx`.
  `SectionNav.tsx` es la navegación por secciones de `OptimizeTab` y `CompatTab`.
- **Lógica pura TS**: `src/lib/recommendations.ts`, `src/lib/ranking.ts`,
  `src/lib/umbrales.ts`.
- **Tests**: `catalogo_lanzadores.rs::pruebas` · `engine.rs::tests`, `rules.rs::tests`,
  `requirements.rs::tests` · `components/OptimizeTab.test.tsx`, `TarjetaAjuste.test.tsx`,
  `OptimizationSummary.test.tsx`, `RequirementsCard.test.tsx`, `RankingCard.test.tsx`,
  `SectionNav.test.tsx` · `lib/recommendations.test.ts`, `ranking.test.ts`,
  `umbrales.test.ts`.

---

## Biblioteca, lanzadores externos y carátulas

Unifica Steam, Lutris, Heroic y Heronite en un solo formato de juego.

- **Rust**: `src-tauri/src/launchers.rs`, `src-tauri/src/steam.rs`,
  `src-tauri/src/steam_launch.rs` (`localconfig.vdf`, **solo lectura**),
  `src-tauri/src/covers.rs`.
- **IPC**: `scan_library`, `scan_external_launchers`, `refresh_library`, `search_store`,
  `steam_launch_options`, `steam_launch_options_para_juego`, `heronite_uninstalled_count`,
  `get_local_cover`, `diagnose_covers`, `game_info`, `open_url`. En `main.rs`.
- **Contrato TS**: `src/lib/types.ts::Game`, `OpcionesSteam`, `GameInfo`,
  `LauncherFilter`, `BinaryInfo`.
- **Cliente**: `api.scanLibrary`, `scanExternalLaunchers`, `refreshLibrary`, `searchStore`,
  `steamLaunchOptions`, `steamLaunchOptionsParaJuego`, `heroniteUninstalledCount`,
  `getLocalCover`, `diagnoseCovers`, `gameInfo`, `openUrl`.
- **Hook/UI**: `src/hooks/useLibrary.ts` (`toGame`, `summarizeLaunchers`),
  `useLocalCover.ts`, `useSteamLaunch.ts` (`usarOpcionesSteam`), `useGameDetails.ts` →
  `src/components/LibraryView.tsx`, `GameSidebar.tsx`, `GameCard.tsx`, `GameBadges.tsx`,
  `GameTabs.tsx`, `AppidPicker.tsx`, `AvisoSteamDesincronizado.tsx`.
- **Lógica pura TS**: `src/lib/gameView.ts`, `src/lib/appid.ts`,
  `src/lib/appidOverride.ts`, `src/lib/dedupe.ts`, `src/lib/links.ts`,
  `src/lib/navigation.ts`.
- **Tests**: `launchers.rs::tests`, `pruebas_lutris_yml`, `pruebas_adaptadores_reales`,
  `pruebas_integracion_adaptadores` · `steam.rs::tests`, `steam_launch.rs::tests`,
  `covers.rs::tests` · `components/LibraryView.test.tsx`, `GameCard.test.tsx`,
  `GameSidebar.test.tsx`, `GameBadges.test.tsx`, `GameTabs.test.tsx`,
  `AppidPicker.test.tsx`, `AvisoSteamDesincronizado.test.tsx` · `hooks/useLibrary.test.ts`,
  `useLocalCover.test.ts`, `useSteamLaunch.test.tsx`, `useGameDetails.test.ts` ·
  `lib/gameView.test.ts`, `appid.test.ts`, `appidOverride.test.ts`, `dedupe.test.ts`,
  `navigation.test.ts`.

---

## Perfiles de configuración

- **Rust**: `src-tauri/src/profiles.rs` (`PROFILE_SCHEMA_VERSION`, `get_profiles_file`).
- **IPC**: `save_profile`, `load_profiles`, `load_profiles_for_game`, `update_profile`,
  `delete_profile`, `reorder_profiles`, `export_profile`, `export_all_profiles`,
  `import_profile_from_text`, `preview_import_profile`. En `profiles.rs`.
- **Contrato TS**: `src/lib/types.ts::Profile`, `ImportPreview`, `ImportedProfilePreview`.
- **Cliente**: `api.loadProfiles`, `saveProfile`, `updateProfile`, `deleteProfile`,
  `reorderProfiles`, `exportProfile`, `exportAllProfiles`, `importProfileFromText`,
  `previewImportProfile`.
- **Hook/UI**: `src/hooks/useProfiles.ts` → `src/components/PerfilesTab.tsx`,
  `PerfilesDelJuego.tsx`, `SaveProfileModal.tsx`, `ImportPreviewDialog.tsx`,
  `MenuRestablecer.tsx`, `SortableProfile.tsx`.
- **Tests**: `profiles.rs::tests` · `components/PerfilesTab.test.tsx`,
  `PerfilesDelJuego.test.tsx`, `SaveProfileModal.test.tsx`, `ImportPreviewDialog.test.tsx`,
  `MenuRestablecer.test.tsx` · `hooks/useProfiles.test.ts` ·
  `lib/regresionPerfiles.test.ts`.

---

## Benchmarks y métricas

Ingesta de CSV de MangoHud y cálculo de métricas propias. Historial por juego.

- **Rust**: `src-tauri/src/benchmarks.rs` (`analyze_mangohud_log`, `Benchmark`,
  `MuestrasTemporales`, `ApoyoDeMetricas`, `ApoyoDePercentil`, `ContabilidadDeMuestras`,
  `noise_percent`, `combined_noise`, `shader_stutter`, `MetodologiaCalculo`),
  `src-tauri/src/mangohud.rs` (`MetadatosDeGrabacion`), `src-tauri/src/log.rs`,
  `src-tauri/src/log_watcher.rs`, `src-tauri/src/log_watcher_state.rs`.
  **`benchmarks.rs` es multi-dominio y tiene ~10 900 líneas: no lo leas entero, ve por
  símbolo** (`symbol_search`, `read_symbol`).
- **IPC**: `analyze_mangohud_log_command`, `save_benchmark`, `load_benchmarks`,
  `load_benchmarks_for_game`, `is_log_already_saved`, `delete_benchmark`,
  `delete_benchmarks_for_game`, `delete_all_benchmarks`, `ultimo_comando_registrado`,
  `detect_mangohud_log_path`, `check_mangohud_installed`, `get_mangohud_log_duration`,
  `set_mangohud_log_duration`, `restore_mangohud_conf`; más `scan_new_mangohud_logs`,
  `list_mangohud_logs`, `mark_logs_processed`, `processed_logs_count`,
  `forget_processed_logs` (en `log_watcher.rs`).
- **Contrato TS**: `src/lib/types.ts::Benchmark`, `MuestrasTemporales`, `SnapshotConfig`;
  `src/lib/api.ts::SaveBenchmarkPayload`.
- **Cliente**: `api.loadBenchmarks`, `loadBenchmarksForGame`, `analyzeMangohudLog`,
  `saveBenchmark`, `isLogAlreadySaved`, `deleteBenchmark`, `deleteBenchmarksForGame`,
  `deleteAllBenchmarks`, `ultimoComandoRegistrado`, `detectMangohudLogPath`,
  `checkMangohudInstalled`, `getMangohudLogDuration`, `setMangohudLogDuration`,
  `restoreMangohudConf`, `scanNewMangohudLogs`, `listMangohudLogs`, `markLogsProcessed`,
  `processedLogsCount`, `forgetProcessedLogs`.
- **Hook/UI**: `src/hooks/useBenchmarks.ts` (`MismoJuego`, `MAX_BENCHMARKS_PER_GAME`),
  `useMangoHud.ts`, `useLogWatcher.ts` → `src/components/BenchmarksView.tsx`,
  `PruebasJuego.tsx`, `PruebaN.tsx`, `GraficoTemporal.tsx`, `DashboardView.tsx`.
- **Lógica pura TS**: `src/lib/noise.ts`, `src/lib/estabilidad.ts`,
  `src/lib/pruebaLabel.ts`, `src/lib/duracion.ts`.
- **Tests**: `benchmarks.rs::tests`, `pruebas_quinta_correccion`, `pruebas_aislamiento` ·
  `mangohud.rs::tests`, `log_watcher.rs::tests`, `log_watcher_state.rs::tests` ·
  `components/BenchmarksView.test.tsx`, `PruebasJuego.test.tsx`, `PruebaN.test.tsx`,
  `GraficoTemporal.test.tsx`, `DashboardView.test.tsx` · `hooks/useBenchmarks.test.tsx`,
  `useMangoHud.test.ts`, `useLogWatcher.test.tsx` · `lib/noise.test.ts`,
  `estabilidad.test.ts`, `pruebaLabel.test.ts`, `duracion.test.ts` ·
  fixtures reales en `tests/fixtures/mangohud/` (no se modifican).

---

## Comparación individual evaluada

Dos grabaciones, cinco dimensiones inseparables. Es la superficie de confianza mejor
documentada: motor puro en Rust, sin recálculo en la interfaz.

- **Rust**: `src-tauri/src/evaluacion.rs` — `evaluar`, `condiciones_de`,
  `ConfirmacionDeVistaPrevia`, `comprobar_confirmacion`, `MotivoDeRechazo`,
  `MIN_TIE_PERCENT`, `METODOLOGIA_INTERPRETACION` y los códigos `CONFIANZA_*`
  (en `benchmarks.rs`: `comparacion_entre`, `coinciden_por_identidad`, `mismo_juego`).
- **IPC**: `compare_benchmarks_evaluada`, `save_comparison_evaluada` (en `benchmarks.rs`).
- **Contrato TS**: `src/lib/api.ts::EvaluacionPersistida`, `EvaluacionDeMetrica`,
  `SavedComparison`; `src/lib/types.ts::BenchmarkComparison`.
- **Cliente**: `api.compareBenchmarksEvaluada`, `saveComparisonEvaluada`.
- **Hook/UI**: `src/hooks/useComparison.ts` (`claveDePar`, `claveDeSeleccion`) →
  `src/lib/presentacionEvaluacion.ts` (`presentarEvaluacion`, `tituloDeCabecera`) →
  `src/components/BenchmarksView.tsx`, `PruebasJuego.tsx`.
- **Lógica pura TS**: `src/lib/etiquetas.ts`.
- **Tests**: `evaluacion.rs::pruebas`, `::pruebas_c2`, `::pruebas_c3_20` (en
  `src-tauri/src/evaluacion/`) · `benchmarks.rs::tests` · `hooks/useComparison.test.tsx` ·
  `lib/presentacionEvaluacion.test.ts`, `sinReinterpretacion.test.ts`,
  `regresionesC3.test.ts` · coherencia entre vistas: `components/matrizCoherencia.test.tsx`,
  `consistencia.test.tsx`, `verdict.test.tsx`.

---

## Comparación de grupos

Resumen estadístico de varias grabaciones por lado. **No persistida y no metodológicamente
validada** — ver `docs/ARCHITECTURE.md` §6.

- **Rust**: `src-tauri/src/stats.rs` — `compare_groups`, `GroupComparison`, `Metric`,
  `MIN_RUNS`, `PERCENTILE_NOISE_FLOOR_PERCENT`. Único consumidor: `main.rs`.
- **IPC**: `compare_benchmark_groups` (en `main.rs`, no en `benchmarks.rs`).
- **Contrato TS**: `src/lib/types.ts::GroupComparison`.
- **Cliente**: `api.compareBenchmarkGroups`.
- **Hook/UI**: `src/components/BenchmarksView.tsx::useGroupComparison` (partición por mitades
  en el frontend) y `PruebasJuego.tsx` → `src/components/GroupComparisonCard.tsx`.
- **Puntos de entrada múltiples**: `GroupComparisonCard` se renderiza desde **dos** vistas
  (`BenchmarksView` y `PruebasJuego`, esta última montada por `App.tsx`). Cualquier cambio de
  contrato afecta a las dos.
- **Tests**: `stats.rs::tests` · `components/GroupComparisonCard.test.tsx`,
  `PruebasJuego.test.tsx`. **No hay test del comando IPC ni de `useGroupComparison`.**

---

## Snapshots de configuración e instantáneas

Fotografía inmutable por prueba: identidad, configuración y entorno declarado.

- **Rust**: `src-tauri/src/benchmarks.rs` — `SnapshotConfig`, `IdentidadInstantanea`,
  `ConfigInstantanea`, `EntornoInstantanea`, `HardwareDeLaGrabacion`, `ResumenOriginal`,
  `ConProcedencia`, `Procedencia`, `SNAPSHOT_SCHEMA_VERSION`. Detección del entorno actual en
  `src-tauri/src/hw.rs::detect`.
- **IPC**: dentro de `save_benchmark` / `load_benchmarks`; no hay comando propio.
- **Contrato TS**: `src/lib/types.ts::SnapshotConfig`, `IdentidadInstantanea`,
  `ConfigInstantanea`, `EntornoInstantanea`, `HardwareDeLaGrabacion`, `ResumenOriginal`,
  `ConProcedencia`, `Procedencia`.
- **Hook/UI**: `src/hooks/useBenchmarks.ts` → `src/components/InstantaneaCard.tsx`.
- **Tests**: `components/InstantaneaCard.test.tsx` · `hw.rs::tests`, `mangohud.rs::tests` ·
  guardas de datos: `src/lib/hardwareData.test.ts` (valida `src-tauri/games.json` contra los
  tags de `hw.rs`) y `src/lib/origen.test.ts` (valida `sufijoOrigen`/`textoComando` de
  `BenchmarksView`) **sin módulo fuente propio** — no borrar como «huérfanos» ·
  interpretación de métricas en `docs/mangohud-summary.md`.

---

## Diagnóstico de shaders

- **Rust**: `src-tauri/src/shaders.rs` — `get_shader_cache`, `ShaderCache`.
  Ventana y umbral en `benchmarks.rs` (`SHADER_WINDOW_SECONDS`,
  `SIGNIFICANT_SHADER_PENALTY`, `ShaderStutter`).
- **IPC**: `get_shader_cache` (en `shaders.rs`).
- **Cliente**: `api.getShaderCache`.
- **Hook/UI**: `src/hooks/useShaderCache.ts` → `src/components/ShaderStutterCard.tsx`.
- **Lógica pura TS**: `src/lib/shaders.ts`.
- **Tests**: `shaders.rs::tests` · `components/ShaderStutterCard.test.tsx` ·
  `lib/shaders.test.ts`.

---

## Hardware y rankings

Dos fuentes que **no se mezclan**: lo que registró MangoHud y lo detectado al analizar.

- **Rust**: `src-tauri/src/hw.rs` (`Hardware`, `Vendor`, `detect`),
  `src-tauri/src/cpu_rank.rs` (`CPUS`), `src-tauri/src/gpu_rank.rs` (`GPUS`).
- **IPC**: `get_hardware` (en `main.rs`).
- **Contrato TS**: `src/lib/types.ts::Hardware`, `HardwareResponse`.
- **Cliente**: `api.getHardware`.
- **Hook/UI**: `src/hooks/useHardware.ts` → `src/components/DashboardView.tsx`,
  `RankingCard.tsx`, `GameCard.tsx`.
- **Lógica pura TS**: `src/lib/ranking.ts`, `src/lib/format.ts`.
- **Tests**: `hw.rs::tests`, `cpu_rank.rs::tests`, `gpu_rank.rs::tests` ·
  `components/DashboardView.test.tsx`, `RankingCard.test.tsx` · `lib/ranking.test.ts`,
  `format.test.ts`, `hardwareData.test.ts` (guarda de `games.json`, ver *Snapshots*).

---

## Anticheat, ProtonDB y requisitos

- **Rust**: `src-tauri/src/anticheat.rs`, `src-tauri/src/proton_pulse.rs`,
  `src-tauri/src/requirements.rs`, `src-tauri/src/net.rs` (`MAX_RESPONSE_BYTES`,
  `MAX_CATALOG_BYTES`).
- **IPC**: `get_anticheat_info` (en `anticheat.rs`), `get_proton_pulse_reports`
  (en `proton_pulse.rs`), `get_requirements` (en `main.rs`).
- **Cliente**: `api.getAnticheatInfo`, `getProtonPulseReports`, `getRequirements`.
- **Hook/UI**: `src/hooks/useGameDetails.ts` → `src/components/CompatTab.tsx`,
  `ProtonPulseCard.tsx`, `RequirementsCard.tsx`.
- **Lógica pura TS**: `src/lib/anticheat.ts`.
- **Tests**: `anticheat.rs::tests`, `proton_pulse.rs::tests`, `requirements.rs::tests`,
  `net.rs::tests` · `components/CompatTab.test.tsx`, `ProtonPulseCard.test.tsx`,
  `RequirementsCard.test.tsx` · `lib/anticheat.test.ts`. El test
  `anticheat::tests::manual_descarga_y_busca_appids_reales` está `#[ignore]`: usa red real.

---

## Observador de sesiones

Experimental, **apagado por defecto**. Único componente que lee `/proc` de forma continua.
Arquitectura y garantías: `docs/sesiones-de-ejecucion.md`.

- **Rust**: `src-tauri/src/sessions_service.rs` (`ServicioDeSesiones`, `servicio`,
  `servicio_en`, `EstadoDelObservador`, `ruta_por_defecto`),
  `src-tauri/src/sessions.rs` (`SesionDeEjecucion`, `IdentidadDeSesion`, `RasgosDeProceso`,
  `EvidenciaProceso`, `OrigenAppId`, `decidir_asociacion`, `SESSIONS_SCHEMA_VERSION`),
  `src-tauri/src/proceso_linux.rs`, `src-tauri/src/evidencia_proceso.rs`,
  `src-tauri/src/diagnostico_procesos.rs` (`VARIABLE`),
  `src-tauri/src/fixtures_procesos.rs` (topologías de `/proc` de prueba).
- **IPC**: `estado_observador`, `configurar_observador`, `listar_sesiones`,
  `sesion_de_benchmark`, `asignar_juego_a_sesion`, `fijar_pista_de_juego`,
  `fijar_configuracion_de_sesion`. Todos en `main.rs`.
- **Cliente**: `api.estadoObservador`, `configurarObservador`, `listarSesiones`,
  `sesionDeBenchmark`, `asignarJuegoASesion`, `fijarPistaDeJuego`,
  `fijarConfiguracionDeSesion`.
- **Hook/UI**: sin hook propio. Consumidores directos: `src/App.tsx`
  (`fijarConfiguracionDeSesion`), `src/components/InstantaneaCard.tsx`
  (`sesionDeBenchmark`), `src/components/BenchmarksView.tsx` (`fijarPistaDeJuego`).
- **Tests**: `sessions.rs::tests`, `sessions_service.rs::tests`, `pruebas_id_de_sesion`,
  `pruebas_continuidad_de_sesion` · `proceso_linux.rs::tests`,
  `pruebas_topologia_real` · `evidencia_proceso.rs` (6 módulos) ·
  `diagnostico_procesos.rs::pruebas`.

---

## Persistencia, versiones y cerrojos

No hay comando propio: cada operación escribe a través de su módulo dueño.

- **Rust**: `benchmarks.rs` — `get_benchmarks_dir`, `get_benchmarks_file`,
  `SavedComparisonsData`, `ErrorDatos`, `escribir_atomico`, `CERROJO_BENCHMARKS`,
  `CERROJO_COMPARACIONES`, `BENCHMARKS_SCHEMA_VERSION`, `COMPARISONS_SCHEMA_VERSION`.
  `profiles.rs` — `get_profiles_dir`, `get_profiles_file`.
  `sessions_service.rs::ruta_por_defecto`. `log_watcher_state.rs::state_file`.
- **IPC**: `load_comparisons`, `delete_comparison`, `delete_all_comparisons` (en
  `benchmarks.rs`).
- **Contrato TS**: `SavedComparison`, `ComparacionLeida`, `EstadoDeLaEvaluacionLeida`.
- **Cliente**: `api.loadComparisons`, `deleteComparison`, `deleteAllComparisons`.
- **Hook/UI**: `src/hooks/useSavedComparisons.ts` (`COMPARISONS_CHANGED_EVENT`,
  `notifyComparisonsChanged`) → `src/components/SavedComparisonDetail.tsx`.
- **Tests**: `benchmarks.rs::pruebas_aislamiento` · `profiles.rs::tests` ·
  `sessions_service.rs::tests` · `log_watcher_state.rs::tests` ·
  `components/SavedComparisonDetail.test.tsx`, `comandoRegistrado.test.tsx` (prueba
  `SavedComparisonDetail`, `diffComandos`, `tokenizarComando`) · `lib/api.test.ts`.
- **Antes de tocar un formato persistido**: contrato completo en
  `docs/ARCHITECTURE.md` §4.

---

## Diagnóstico, entorno y arranque

- **Rust**: `src-tauri/src/main.rs` (setup, un solo binario, sin `lib.rs`),
  `src-tauri/src/preflight.rs` (`get_runtime_env_command`),
  `src-tauri/src/diagnostico_procesos.rs`, `src-tauri/src/test_entorno.rs`.
- **IPC**: `get_runtime_env_command` (en `preflight.rs`).
- **Contrato TS**: `src/lib/types.ts::RuntimeEnv`.
- **Cliente**: `api.getRuntimeEnv`.
- **Hook/UI**: `src/hooks/useRuntimeEnv.ts` → `src/components/Topbar.tsx`, `App.tsx`.
- **Lógica pura TS**: `src/lib/preflight.ts`.
- **Tests**: `preflight.rs::tests`, `test_entorno.rs::tests`,
  `diagnostico_procesos.rs::pruebas` · `lib/preflight.test.ts`.

---

## Presentación transversal

Afecta a todas las vistas; no pertenece a una funcionalidad.

- **Modelo único**: `src/lib/presentacionEvaluacion.ts` — `presentarEvaluacion`,
  `presentarGuardada`, `estadoDeEvaluacionGuardada`, `tituloDeCabecera`,
  `ModeloDePresentacion`.
- **Etiquetas sin interpretar**: `src/lib/etiquetas.ts`.
- **Consumidores**: `BenchmarksView.tsx`, `SavedComparisonDetail.tsx`, `DashboardView.tsx`.
- **Envoltorio de errores y UI base**: `src/components/BarreraDeErrores.tsx`,
  `Toggle.tsx`, `ConfirmDialog.tsx`, `Topbar.tsx`, `Sidebar.tsx`; hooks `useConfirm.ts`.
  `Sidebar.tsx` y `Topbar.tsx` los monta `src/App.tsx`; no tienen hook ni comando propio.
- **Tests transversales**: `components/matrizCoherencia.test.tsx`,
  `consistencia.test.tsx`, `verdict.test.tsx`, `responsive.test.tsx`,
  `regresionPantallaNegra.test.tsx`, `regresionesResiduales.test.tsx`,
  `BarreraDeErrores.test.tsx`, `Toggle.test.tsx`, `Sidebar.test.tsx`, `Topbar.test.tsx` ·
  `lib/presentacionEvaluacion.test.ts`, `sinReinterpretacion.test.ts` · `src/build.test.ts`.

---

## 3. Comandos IPC

**74 comandos registrados, 70 expuestos en `api.ts`.** Índice completo con la correspondencia
comando → método, el módulo dueño y las 4 excepciones verificadas: **`docs/IPC_MAP.md`**.

Aquí solo lo que hace falta para orientarse: registrar un comando nuevo exige **tres** sitios
(`#[tauri::command]`, `generate_handler!` en `main.rs`, método en `src/lib/api.ts`); el módulo
dueño de cada comando se indica en su ficha de §2; y 7 métodos de `api.ts` no tienen consumidor
(listados en `IPC_MAP.md`).

---

## 4. Contratos persistidos

Puntero: detalle, garantías y límites en `docs/ARCHITECTURE.md` §4. Aquí solo el símbolo raíz.

| Fichero | Módulo dueño | Símbolo raíz |
|---|---|---|
| `benchmarks.json` | `benchmarks.rs` | `BenchmarksData` |
| `saved_comparisons.json` | `benchmarks.rs` | `SavedComparisonsData` |
| `sessions.json` | `sessions_service.rs` | servicio de sesiones |
| `profiles.json` | `profiles.rs` | `Vec<Profile>` |
| `watch_state.json` | `log_watcher_state.rs` | `WatchState` |

---

## 5. Tests

**Colocados junto al código, sin directorio central.** 81 ficheros TS; `#[cfg(test)]` en 31
módulos Rust.

| Ámbito | Dónde | Notas |
|---|---|---|
| Componentes React | `src/components/*.test.tsx` (43) | Junto al componente |
| Hooks | `src/hooks/*.test.ts(x)` (10) | — |
| Lógica pura TS | `src/lib/*.test.ts` (27) | — |
| Guardas de datos | `src/lib/hardwareData.test.ts`, `src/lib/origen.test.ts` | **Sin módulo fuente**: validan un JSON y funciones de un componente |
| Build | `src/build.test.ts` | Único no colocado |
| Módulos Rust | `mod tests` / `mod pruebas` dentro del propio `.rs` | 31 módulos |
| Motor de evaluación | `src-tauri/src/evaluacion/pruebas*.rs` | `pruebas`, `pruebas_c2`, `pruebas_c3_20` |
| Fixtures reales | `tests/fixtures/mangohud/` | No se modifican ni se borran |

**Ejecutar:** los comandos están en `AGENTS.md` § Validaciones; no se repiten aquí.

**Qué no demuestran:** un fixture en verde no prueba la detección en el equipo real. Ver
`docs/validacion-real.md`.

---

## 6. Trampas de navegación

- **`benchmarks.rs` (~10 900 líneas, 458 funciones) es multi-dominio**: métricas, snapshots,
  comparaciones, MangoHud y persistencia. Ve siempre por símbolo.
- **`sessions_service.rs` (~4 900 líneas)** es un singleton con estado en disco; sus pruebas
  usan `servicio_en` para aislarse.
- **`launch_command.rs` (~4 500 líneas)** tiene el parseo concentrado; los comandos de
  `main.rs` solo lo envuelven.
- **`compare_benchmark_groups` vive en `main.rs`**, no en `benchmarks.rs` como el resto de
  comandos de medición.
- **`fixtures_procesos.rs`** no es producción: son topologías de `/proc` de prueba.
