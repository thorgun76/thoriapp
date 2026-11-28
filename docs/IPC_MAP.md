# Índice de comandos IPC

Frontera Rust ↔ interfaz de Thoriapp. **Completo pero sin firmas**: dice qué comando existe,
dónde se registra y qué método lo invoca, no cómo se usa. Consulta este fichero solo cuando
toques la frontera IPC; para navegar por funcionalidad usa `docs/CODE_MAP.md`.

**Recuentos verificados sobre `85d640c`.** El código es la fuente de verdad: si algo no
coincide, gana el código y se corrige aquí en el mismo cambio.

**74 comandos `#[tauri::command]`, 74 registrados** en
`main.rs::generate_handler!`, **70 expuestos en `src/lib/api.ts`**. Los cuatro restantes son
excepciones verificadas, no huecos.

Un comando nuevo requiere tocar **tres** sitios: la función con `#[tauri::command]`, la lista
de `generate_handler!` en `main.rs`, y un método en `src/lib/api.ts`.

### main.rs (36)

| Comando | Método en `api.ts` |
|---|---|
| `get_hardware` | `getHardware` |
| `get_rules` | `getRules` |
| `get_requirements` | `getRequirements` |
| `scan_library` | `scanLibrary` |
| `search_store` | `searchStore` |
| `scan_external_launchers` | `scanExternalLaunchers` |
| `refresh_library` | `refreshLibrary` |
| `heronite_uninstalled_count` | `heroniteUninstalledCount` |
| `get_local_cover` | `getLocalCover` |
| `diagnose_covers` | `diagnoseCovers` |
| `steam_launch_options` | `steamLaunchOptions` |
| `steam_launch_options_para_juego` | `steamLaunchOptionsParaJuego` |
| `game_info` | `gameInfo` |
| `analyze_log` | **—** (excepción 1) |
| `open_url` | `openUrl` |
| `detect_engine` | `detectEngine` |
| `get_engine_recommendations` | **—** (excepción 2) |
| `get_engine_recommendations_for_gpu` | `getEngineRecommendations` |
| `get_community_recommendations` | `getCommunityRecommendations` |
| `catalogo_de_ajustes` | `catalogoDeAjustes` |
| `analyze_launch_command` | `analyzeLaunchCommand` |
| `serialize_launch_command` | `serializeLaunchCommand` |
| `validate_launch_command` | `validateLaunchCommand` |
| `launch_command_states` | `launchCommandStates` |
| `launch_command_changes` | `launchCommandChanges` |
| `launch_command_remove_segment` | `launchCommandRemoveSegment` |
| `escape_launch_value` | **—** (excepción 3) |
| `known_wrappers` | `knownWrappers` |
| `compare_benchmark_groups` | `compareBenchmarkGroups` |
| `estado_observador` | `estadoObservador` |
| `configurar_observador` | `configurarObservador` |
| `listar_sesiones` | `listarSesiones` |
| `sesion_de_benchmark` | `sesionDeBenchmark` |
| `asignar_juego_a_sesion` | `asignarJuegoASesion` |
| `fijar_pista_de_juego` | `fijarPistaDeJuego` |
| `fijar_configuracion_de_sesion` | `fijarConfiguracionDeSesion` |

### benchmarks.rs (19)

| Comando | Método en `api.ts` |
|---|---|
| `save_benchmark` | `saveBenchmark` |
| `load_benchmarks` | `loadBenchmarks` |
| `load_benchmarks_for_game` | `loadBenchmarksForGame` |
| `is_log_already_saved` | `isLogAlreadySaved` |
| `delete_benchmark` | `deleteBenchmark` |
| `delete_benchmarks_for_game` | `deleteBenchmarksForGame` |
| `delete_all_benchmarks` | `deleteAllBenchmarks` |
| `ultimo_comando_registrado` | `ultimoComandoRegistrado` |
| `analyze_mangohud_log_command` | `analyzeMangohudLog` |
| `detect_mangohud_log_path` | `detectMangohudLogPath` |
| `check_mangohud_installed` | `checkMangohudInstalled` |
| `get_mangohud_log_duration` | `getMangohudLogDuration` |
| `set_mangohud_log_duration` | `setMangohudLogDuration` |
| `restore_mangohud_conf` | `restoreMangohudConf` |
| `compare_benchmarks_evaluada` | `compareBenchmarksEvaluada` |
| `save_comparison_evaluada` | `saveComparisonEvaluada` |
| `load_comparisons` | `loadComparisons` |
| `delete_comparison` | `deleteComparison` |
| `delete_all_comparisons` | `deleteAllComparisons` |

### profiles.rs (10)

`save_profile` → `saveProfile` · `load_profiles` → `loadProfiles` ·
`load_profiles_for_game` → **—** (excepción 4) · `update_profile` → `updateProfile` ·
`delete_profile` → `deleteProfile` · `reorder_profiles` → `reorderProfiles` ·
`export_profile` → `exportProfile` · `export_all_profiles` → `exportAllProfiles` ·
`import_profile_from_text` → `importProfileFromText` · `preview_import_profile` →
`previewImportProfile`.

### Otros módulos (9)

| Comando | Módulo | Método en `api.ts` |
|---|---|---|
| `scan_new_mangohud_logs` | `log_watcher.rs` | `scanNewMangohudLogs` |
| `list_mangohud_logs` | `log_watcher.rs` | `listMangohudLogs` |
| `mark_logs_processed` | `log_watcher.rs` | `markLogsProcessed` |
| `processed_logs_count` | `log_watcher.rs` | `processedLogsCount` |
| `forget_processed_logs` | `log_watcher.rs` | `forgetProcessedLogs` |
| `get_shader_cache` | `shaders.rs` | `getShaderCache` |
| `get_anticheat_info` | `anticheat.rs` | `getAnticheatInfo` |
| `get_proton_pulse_reports` | `proton_pulse.rs` | `getProtonPulseReports` |
| `get_runtime_env_command` | `preflight.rs` | `getRuntimeEnv` |

### Excepciones verificadas

| Comando | Estado | Comprobación |
|---|---|---|
| `analyze_log` | Registrado, **sin método**. Su lógica la cubre `analyze_mangohud_log_command`; se menciona en un comentario de `hooks/useBenchmarks.ts`. Vía heredada. | `rg 'analyze_log'` en `src/` solo da comentarios |
| `get_engine_recommendations` | Registrado, **sin método**. El frontend usa la variante `_for_gpu`. | `api.ts` solo invoca `get_engine_recommendations_for_gpu` |
| `escape_launch_value` | Registrado, **sin método ni consumidor**. Wrapper de `launch_command`. | `rg 'escape_launch_value'` en `src/` → 0 |
| `load_profiles_for_game` | Registrado, **sin método**. El filtrado por juego se hace en el frontend sobre `load_profiles`. | `rg` en `src/` solo da un comentario en `hooks/useProfiles.ts` |

**Métodos de `api.ts` declarados sin consumidor** (no son comandos): `asignarJuegoASesion`,
`forgetProcessedLogs`, `knownWrappers`, `listMangohudLogs`, `processedLogsCount`,
`scanExternalLaunchers`, `validateLaunchCommand`. Verificado con `rg` sobre `src/`.

---
