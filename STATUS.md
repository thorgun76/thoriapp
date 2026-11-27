# Estado del proyecto

Última actualización: **5.3 CERRADA** con una limitación conocida —validada en el equipo real
en Steam, Heroic, Heronite y Lutris; la simultaneidad Heroic + Heronite queda sin garantizar y
la incidencia se aplaza.

## Fases

| Fase | Estado |
|---|---|
| 1 — Motor de comandos | Cerrada |
| 1.1 — Precedencia de wrappers | Cerrada |
| 2 — Catálogo y ajustes | Cerrada |
| 3 — Perfiles y Steam | Cerrada |
| 4 — Integridad de perfiles | Cerrada y verificada por el usuario |
| **5.1 — Almacenamiento e integridad** | **Cerrada** |
| **5.2 — Instantáneas de configuración** | **Cerrada** (incluye metadatos de MangoHud) |
| **5.3 — Captura temprana y evidencias de ejecución** | **Cerrada con limitación conocida.** El observador es **experimental y opcional, apagado por defecto**. Compatibilidad **individual validada en el equipo** en Steam, Heroic, Heronite y Lutris (Wine); simultaneidad validada en **Steam + Steam** (Palworld + Deskrawl) y **Lutris + Steam** (Starsector + Deskrawl). **Heroic + Heronite no garantizada** (ver «Limitación conocida»). |
| 5.4 — Calidad de comparación | **En curso.** 5.4.0 (auditoría estadística) **cerrada**. 5.4.1 dividida en A/B/C; **bloque A cerrado**, **bloque B cerrado** (persistencia de evaluaciones); **C.1 cerrado** (motor puro desconectado de producción); C.2 y C.3 sin comenzar. |
| 6 — Rediseño visual | Pendiente |
| 7 — (reservada) | Pendiente |
| **8 — Internacionalización (i18n)** | **Pendiente, no empezada.** Registrada en `docs/planificacion-fases.md`. |
| **9 — Seguridad y hardening** | **Pendiente, no empezada.** Auditoría avanzada; **no** autoriza a posponer vulnerabilidades conocidas. |
| **10 — Distribución pública** | **Pendiente, no empezada.** |

## 5.4.1.C.1 — motor de interpretación (**cerrado**)

Motor **puro** que traduce el contrato numérico de B (`BenchmarkComparison`) en el
semántico (`EvaluacionPersistida`): qué se puede afirmar de una comparación y con qué
apoyo. Sin reloj, sin disco, sin IPC y **sin conectar a producción**: el módulo se
registra con `#[cfg(test)] mod evaluacion;` y no lo referencia ningún camino de ejecución.

### Qué hace

- `evaluar(&BenchmarkComparison, &CondicionesDeComparacion) -> EvaluacionDeComparacion`.
  Sin `Result`: todo caso admitido tiene representación. Lo que falta es «no consta»,
  nunca cero.
- `condiciones_de(&BenchmarkComparison, Option<VariableEvaluada>)
  -> CondicionesDeComparacion`. Aplica el margen **`max(5 %, ruido combinado)`** y
  distingue tres cosas: ruido original **ausente** (`None`), **estimación con suelo**
  (el margen no baja de 5 %) y **margen aplicado** (lo que se guarda).

### Distinciones que el motor mantiene separadas

magnitud · dirección · apoyo · elegibilidad · preferencia · participación. Una métrica
puede tener magnitud y dirección observadas y **no** ser elegible: se registra y no decide.

Los niveles de **confianza** son un vocabulario cerrado y propio —`no_concluyente`,
`observada`, `insuficiente`—, **distinto** del de estado. Un estado dice *qué* cambió; la
confianza dice *qué lo sostiene*. Escribir `mejora` en el campo `confianza` es un error.

Códigos emitidos, sobre el campo `limitaciones`/`motivos` **existente** de B (sin campos ni
estados nuevos): `participo`, `dentro_del_margen`, `favorece_candidata`,
`favorece_referencia`, `sin_datos`, `valor_no_valido`, `no_elegible`, `apoyo_no_consta`,
`apoyo_insuficiente`, `evaluacion_parcial`, `perdida_observada_sin_evaluar`,
`cobertura_insuficiente_para_conclusion_global`, `sin_metricas_medibles`,
`compatibilidad_no_establecida`, `sin_margen`.

### Correcciones aplicadas durante la revisión final de C.1

1. **Un llamador podía fabricar compatibilidad.** `evaluar` aceptaba
   `CompatibilidadMetodologica::Establecida` para dos grabaciones sin metodología
   declarada y producía `mejora`. Ahora la compatibilidad se toma de los **benchmarks** y
   gana siempre el criterio más conservador: una condición externa no puede afirmar una
   comparabilidad que las grabaciones no establecen.
2. **`sin_cambio` sin margen.** Sin margen no se juzgó nada; el estado vacío caía en
   `sin_cambio`, que afirma que todo quedó dentro de un margen inexistente. Ahora es
   `datos_insuficientes` con motivo `sin_margen`.
3. **Preferencia superviviente a `solo_descriptiva`.** El estado decía «descriptiva»
   mientras cada métrica conservaba `FavorDeCandidata`: un ganador alcanzable por
   cualquier consumidor. Ahora la preferencia se neutraliza en las métricas.
4. **Ruido medido de cero confundido con ausente.** Se colapsaba `Some(0.0)` a `None`.
   Un 0 medido dice «no se detectó variación»; `None` dice «no se estimó».
5. **Participación derivada de la preferencia.** Una métrica dentro del margen participa
   (es la razón de `sin_cambio`) sin favorecer a nadie. `participo` se registra aparte.
6. **La confianza contenía códigos de estado.** `confianza` llevaba `mejora`,
   `sin_cambio`, `solo_descriptiva` o `datos_insuficientes`: el estado repetido. Ahora usa
   solo niveles de confianza. Además, superar el margen y ser elegible **no** dan confianza
   alta: como mucho, `observada` (se midió por encima del ruido de muestreo), que no es
   reproducibilidad demostrada.
7. **Una métrica excluida conservaba `favorece_referencia`.** El campo `preferencia` se
   neutralizaba, pero el motivo de preferencia ya se había añadido: el motivo afirmaba por
   la puerta de atrás lo que el campo negaba. Ahora se retiran los dos a la vez. La misma
   contradicción existía en `solo_descriptiva` (se retiraba `participo`, no `favorece_*`) y
   se ha corregido también. La métrica excluida conserva magnitud, dirección numérica y
   motivo de exclusión, pero no favorece a nadie.
8. **Fórmula de ruido duplicada.** El motor replicaba la cuadratura de
   `combined_noise`. Ahora la reutiliza directamente (`pub(crate)`). El valor combinado
   que se registra es el que devuelve esa función, con su suelo `MIN_NOISE`; el margen,
   en cambio, tiene su propio suelo del 5 %. Por eso 0/0 registra `3.0` de ruido y `5.0`
   de margen: no es una contradicción, son dos suelos distintos.
   Nota: `combined_noise` filtra `> 0`, así que no puede distinguir «0 medido» de
   «ausente»; esa distinción se resuelve en `condiciones_de` mirando los valores
   originales, sin tocar la función.

### Validación ejecutada

- `cargo test --locked --bins`: **991 aprobadas, 0 fallidas, 1 ignorada** (`--list`: 992).
  La ignorada es `anticheat::tests::manual_descarga_y_busca_appids_reales`, previa y manual.
- Inventario de pruebas frente a `ec32935` (`git worktree`, conteo real): **953 → 992**,
  **+39, 0 eliminadas**, las 39 de `evaluacion::`. No se modificó ninguna prueba anterior.
- `cargo clippy --locked --bins -- -D warnings`: **exit 0**.
- `cargo clippy --locked --bins --tests`: **exit 0**, **0 avisos en `evaluacion.rs`**.
  Quedan **42** avisos, todos de deuda previa (12 `evidencia_proceso.rs`, 8 `benchmarks.rs`,
  6 `launch_command.rs`, 6 `hw.rs`, y el resto repartidos). Ninguno corresponde a
  `combined_noise`. La deuda registrada **no se amplió**; no se añadió ningún `#[allow]`.
- `npx tsc --noEmit`: exit 0. `npx vitest run`: **1058 aprobadas**, igual que en B.
- `npm run build`: exit 0. `npm audit --omit=dev`: 0 vulnerabilidades.
- `cargo audit --file Cargo.lock`: exit 0 con **3 avisos visibles**: `chrono`
  (RUSTSEC-2026-0543, unmaintained), `proc-macro-error` (RUSTSEC-2024-0370, unmaintained) y
  `glib` (RUSTSEC-2024-0429, unsound). No hay `audit.toml`, `deny.toml` ni exclusiones:
  el exit 0 viene de que son avisos y no vulnerabilidades, no de silenciarlos.
- `git diff --check`: exit 0. `rustfmt --check` en los dos ficheros nuevos: limpio.
- **Desconexión comprobada dinámicamente**, no por búsqueda de texto: `nm -C` sobre el
  binario de producción no contiene `thoriapp::evaluacion::`. Además, mutar
  `MIN_TIE_PERCENT` de 5 a 99 hace **fallar 4 pruebas**, lo que demuestra que la suite
  ejercita el motor de verdad (fichero restaurado e idéntico).
- **Reutilización del ruido comprobada por prueba**
  (`c1_el_motor_reutiliza_la_misma_funcion_de_ruido`): para nueve pares de entrada, lo que
  el motor registra coincide con `benchmarks::combined_noise`, y el margen con
  `max(combinado, 5 %)`.
- **Confianza y preferencia comprobadas por prueba**: `c1_la_confianza_usa_solo_codigos_de_confianza`
  (recorre los seis estados y exige un nivel de confianza, nunca el estado),
  `c1_una_metrica_excluida_no_favorece_a_nadie` y
  `c1_los_estados_sin_veredicto_no_favorecen_ni_fingen_confianza` (ningún estado sin
  veredicto deja preferencia ni por campo ni por motivo).
- **Compatibilidad con B comprobada por prueba** (`c1_la_evaluacion_parcial_es_soportada_por_b`):
  una evaluación parcial real sigue siendo `es_soportada()` y `corresponde_a()`, y conserva
  sus códigos tras la vuelta a disco. `es_soportada` valida solo versión y estado; los
  motivos y limitaciones son códigos abiertos por diseño, así que **C.2 no necesita
  adaptar nada** de la clasificación.

  **Matiz importante:** que `es_soportada()` sea `true` **no** garantiza la validez
  semántica de todos los campos. Solo comprueba la versión de metodología y que el estado
  esté en el vocabulario conocido; **no** valida `confianza`, `motivos`, `limitaciones` ni
  las métricas. Por eso los dos defectos anteriores (confianza con códigos de estado y
  preferencia contradictoria) pasaban `es_soportada()` sin objeción. La coherencia interna
  la garantizan las pruebas del motor, no el clasificador. **C.2 no debe ampliar el
  clasificador**; si en el futuro se quiere validar esa coherencia al leer de disco, es una
  decisión aparte y no se toma aquí.

### Limitaciones que se conservan (no las introduce C.1)

- Superar el margen **no** es una mejora reproducible ni significación validada: solo que
  las dos grabaciones midieron distinto más allá del ruido de muestreo.
- Un ruido de cero **no** demuestra ausencia de variabilidad real.
- La elegibilidad del 0,1 % low reutiliza el criterio operativo de `low01Reliability`
  (frontera real **9500** muestras: 9499 no, 9500 sí). Es un criterio operativo, **no** un
  mínimo estadístico validado.
- **No existe** criterio validado de elegibilidad para el 1 % low: decide cuando sus
  valores constan. Su revisión es 5.4.3.
- El ruido combinado **no** se duplica: el motor llama a `benchmarks::combined_noise`, que
  pasó de `fn` a `pub(crate)` (cambio de visibilidad de una línea, sin tocar fórmula,
  entradas ni resultados).
- D1 (`save_comparison` guarda `evaluacion: None`), D2, D3 y D9 **siguen abiertos**: son
  C.2/C.3.

### Cierre

C.1 queda **cerrado** con aprobación explícita. El motor es lógica pura: no se ha tocado la
persistencia (`save_comparison` sigue guardando `evaluacion: None`) ni se ha conectado a
producción. La única modificación fuera del módulo es el cambio de visibilidad de
`combined_noise` (`fn` → `pub(crate)`) y el registro `#[cfg(test)] mod evaluacion;`.

**Recuento de pruebas**

| Plano | Aprobadas |
|---|---|
| Rust (`cargo test --locked --bins`) | **991** |
| TypeScript (`vitest`) | **1058** |
| **Total** | **2049** |

- **39 pruebas nuevas** respecto a B (`ec32935`), todas de `evaluacion::`.
- **Cero eliminadas.** Inventario: 953 → 992 registradas.
- **Una ignorada preexistente**: `anticheat::tests::manual_descarga_y_busca_appids_reales`
  (prueba manual con red, ya estaba así en B).
- **42 avisos de Clippy en código de pruebas**, todos de deuda previa; ninguno en
  `evaluacion.rs` y sin ningún `#[allow]` añadido. `--bins` limpio con `-D warnings`.
- **Tres avisos de `cargo audit`** (`chrono` RUSTSEC-2026-0543, `proc-macro-error`
  RUSTSEC-2024-0370, `glib` RUSTSEC-2024-0429), visibles y **sin exclusiones** ni
  `audit.toml`: el exit 0 refleja que son avisos y no vulnerabilidades.

**Estado del motor**: puro y **desconectado de producción**, comprobado sobre el binario
(`nm -C` no contiene `thoriapp::evaluacion::`) y por el registro `#[cfg(test)]`.

### Pendiente para C.2 y C.3

- **Activación coherente** de evaluación, persistencia y presentación: que
  `save_comparison` calcule y persista la evaluación en la **misma escritura**, con
  `evaluada_en` e ids, sin confiar en el frontend.
- **Correspondencia entre vista previa confirmada y evaluación guardada**: lo que el
  usuario ve antes de guardar debe ser lo que queda en disco.
- **Coherencia semántica de las evaluaciones generadas**: hoy la garantizan las pruebas del
  motor; al activar la persistencia hay que asegurar que sigue habiendo **una** fuente de
  verdad y no dos intérpretes en desacuerdo (D2).
- **Distinción entre evaluación soportada y evaluación semánticamente válida**:
  `es_soportada()` solo valida versión de metodología y vocabulario del estado; **no**
  valida `confianza`, `motivos`, `limitaciones` ni métricas. No debe presentarse como
  garantía de corrección semántica.
- **Presentación de la confianza observada sin afirmar reproducibilidad**: `observada` dice
  que se midió por encima del ruido de muestreo, no que el resultado se reproduzca.

### Limitaciones metodológicas (pendientes de 5.4.3)

- Superar el margen **no** es mejora reproducible ni significación validada.
- Un ruido de cero **no** demuestra ausencia de variabilidad real.
- La elegibilidad del 0,1 % low reutiliza el criterio operativo de `low01Reliability`
  (frontera 9500 muestras): es operativo, **no** un mínimo estadístico validado.
- **No existe** criterio validado de elegibilidad para el 1 % low.
- D1, D2, D3 y D9 siguen abiertos: son C.2/C.3.

## 5.4.1.C.2 — integración y persistencia atómica (**cerrado**)

Ruta interna que genera la evaluación en Rust y la guarda **en la misma escritura atómica**
que la comparación, con confirmación de la vista previa. **No está conectada a ningún
comando IPC**: `main.rs` no se ha tocado y el binario de producción no contiene ningún
símbolo nuevo. La activación es C.3.

### Qué se ha añadido

- `evaluacion::ConfirmacionDeVistaPrevia { comparacion, evaluacion }`: la confirmación no
  es una huella ni un testigo, son **los dos objetos** que Rust generó. React los devuelve
  como **expectativa**; nunca son datos autoritativos.
- `evaluacion::comprobar_confirmacion(&confirmada, &recalculada) -> Result<(), MotivoDeRechazo>`
  con cuatro motivos: `valor_no_persistible`, `evaluacion_no_corresponde`,
  `entradas_cambiadas`, `significado_cambiado`.
- `benchmarks::vista_previa_evaluada`: vista previa **real**, con las mismas funciones que
  el guardado (`comparacion_entre` + `evaluar_comparacion`) y búsqueda **inequívoca**.
- `benchmarks::comparacion_entre`: única función que construye la comparación;
  `comparar_en` (comandos antiguos) y la vista previa desembocan en ella, así que no hay
  dos formas de comparar.
- `benchmarks::buscar_unico`: exige que un identificador corresponda a **exactamente un**
  registro; dos o más lo rechazan. Es más estricta que `find` y solo la usa la ruta nueva.
- `benchmarks::evaluar_comparacion`: **único** punto donde se aplica la metodología; sella
  `evaluada_en` e ids.
- `benchmarks::comprobar_coherencia`: verificación estructural antes de escribir. **No**
  recalcula el veredicto; comprueba vocabulario de confianza, concordancia
  preferencia/motivos, excluidas sin preferencia y compatibilidad. No amplía
  `es_soportada()`.
- `benchmarks::guardar_comparacion_evaluada(rutas, confirmacion, variable)`.
- `benchmarks::adquirir_cerrojos`: orden único `CERROJO_BENCHMARKS` →
  `CERROJO_COMPARACIONES`.
- Extracciones sin duplicar lógica: `comparar_en` (usada por `compare_benchmarks` y por la
  ruta nueva) y `try_load_benchmarks_data_de`.

### Por qué la confirmación son dos objetos y no un hash

El estado global no identifica una comparación: dos `mejora` pueden diferir en métricas,
margen, preferencias y limitaciones. Y la evaluación sola no basta: hay cambios en las
entradas (comando, procedencia) que **no** alteran el veredicto y sí alteran lo que se
guardaría. Hay prueba de ambos casos
(`c2_cambio_de_entradas_que_conserva_la_misma_evaluacion_se_rechaza`).

`evaluada_en` queda **fuera** de la igualdad de significado: es el instante del registro, no
lo que se afirma. No hay tolerancias numéricas.

### La igualdad se corrigió: la forma serializada perdía información

La primera versión comparaba la **forma serializada** para evitar que `NaN != NaN` hiciera
imposible guardar. Al revisarla se comprobó que ese mecanismo **funde casos distintos**:
`serde_json` serializa `NaN`, `+inf` y `-inf` **los tres como `null`**, y `null` es también
lo que produce un valor ausente. Cuatro entradas distintas —tres no finitas y «no consta»—
tenían la misma representación, así que dos confirmaciones diferentes se habrían aceptado
como iguales.

Ahora la igualdad es **campo por campo**: los `f64` se comparan por bits (`to_bits`, función
total, y `NaN` coincide consigo mismo), los opcionales distinguen **ausente** de
**presente**, y el resto usa su propia igualdad. Se cubren los dos riesgos opuestos: no
colapsar lo distinto y no rechazar lo idéntico.

Queda una diferencia real entre Rust y JavaScript que **no** se puede resolver en el
mecanismo: `JSON.stringify` convierte `-0` en `0` y pierde precisión por encima de 2⁵³. Por
eso no se confía en la representación de React: la decisión se toma sobre lo que Rust
recalcula. Una confirmación que sobreviva al viaje con diferencias de redondeo no coincide
con la recalculada y se rechaza, que es el lado seguro.

### Los valores no finitos se rechazan antes de escribir

Un `f64` no finito en un campo **obligatorio** se serializa como `null` y el archivo deja
de poder leerse (`invalid type: null, expected f64`): no es un problema de presentación, es
un archivo escrito que se pierde. `save_benchmark` **no** sanea esos valores (0
comprobaciones `is_finite`). La ruta evaluada sí: `comprobar_confirmacion` devuelve
`valor_no_persistible`. El análisis real filtra no finitos, así que un valor así solo puede
llegar de datos históricos o de un llamador que se equivoque; el guardado no lo propaga.

### Cerrojos

Se toman **los dos**, una sola vez, antes de leer, y cubren lectura, evaluación,
comprobación y escritura. Si la confirmación falla, **no se escribe nada**. Ninguna función
llamada bajo ellos toma cerrojo (`load_*`, `save_*`, `comparar_en`: 0 apariciones), así que
no hay adquisición recursiva. No se mantiene ningún cerrojo entre viajes de IPC.

### Contradicción de la auditoría previa sobre `save_benchmark` — resuelta

La afirmación anterior de que `save_benchmark` «solo añade» era **incorrecta**. Recibe un
`Benchmark` completo, con su `id`, del llamador y hace `push` **sin comprobar unicidad**. El
`id` se genera con resolución de **segundo** (`bench_{timestamp}`), así que dos análisis en
el mismo segundo producen el mismo id. Consecuencia: **puede haber ids duplicados** y
`find` se queda con el primero. Queda registrado como comportamiento real con prueba
(`c2_save_benchmark_no_garantiza_ids_unicos`, `c2_con_ids_repetidos_se_toma_la_primera`).
No se ha cambiado: deduplicar sería alterar el contrato de B.

La ruta evaluada **no hereda** esa ambigüedad: `buscar_unico` rechaza un identificador que
corresponda a dos registros, tenga el contenido idéntico o distinto, sin escribir nada. Los
duplicados **ajenos** a la comparación pedida no la bloquean (si no, un duplicado antiguo de
otro juego impediría comparar cualquier cosa).

Deuda técnica **independiente**: el generador de ids tiene resolución de segundo y
`save_benchmark` no comprueba unicidad. La ruta evaluada lo rodea, no lo arregla.

### Validación ejecutada

- `cargo test --locked --bins`: **1042 aprobadas, 0 fallidas, 1 ignorada** (la manual de
  anticheat, preexistente). **+51** respecto a C.1 (991 → 1042). **0 pruebas eliminadas.**
- `cargo clippy --locked --bins -- -D warnings`: **exit 0**.
- `cargo clippy --locked --bins --tests`: **42 avisos, exactamente la deuda previa**;
  **0 en `evaluacion*` y 0 en la zona nueva de `benchmarks.rs`**. Sin `#[allow]`.
- `rustfmt --check` en los ficheros de `evaluacion`: limpio. No se reformateó nada ajeno.
- `cargo build --locked --bins`: exit 0. `cargo audit`: exit 0, los mismos **3 avisos**.
- Frontend **sin cambios**: `tsc` exit 0, `vitest` **1058**, `build` exit 0, `npm audit` 0.
- `git diff --check`: exit 0.
- **Ruta inactiva comprobada dinámicamente**: `nm -C target/debug/thoriapp` no contiene
  `thoriapp::evaluacion`, `guardar_comparacion_evaluada` ni `adquirir_cerrojos` (0).
  `main.rs` sin cambios: ningún comando IPC nuevo.

### Contrato de transporte (serialización por IPC)

`ConfirmacionDeVistaPrevia` deriva `Serialize`/`Deserialize` porque viajará por IPC en C.3.
**No es un cambio de esquema persistido**: no se escribe en ningún archivo en esa forma, y
el archivo de comparaciones sigue con su versión y sus campos.

El viaje se prueba **de verdad**, no simulado: Rust → JSON → `JSON.parse` +
`JSON.stringify` en Node → deserialización Rust → confirmación → persistencia. Si Node no
está disponible, la prueba **falla** en vez de omitirse, porque el viaje es justamente lo
que se prueba (`c2_transporte_por_node_conserva_la_confirmacion`,
`c2_transporte_conserva_valores_normales`).

El resultado es el que importa para decidir el diseño:

- **Datos normales**: se conservan **bit a bit**. FPS con decimales (`59.94`, `143.98`), el
  clásico `0.1 + 0.2`, porcentajes con muchos decimales, `2.5`, cero real y
  `total_frames` (144 000 — siete órdenes de magnitud por debajo de 2⁵³). Una vista previa
  **sin modificar se confirma y se guarda** después del viaje.
- **Extremos sintéticos**: `-0.0` pierde el signo y un entero por encima de 2⁵³ pierde
  precisión. Ninguna medición real los produce; si llegaran, la igualdad canónica los
  detecta y la confirmación se **rechaza** (el lado seguro). El límite exacto 2⁵³ sí se
  conserva.

**No hay problema de transporte con datos normales**, así que no se ha cambiado el diseño:
ni tolerancias, ni huellas, ni testigos. La pérdida de precisión solo aparece en casos que
la aplicación no genera.

### Limitaciones que se conservan

- La ruta nueva **no está conectada**: `save_comparison` sigue guardando
  `evaluacion: None` (D1). C.3 es quien la activa.
- `es_soportada()` **no** valida la coherencia semántica completa; `comprobar_coherencia`
  sí comprueba lo estructural, pero sigue siendo una comprobación de invariantes, no una
  segunda interpretación.
- Los cerrojos son **intraproceso**: dos instancias de la aplicación no se protegen entre
  sí (fase 9).
- Sigue sin criterio validado de elegibilidad del 1 % low ni significación validada
  (5.4.3). Superar el margen no es una mejora reproducible.
- D2 (dos intérpretes) y D9 (ganador por signo en `TablaMetricas`) siguen abiertos: C.3.

**Recuento**: Rust **1042** + TypeScript **1058** = **2100** aprobadas.

### Cierre

C.2 se **cierra** con la aprobación oficial tras dos rondas de revisión. El criterio del
cierre es que la ruta interna generada por Rust quede **demostrada y sin activar**:

- La confirmación es **conjunta** (comparación + evaluación) y la vista previa que la
  produce usa las mismas funciones que el guardado.
- Un rechazo **no toca ningún archivo** (contenido y `mtime` idénticos).
- La persistencia es **una sola escritura atómica** con la comparación y su evaluación, y
  lo guardado sale del **recálculo**, nunca del llamador.
- La igualdad de significado **no pierde información** (corregida tras detectar el colapso
  de `NaN`/`±inf`/ausente en la forma serializada) y el contrato de transporte se verificó
  con un viaje real por Node.
- Los identificadores ambiguos se **rechazan** en la ruta nueva.
- La ruta está **inactiva**: `main.rs` sin cambios, 0 símbolos nuevos en el binario, ningún
  comando IPC (`nm` y `git diff` como evidencia).

**Lo que C.2 no cierra**: la activación es C.3 (`save_comparison` sigue guardando
`evaluacion: None`, D1); el generador de ids con resolución de segundo y la falta de
comprobación de unicidad en `save_benchmark` quedan como **deuda técnica independiente**;
la coherencia semántica completa y los criterios de 5.4.3 y 9 siguen abiertos.

### Correspondencia con la auditoría de C.2

La primera revisión añadió **18** pruebas sobre las 25 comunicadas (43). La segunda añadió
**8** más (51 en total): contrato de transporte (3), fallo de escritura (1), rechazo sin
tocar archivos (1), no finitos opcionales/obligatorios (3).

| Punto | Qué se añadió | Pruebas |
|---|---|---|
| 1. Igualdad sin pérdida | no finitos distintos entre sí y de ausente, `0.0`/`-0.0`, `None`/`Some(0)`, finitos distintos, regresión del colapso serializado, rechazo de `valor_no_persistible`, **cobertura de todos los campos** | 7 |
| 2. Identificadores ambiguos | duplicado distinto, duplicado idéntico, ausente, único, duplicados ajenos | 5 |
| 3. Vista previa real | flujo completo vista previa → JSON → confirmación → guardado, `evaluada_en` como instante del guardado | 2 |
| 4. Concurrencia | guardados concurrentes sin interbloqueo, lectura concurrente con guardado | 2 |
| 5. Persistencia | lo persistido viene del recálculo, margen registrado | 2 |
| 1b. Contrato de transporte | viaje real por Node (vista previa completa y valores normales), extremos sintéticos | 3 |
| 4b. Fallo de escritura | `rename` fallido no deja registro parcial ni `.tmp` | 1 |
| 4c. Rechazo sin efectos | contenido **y `mtime`** intactos en los dos archivos | 1 |
| 1c. No finitos | obligatorio → archivo ilegible; opcional → se vuelve ausente; los dos se rechazan antes de escribir | 3 |

Las pruebas de la **cobertura de campos** y de la **igualdad de no finitos** encontraron
defectos reales en la primera versión (colapso de `NaN`/`±inf`/ausente), que se corrigieron.

Una prueba de concurrencia escrita para esta revisión **se interbloqueó**: tomaba los dos
cerrojos en el hilo padre y los retenía mientras los hilos hijos intentaban adquirirlos
(`Mutex` no es reentrante). El defecto era de la prueba, no del código; se corrigió y ahora
cada hilo toma los suyos.

## Cierre de 5.3 — validación real y limitación conocida

La fase se **cierra** con una limitación declarada, tras la ronda de validación en el equipo
(CachyOS). El observador queda como funcionalidad **experimental y opcional, apagada por
defecto**: no se lee `/proc` si el usuario no lo pide.

### Validado en el equipo real

| Comprobación | Resultado |
|---|---|
| Tattoo Tycoon (Heroic), Eslabong (Heronite), Deskrawl y Palworld (Steam), Star Valor (Lutris/Wine) — individual | Detección correcta |
| Starsector (Lutris, nativo) — individual | Nombre comercial correcto, estable 120 s, cierre correcto |
| Cierre de los seis juegos | Correcto |
| Palworld + Deskrawl (Steam + Steam) | Correcto, sin duplicados |
| Starsector + Deskrawl (Lutris + Steam) | Correcto |
| Cierre independiente de juegos | Correcto en los escenarios probados |
| Reinicio de Thoriapp | Sin sesiones fantasma |

### Limitación conocida

**La detección simultánea de Eslabong (Heronite) y Tattoo Tycoon (Heroic) es incompleta en
ambos órdenes de lanzamiento.** La causa exacta **no está demostrada** y la incidencia se
**aplaza** a una revisión futura: no bloquea 5.4.

Lo que sí se descartó, con datos: la fusión de las dos ejecuciones que llegó a atribuirse al
`systemd` compartido era un artefacto del fixture. Los árboles reales tienen raíces separadas
(procesos huérfanos adoptados por init) y con las topologías y catálogos reales el pipeline
forma dos grupos independientes. Por eso **no** se cambió `elegir_principal_con_catalogo`, no
se relajó `dentro_del_juego` y no se añadió ninguna excepción por nombre de proceso.

**Limitación del diagnóstico existente:** los registros disponibles no cubren una pasada con
ambas ejecuciones completas y sus catálogos simultáneos, así que no permiten aislar el punto
exacto de pérdida. Reabrir esto exige **capturar una nueva pasada** con el observador activo en
ese escenario, no más análisis sobre los datos actuales.

### Garantías de integridad de benchmarks (auditadas)

Comprobadas en el código y en los tests, porque el observador no puede ser imprescindible:

1. **No es imprescindible** para registrar, importar, guardar o comparar. Sin sesión, el
   benchmark se guarda con `sesion_id: None` y `snapshot` se construye igual: la sesión se pasa
   como `Option<&SesionDeEjecucion>` (`construir_snapshot`).
2. **Una detección incompleta no provoca asociaciones incorrectas.** `corresponde_al_juego:
   false` deja la configuración en `None`; la inferencia por nombre tiene procedencia propia
   (`Inferido`) y nunca se presenta como observación. Tests `s53f_*` de aislamiento.
3. **La asociación manual tiene prioridad.** `asignar_juego` escribe
   `OrigenAppId::ConfimadoPorUsuario`, por encima de cualquier inferencia.
4. **Una sesión observada no acredita la configuración ejecutada.** `ejecucion_verificada`
   queda en `None` por diseño, y la procedencia `VerificadoEnEjecucion` sigue sin usarse: nada
   afirma el efecto de una optimización.
5. **Los snapshots históricos son inmutables.** Se construyen una vez, al guardar el benchmark,
   y nunca se reconstruyen desde el estado actual; renombrar un perfil o cambiar Steam no los
   toca.
6. **Desactivar el observador no impide usar Thoriapp.** Arranca apagado
   (`instalacion_nueva_arranca_con_el_observador_apagado`) y `paso()` devuelve `Apagado` sin
   tocar `/proc`.

## Novena corrección de 5.3 — segunda auditoría: incidencias A y B

Segunda auditoría independiente sobre el código con la octava corrección ya aplicada. Se
investigaron dos incidencias sin tocar el contrato de identidad ni añadir heurísticas.

### Incidencia B — el nombre observado salía de un proceso ajeno (corregida)

El diagnóstico midió `Heronite-x86_64` como nombre observado en Tattoo Tycoon, Eslabong y
Starsector, incluso cuando el nombre comercial era correcto. Se localizaron dos orígenes, los
dos medidos y los dos corregidos:

1. `armar_grupo_con_catalogo` recogía `ejecutable_del_juego` de **todos** los miembros en orden
   de aparición, y quien consumía el grupo tomaba `.first()`. Ahora el nombre del
   **representante** va primero; el resto se conserva detrás como contexto.
2. `sesion_de_grupo` usaba la `pista` del frontend como respaldo de `nombre_observado`, aunque
   la pista está declarada «no evidencia de identidad». Se eliminó el respaldo **y el
   parámetro**, para que no pueda reintroducirse por descuido.

El nombre comercial confirmado ya prevalecía en el estado publicado (`take().or_else(…)`, nunca
se pisa); ahora hay un test que lo fija con el caso real de Starsector.

### Incidencia A — la simultaneidad Heroic + Heronite: **no se reproduce**

La primera reproducción era **falsa** y así se documenta, porque el error es instructivo:

- pasaba `None` como catálogo, cuando ambos juegos **sí** están en catálogo y por separado
  funcionan;
- inventaba un `systemd` común del que colgaban las dos ejecuciones.

Los árboles reales no comparten raíz: son procesos **huérfanos adoptados por init**
(`ppid = 1`). El `systemd` común era una invención del fixture, y de ahí salía la fusión que se
creía observar. Con las topologías reales (`ESLABONG_TOPOLOGIA_REAL` + `TATTOO_TOPOLOGIA_REAL`)
y los dos catálogos reales a la vez, el resultado es **dos grupos independientes**.

La causa de que la primera reproducción muriera en `elegir_principal_con_catalogo` era el
`None`: sin índice no hay `confirma()` que pueda confirmar nada. No había defecto que corregir
ahí.

Conclusión: **no se cambia `elegir_principal_con_catalogo`**, no se relaja `dentro_del_juego` y
no se toca el contrato de identidad desconocida. Tampoco se añade `heronite-rs` a la lista de
infraestructura: es una excepción por nombre sin necesidad demostrada, y el criterio «el hijo
inmediato es el juego» no distingue un lanzador persistente de un envoltorio exclusivo.

### Verificación

- `cargo test --bins`: **894 pasan, 0 fallan, 1 ignorado**.
- `cargo clippy --bins` y `cargo check --bins`: sin errores.
- `npx tsc --noEmit`: limpio.

### Límite explícito

Esto es reproducción **simulada** con fixtures reales; no demuestra que la simultaneidad
funcione en el equipo. La fase **sigue abierta** hasta la revalidación real.

## Octava corrección de 5.3 — el defecto de entrada (auditoría independiente)

La séptima corrección se validó en el equipo y **falló**: Tattoo Tycoon, Eslabong y Palworld
seguían duplicándose, y las tres pruebas simultáneas fallaron. Una auditoría independiente del
código, con el diagnóstico real de 367 registros, encontró un defecto que ninguna prueba
anterior podía ver.

### P0 — producción y pruebas no ejecutaban el mismo pipeline

En `proceso_linux.rs`, el bucle por `/proc` descartaba los procesos que no pasaban el
prefiltro con `continue`, y `topologia.push(...)` estaba **después** de ese `continue`. Solo
había una llamada a `topologia.push` en todo el fichero.

Consecuencia: **la topología de producción contenía únicamente los candidatos**. Los eslabones
descartados (`python3`, `reaper`, `srt-bwrap`) no estaban, así que la cadena se cortaba y un
mismo lanzamiento salía en dos grupos. El camino de pruebas, en cambio, llamaba a
`agrupar_con_topologia(&validos, procesos)` con la topología **completa**: probaba un pipeline
que producción no ejecutaba. Esa divergencia es la razón exacta de que 885 tests pasaran
mientras el equipo fallaba.

Era, además, una violación del contrato aprobado: los descartados deben conservarse como
eslabones topológicos sin poder acreditar ejecución. El comentario del código decía una cosa y
la implementación hacía otra.

Correcciones:

1. **Topología barata**: `rasgos_de_topologia` lee solo `/proc/<pid>/stat` y el nombre del
   ejecutable. Se construye para **todos** los procesos observados, **antes** del prefiltro.
   Sin argumentos, sin entorno, sin `cwd`: no encarece la pasada.
2. **Un solo criterio de prefiltro**: `puede_ser_juego` decide sobre rasgos ya leídos y lo usan
   **ambos** caminos. `parece_juego` —la versión paralela leída de `/proc`— se eliminó para que
   no puedan volver a divergir.
3. Los descartados siguen sin ser miembros ni representantes: no llevan evidencia de juego.

### P2 — continuidad de identidad confundida con continuidad de instancia

En `sessions_service.rs` el refresco del árbol aceptaba cualquier clave de la sesión que
siguiera viva en `/proc` (`|| self.sigue_vivo(c)`), sin comprobar que perteneciera al grupo
actual. Eso da continuidad sin demostrar pertenencia: un proceso vivo de otra partida, o un
auxiliar superviviente, bastaba para adoptar el grupo actual. Es la vía por la que un juego
podía cambiar de ejecución sin cerrar sesión.

Ahora la coincidencia tiene que ser con un proceso **de este grupo**. Pertenecer al árbol es
topología; seguir vivo no dice de quién.

### P1 y P3 tras corregir la entrada

- **P1** (infraestructura sobre información incompleta): era consecuencia de P0. Con la
  topología completa, `es_infraestructura` distingue bien: los `systemd` de usuario no son
  miembros y el `systemd-inhibit` del lanzamiento sí es su ancla. No se cambió la heurística.
- **P3** (separación por AppID): se midió con las cuatro cadenas reales (Palworld, Deskrawl,
  Tattoo Tycoon, Eslabong) y **no fragmenta** ningún lanzamiento: ningún grupo queda formado
  solo por envoltorios sin ejecución acreditada. Se deja como está; ya se había intentado
  heredar el AppID y se revirtió por una razón medida.

### Verificación

- `los_descartados_siguen_siendo_eslabones_de_la_cadena` — sobre el camino de producción
  (`escanear_desde_con_catalogo`): el envoltorio y el juego quedan juntos y el `python3`
  descartado no es miembro.
- `la_infraestructura_compartida_no_une_ejecuciones_distintas`.
- `un_eslabon_sin_appid_no_forma_grupo_propio` — las cuatro cadenas reales.
- `una_coincidencia_de_nombre_no_adopta_procesos_de_otra_ejecucion` — P2.

Comprobaciones: `cargo test --bins` **889 passed, 0 failed, 1 ignored**; `clippy --bins` 0
errores (1 aviso preexistente de 9 argumentos); `tsc --noEmit` limpio; `vitest` **1027
passed**.

Se conserva lo ya validado en el equipo: Starsector estable, Deskrawl individual correcto,
Star Valor correcto y el cierre correcto de los seis juegos.

### Límites

- **Sigue sin ser validación real.** Falta reejecutar el protocolo en CachyOS, sobre todo las
  tres combinaciones simultáneas, que son las que fallaron.
- No se ha corregido el volcado de diagnóstico (campos del enriquecimiento), que sigue como
  trabajo futuro: no es causa de ningún defecto observado, pero dificulta auditar.
- Que el `cwd` pueda acreditar por sí solo en un árbol sin AppID ni catálogo no está medido
  con datos reales.

## Séptima corrección de 5.3 — duplicaciones y el bucle de Starsector

Segunda validación real en CachyOS, sobre `diag_ciclo_vida.json` (1327 registros, 626
escaneos). Lo que sigue sale de esos datos, no de fixtures.

### Lo que ya funcionaba y se conserva

El ciclo de vida quedó validado: `ses_20261126121612` (Tattoo Tycoon) terminó con
`evidencia_proceso: terminada`, `vivos_de_la_ejecucion: []`, `arbol_del_lanzamiento: []`,
`cerrada_en 12:44:14` y `pasadas_sin_verlo: 3`. Dolphin, Steam y Heroic siguieron abiertos sin
mantenerla, reiniciar Thoriapp no la resucitó, y los seis juegos desaparecen al cerrarse.
Star Valor (Lutris/Wine) entra correcto. Nada de esto se ha tocado.

### A y B: el ascenso topológico daba raíces distintas al mismo lanzamiento

Causa exacta, reconstruida con las cadenas del diagnóstico:

| Juego | Candidato | `ppid` | Raíz antigua | Por qué |
|---|---|---|---|---|
| Palworld (reg. 1219) | `426782` wine64-preloader | 426674 | **426577** | se paraba en `426577`, que era genérico |
| Palworld | `426577` systemd-inhibit | 426576 | **426576** | su propio padre no era genérico, subía uno más |
| Deskrawl (reg. 1089) | `421933` wine-preloader | 421927 | **421894** | se paraba en `pv-adverb` |
| Deskrawl | `421894` pv-adverb | 421818 | **3355** | subía hasta el `systemd` de usuario |

Dos raíces distintas con el **mismo** AppID → dos grupos → dos sesiones. Por eso Deskrawl y
Palworld se duplicaban aunque compartieran AppID: el `(raíz, AppID)` no era la explicación,
la asimetría de la raíz sí.

`raiz_en_topologia` subía mientras el nodo *actual* no fuese genérico y devolvía `actual`, así
que cada eslabón se paraba donde le tocaba. Ahora se sube siempre hasta el **eslabón propio
más alto**, el primero por debajo de la infraestructura, de modo que toda la cadena obtiene
el mismo nodo.

Qué es infraestructura se decide por estructura, no por nombre: un genérico **con hijos no
genéricos** encabeza una ejecución concreta y es su ancla (el `systemd-inhibit` que Steam
lanza para un juego); un genérico cuyos hijos son todos genéricos encabeza varias y no une
nada (el `systemd` de usuario). Los `systemd` de usuario ya no entran en el grupo del juego y
PID 1 no se usa como raíz unificadora.

Esto **también era B**: `pv-adverb` y `wine-preloader` acaban siendo representantes de grupos
separados en lugar de quedar unidos al árbol del juego. No estaba resuelto por la corrección
de C.

### C: el intérprete nativo no acreditaba ejecución

Regresión real de Starsector (registros 1249-1262): escaneo con 1 grupo → sesión CREADA →
×3 NO duplicada → CREADA otra vez con el **mismo PID y el mismo `starttime`** → ×2 NO
duplicada. El bucle «Juego detectado → buscando juego → juego detectado» no era visual.

Cadena demostrada: `java` no es auxiliar ni genérico, pero `ejecuta_de_verdad` es `null` (no
es un `.exe`) → `tiene_evidencia_de_juego` miraba `ejecutable_del_juego`, `appid_del_entorno`,
`appid_de_argumentos` y `carpeta_declarada`, y **no el `cwd`** → `false` →
`vivos_de_la_ejecucion` vacío → `solapa_el_juego` no reconoce la instancia → sesión nueva.
La asimetría con `claves_del_juego_de`, que **sí** caía al representante, era el resto del
defecto.

Se admitió el `cwd` con la condición estricta acordada: solo si existe una relación
verificable con una carpeta conocida (la que el proceso declara, la de `install_dirs` del
grupo, o la que la correlación del catálogo dejó escrita al abrir la sesión). No se admite
por parecido de nombres, y la pertenencia al directorio no convierte en ejecutable del juego a
cualquier proceso que pase por ahí. No hay excepción para Java ni para Starsector.

### Verificación

Fixtures nuevos fieles a las cadenas reales (`PALWORLD_REAL_5_3`), con los `ppid` del
diagnóstico y no `ppid: 1` como los antiguos; eso último es lo que hacía que las regresiones
pasaran mientras producción fallaba.

- `palworld_real_5_3_da_un_solo_grupo` — un lanzamiento, un grupo.
- `palworld_real_5_3_no_mete_los_systemd_en_el_grupo_del_juego`.
- `dos_lanzamientos_bajo_el_mismo_systemd_no_se_fusionan` — dos ejecuciones con ancestro
  común conservan grupos distintos. **Este test encontró un defecto real en el primer intento
  de arreglo**: la regla «más de una rama» fusionaba ambas. Se corrigió la regla, no el test.
- `regresion_starsector_siete_pasadas_conservan_una_sola_sesion` — las siete pasadas del
  diagnóstico, con PID y `starttime` constantes, comprobando también que
  `vivos_de_la_ejecucion` no queda vacío y que la evidencia es `Observada` en todas.

Comprobaciones: `cargo test --bins` **885 passed, 0 failed, 1 ignored**; `clippy --bins` 0
errores (1 aviso preexistente de 9 argumentos); `tsc --noEmit` limpio; `vitest` **1027
passed (75 ficheros)**.

### Límites y trabajo futuro

- **Nada de esto es validación real.** Falta reejecutar el protocolo en CachyOS con los seis
  juegos y comprobar que ningún juego se duplica.
- El volcado de diagnóstico muestra `candidatos` con `ejecuta_de_verdad`, `steam_app_id` y
  `wine_prefix` a `null` y `recorrido_del_ejecutable` vacío aunque las decisiones posteriores
  usen prefijo e identidad: parece registrarse **antes** del enriquecimiento. Queda como
  trabajo futuro, junto con alinear los nombres de campos anidados; no se ha tocado en esta
  ronda para no ampliar el alcance.
- Que dos juegos compartan un `systemd` de usuario y se lancen por la misma cadena de
  `systemd` (no mediante `systemd-inhibit`) no está cubierto por datos reales: el criterio
  estructural lo separa, pero no se ha medido.
- Mejoras marginales pendientes: revisar si `es_ancestro_generico` necesita `srt-logger` y
  otros helpers de Steam, y si el tope de 64 saltos del ascenso conviene hacerlo configurable.

## Sexta corrección de 5.3 — la sesión se sostenía con un auxiliar

La validación real en CachyOS **falló** y contradijo las pruebas automáticas. Esto es lo que
demostró el diagnóstico frente a lo que decían los fixtures.

### Estado de los bloques según la validación real

| Bloque | Estado real | Qué se vio |
|---|---|---|
| **A** — un grupo por juego | ❌ **No pasa** | Eslabong ×2 grupos, Tattoo ×2, Palworld ×3. Un juego abrió sesión llamada `steam` con AppID 1623730. |
| **B** — auxiliares no abren sesión | ❌ **No pasa** | `systemd-inhibit` y `pv-adverb` seguían creando sesiones en producción. |
| **C** — continuidad de sesión | ❌ **No pasa** | Sesión de Tattoo Tycoon `OBSERVADA` durante horas con el juego cerrado. |
| D — IDs únicos | ✅ Correcto | Se vieron sufijos `_2`, `_3`. |
| E — Wine/Proton | ✅ Correcto | `bajo_proton` solo con Proton. |
| F — ejecutable de Heroic | ✅ Correcto | `exec=/home/jugador/Games/Heroic/TattooTycoon/Tattoo Tycoon.exe`. |

### La causa raíz de C (y su relación con A y B)

La sesión residual `ses_20261126121612` (Tattoo Tycoon, PID 327412) tenía 13 claves en
`vivos_de_la_ejecucion`. De esos 13 procesos solo sobrevivían dos, y **ninguno era el juego**:
`dolphin` (PID 16137) y `srt-logger` (PID 127349). Tres defectos encadenados:

1. **`claves_de_la_ejecucion` guardaba todos los miembros del grupo**, sin filtrar. La
   pertenencia topológica se estaba usando como evidencia de ejecución.
2. **`paso()` decidía la vida con `.any()` sobre esa lista**, así que un auxiliar vivo bastaba
   para poner `pasadas_sin_verlo` a 0 en cada pasada. Se medían 465–491 procesos y **0 grupos**
   en los escaneos 69/72/222/225 y la sesión seguía publicándose como `OBSERVADA`.
3. **El ciclo de vida no consultaba el escaneo**: miraba `/proc` por su cuenta, así que «el
   escaneo no ve ningún juego» no podía cerrar nada.

A esto se sumaba un defecto independiente: **`stat()` descartaba el campo de estado de
`/proc/<pid>/stat`** con `let _ =`. Un proceso terminado y no recogido conserva su `starttime`,
así que constaba vivo indefinidamente. Se reprodujo con un fork/zombi real: `estado='Z'` y
`sigue_vivo()` devolvía `true`.

### La corrección: cuatro conceptos separados

Se separó lo que estaba mezclado en un solo campo:

- **Topología** → `miembros` del grupo.
- **Identidad del juego** → `identidad` (AppID, nombre, carpeta).
- **Continuidad de instancia** → `arbol_del_lanzamiento` (nuevo campo, `serde(default)`).
- **Ejecución activa** → `vivos_de_la_ejecucion`, ahora **solo** con procesos que acreditan.

`acredita_ejecucion` exige `ejecuta_de_verdad == Some(true)` o ser representante con evidencia
propia, y **nunca** admite auxiliares ni ancestros genéricos. Ni el AppID, ni el catálogo, ni el
prefijo de Wine bastan por separado. No se añadió ninguna lista negra de `dolphin` ni de
`srt-logger`: el defecto era estructural.

`paso()` ahora escanea **antes** de decidir el ciclo de vida y exige que el escaneo vea el
juego. Un escaneo válido con 0 grupos cierra por el ciclo normal; sin visibilidad no se cierra
nada. `ultima_observacion` solo avanza cuando se ve el juego.

Las **sesiones persistidas se revalidan al cargar**: las listas contaminadas escritas por
versiones anteriores no se usan como prueba. Si el escaneo no ve el juego, la sesión pasa a
`VisibilidadPerdida` (no se cierra de golpe, no se borra nada, no se tocan IDs ni fechas).

`EstadoDeLaClave` distingue ahora cinco casos: vivo, terminado, PID reutilizado, zombi e
inaccesible. `Z`, `X` y `x` no acreditan vida; «no lo veo» no es «terminó».

### Pruebas con los datos reales

Se añadió el árbol **real** de Tattoo Tycoon (9 procesos: `umu-run`, `python3`, `heroic`,
`wine-preloader`, `wine64-preloader`, `pv-adverb`, `dolphin` 16137 y `srt-logger` 127349), no
un fixture simplificado. Regresiones nuevas, todas por el pipeline completo:

- `el_juego_se_cierra_y_dolphin_y_srt_logger_no_mantienen_la_sesion` — el contador progresa y
  la sesión termina; comprueba explícitamente que ni 16137 ni 127349 están en la ejecución.
- `ultima_observacion_no_avanza_por_un_auxiliar_superviviente`.
- `una_sesion_persistida_contaminada_no_revive_al_reiniciar` — siembra el disco con la lista
  contaminada y verifica que no se publica `Observando` ni se inventa un cierre.
- `una_sesion_persistida_valida_se_recupera_al_reiniciar` — conserva id, fecha y snapshot.
- Más las 7 del bloque C, que siguen verdes (los 4 fallos intermedios eran reales y se
  corrigieron sin aflojar la regla).

### Verificación

- `cargo test --bins`: **881 passed, 0 failed, 1 ignored**.
- `cargo clippy --bins`: 0 errores (1 aviso preexistente de 9 argumentos).
- `npx tsc --noEmit`: limpio. `npx vitest run`: **1027 passed (75 ficheros)**.

### Límites que siguen abiertos

- **A no se declara resuelto.** El agrupador sigue separando por `(raíz, AppID)`. Que dos
  grupos de Palworld (`systemd-inhibit`/`pv-adverb`/`wine-preloader`) fueran además rechazados
  al crear sesión por la corrección de B no demuestra que la agrupación sea correcta. Hace
  falta reejecutar el protocolo real con el árbol de Palworld completo y comparar.
- **B necesita confirmación real**: los tests dicen que un auxiliar no abre sesión, pero eso ya
  lo decían antes y en producción sí ocurría. Solo el equipo lo dirá.
- Nada de esto sustituye al protocolo de validación real de 5.3.

## 5.3 — Topología, clasificación, IDs y Wine/Proton (ronda de corrección)

Causa raíz de la duplicación: `agrupar_por_padre` agrupaba por **padre inmediato** y solo
veía los procesos que habían pasado el filtro. Al descartar un eslabón intermedio, el árbol se
partía en varios grupos con la misma identidad, y cada grupo abría su propia sesión. No era un
problema de nombres ni de AppID: era topología perdida.

### Qué se corrigió

- **Topología separada de elegibilidad** (`agrupar_con_topologia`): se guarda la relación
  PID/PPID de **todos** los procesos observados y se agrupa subiendo por esa topología. Un
  proceso descartado sigue sirviendo de puente sin ser miembro, ni puntuar, ni representar.
  Índice por PID, sin recorrido cuadrático. Se corta en ancestros genéricos (systemd, escritorio,
  lanzadores) para no fusionar dos juegos distintos por su abuelo común.
- **Clasificación**: el prefijo de Wine **ya no confirma** que un proceso sea el juego, solo lo
  admite. Era la causa del falso positivo de `pv-adverb` (252630), que abría una sesión falsa
  de Tattoo Tycoon antes de que arrancara nada.
- **IDs únicos** (`id_de_sesion`): dos sesiones del mismo segundo ya no comparten identificador.
  El primero conserva el formato histórico `ses_<timestamp>`; los siguientes llevan sufijo
  `_2`, `_3`… No se reescribe ningún id guardado.
- **Wine vs Proton**: `bajo_proton` significa ahora Proton **específicamente**; el campo nuevo
  `bajo_compatibilidad` responde «ejecuta un binario Windows bajo Wine *o* Proton», que es lo
  que necesita la detección. Star Valor (Lutris + Wine) deja de aparecer como Proton.
- **Heroic**: si `install.executable` viene vacío, se complementa desde
  `store_cache/legendary_library.json` emparejando por **id de Epic exacto**, nunca por título.
- **Diagnósticos**: `raiz_calculada` ya se rellena (antes siempre `None` aunque la cadena estaba
  calculada); los descartes llevan su `ancestros`, para poder explicar dónde se rompió el árbol.
- **Continuidad de sesión (bloque C, faltaba)**: la sesión se identificaba por la clave del
  **representante** (`pid@starttime`), pero esa clave identifica un *proceso*, no una
  *ejecución*. En Deskrawl el representante pasó de `210465` a `210612` dentro de la misma
  partida y eso abrió **dos sesiones** de una sola ejecución. Ahora la sesión guarda
  `vivos_de_la_ejecucion` (todas las claves de su grupo) y la continuidad se decide por
  cualquiera de ellas: si el representante cambia pero el proceso del juego o un envoltorio de
  la misma ejecución sigue, la sesión es la misma. La comparación **no** usa la lista entera a
  secas —un envoltorio sobrevive de una partida a la siguiente y fusionaría ejecuciones
  distintas—, y `starttime` sigue distinguiendo un PID reutilizado. Campo con
  `#[serde(default)]`: las sesiones ya guardadas no se migran ni se reescriben.

### Verificación

Se añadieron los tres árboles reales capturados en CachyOS como fixtures
(`ESLABONG_TOPOLOGIA_REAL`, `TATTOO_TOPOLOGIA_REAL`, `PALWORLD_TOPOLOGIA_REAL`) y sus pruebas
exigen **un solo grupo** cada uno, que el envoltorio suelto no sea representante, que los falsos
positivos del arranque no abran sesión y que dos instancias del mismo juego sigan siendo dos
grupos. Para el bloque C hay siete pruebas por el pipeline de producción (cambio de
representante, desaparición del representante, conservación de id/instante/instantánea/
grabaciones, PID reutilizado, cierre y reapertura, dos instancias y pérdida temporal de
visibilidad). El bloque F se verifica de extremo a extremo: el ejecutable de
`legendary_library.json` llega al `EntradaDeCatalogo` y el `IndiceDelCatalogo` del observador lo
reconoce. 877 pruebas Rust y 1027 de TypeScript en verde; `clippy` sin errores.

### Límites honestos

- Esto está demostrado con los **árboles capturados**, no con el juego corriendo ahora mismo.
  El protocolo real de 5 s sigue pendiente en el equipo.
- La fusión se apoya en la topología observada en **una** pasada. Un proceso cuyo padre no se
  ve (namespace distinto, permisos) puede seguir partiendo el árbol; el diagnóstico ya lo deja
  ver con `ancestros` y `raiz_calculada`.
- Lutris no se ha tocado: sus dos casos validados siguen cubiertos por sus pruebas.

## 5.3 — Corrección del pipeline multi-lanzador

Corrección del fallo que dejaba fuera a los juegos externos. **Pendiente de validación real en
CachyOS**: lo que sigue está probado con fixtures de datos reales, no con el equipo del usuario.

### Causa raíz

El pipeline tenía **dos mundos desconectados**. El catálogo de lanzadores (Steam + Lutris +
Heroic/UMU) estaba completo y la correlación por carpeta, prefijo de Wine y ejecutable
funcionaba —hay pruebas de ello—, pero **nunca llegaba a consultarse**:

1. **El prefiltro de `/proc` solo admitía evidencia de Steam**: ruta de Proton, ruta dentro de
   `steamapps/common`, `.exe` en la línea de comandos o `comm` con extensión. Un juego de Lutris
   o Heronite no cumple ninguna, así que se descartaba antes de que nadie mirase el catálogo.
2. **`escanear_desde`** volvía a filtrar por lo mismo (nombre, AppID, `steamapps/common`).
3. **`armar_grupo`** exigía nombre, AppID o carpeta de Steam para aceptar un grupo.
4. **`elegir_principal`** solo aceptaba como representante a un proceso con rol
   `EjecutableDelJuego`, y ese rol solo se concedía con evidencia de Steam.
5. **`resolver_identidad`** solo consultaba la biblioteca de Steam; el catálogo únicamente
   rellenaba el nombre comercial, nunca decidía la identidad.

Los tres juegos externos se perdían en el eslabón 1, y los que hubieran pasado habrían muerto en
el 2, el 3 o el 4. No era un catálogo roto: era un catálogo al que no se preguntaba.

### Por qué las pruebas no lo detectaron

Las pruebas existentes del catálogo construían los `EntradaDeCatalogo` y los `RasgosDeProceso` a
mano y llamaban directamente a `correlacionar_con_el_catalogo`. Probaban el eslabón 8 de la
cadena y saltaban el 2. Es el mismo error que el fixture anterior de Deskrawl: datos ideales que
la producción nunca genera. Ahora los tests de integración recorren el prefiltro real.

### Qué se corrigió

| Pieza | Corrección |
|---|---|
| `IndiceDelCatalogo` (`proceso_linux.rs`) | Huella del catálogo construida **una vez por pasada** desde la caché compartida. Nada de SQLite ni YAML por proceso. |
| `parece_juego` | El catálogo se consulta **antes** de decidir, y su evidencia se admite antes de la lista de procesos del sistema. |
| `escanear_desde_con_catalogo` | El filtro y la formación de grupos aceptan la evidencia del catálogo. |
| `elegir_principal_con_catalogo` / `repartir_papeles_con_catalogo` | Un juego externo puede llegar a `EjecutableDelJuego`, que es la regla dura para representar la sesión. |
| `resolver_identidad_con_catalogo` | Nueva vía 5b-bis por **directorio de trabajo**, 5a carpeta declarada, 5b prefijo de Wine, 5c ejecutable. Identidad sin AppID. |
| `appid_consensuado` | AppID `0` es **ausencia**, no identidad. Dos ceros coincidentes tampoco identifican. |
| `RasgosDeProceso::cwd` | Nuevo campo, leído de `/proc/<pid>/cwd`, para los juegos que arrancan por intérprete. |
| `WINEPREFIX` | Añadido a `VARIABLES_PERMITIDAS`; se normaliza y se usa solo como evidencia de correlación. |

### El caso de los intérpretes (Starsector)

El árbol real es `lutris-wrapper` (python) → `starsector.sh` (bash) → `java`. Ninguno de los tres
nombra el juego ni lleva variables del lanzador. La única evidencia es que **trabajan en la
carpeta del juego**, así que el catálogo aporta esa carpeta y la clasificación por estructura
decide quién representa la sesión: el `java` que ejecuta la clase del juego.

Sin listas de nombres: nada de `java`, `Starsector` ni `StarfarerLauncher` codificados.

### Protección contra el efecto contrario

Admitir por el catálogo no puede convertir en juego a cualquier proceso:

- Un catálogo vacío **no admite nada** (sin datos no se adivina).
- `java`, `python3` y `bash` ajenos quedan fuera: el primero que probé lo demostró, porque
  `/home/jugador` acabó en el índice y admitía todo el árbol del usuario. La comparación de rutas no
  es simétrica: la ruta del proceso debe estar **por debajo** de la instalación, nunca por encima.
- La carpeta de un ejecutable **no** se trata como instalación declarada: se guarda aparte como
  derivada, para no presentarla como algo que el lanzador no dijo.
- El `cwd` solo se compara **exacto** contra la carpeta del ejecutable. Por sí solo no confirma.

### Coste medido

Método: `Instant` sobre el índice y el prefiltro con un catálogo de 4 entradas y el árbol real de
Starsector, 10 000 vueltas.

| Operación | Coste |
|---|---|
| Construir el índice desde la caché | ~19 µs por pasada |
| Prefiltro de 3 procesos | ~16 µs por pasada |

Sin accesos a disco: el índice se construye sobre la caché de `catalogo_lanzadores::obtener()`,
que sigue revalidando cada 5 minutos o cuando se invalida al activar el observador.

### Pruebas

- **828** pruebas Rust (antes 818), 1 ignorada a propósito; `cargo check` y `clippy` limpios;
  `rustfmt` sin diferencias en los ficheros tocados.
- **1027** pruebas TypeScript, `tsc --noEmit` limpio.
- Nuevas pruebas de integración que recorren el prefiltro real: Starsector (script + intérprete),
  Star Valor (Wine sin AppID), Eslabong (UMU con AppID 0), dos juegos externos a la vez, catálogo
  vacío, intérpretes ajenos, evidencia ambigua y regresión de Deskrawl.

### Limitaciones reconocidas

- **Nada de esto está validado en la máquina real.** Los fixtures son datos reales procesados por
  código real, pero no es el equipo del usuario. La validación de CachyOS sigue pendiente.
- El adaptador de Lutris sigue descartando el juego si falta el `.yml`; no se ha tocado en esta
  corrección.
- El alias de un juego presente en dos lanzadores se agrupa por título, no por identificador de
  plataforma: dos juegos distintos con el mismo título se confundirían.
- `java` con la clase del juego en una carpeta **compartida** por dos juegos da identidad
  desconocida, no una al azar. Es el comportamiento buscado, pero conviene saberlo.

## 5.3 — Validación real en CachyOS: resultados y regresión de Steam

Esta ronda parte de **pruebas reales en el equipo del usuario**, no de fixtures. Es la
primera vez que hay resultados de la máquina real para el pipeline multi-lanzador.

### Lo que quedó validado de verdad

| Caso | Resultado real |
|---|---|
| Starsector — Lutris, Linux nativo | Detección e identidad correctas |
| Starsector — cierre y reapertura | Correcto |
| Star Valor — Lutris, Wine | Detección correcta |

Esto cierra la duda principal de la ronda anterior: el prefiltro por catálogo y la
correlación por `cwd` funcionan fuera de las pruebas. **Se preserva ese comportamiento** y
las regresiones nuevas lo comprueban junto al resto.

Queda pendiente comprobar sobre datos reales los contratos de persistencia de las sesiones
(clave de proceso, continuidad tras pérdida de visibilidad, no reutilización al reabrir).

### La regresión de Steam, y su causa demostrada

Síntoma real: Deskrawl se publicaba como «juego sin identificar» y, con Palworld abierto a
la vez, la pantalla decía «Varios juegos en ejecución: Palworld, Palworld, Deskrawl: Idle
ARPG».

**Causa raíz (una sola, demostrada con reproducción antes de corregir):** al resolver una
identidad por el **entorno del proceso**, `resolver_identidad_con_catalogo` devolvía en
seguida con `nombre_comercial: None`. El nombre se rellenaba después, en `sesion_de_grupo`,
a partir del AppID. La corrección anterior dejó de pisar ese nombre (era correcto, porque
pisaba también el de los juegos externos que sí se habían resuelto), y con ello un juego de
Steam se quedó sin título: el AppID era correcto, el nombre no había quién lo pusiera.

Es un caso de dos motores de identidad para la misma decisión. La corrección **elimina el
segundo**, no lo parchea: la identidad por entorno resuelve ahora el título contra el
**mismo** catálogo y con el mismo criterio que un juego externo.

**Qué se comprobó y qué no** (no se da por buena una hipótesis sin mirarla):

- Se comprobó si el catálogo admitía wrappers o runtimes como grupos propios. Con los
  árboles reales de Palworld y Deskrawl, **antes y después** de la corrección, el escaneo da
  **un grupo por juego**. La hipótesis «la ampliación de vías duplica grupos» es **falsa**
  con los datos disponibles.
- No se ha podido reproducir la duplicación «Palworld, Palworld» desde el escaneo: aparece en
  lo **publicado**, no en los grupos. Por eso la regresión nueva compara lo que se publica a
  la interfaz, que es donde se veía el defecto.
- Un intento de reproducirla con dos árboles a la vez **sí** destapó un defecto, pero era
  del propio test (renumerar PIDs partía la cadena `pv-adverb → juego` en dos raíces y
  fabricaba grupos duplicados que producción no da). Está documentado en el helper
  `desplazar_pids`, que ahora conserva la forma del árbol.

### Regresiones añadidas

Recorren el pipeline completo (catálogo → índice → prefiltro → agrupación → correlación →
identidad → sesión) y comparan **lo publicado**, no solo lo guardado:

- Palworld solo: una entrada, con nombre.
- Palworld con `systemd-inhibit`: una entrada (el inhibidor hereda el AppID, pero no es otro juego).
- Deskrawl (dos variantes de envoltorio): una entrada, con nombre, y el auxiliar fuera.
- Palworld y Deskrawl a la vez: exactamente dos entradas.
- Dos instancias legítimas del mismo juego: **dos sesiones** (no se deduplica por AppID).
- Catalogo de cajón (`Descargas`) no confirma ningún juego.

### Heronite y Heroic: causas localizadas, corrección bloqueada por falta de datos

**No hay corrección especulativa.** Las dos causas están demostradas leyendo el código, pero
arreglarlas exige saber qué campos traen los ficheros reales del host, y eso no se puede
inventar.

**Heronite (eslabong).** El adaptador **no puede** rellenar `id_plataforma`, `install_dir`,
`wine_prefix` ni `exec` porque el struct `HeroniteGame` **no tiene campos** para el prefijo ni
para el ejecutable: solo lee `install.executable` y la carpeta de instalación. Los datos de
lanzamiento viven en los ficheros de runner de Heronite, que la app no interpreta. Por eso las
ocho entradas salen con esos campos a `null`, y por eso eslabong no correlaciona. **La ronda
anterior no documentó ninguna corrección de este adaptador porque no la hubo.**

**Heroic (Tattoo Tycoon).** El adaptador de Heroic lee la biblioteca de la tienda
(`legendary_library.json`), que trae título, AppID de Epic e `install_path`, pero **no lee**
la configuración por juego, que es donde Heroic guarda el prefijo y el ejecutable. Con
`install_dir` = `/home/jugador/Games/Heroic/TattooTycoon`, la correlación por carpeta debería
funcionar; si no lo hace, hay que ver el catálogo efectivo real antes de tocar nada.

### Qué falta para cerrar

1. Datos reales de Heronite y Heroic (comandos abajo).
2. Repetir la validación real de Steam, Deskrawl y Palworld con la corrección.
3. Comprobar los contratos de persistencia de sesiones sobre datos reales.

## 5.3 — Adaptadores Heronite y Heroic: implementación con datos reales

Datos de partida: los ficheros reales de CachyOS transcritos por el usuario. Nada de esto está
validado todavía en ejecución.

### Causa raíz común

Ni Heronite ni Heroic leían su `GamesConfig/<id>.json`. El código ni siquiera nombraba ese
directorio. El prefijo y el runtime se escribían a `None` en el adaptador, y por eso las ocho
entradas de Heronite salían con `wine_prefix` y `exec` a `null` y Tattoo Tycoon no se
detectaba. No era un problema de correlación: faltaban los datos.

### Heronite

- Se lee `GamesConfig/<appName>.json` con la ruta **construida** desde el identificador de esa
  entrada. No se busca "el JSON que tenga un prefijo": eso cogería el de otro juego (hay
  prueba que lo impide).
- Relación de fuentes por identificador exacto (`app_name` / `appName` / nombre del fichero),
  nunca por título.
- **Precedencia de ejecutables**: `remappedExecutable` (de `local_library`) manda sobre
  `install.executable`. No es preferencia de estilo: es lo que hace el lanzador cuando el
  usuario cambia el ejecutable desde su interfaz. Se conservan los dos.
- `install_path` pasa a ser la carpeta de instalación (`folder_name`), no `launcher_args`,
  que era la línea de arranque.
- `playniteId` con prefijo `heroic_` **no** se usa como prueba de lanzador ni de tienda.
- Las portadas con AppID de Steam **no** generan identidad: `appid` sigue siendo `None`.

### Heroic

- Se lee `GamesConfig/<app_name>.json` y de ahí salen prefijo y runtime configurado.
- El ejecutable de Epic viene **dentro de `install`**, relativo y con espacios
  (`Tattoo Tycoon.exe`). Buscarlo en la raíz devolvía `None`; se descubrió con una prueba.
  Se resuelve contra `install_path` sin recortar ni partir por espacios.
- Identidad: `app_name` de Epic prefijado con la tienda. `legendary` queda como runner de
  **gestión**, no como runtime de ejecución.

### Compartido (sin framework)

`ConfigDelJuego` + `leer_config_del_juego` en `launchers.rs`: una lectura y un tipo para los dos
lanzadores.

**Estructura real confirmada** (antes era una suposición): `GamesConfig.json` es un **mapa de
identificador a configuración**; la clave del objeto raíz es el propio id del juego.

```json
{ "5WzVLqXjCNJWsvmPCz4M4N": { "wineVersion": {…}, "winePrefix": "…" }, "otroId": {…} }
```

Por eso la lectura es acceso directo por clave: `json.get(id)`. La búsqueda recursiva de hasta
seis niveles que había se **eliminó** — era una precaución por no conocer el formato, y con el
formato conocido solo servía para poder coger la configuración de otro juego. Se lee
`winePrefix` y `wineVersion.{bin,name,type}` de la entrada pedida, y nada más. Sin respaldos
para formatos hipotéticos.

**La ruta del fichero es `base/GamesConfig/<id>.json`**, un fichero por juego: la
simplificación afectó solo a la navegación interna del JSON, no a la ubicación en disco.

### Regresión de rutas detectada y corregida (misma ronda)

Al simplificar, `leer_config_del_juego` pasó a recibir `base/GamesConfig.json` en ambas
llamadas. Era un fallo de borde en la refactorización, no del análisis: la estructura interna
era peculiar, la del fichero no. Corregido a `base/GamesConfig/<id>.json` en los dos
adaptadores. Lo cubren dos pruebas de **integración** que crean el árbol real y ejecutan
`scan_heronite_games` y `scan_heroic_epic_games` con `HOME` apuntando a él.

### Segundo fallo real, descubierto por esas pruebas de integración

Heroic leía `install_path` de la raíz de la entrada. En los datos reales de Tattoo Tycoon
está **dentro de `install`**, así que `install_path` salía `None`, `path_exists` daba `false`
y el juego se descartaba como "carpeta no existe" —antes incluso de mirar su configuración.
Corregido con `install.install_path` y respaldo en la raíz, porque
`legendaryConfig/legendary/installed.json` sí lo escribe arriba. El respaldo está justificado
por un contrato existente, no por un formato hipotético.

### Separaciones que se mantienen

- `runner` (quién gestiona: `sideload`, `legendary`) ≠ `runtime_configurado` (Proton/Wine
  configurado). Campo nuevo y separado en `ExternalGame` y `EntradaDeCatalogo`.
- Lo configurado **no** se presenta como observado: que Heroic tenga Proton configurado no
  demuestra que el proceso use esa versión.
- El prefijo sigue siendo evidencia complementaria. La normalización base↔`pfx` ya existía
  (`normalizar_prefijo`) y no se ha tocado: no se elimina `pfx` de rutas arbitrarias.

### Verificación

- `cargo test --bins`: **850 passed / 0 failed / 1 ignored** (9 nuevas, entre ellas
  `varios_identificadores_en_el_mismo_documento_no_se_mezclan`, que impide volver a una
  búsqueda "del primer prefijo que aparezca", y una que cubre fichero ausente, JSON inválido
  y campos opcionales).
- `cargo check --bins` limpio; clippy solo el aviso preexistente de `benchmarks.rs` (9/7).
- TS: `tsc --noEmit` limpio; `vitest` **1027 passed** (75 ficheros).
- `rustfmt` aplicado a los 4 ficheros tocados.

**Todo esto está probado con fixtures que transcriben los ficheros reales. No es validación
real.** La fase 5.3 sigue ABIERTA hasta ejecutar el protocolo en CachyOS.

## 5.3 — Validación real en CachyOS (ronda anterior)

### Lo que la máquina real confirmó que funciona

| Caso | Resultado |
|---|---|
| Sin juegos abiertos | OK — sin falsos positivos |
| Deskrawl (Steam + Proton) | OK — una sesión, AppID 4623570, nombre comercial, PID representante = el que ejecuta `Deskrawl.exe` |
| Cerrar Deskrawl | OK — vuelve a «Buscando un juego…» |
| Reabrir Deskrawl | OK — sesión nueva, PID nuevo; la anterior queda cerrada |
| Palworld | Detección visual OK; falta verificar PID representante y sesión persistida |
| Envoltorios y auxiliares | Ya no abren sesiones propias |

Las correcciones de Deskrawl de la ronda anterior se sostienen en la máquina real.

### Lo que falla (los tres juegos externos)

| Juego | Lanzador | Runtime | Resultado |
|---|---|---|---|
| Eslabong | Heronite | UMU + Proton | **No detectado** |
| Star Valor | Lutris | Wine | **No detectado** |
| Starsector | Lutris | Linux nativo | **No detectado** |

Los tres aparecen en la biblioteca de Thoriapp. El observador no los detecta al ejecutarse.
Starsector es el caso decisivo: sin Wine, sin Proton, sin UMU y sin rutas Windows, el fallo
no puede atribuirse a la representación de ejercicios de Wine.

### Causa raíz auditada (código, no hipótesis)

El prefiltro de `/proc` decide qué proceso llega a ser candidato, y **solo admite tres
evidencias, las tres de Steam**:

`proceso_linux.rs:519-628` (`parece_juego`):

1. La ruta del ejecutable es de Proton (`ruta_de_proton`) → evidencia de Steam.
2. El ejecutable está en `steamapps/common` (`install_dir_de_ruta`, `evidencia_proceso.rs:508`) →
   **solo reconoce `steamapps/common`**; una carpeta de Lutris o Heronite no encaja.
3. La línea de comandos menciona un `.exe` → solo lanzamientos Windows.
4. El `comm` contiene `.exe` o `.x86_64` → convenciones de Windows y de binario nativo Steam.

Nada de eso se cumple para un juego de Lutris/Heronite. Y hay cuatro dependencias de Steam más,
confirmadas por lectura:

- `SteamAppId`/`SteamGameId` son las únicas variables de identidad que se leen. Con UMU llegan
  como **`0`**, que `appid_consensuado()` devuelve como `Ok(Some(0))` — un valor, no una
  ausencia — y el AppID 0 no identifica ningún juego.
- `WINEPREFIX` **no estaba** en la lista blanca del entorno (`VARIABLES_PERMITIDAS`), así que el
  prefijo nunca llegaba al dominio y la correlación por prefijo no tenía con qué comparar.
- `resolver_identidad` (`sessions.rs:858`) consulta **solo** la biblioteca de Steam
  (`JuegoDeBiblioteca`). El catálogo multi-lanzador entra únicamente como último recurso para
  poner `nombre_comercial`, nunca para decidir identidad ni elegibilidad.
- `asignar_juego` rechaza AppID 0, lo que confirma que cero nunca fue una identidad válida.

**Efecto medido:** un juego externo es descartado por el prefiltro antes de llegar al catálogo.
El catálogo está bien construido y la correlación por carpeta y por prefijo funciona (hay tests
que lo demuestran), pero **nunca se le pregunta**, porque el proceso no sobrevive al filtro.

### Por qué los tests no lo detectaron

Ningún test existente pasa un juego externo por el **prefiltro real** (`parece_juego`). Los tests
de catálogo y de correlación construyen `EntradaDeCatalogo` y `RasgosDeProceso` a mano y llaman
directamente a `correlacionar_con_el_catalogo`; es decir, prueban el eslabón 8 de la cadena
saltándose el 2, que es donde se pierde el juego. Es la misma clase de error que el fixture de
Deskrawl de la ronda anterior: datos ideales que producción nunca genera.

Se añadieron cuatro tests de auditoría que fijan los hallazgos (sin corregirlos aún):

- `auditoria_un_juego_externo_nativo_no_pasa_el_filtro` (Starsector/`java`)
- `auditoria_eslabong_eslabong_con_appid_cero` (UMU con `SteamAppId=0`)
- `auditoria_lutris_correlaciona_por_prefijo_cuando_no_hay_carpeta` (Lutris sin `directory`)
- `auditoria_la_biblioteca_de_steam_no_es_el_catalogo_del_observador`

### Instrumentación ampliada (opt-in, sin datos privados)

`THORIAPP_DIAGNOSTICO_PROCESOS=<fichero>` ahora registra además:

- `carpeta_declarada`, `wine_prefix` y `ejecuta_de_verdad` de cada candidato.
- El **catálogo efectivo** del observador (`catalogo`), que no es el de la biblioteca.
- La **decisión de correlación** por grupo (`correlaciones`): por qué vía correlacionó o por qué
  no, con la carpeta, el prefijo y el tamaño del catálogo.

No se registran entornos completos, credenciales ni rutas personales innecesarias.

### Detalles menores registrados (fuera de la investigación principal)

- Al representante de Deskrawl se le guarda `wine64-preloader` como ejecutable Linux, y sería
  útil conservar aparte la identidad del ejecutable Windows (`Deskrawl.exe`).
- La evidencia de identidad lista «ejecuta UnityCrashHandler64», que es un auxiliar.

### Bloqueos

- **La fase 5.3 no se puede cerrar**: tres de los cinco casos de validación real fallan.
- **No se ha aplicado ninguna corrección funcional**: falta la evidencia de la máquina real con
  la instrumentación ampliada para confirmar el eslabón exacto de cada juego.

## 5.3 — Integración multicatálogo, prefijo y ciclo de vida (ronda anterior)

Lo que cambió en esta ronda, con motivo:

### Catálogo unificado con caché híbrida (`catalogo_lanzadores.rs`)

- Una sola vista `EntradaDeCatalogo` sobre Steam + Lutris + Heroic + Heronite. No es una
  abstracción nueva: reutiliza `scan_all_launchers` y `catalogo_para_identidad`.
- **Caché con huella**, no con `mtime`: la huella es el conjunto ordenado de
  `id_plataforma|plataforma|launcher|runner|titulo|install_dir`. Detecta un cambio de
  carpeta o de runner aunque el identificador no cambie (cosa que `mtime` no distingue
  bien con SQLite en modo WAL).
- **Revalidación cada 5 min** (`REVALIDACION`), más `invalidar()` al encender el observador
  y `actualizar()` para el botón de biblioteca. Fuera de eso, **no se abre SQLite ni se
  relee el catálogo en cada pasada**.
- **Vacío legítimo ≠ fallo de lectura**. `Lectura::{Bien, Fallo}` separa ambos casos: un
  catálogo vacío se cachea como vacío; un fallo conserva lo último bueno y marca la caché
  como obsoleta, y la identidad lo dice con una evidencia explícita («catálogo
  desactualizado…») en vez de presentarlo como verificado.
- Contadores en `mod medida` para medir por separado el coste de actualizar el catálogo y
  el de una pasada normal, con datos reales.

### Prefijo de Wine: evidencia, nunca identificador

- `EntradaDeCatalogo::comparte_prefijo` normaliza el `pfx` final, porque Heronite guarda el
  directorio base y el proceso declara `…/pfx`. Se compara por componentes, no por texto.
- Se usa **solo como último recurso**, cuando la carpeta de instalación no decide (el caso
  real de Lutris con `directory` vacío). Si dos juegos comparten prefijo, no se elige
  ninguno: compartir prefijo no es compartir juego.
- **Nunca se fusionan sesiones por prefijo.** PID + starttime siguen siendo la identidad de
  instancia.

### Ciclo de vida de la sesión

- Nuevo estado `VisibilidadPerdida`: una pasada sin ver el proceso **no** cierra la sesión.
  Tras `PASADAS_SIN_VERLO` (3) pasadas seguidas sí se marca `Terminada`. Si vuelve a verse
  antes, se recupera y el contador vuelve a cero.
- Se **elimina** el descarte de grupos por AppID compartido. Confundía el AppID (identifica
  el **juego**) con la sesión (identifica la **ejecución**): cerrar y reabrir un juego
  perdía la segunda ejecución dentro de la primera. La continuidad la da la clave
  PID + starttime; el mismo PID con otro starttime es **otra** ejecución.
- Se eliminó también la herencia de AppID por raíz de lanzamiento: sin AppID propio, el
  envoltorio forma su propio grupo y no se le atribuye el de un hijo.
- El observador **escanea siempre**, haya o no una sesión abierta: antes, con un juego en
  marcha no se veía un segundo juego simultáneo.

### Confirmación de juego: sin listas infinitas

- Un `.exe` bajo Wine **no** es prueba de juego (hay aplicaciones de escritorio). Sin
  evidencia suficiente se queda en `CandidatoSinConfirmar`: no se abre sesión y no se
  descarta.
- La estructura sola tampoco basta: una herramienta de configuración o un manejador de
  fallos dentro de la carpeta del juego no lo convierten en el ejecutable del juego. Se
  confirma por catálogo, por ejecutable declarado por el lanzador, o por binario nativo en
  `steamapps/common`. Las listas de nombres (`AUXILIARES`) son **protección
  complementaria**, no el mecanismo principal: la clasificación es estructural.

### Aviso sobre el alcance

Nada de esto está validado contra el equipo real. Todo lo anterior son pruebas automáticas
con fixtures. La validación real en CachyOS sigue pendiente y es la única que puede afirmar
que la detección funciona; ver `docs/validacion-real.md`.

## 5.2 — Qué hay

- Instantánea inmutable por prueba: identidad, configuración y entorno.
- Procedencia por dato (`ConProcedencia<T>`): motor de Rust, Thoriapp, Steam,
  **MangoHud**, sistema, usuario, ejecución, desconocido.
- **Dos bloques de hardware independientes**:
  - `hardware_grabacion`: lo que registró MangoHud en el CSV (CPU, GPU, RAM en KiB + GiB,
    kernel, driver, planificador, intervalo de muestreo).
  - `entorno`: lo detectado **al analizar**.
  - Ninguno rellena los huecos del otro. Si difieren, la interfaz lo avisa sin modificar
    nada.
- `resumen_mangohud`: el `_summary.csv` original, como fuente separada. No sustituye a las
  métricas de Thoriapp ni se usa para comparar.
- `InstantaneaCard`: tarjeta plegable en la vista de benchmarks y en el detalle de
  comparaciones guardadas.

## Criterios que no se negocian

- Un hueco histórico se queda en «no consta»; nunca se rellena con datos de hoy.
- Configurar Proton ≠ haber usado Proton. Hasta 5.3, «ejecución real: no verificado».
- Un cero constante **no** implica sensor ausente: depende de la métrica.
- Correlación temporal ≠ causa.
- `os` del CSV con Proton es el runtime de Steam, **no** la distribución.
- `driver` vacío no se deduce del nombre de la GPU.
- El AppID no se toma del juego abierto: si no hay evidencia, queda desconocido.
- Las grabaciones originales de `tests/fixtures/mangohud/` no se modifican ni se borran.

## 5.4.1.A — Integridad de datos y metodología de cálculo (implementado, pendiente de revisión)

Cierra el defecto de `drop_stuck_samples` que la auditoría 5.4.0 midió sobre las funciones
reales de v1.2.0. **No se cambia ninguna fórmula estadística**: cambia **qué filas entran**
en ellas, y eso queda declarado por la metodología v2.

### El defecto que corrige

`drop_stuck_samples` descartaba filas consecutivas con `fps` y `frame_time` idénticos. En una
serie planchada —limitador de FPS, pausa, tramo lento— esa coincidencia **es el dato**.
Medido con el código de producción antes del cambio:

| Caso sintético | v1 (antes) | v2 (ahora) |
|---|---|---|
| Limitador 60 FPS constante, 600 filas | descarta 599 → 1 muestra → **rechazo por `MIN_SAMPLES`** | 0 descartadas, 500 analizadas |
| Caída sostenida 20 s a 20 FPS | descarta 597 → 3 muestras → **rechazo** | 0 descartadas, caída conservada |
| Patrón periódico 6×60,2 ms + 1×16,7 ms | descarta 429 → media **38,2458** (inflada +68,1 % sobre la real 22,7555) | 0 descartadas, media ≈ 22,76 |
| Mismo patrón invertido | **38,2458** (idéntico) | distinto del anterior (distribuciones distintas) |
| Duplicado con timestamp idéntico | descarta 600 → 1 → rechazo | conservado y **declarado** como incidencia |
| Sin `elapsed` | descartaba igual (599 de 600) | conserva las 600, sin deduplicar |

El fallo grave no era el sesgo, sino que **el descarte vaciaba la serie y `MIN_SAMPLES`
llegaba tarde**: una caída de 20 s se presentaba como «graba más tiempo», mandando al usuario
a repetir una prueba que habría fallado igual.

### Reglas aplicadas (v2)

- FPS y frametime iguales con **timestamps distintos** → se conservan.
- Filas **completamente idénticas** (mismo `elapsed`) → se conservan y **se registra la
  incidencia**. No hay evidencia de que sean doble registro; no se eliminan.
- Sin `elapsed` → se conserva todo y **no se deduplica**.
- Timestamps repetidos, desordenados, negativos o no finitos → **se detectan y registran**.
  No se ordenan, ni se interpolan, ni se corrigen, ni se inventan.
- Ninguna fila con rendimiento válido se excluye por su timestamp.

### Eje temporal y métricas que dependen del reloj

Un eje se declara fiable solo si la columna existe, **todas** las filas traen reloj, ninguno
es negativo y el orden es estrictamente creciente. Sin eje fiable:

| Métrica | Comportamiento |
|---|---|
| Duración | `duracion_medida = None`; **no se inventa**. `duration_seconds` queda `0.0` por contrato y ese cero significa «no consta» |
| Calentamiento | **no se aplica**; se declara el motivo en `calentamiento_no_aplicado` |
| `spikes_per_min` | `None` (sin divisor no hay tasa) |
| `pct_over_33ms`, `jitter_ms`, `worst_frame_ms` | se calculan (solo dependen de los valores) |
| Percentiles, medias, `noise_percent` | se calculan (no dependen del reloj) |
| Ventana de shader, serie temporal con eje X | no disponible |

`noise_percent` se calcula sobre bloques **contiguos** de la serie, así que no depende de
`elapsed`; su limitación es otra y ya consta: mide la variación **dentro** de una grabación,
no entre partidas, y no es un intervalo de confianza validado.

### Contabilidad de muestras

`ContabilidadDeMuestras` reparte las filas de datos sin doble conteo:

```
filas_originales = filas_con_rendimiento_invalido
                 + excluidas_por_calentamiento
                 + muestras_analizadas

filas_con_rendimiento_valido = excluidas_por_calentamiento + muestras_analizadas
```

`filas_originales` cuenta las líneas **entre la cabecera de datos y el final**, sin los
metadatos iniciales (`OS`, `CPU`, `GPU`…), sin la cabecera y sin líneas en blanco. Las
incidencias temporales se contabilizan **aparte**: no son exclusiones y una misma fila puede
acumular varias. El invariante se comprueba en pruebas además de con `debug_assert!`.

### Campo `excluidas_por_reloj_no_fiable`: eliminado

Se descartó por diseño: una fila con FPS y frametime válidos **no se excluye porque su
timestamp sea inválido**. Los puntos que no pueden representarse en un eje temporal se
describen con `eje_temporal_fiable = false` e `incidencias`, no como exclusiones de
rendimiento.

### Compatibilidad histórica

- `metodologia_calculo` es `Option<MetodologiaCalculo>` con `#[serde(default)]` y
  `skip_serializing_if`. Los benchmarks anteriores quedan en **`None` = «no consta»**: no se
  rellenan con `v1` ni se recalculan.
- `MetodologiaCalculo` **solo tiene `V2`**: no queda un segundo pipeline v1 activo.
- Los campos nuevos ausentes en datos históricos siguen desconocidos; nunca se convierten en
  cero al releer.
- Leer un archivo histórico **no cambia sus bytes ni su `mtime`** (hay prueba que lo mide).
- `EvaluacionPersistida` y `SavedComparison` **no se tocan**: son el bloque B.

### Contrato de duración: una sola regla

`duration_seconds` sigue siendo `f64` por compatibilidad con los JSON guardados, pero **no se
lee directamente en ningún sitio**. La interpretación vive en un único punto por lenguaje:

- Rust: `Benchmark::duracion()` → `DuracionDeLaGrabacion`.
- TypeScript: `duracionDe()` en `src/lib/duracion.ts`.

Los tres estados son explícitos:

| Estado | Cuándo | `duration_seconds` | Resultado |
|---|---|---|---|
| Histórica | Sin `metodologia_calculo` | Es el valor medido entonces | Se usa tal cual |
| v2 medida | `metodologia_calculo` y `duracion_medida` | 0 | Se usa `duracion_medida` |
| v2 desconocida | `metodologia_calculo` sin `duracion_medida` | 0 | **No consta**; **no** se cae a `duration_seconds` |

Consumidores corregidos: `atribuir_a_sesion` (ventana del CSV), el aviso de duración en
`comparar_benchmarks`, el aviso de duración en `interpretacion.ts`, la marca de grabación
corta y el texto de duración en `BenchmarksView.tsx`, y las dos duraciones de
`SavedComparisonDetail.tsx`. La interfaz escribe «duración no registrada» en vez de «0 s».

### 5.4.1.B — Evaluaciones persistidas

`SavedComparison` gana `evaluacion: Option<EvaluacionPersistida>`. **`None` no es un
veredicto neutro**: significa *no registrada* (comparación histórica), y es distinto de
`estado: datos_insuficientes`, que es una evaluación que sí existe.

**El bloque B no genera evaluaciones.** `save_comparison` solo recibe dos ids: no hay
parámetro por el que React pueda colar una evaluación, y no existe ningún motor
provisional. Las escribirá el motor Rust del bloque C, en la misma operación que crea la
comparación.

**La clasificación va en la lectura, no en un comando aparte.** `load_comparisons()`
devuelve cada comparación con su `estado_de_la_evaluacion` ya calculado:
`no_registrada` (no hay evaluación), `soportada` (se entiende), `no_soportada` (existe,
pero con códigos o metodología que esta versión no conoce) o `malformada` (existe, pero no
se puede leer: el resto de la comparación y el JSON original se conservan). Lo decide el
backend; la interfaz lo pinta y no lo deduce.

No existe un comando aparte para preguntarlo. Lo hubo durante la implementación, sin
consumidor real, y se eliminó: un `invoke` más para leer un dato que la lista ya trae no aportaba
nada, y su único efecto era callar un aviso de código muerto.

### Limitaciones aceptadas

- **Concurrencia entre procesos independientes**: `CERROJO_COMPARACIONES` es un `Mutex`
  estático y protege el ciclo completo lectura-modificación-escritura **dentro del
  proceso**. Dos instancias de Thoriapp a la vez no están cubiertas por él. La escritura
  atómica evita un archivo a medias, **no** una actualización perdida. Se revisará en la
  fase 9 (seguridad y hardening).
- **El bloque B no escribe evaluaciones**: ningún camino de producción las genera todavía;
  solo se leen y se conservan. Lo hará C, de forma atómica con la creación de la
  comparación.

### Cuatro versiones que no evolucionan juntas

| Versión | Dónde | Valor |
|---|---|---|
| Esquema de `saved_comparisons.json` | constante global | **2** |
| Esquema de `benchmarks.json` | constante global | 1 |
| Metodología de cálculo | por benchmark | `v2` o ausente |
| Metodología de interpretación | por evaluación | `v1` |
| Aplicación | `Cargo.toml` / `package.json` | 1.2.0 |

Subir el esquema de comparaciones a 2 no migra nada: los ficheros v1 se leen igual y **no
se reescriben al leerlos**. El cambio de versión se escribe solo cuando aparece contenido
que exige v2. Un fichero v2 **nunca baja a v1**, ni aunque se borren todas sus
evaluaciones: si bajara, una versión antigua volvería a abrirlo y descartaría en silencio
lo que no entiende.

### Escribir sin perder lo que esta versión no entiende

El riesgo era real: `save_saved_data` reescribía el archivo **entero** desde structs
tipados, así que cualquier campo de una versión futura desaparecía. Y no era teórico:
borrar una comparación con Thoriapp v1.2.0 habría eliminado la `evaluacion` de **todas las
demás**.

Cómo se resuelve, sin infraestructura genérica:

- La lectura guarda el `serde_json::Value` **original** de cada comparación y del nivel
  raíz.
- Al escribir, si el id ya estaba en disco se emite **ese `Value`**, no una
  reserialización del struct. Los registros nuevos, y solo ellos, se serializan.
- La conservación es **semántica, del JSON entero**: tipos, orden de claves que importa,
  campos desconocidos a cualquier profundidad — dentro de `evaluacion`, de los snapshots
  y de los arrays anidados —. No es conservación byte a byte del fichero: se reindenta con
  `to_string_pretty`, y comparar bytes como criterio sería equivocado. Lo que se fija con
  pruebas es que **leer y volver a escribir no pierde ni inventa ningún dato**.
- La guarda de `VersionFutura` deja de ser decorativa: al escribir v2, v1.2.0 rechaza el
  archivo en vez de destruirlo.

No hay fusión recursiva de campos ni mecanismo para añadir una evaluación a posteriori.
Ningún camino de producción modifica un registro existente: `save_comparison` solo añade,
`delete_comparison` y `delete_all_comparisons` solo quitan. Por eso el código de fusión
preventiva que llegó a escribirse se **eliminó**: era complejidad para un camino que no
existe. Cuando el bloque C cree la comparación con su evaluación, ambas se guardarán
juntas, de una vez.

Un id duplicado en disco se detecta y se reporta como archivo corrupto: no se adivina cuál
vale. Las escrituras siguen siendo atómicas y pasan por `CERROJO_COMPARACIONES`.

### Magnitudes: FPS y frametime no son intercambiables

Dos patrones periódicos opuestos —6 muestras de 60,2 ms + 1 de 16,7 ms, y su inverso— dan,
**tras el calentamiento de 10 s (500 muestras analizadas de 600)**:

| Patrón | Media de **FPS** | Media de **frametime** |
|---|---|---|
| A: 6×60,2 ms + 1×16,7 ms | **22,755486** | **54,023** ms |
| B: 6×16,7 ms + 1×60,2 ms | **53,736049** | **22,877** ms |

Medias de frametime por **ciclo completo** (7 muestras, sin calentamiento): 53,985714 ms
para A y 22,914286 ms para B. Las dos cifras son correctas y **no se comparan entre sí**:
una es media de FPS y otra de frametime. v1 daba 38,2458 para ambos patrones.

### Incidencias temporales: el parser ya no las colapsa

`RelojDeFila` distingue `ColumnaAusente`, `CeldaAusente`, `CeldaVacia`, `Malformado`,
`NoFinito` y `Valor(f64)`. Antes los cuatro primeros casos acababan en el mismo `None` y no
se podía decir al usuario qué le pasaba a su CSV. `filas_repetidas` cuenta relojes repetidos
**aunque el resto de la fila no coincida**.

La distinción **se persiste**, no solo se usa durante el parseo: `IncidenciasTemporales` lleva
`columna_elapsed_ausente`, `celda_reloj_ausente`, `celda_reloj_vacia` y `celda_reloj_ilegible`,
que suman exactamente `reloj_incompleto`. El invariante se comprueba en `preparar_muestras`.

Las series temporales sin eje fiable **no inventan segundos**: `elapsed_s` queda vacío y el
gráfico usa el índice de muestra.

### `spikes_per_min` sin reloj: era un cero inventado

`StutterStats::spikes_per_min` era `f64` y devolvía `0.0` cuando la duración no constaba, es
decir, «ningún pico» sin saberlo. Ahora es `Option<f64>`: `None` = no consta. El campo
público del `Benchmark` ya era `Option` y el frontend ya lo comprobaba, así que no hay cambio
de contrato.

### Qué métricas sobreviven sin eje temporal fiable

| Métrica | Sin eje | Por qué |
|---|---|---|
| `avg_fps`, percentiles, `frame_time_avg` | Sí | Solo dependen de los valores |
| `noise_percent` | Sí | Bloques contiguos **por índice**, no por tiempo: mide la variación dentro de la grabación, no un intervalo de confianza validado |
| `pct_over_33ms`, `jitter_ms`, `worst_frame_ms` | Sí | Solo dependen de los valores |
| `spikes_per_min` | No (`None`) | Sin duración no hay divisor |
| Duración | No (`None`) | No se estima a partir de una frecuencia de muestreo supuesta |
| Calentamiento | No | Se declara `MotivoSinCalentamiento::SinEjeTemporal` |

`MIN_SAMPLES` no cambia y se sigue evaluando sobre las muestras analizadas reales.

### Pendiente (bloques B y C)

Persistencia completa de `Evaluacion` en `SavedComparison` y marca de comparación histórica
(B, hecho: bloque 5.4.1.B); integración, persistencia atómica y conexión de la presentación
tipada (C; C.2 implementado, C.3 pendiente).

## Pruebas

- Rust: **1042** (1 ignorada a propósito: falla si GGG añade re-endpoint de consulta).
- TypeScript: **1058**, en 76 ficheros.
- `tsc --noEmit` limpio.
- `cargo clippy --locked --bins -- -D warnings`: **sin avisos**. `save_benchmark` lleva un
  `#[allow(clippy::too_many_arguments)]` localizado porque su firma plana es el contrato IPC
  con el frontend (mismo criterio ya aplicado en `save_profile`).
- `cargo audit` sobre `src-tauri/Cargo.lock`: sin vulnerabilidades. Quedan tres avisos
  informativos de la base de RustSec, visibles y no bloqueantes (ver `CHANGELOG.md`).

## Deuda técnica registrada

### Avisos de Clippy en el código de pruebas (`--tests`)

El CI ejecuta `cargo clippy --locked --bins -- -D warnings`, que **no analiza el código de
pruebas**. Con `--tests` (o `--all-targets`) aparecen **40 avisos** en pruebas y fixtures,
todos preexistentes: los ficheros afectados (`fixtures_procesos.rs`, `evidencia_proceso.rs`)
están sin modificar respecto a los commits anteriores a esta entrega.

Aclaración de la discrepancia 40/41: la ejecución imprime 41 líneas que empiezan por
`error`, pero solo **40 son avisos**; la 41 es la línea de resumen
`error: could not compile ... due to 40 previous errors`. El recuento correcto es 40.

Prioridad alta, porque puede tratarse de un defecto de los fixtures y no solo de estilo:

- **Escapes octales ambiguos** (3): `"lutris-wrapper\0Star Sector\00\0starsector.sh\0"`,
  `"…Star Valor\00\0wine\0…"` y `b"/usr/bin/sleep\0300\0"`. En Rust, `\00` y `\0300` son
  escapes octales, no dos caracteres: pueden no describir el `cmdline` que la prueba cree
  estar describiendo. Hay que decidir si el fixture debe llevar `NUL` reales (`\0` + `"0"`)
  y corregirlo. Hasta entonces, esas pruebas podrían estar pasando por el motivo equivocado.

Resto de avisos, prioridad menor (estilo y limpieza):

- 8 funciones `*_publico` nunca usadas (puentes de prueba que quedaron sin llamadores).
- 7 usos innecesarios de `clone()`/`collect()`/`vec!`.
- 5 elementos de lista en documentación sin sangría.
- 3 asignaciones de campo fuera del inicializador con `Default::default()`.
- 2 aserciones con valor constante (no pueden fallar: no prueban nada).
- 2 `replacing text with itself` (sustituciones que no sustituyen nada).
- 2 bloques de `items after a test module`.
- Y 8 sueltos: lifetimes elidibles, `&*` redundante, `field_reassign_with_default`, etc.

No se corrigen en esta entrega, que solo pretendía arreglar el CI. Corregirlos es una tarea
aparte, y conviene empezar por los escapes octales.

### Otras

- `revision_motor` de la instantánea es `null`: el motor no publica una revisión estable.

## Limitaciones conocidas

- `revision_motor` de la instantánea es `null`: el motor no publica una revisión estable.
- `ejecucion_verificada` sigue siempre `null`: 5.3 observa el **proceso**, no el **efecto**
  de cada optimización. Observar Proton no prueba que FSR o NTSYNC entraran en acción.
- `driver` y versión de Proton no se registran cuando el CSV no los trae.
- Las métricas adicionales del CSV (temperaturas, potencias, VRAM, RAM, swap, RSS) se
  leen y clasifican, pero **no entran en las comparaciones**: eso es 5.4.
- El resumen de MangoHud se guarda pero no se usa para decidir nada.

## Referencias

- `docs/sesiones-de-ejecucion.md` — decisiones de la subfase 5.3 (observación, identidad, privacidad).
- `docs/mangohud-summary.md` — unidades, muestreo y metodología del summary (experimental).
- `tests/fixtures/mangohud/` — dos grabaciones reales con sus resúmenes.

## 5.3 — Captura temprana y evidencias de ejecución (implementada)

### Arquitectura

Tres piezas, con las dependencias en un solo sentido:

| Módulo | Responsabilidad | Sabe de |
|---|---|---|
| `sessions.rs` | Dominio: modelos, identidad del juego, asociación grabación↔sesión, ciclo de vida. Puro, sin E/S. | Nada del sistema |
| `proceso_linux.rs` | Adaptador `/proc`: implementa `ObservadorDeProcesos`. | Linux |
| `sessions_service.rs` | Servicio: hilo único del observador, sondeo adaptativo, persistencia con el cerrojo de 5.1, asociación al guardar. | Dominio y adaptador |
| `diagnostico_procesos.rs` | Volcado opcional de una pasada (`THORIAPP_DIAGNOSTICO_PROCESOS`). Apagado por defecto, sin entorno completo ni línea de comandos entera. | Dominio (solo lectura) |

El puerto `ObservadorDeProcesos` permite la implementación real y una simulada, y es lo que
hace medibles los escenarios que aquí no se pueden reproducir (Docker sin ver el anfitrión,
SteamAppId contradictorio, PID reutilizado).

### Decisiones

- **Una sola instancia**, un solo hilo (`OnceLock` + `start()` idempotente). No hay dos
  sondeos a la vez por construcción: el bucle duerme entre pasadas.
- **Independiente de la pantalla y del foco.** Arranca con la app y sigue con la ventana
  minimizada. No hay temporizador en React que lo gobierne.
- **Sondeo adaptativo**: 5 s buscando, 10 s siguiendo un juego, 30 s sin visibilidad, 0 s
  apagado. Si una pasada tarda más de un cuarto del intervalo, la espera se estira
  (`coste × 4`), así que el observador nunca puede comerse la CPU que está midiendo.
- **Desactivable** desde el pie de la barra lateral. Apagado no se hace ni una lectura de
  `/proc` (comprobado: `sessions.json` deja de reescribirse y el interruptor pasa a gris).
- **Identidad**: entorno del proceso > carpeta `steamapps/common` > coincidencia única por
  nombre (marcada como **inferida**). Varias coincidencias o `SteamAppId` ≠ `SteamGameId` ⇒
  ambigua, sin elegir. El juego abierto en la interfaz **no** es evidencia.
- **Asociación**: temporal (ventana del CSV dentro de la vida del proceso, ±60 s) **y** de
  identidad. Una grabación de hace un mes no se cuelga de un proceso de hoy aunque el juego
  se llame igual.
- **`sesion_id` es solo un enlace.** Todo lo interpretable está en la instantánea del
  benchmark: borrar sesiones no deja pruebas ilegibles, y borrar pruebas no rompe sesiones.
- **Privacidad**: lista blanca de cuatro variables (`SteamAppId`, `SteamGameId`,
  `SteamClientLaunch`, `PROTON_VERSION`), validadas antes de serializar. Se lee `environ`
  **solo** de los candidatos que pasan un filtro barato por nombre. Nunca `cmdline`.
- **`configuracion_inicial` se marca `posterior_al_arranque: true`**: el observador ve el
  proceso cuando ya lleva rato corriendo, así que no puede llamarlo «comando de arranque».

### Arreglo de identidad

`norm_nombre` no quitaba espacios ni guiones bajos: `7DaysToDie` no resolvía contra
`7 Days to Die` y la grabación se guardaba con AppID 0. Ahora la normalización es única
(`sessions::normalizar_nombre`) y, cuando varias coincidencias son posibles, gana la más
específica (`Palworld-Win64-Shipping` → `Palworld-Win64` sobre `Palworld`); si empatan, no
se elige.

### Medición del coste

- Una pasada completa sobre 68 entradas de `/proc`: **0,66 ms** (test `medir_el_coste_de_una_pasada`).
- En la app real: **1,6 ms por pasada** (se lee del pie de la barra lateral).
- Con 5 s de intervalo son ~0,03 % de un núcleo; con 10 s, la mitad.

### Limitaciones

- En Docker no se ve Steam del anfitrión: la sesión queda `sin_visibilidad` y la interfaz
  dice «No consta una ejecución observada», nunca «el juego no está corriendo».
- `revision_motor` sigue siendo `None`: el motor no publica revisión estable.
- No se afirma el efecto de ninguna optimización. Ver un proceso de Proton no demuestra que
  FSR, NTSYNC o MangoHud estuvieran activos.
- La prueba con un juego real de Steam está pendiente en CachyOS; aquí se han hecho con
  procesos simulados y con el observador simulado (indicado en cada test).

## Corrección de 5.3 — la detección que no detectaba

La primera prueba real del usuario en CachyOS **no pasó**: con Palworld abierto, la barra
lateral seguía diciendo «Buscando un juego · 2,3 ms por pasada» y la Prueba 14 no registraba
sesión ni AppID.

### Causa raíz

El puerto `ObservadorDeProcesos` exponía `buscar_proceso(nombre)`. Es decir, el observador
buscaba **un proceso Linux que se llamara como el juego**. Eso vale para un juego nativo y
falla para todos los de Proton:

| Dato real del equipo | Valor |
|---|---|
| `comm` | `GameThread` |
| `exe` | `…/compatibilitytools.d/proton-cachyos-slr/files/lib/wine/i386-unix/wine64-preloader` |
| `cmdline` | `S:\steamapps\common\Palworld\Pal\Binaries\Win64\Palworld-Win64-Shipping.exe Pal -dx12` |
| Entorno | `SteamAppId=1623730`, `SteamGameId=1623730`, `SteamClientLaunch=1`, **sin** `PROTON_VERSION` |

Ninguno de esos datos contiene el nombre «Palworld» en el sitio donde se buscaba, y el módulo
además **no leía `cmdline`** (lo decía explícitamente: «nunca `cmdline`»). Había tres defectos
encadenados:

1. **Se filtraba antes de leer la evidencia.** `parece_juego` descartaba por el ejecutable
   (`/usr/bin/…`, `/usr/lib/…`) **antes** de mirar la línea de comandos. Un proceso con ruta
   de juego en sus argumentos se perdía por su `exe`.
2. **El escaneo dependía de la pista.** Sin pista fijada, `paso()` no miraba `/proc`.
3. **Se exigía una coincidencia de nombre** que un juego de Proton no puede dar.

Además la lista de descartes contenía `sleep`, `python3` y `reaper` como nombres, lo que
mezclaba «auxiliar» con «proceso del sistema», y el guard de duplicados releía el fichero,
donde la sesión recién creada aún no estaba: **una sesión duplicada por pasada**.

### Detección actual

`evidencia_proceso.rs` (nuevo, funciones puras) reúne la evidencia y `proceso_linux.rs`
recorre `/proc` una vez por pasada, agrupando:

| Evidencia | Peso |
|---|---|
| `SteamAppId` = `SteamGameId` | identifica el AppID |
| `SteamLaunch AppId=` en `args` | misma fuerza que el entorno |
| Ejecutable Windows (`.exe`) en `cmdline` | nombre del juego |
| Ruta Proton (`compatibilitytools.d`, `proton_*`) | marca `bajo_proton` |
| `steamapps/common/<dir>` | nombre desde la ruta |
| Jerarquía de procesos (`ppid`) | agrupa una sesión por juego |

Ninguna coincidencia aislada basta: hacen falta al menos una evidencia de identidad (AppID) o
una de nombre **y** que el proceso no sea un auxiliar conocido (`wineserver`, `winedevice`,
`steamwebhelper`, `reaper`, `python3`…). Con dos AppID distintos no se elige; queda ambigua.
De la línea de comandos **solo** se extrae el nombre del ejecutable y el AppID: no se guarda
ni se registra el texto completo, y menos sus opciones.

### Verificación en ejecución (no solo tests)

Capturas en `/tmp` no forman parte del repositorio; lo comprobado:

| Escenario | Resultado |
|---|---|
| `comm=GameThread`, exe `wine64-preloader` en `proton-cachyos-slr`, ruta Windows, AppID en entorno | **1 sesión, AppID 1623730**, nombre `Palworld-Win64-Shipping`, `bajo_proton: true` |
| Tres pasadas seguidas | **1 sesión**, sin duplicados |
| Cerrar el proceso | sesión **`terminada`** con hora de cierre, no borrada |
| Contenedor sin visibilidad | `sin_visibilidad`, interfaz «No consta una ejecución observada» |

Interruptor «Observar procesos», medido sobre píxeles:

| Estado | Carril (x) | Círculo (x) | ¿Dentro? |
|---|---|---|---|
| Activado | 26 → 67 | 50 → 63 | sí (4 px de margen) |
| Desactivado | 6 → 47 | 10 → 23 | sí |

Antes el círculo se salía: el carril medía 36 px y el desplazamiento era de 18 px con un
círculo de 16 px. Ahora reutiliza el patrón de `OptimizeTab` (`h-6 w-11`, `translate-x-1` /
`translate-x-6`), sin tocar el resto de la barra lateral.

### Archivos

| Archivo | Cambio |
|---|---|
| `src-tauri/src/evidencia_proceso.rs` | **Nuevo.** Argumentos, rutas (Windows y Linux), auxiliares, agrupación, elección de principal. |
| `src-tauri/src/fixtures_procesos.rs` | **Nuevo.** Procesos reales del usuario y escenarios derivados. |
| `src-tauri/src/proceso_linux.rs` | `escanear()` completo, orden de filtros corregido, `escanear_desde` para pruebas. |
| `src-tauri/src/sessions.rs` | `RasgosDeProceso` ampliado, `EscaneoDeProcesos`, `GrupoDeProcesos`, puerto nuevo. |
| `src-tauri/src/sessions_service.rs` | `paso()` sin depender de la pista, `sesion_de_grupo`, guard de duplicados en memoria, observador inyectable. |
| `src/components/Sidebar.tsx` | Interruptor con la geometría correcta. |
| `src/components/Sidebar.test.tsx` | **Nuevo.** Regresión visual del interruptor. |
| `src/App.tsx` | La configuración vigente se manda al observador al cambiar comando o perfil. |
| `src/hooks/useBenchmarks.ts` | Fija la pista del juego al analizar una grabación. |

### Pruebas

| Grupo | Qué cubre |
|---|---|
| `proceso_linux::pruebas_de_regresion` (10) | Palworld/Proton, nativo, dos juegos a la vez, AppID contradictorio, contenedor, `wineserver` suelto, orden de filtros, que no salga texto personal |
| `sessions_service::aceptacion_53` (11) | Sesión única con AppID, sin duplicados, evidencia de Proton, sesión tras cerrar, asociación por identidad y tiempo, degradación en contenedor, escaneo vacío |
| `sessions_service::tests` (regresión) | Que la interfaz lea el mismo estado que decidió la pasada (`Observando` con AppID), no uno recalculado |
| `diagnostico_procesos::pruebas` (4) | Apagado sin variable, JSON con nulos explícitos, escapado de comillas y saltos, y que no se vuelca el entorno completo |
| `Sidebar.test.tsx` (6) | Geometría del interruptor en ambos estados, aviso con el valor nuevo, estado en palabras |

Totales: **733 Rust** (1 ignorado) + **1016 TS**; `tsc` limpio; `clippy` sin avisos nuevos.

### Lo que estas pruebas **no** demuestran

- Son procesos reales del usuario procesados por código simulado, más un proceso local que
  imita la forma exacta (`comm`, `exe` en `compatibilitytools.d`, ruta Windows, entorno de
  Steam). **No sustituyen** a probar con Steam y un juego real.
- No se ha podido leer `/proc` de un Steam real desde este entorno (Docker no ve el anfitrión
  y no se ha pedido acceso privilegiado).
- No se afirma que ninguna optimización surtiera efecto: ver un proceso de Proton no dice qué
  ajustes estaban activos.

### Prueba pendiente del usuario (CachyOS)

1. Abrir Palworld desde Steam → el pie debe decir **«Juego a la vista»** en ≤5 s.
2. Grabar con MangoHud, cerrar el juego, analizar el CSV → la ficha debe mostrar ejecutable y
   ventana de la sesión, y el AppID debe ser **1623730**.
3. Repetir con **7 Days to Die** (nativo) → sin AppID 0.
4. Cambiar de perfil con el juego corriendo → la prueba debe reflejar la configuración de la
   sesión, no la del momento de analizar.

## Cuarta corrección de 5.3 — el observador se encendía solo y no decía qué juego

Dos fallos reportados en la tercera prueba real, con causa distinta.

### 1. El observador se activaba al arrancar

**Causa**: `arrancar()` ejecutaba `self.estado.activo.store(true, …)` sin condición. No
existía persistencia alguna: `fijar_activo` solo cambiaba un `AtomicBool` en memoria, así que
la preferencia moría al cerrar. En React, además, el interruptor arrancaba en
`useState(observador?.activo ?? true)`: la primera pintura lo enseñaba encendido antes de
que llegara el estado real.

**Corrección**:
- `SesionesData.observador_activo: Option<bool>` con `serde(default)`. `None` (instalación
  nueva) = **desactivado**: ante la duda no se mira `/proc`.
- `fijar_activo` persiste la preferencia en el mismo archivo de sesiones, sin segundo
  almacén, y no reescribe si no cambió.
- `hidratar_preferencia()` lee la preferencia guardada y se llama **antes** de `arrancar()`
  en el setup de `main.rs`. Si no puede leer, queda apagado.
- `arrancar()` ya no fuerza nada: solo crea el hilo.
- En React el valor inicial pasa a `false`. Ninguna ruta de React (navegar, seleccionar
  juego, cargar perfil, analizar benchmark) escribe la preferencia: solo el interruptor, y
  llamando al comando. **Rust es la única fuente de verdad.**

### 2. «Juego a la vista» en vez del nombre

**Causa**: `EstadoDelObservador::Observando` llevaba `appid`, pero `main.rs` lo descartaba
con `..` y el IPC no tenía dónde poner un nombre. `textoEstado()` devolvía etiquetas
genéricas. Además el bucle de `paso()` asignaba `estado` por cada grupo, así que con varios
juegos solo sobrevivía el último.

**Corrección**:
- `Observando { juegos: Vec<JuegoObservado> }`, con `clave`, `appid` y `nombre`.
- `IdentidadDeSesion.nombre_comercial: Option<String>`, resuelto por AppID contra la
  biblioteca de Steam. Es el **único** nombre que la interfaz puede enseñar. Si el AppID no
  consta, queda `None` y se dice «identidad desconocida»; nunca se cae al ejecutable, al
  envoltorio ni al runtime.
- Los juegos se reúnen en `vivos` y el estado se fija **una sola vez** al final de la
  pasada: con dos juegos se dicen los dos.
- `EstadoObservador` (IPC) gana `juegos`. React no deduce nada.
- `textoEstado()` cubre los estados pedidos: «Observador desactivado», «Buscando un juego…»,
  «`NOMBRE` en ejecución», «Varios juegos en ejecución: …», «Juego detectado · Identidad
  desconocida», «Sin visibilidad de procesos».
- Los milisegundos por pasada salen de la barra lateral. Siguen en el IPC como dato técnico.

### Pruebas nuevas

Rust (7): instalación nueva apagada; reinicio desactivado sigue desactivado; reinicio
activado sigue activado; desactivado no sondea ni con un juego delante; el estado lleva el
nombre comercial y no el del ejecutable/envoltorio/runtime; el estado sigue al cambio de
juego; con varios juegos se llevan todos.
TypeScript (10): los seis estados diferenciados, que «desactivado» ≠ «sin visibilidad», que
arranca apagado sin datos del backend, que refleja lo que diga Rust, y que no enseña los ms
de forma permanente.

### Verificación real (contenedor, Xvfb)

1. Instalación sin preferencia → interruptor apagado (`#353D4D`), «Observador desactivado»,
   `/proc` sin tocar: el archivo de diagnóstico no llegó a crearse.
2. Clic real → verde `#38C9A3`, `observador_activo: true` guardado en `sessions.json`.
3. Reinicio → arranca activado y **sí** escanea (22 líneas de diagnóstico).
4. Clic para desactivar, reinicio → arranca **apagado**, cero escaneos en 60 s, diag sin crear.

**Límite, dicho tal cual**: en este contenedor `/proc` casi no muestra procesos de juego, así
que la identificación por nombre se comprobó con las fixtures y las pruebas, no con un juego
real lanzado. Falta la prueba del usuario en CachyOS.

## Tercera corrección de 5.3 — identidad, envoltorios y captura

La prueba real 15 de CachyOS confirmó lo que ya funcionaba («Juego a la vista», grabación
asociada a una sesión, `SteamAppId`/`SteamGameId` a 1623730) y destapó tres fallos más.
Ninguno era de detección: los tres estaban **después** de detectar bien.

### 1. «AppID desconocido» con el AppID guardado al lado

El encabezado de la configuración registrada decía «AppID desconocido» mientras la sesión
asociada tenía el AppID 1623730. Causa, en `benchmarks.rs`:

- `construir_snapshot()` solo miraba el benchmark y el contexto de la interfaz. **Nunca
  consultaba la sesión**, que es donde vive el AppID observado.
- Y había un fallo de **orden**: `benchmark.snapshot = Some(construir_snapshot(...))` se
  ejecutaba **antes** de `atribuir_a_sesion()`. Aunque la función hubiera querido leer la
  sesión, en ese momento todavía no existía.

Corrección: la sesión se asocia **antes** de construir la instantánea, y el AppID se
resuelve en un único sitio (`resolver_appid`) con su procedencia. El orden de mando es
benchmark → sesión (solo si es observación directa) → contexto de la interfaz, y nunca se
pisa el AppID propio de la prueba. La interfaz añade `appid_procedencia` y escribe
«AppID 1623730 (observado en el proceso)» o «(inferido por nombre)». Un AppID sin evidencia
directa sigue siendo `None`: no se convierte una inferencia en certeza.

### 2. «Se vio el juego en ejecución (systemd-inhibit)»

La sesión presentaba como ejecutable del juego al **envoltorio** que lo lanzó. Dos causas
encadenadas:

- `systemd-inhibit` no estaba clasificado como lanzador. Hereda `SteamAppId`/`SteamGameId`
  del árbol de Steam, así que puntuaba como el juego.
- Y lo más grave: `agrupar_por_padre()` agrupaba por **padre inmediato**. En una cadena real
  (`reaper → systemd-inhibit → preloader → juego`) cada eslabón caía en un grupo distinto,
  así que el envoltorio abría **su propia sesión**. De ahí que apareciera como «el juego».

Corrección: se agrupa por la **raíz de la cadena de lanzamiento** (`raiz_conocida`), de modo
que juego y envoltorios quedan juntos, y se añade `es_lanzador()` (`systemd-inhibit`,
`gamemoderun`, `mangohud`, `env`…) que resta peso al elegir el proceso representativo. La
evidencia del envoltorio **no se borra**: sigue contando para el AppID y para Proton. No hay
ninguna excepción para Palworld.

Efecto lateral detectado por la prueba: con la agrupación antigua, un lanzamiento envuelto
producía **dos** grupos (el wrapper y el juego). Ahora produce uno.

### 3. «Configuración capturada después de terminar la grabación»

Era correcto por diseño mientras no existiera configuración observada, pero se seguía
afirmando aunque la sesión hubiera guardado `configuracion_inicial` **durante** la
observación, antes de arrancar el juego. `posterior_a_la_grabacion` estaba fijado a `true`.

Corrección: se decide comparando la fecha de esa foto con el instante de captura. Si la foto
es anterior, la configuración **no** es posterior a la grabación. Sin sesión se mantiene
`true`, que es lo honesto: no hay nada que afirmar.

### Verificación

- Rust: **726 pruebas** (antes 719); `clippy --bins` sin avisos nuevos (queda el conocido
  `save_benchmark` 9/7, idéntico en `HEAD`).
- TypeScript: **1006 pruebas** (antes 1003), `tsc` limpio.
- Pruebas de regresión nuevas, todas nacidas de un fallo real: la cabecera no pierde el
  AppID observado, un AppID inferido no se presenta como observado, la sesión no pisa el
  AppID de la prueba, la foto del arranque no es «posterior», y el envoltorio no se presenta
  como el juego.
- Prueba real en el contenedor con el árbol de Steam reproducido (`systemd-inhibit` padre,
  preloader de Proton hijo, `SteamAppId=1623730`): **un solo grupo**, representativo el
  proceso del juego, AppID 1623730, y la barra lateral diciendo «Juego a la vista».

### Límites, dichos tal cual

No se puede reproducir Steam de verdad dentro del contenedor: `systemd-inhibit` no arranca
sin dbus, y no hay `/proc` de un juego real. Lo verificado aquí es la **forma** del árbol de
procesos, no la ejecución de Palworld. La comprobación definitiva sigue siendo la del equipo
del usuario.

## Segunda corrección de 5.3 — el estado no llegaba a la pantalla

La segunda prueba real en CachyOS volvió a fallar **con la detección ya corregida**: la barra
lateral seguía diciendo «Buscando un juego · 2,3 ms por pasada» con Palworld abierto.

### Cómo se localizó (medición, no suposición)

En lugar de volver a leer el código a ojo se añadió un **modo de diagnóstico** opcional y se
ejecutó contra un proceso con la misma forma real que el del usuario (`comm=Palworld-Win64`,
ruta Windows en `cmdline`, `SteamAppId=1623730` en el entorno, mismo UID que la aplicación).
El propio proyecto traía dos binarios de compilación desfasados, así que antes se comprobó que
el binario en ejecución contenía el código nuevo.

El diagnóstico mostró la contradicción en una línea:

```
grupo pid=45865 -> sesion CREADA appid=Some(1623730) nombre=Some("Palworld-Win64-Shipping")
```

El backend **sí** identificaba el juego y **sí** creaba la sesión. La pantalla, no obstante,
decía «Buscando».

### Causa raíz

`estado_legible()` —lo único que la interfaz consulta— **nunca podía devolver `Observando`**:

```rust
// antes
pub fn estado_legible(&self) -> EstadoDelObservador {
    if !self.esta_activo() { return EstadoDelObservador::Apagado; }
    if hay_error { return EstadoDelObservador::SinVisibilidad; }
    EstadoDelObservador::Buscando        // ← siempre
}
```

`paso()` calculaba el estado correcto, lo usaba para decidir la espera y **lo tiraba**. No
había ningún sitio donde se guardara, así que la consulta de la interfaz no tenía de dónde
leerlo más que deduciéndolo, y deducía «buscando». Se creaba la sesión, se guardaba con AppID
`1623730`, y el pie de la barra lateral seguía buscando para siempre.

Es un fallo de **una sola fuente de verdad**, el mismo tipo que el usuario proscribió del
resto del proyecto: el estado se calculaba en un sitio y se recalculaba en otro.

### Corrección

- `EstadoCompartido` gana `ultimo_estado: Mutex<Option<EstadoDelObservador>>`.
- `paso()` **publica** lo que ha decidido, y `estado_legible()` lo lee. Una sola fuente.
- Se publica también desde el bucle, para que un fallo de E/S no deje el estado congelado.

### Prueba de regresión

`el_estado_legible_refleja_lo_que_decidio_la_ultima_pasada` fija la propiedad que faltaba:
tras una pasada con el escenario de Palworld, la lectura de la interfaz tiene que decir
`Observando { appid: Some(1623730) }`. **Esa prueba falla con el código anterior**, que es lo
que le da valor.

La suite no lo detectó antes porque todas las pruebas miraban lo que devolvía `paso()`, no lo
que leía la interfaz: se probaba la mitad del camino.

### Verificación en ejecución

| Paso | Resultado |
|---|---|
| Backend: sesión creada | `ses_2221039`, `evidencia_proceso: observada`, `appid: 1623730` |
| Backend: estado publicado | diagnóstico `paso-ya-observando`, clave `45865@…` |
| Interfaz (medido por OCR sobre píxeles) | **«Juego a la vista · 2,3 ms por pasada»** |

## Modo de diagnóstico del observador

Existe porque una función que falló dos veces en un equipo real no se arregla adivinando.

### Activación

Solo con una variable de entorno; sin ella **no hace nada** (ni una lectura extra):

```bash
THORIAPP_DIAGNOSTICO_PROCESOS=/ruta/diagnostico.jsonl thoriapp
```

Escribe **una línea JSON por pasada**, añadiendo al final. Apagado por defecto, sin ruido en
la interfaz y sin cambios permanentes.

### Qué revela

| Campo | Qué responde |
|---|---|
| `procesos_vistos` | cuántas entradas de `/proc` se miraron |
| `inaccesibles` | cuántas no se pudieron leer (permisos, otro usuario) |
| `sin_visibilidad` | si no se puede ver nada (contenedor) |
| `candidatos[]` | `pid`, `comm`, ejecutable, ruta del juego, `SteamAppId`, `SteamGameId`, `AppId=` de argumentos, `bajo_proton` |
| `descartes[]` | **el motivo exacto** de cada descarte |
| `sesiones[]` | si cada grupo produjo sesión creada o rechazada, y por qué |
| `nota` | en qué rama se tomó la decisión (`escaneo`, `paso`, `paso-sin-grupos`, `paso-ya-observando`) |

Un descarte con `TENÍA EVIDENCIA pero el filtro lo descartó` señala directamente el fallo que
importa: un proceso con AppID o con `.exe` de juego que aun así no abre sesión.

### Qué **no** registra

- Nunca el entorno completo: solo las cuatro variables de la lista blanca de producción.
- Nunca la línea de comandos completa: solo el nombre del ejecutable y el `AppId` numérico.
- No pide root, no relaja ningún filtro, no tiene casos particulares para ningún juego.

### Uso para la próxima prueba real

```bash
THORIAPP_DIAGNOSTICO_PROCESOS=$HOME/thoriapp-diagnostico.jsonl thoriapp
# abrir el juego, esperar unos segundos, cerrar la app
```

Con el fichero resultante se sabe, sin adivinar, si el problema está en lo que `/proc` ofrece,
en el filtro, en la identificación o en lo que se publica hacia la interfaz.

## Auditoría de 5.3 — hallazgos verificados

Sondeos hechos en este equipo; lo que sigue es evidencia, no suposición.

### Capacidades actuales

- **Thoriapp no lanza juegos y no observa procesos.** No hay ningún `Command::new` que
  ejecute Steam ni el juego, y en todo el código no se lee `/proc/<pid>` de terceros
  (solo `/proc/self` y `/proc/cpuinfo|meminfo`). No existe concepto de «juego abierto».
- La captura es **tardía por diseño**: `captureSetup()` se llama al detectar el
  `_summary.csv` (`useLogWatcher.ts`), que MangoHud escribe al **cerrar** la grabación.
- El AppID se resuelve por nombre contra `scan_library()` con exigencia de prefijo y
  único candidato (`steam_launch::appid_por_nombre`).

### Bug encontrado: 7 Days to Die no resuelve su AppID

`norm_nombre` no quita espacios ni guiones bajos, así que:

```text
appid_por_nombre("7DaysToDie")         -> None      (biblioteca tiene "7 Days to Die")
appid_por_nombre("7 Days to Die")      -> Some(251570)
```

Reproducido con la biblioteca real instalada. Es un fallo de identidad, no de permisos.
Queda a la espera de aprobación arreglarlo (ver decisión 3 de la auditoría).

### Qué se puede observar de verdad (verificado aquí)

| Fuente | Resultado real |
|---|---|
| `/proc/<pid>/comm`, `exe`, `cmdline`, `stat` | Legibles si es **el mismo usuario** |
| `/proc/<pid>/environ` | Legible mismo usuario; **contiene credenciales** (se vio `NVIDIA_API_KEY`) |
| `/proc/<pid>/environ` de otro usuario / PID 1 | `Permission denied` |
| Herencia de `SteamAppId`/`SteamGameId` a procesos hijos | Confirmado que sí |
| `PPID` para reconstruir el árbol | Disponible en `/proc/<pid>/stat` campo 4 |
| `starttime` (campo 22) y `btime` de `/proc/stat` | Permiten datar el arranque del proceso |
| Steam corriendo en el **host** desde Docker | **Invisible**: solo 7 PIDs visibles, namespace propio |
| Carpeta del juego instalado | Existe pero **vacía** (sin ejecutable) en este entorno |
| `StateFlags` del `appmanifest` | `4` = instalado; no indica «en ejecución» |
| Coste de una pasada de escaneo de `/proc` | ~6 ms con 7 procesos (escala lineal) |

### Implicaciones

- En Docker, cualquier detección de proceso de Steam dará «no hay nada», y eso **no**
  significa que el usuario no esté jugando. Debe tratarse como *sin evidencia*, nunca
  como *no verificado*.
- Leer `environ` exige una **lista blanca** explícita; nunca volcarlo.

### Pendiente para fase 6 (anotado, no implementado)

- Formulario de benchmarks demasiado alto; «Analizar y guardar» poco accesible.
- Aprovechar mejor el espacio horizontal.
- RAM con precisión razonable (31,3 GiB en vez de 31.258648 GiB).
- Revisar etiqueta «Planificador de CPU»: `performance` es el **gobernador de frecuencia**.
- Pendiente de comprobación visual por el usuario: resumen original de MangoHud en la UI.
- **Falsa diferencia de hardware por nombres distintos de la misma GPU**: la misma tarjeta
  puede aparecer con dos cadenas (`AMD Radeon RX 6600` / `RADV NAVI23`) y leerse como un
  cambio de equipo entre pruebas. Normalizar sin inventar modelo.
- **Exceso de decimales** en RAM y métricas (`31.258648` en vez de `31,3`). Es el mismo
  punto de la fase 6, pero afecta también a las comparaciones, no solo al dibujo.
- **Uso incorrecto de «frames»** para muestras periódicas: MangoHud muestrea a intervalos, y
  una muestra no es un fotograma. Corregir el término en la interfaz y en las etiquetas.
- **Diagnóstico de shaders demasiado categórico**: se afirma la causa sin evidencia
  suficiente; debe presentarse como hipótesis cuando no la haya.
- **Ruido visual y contraste insuficiente**, ya anotado para la fase 6.

## Quinta corrección de 5.3 — el envoltorio seguía ganando y la configuración no era la del arranque

Viene de una validación real con **Deskrawl** (AppID 4623570, Prueba 2). Cuatro puntos, todos
defectos de verdad. Lo confirmado antes (AppID con procedencia, asociación benchmark-sesión,
detección de Proton, funcionar con un juego distinto de Palworld) sigue bien.

### 1. El proceso representativo era `srt-bwrap`

`elegir_principal` repartía puntos con números sueltos y listas de nombres (`LANZADORES`,
`AUXILIARES`). Cada envoltorio nuevo exigía una excepción: `systemd-inhibit` ayer,
`srt-bwrap` hoy. No escala, y el usuario lo dijo con razón: no quiere una lista interminable.

Se sustituye por **papeles** deducidos del árbol (`PapelEnElLanzamiento`): `EjecutableDelJuego`,
`Envoltorio`, `Ancla`, `Auxiliar`. Solo un `EjecutableDelJuego` puede representar al grupo; si
no hay ninguno, se declara **identidad desconocida**.

Tres cosas que costaron encontrar y quedan documentadas para no repetirlas:

- Un envoltorio **también** menciona el `.exe` (lo pasa como argumento), así que mirar la línea
  de comandos lo ascendía a juego. Manda la estructura: si tiene hijos que ejecutan el juego,
  él no lo ejecuta.
- `UnityCrashHandler64.exe` vive **dentro** de `steamapps/common/Deskrawl`. La carpeta no
  basta: se exige que el nombre del `.exe` coincida con la carpeta (con sufijos de plataforma
  tipo `-Win64-Shipping`, no una lista de juegos).
- Los tests de fixture usaban una extracción **propia** de la evidencia, más laxa que la de
  producción. Por eso daban por bueno un comportamiento que en el equipo real elegía mal. Ahora
  comparten la misma regla.

`LANZADORES` y `es_lanzador` se eliminan: ya no hay lista de envoltorios.

### 2. Evidencias repetidas

La evidencia se acumulaba por miembro: «corre bajo Proton» salía cuatro veces y «ejecuta
Deskrawl» dos. `nombres.dedup()` solo quitaba duplicados **adyacentes**. Ahora se deduplican
`evidencia` y `nombres` conservando el orden, sin tocar `miembros` (los datos originales).

### 3. La configuración no era la del arranque

La sesión empezó a las 14:16:01 y su configuración figuraba capturada a las 14:20:30. La marca
`posterior_al_arranque` **ya venía bien del backend, pero ningún componente la mostraba**. Se
arregla en la representación: `InstantaneaCard` avisa de que se capturó después del arranque y
dice cuándo empezó la sesión. No se crea un segundo estado.

### 4. Historial y comparaciones podían mezclar juegos

`compare_benchmarks` calculaba todo y añadía un aviso diciendo que la comparación «no es
válida», pero **devolvía el resultado igual**. Y `juego_coincide` caía al nombre cuando faltaba
un AppID.

Ahora hay una sola regla con dos usos explícitos: el **historial** admite el nombre cuando la
grabación antigua no tiene AppID (descartarla escondería pruebas reales); la **comparación** es
estricta y devuelve error si un lado tiene AppID y el otro no.

### Pruebas

- **Rust**: 743 pasan, 1 ignorada. Nuevas: papel del envoltorio, identidad desconocida sin
  ejecutable, evidencia sin repetir, nombres sin repetir, wrapper no representativo, y 6 sobre
  identidad de juego y rechazo de comparaciones entre juegos distintos.
- **TS**: 1018 pasan (75 ficheros). Nuevas: aviso de configuración posterior al arranque y
  ausencia del aviso cuando la marca es falsa.
- `tsc --noEmit` limpio. `clippy --bins` solo el aviso preexistente de `save_benchmark`
  (9/7 argumentos, idéntico a `HEAD`).

### Verificación real (Xvfb, clics con `xdotool`)

1. Sin preferencia guardada → arranca **apagado** (`#353D4D`, «Observador desactivado»),
   **0 líneas** de diagnóstico: cero lecturas de `/proc`.
2. Clic real → activo (`#38C9A3`), persiste `observador_activo: true`.
3. Juego simulado en `steamapps/common/Deskrawl` → sesión con AppID 4623570
   (`entorno_del_proceso`), proceso representativo `Deskrawl` y no un envoltorio, evidencia
   **sin repetir**, `configuracion_inicial.posterior_al_arranque = true`.
4. Clic para apagar → `observador_activo: false`; reinicio → arranca apagado, **0 líneas** de
   diagnóstico en 50 s.

**Límite dicho tal cual**: aquí no hay biblioteca de Steam, así que el nombre comercial sale
`null` y la interfaz muestra «Juego detectado · Identidad desconocida». Es el comportamiento
correcto (no se inventa el nombre), pero el nombre real solo puede comprobarlo el usuario en su
equipo.

## Complemento final de 5.3 — aislamiento de benchmarks y seguridad del borrado

Requisito aportado después, con capturas reales: «Historial de este juego» contenía **siete
pruebas, dos de Deskrawl y cinco de Palworld**. Se auditó primero lo existente y solo se
corrigió lo que seguía mal.

### Lo que ya estaba bien (sin tocar)

- **Lectura filtrada en el backend**: `load_benchmarks_for_game` filtra por `juego_coincide` y
  la vista pide esa ruta cuando hay juego abierto (`useBenchmarks`).
- **Una sola regla de identidad** en todo el módulo (`coinciden_por_identidad`), compartida por
  historial y comparación. Viene de la quinta corrección.
- **Comparaciones directas** de juegos distintos: ya se rechazaban con error.
- **La comparación automática** toma las dos últimas **del juego filtrado**, no del archivo.
- **Texto de confirmación** del borrado: ya decía «de este juego. Los demás juegos no se
  tocan», y su `shown.length` es exactamente lo que la vista enseña.
- **Las pruebas antiguas sin AppID** se conservan; no se les asigna identidad deducida.

### Lo corregido

1. **Las comparaciones guardadas sobrevivían a sus pruebas.** `delete_benchmarks_for_game` y
   `delete_all_benchmarks` borraban pruebas y dejaban en disco informes que citan datos que ya
   no existen. Ahora se purgan por `id` de `baseline`/`test` (`purgar_comparaciones_por_benchmarks`).
   El alcance se calcula **una vez** (`ids_a_borrar_del_juego`) y esa misma lista se usa para
   borrar y para purgar, de forma que no pueden discrepar.

2. **Riesgo de interbloqueo** introducido al purgar: se tomaban `CERROJO_BENCHMARKS` y
   `CERROJO_COMPARACIONES` a la vez. Ahora el primero se suelta al salir de su bloque.

3. **La interfaz no se enteraba de la purga.** Aunque el backend borrara las comparaciones,
   «Mis pruebas» seguía enseñándolas en memoria. Ahora se emite `COMPARISONS_CHANGED_EVENT`
   tras borrar; solo si de verdad se borró algo (avisar cuando no cambió nada es desinformar).

### Limitación detectada y NO corregida (fuera de alcance)

Las pruebas sin AppID forman una clave de recorte (`name:…`) **distinta** de la de su juego
con AppID. Consecuencia: el límite de 10 pruebas se aplica por separado, y una prueba antigua
solo se encuentra por nombre si el nombre guardado coincide con el comercial. El import ya
quita sufijos tipo `-Win64-Shipping`, así que en la práctica coincide, pero **no se unifican**
las dos claves. Unificarlas cambia el recorte del histórico y eso exige decisión explícita.

### Pruebas

- **Rust**: 752 pasan, 1 ignorada. Nuevo módulo `pruebas_aislamiento` (9 tests) con el conjunto
  mixto pedido: 2 de Deskrawl, 5 de Palworld, una antigua sin AppID y una con identidad
  contradictoria (nombre «Palworld», AppID de Deskrawl), más una comparación guardada de cada
  juego. Cubre historial, contadores, comparación directa, automática, por juego, alcance de
  borrado, identidad contradictoria y purga de comparaciones huérfanas.
- **TS**: 1020 pasan. Nuevos: aviso a comparaciones guardadas al borrar, y que **no** se avisa
  cuando no había nada que borrar.
- `tsc --noEmit` limpio; `clippy --bins` solo el aviso preexistente de `save_benchmark`.

### Documentación para la fase 6

Creado `docs/planificacion-fases.md` con la frontera 5.4 / fase 6, los requisitos de fase 6
(historial, navegación, asistente, simplificación de comparaciones, principios de UX) y las
reglas permanentes que un rediseño no puede romper. **Nada de ello está implementado.**

### Límites de validación

- Sin ficheros reales del usuario: el conjunto mixto es de fixture, no su `benchmarks.json`.
- El borrado no se ha probado contra su historial real de 7 pruebas; sí contra el conjunto
  mixto equivalente en Rust y con los 29 tests de `useBenchmarks` en TS.
- La purga de comparaciones no se ha podido verificar en la app real (no hay comparaciones
  guardadas en este entorno), solo por unidad.

## Cierre de 5.3 — alcance del historial de benchmarks

**Aclaración de alcance (del usuario).** La pestaña general de Benchmarks no está limitada a
un juego: enseña el historial de **todos** los juegos. El historial de un juego concreto vive
en la ficha de ese juego. Ambas vistas comparten almacenamiento, lógica de dominio y
servicios. Por tanto, ver tres pruebas de Deskrawl y cinco de Palworld juntas **no es un
defecto**.

### Auditoría: qué hacía ya cada vista

| Vista | Ruta de datos | Alcance real |
| --- | --- | --- |
| Pestaña general, sin juego abierto | `api.loadBenchmarks()` | **Todos** los juegos |
| Pestaña general, con juego abierto | `api.loadBenchmarksForGame()` | **Solo** ese juego |

`useBenchmarks` decide por `currentAppid`; el filtrado es del backend, no de React.

### Defectos corregidos

1. **Encabezado mentiroso.** Con 8 grabaciones de 2 juegos decía «Historial de este juego» y
   ofrecía «Borrar historial» como si fuera de uno. Ahora el alcance se deriva **una sola vez**
   (`alcance`) y lo consumen el título, el `title`, el texto del botón y la confirmación.
   - General: «Historial de todos los juegos» / «Borrar todo el historial».
   - Juego: «Historial de este juego» / «Borrar historial».
2. **Confirmación con alcance falso.** Decía siempre «Se eliminarán las N grabaciones de este
   juego. Los demás juegos no se tocan», también en la vista general. Ahora la general dice
   «…de los N juegos del historial. Se borrarán todas las pruebas guardadas».
3. **Borrado más amplio que lo prometido (el fallo serio).** El botón llamaba **siempre** a
   `deleteAllBenchmarks`, que vacía el archivo entero. Desde la ficha de un juego eso se
   llevaba por delante las pruebas de **todos los demás**, justo lo contrario de lo que la
   confirmación prometía. Ahora cada alcance usa su ruta: `deleteAllForGame` en la ficha,
   `deleteAllBenchmarks` en la general.

### `pv-adverb` como ejecutable representativo

No hizo falta tocarlo. La clasificación por `PapelEnElLanzamiento` es **estructural**
—«si tiene hijos que ejecutan el juego, él no lo ejecuta»— y no depende de ninguna lista de
nombres, así que `pv-adverb` cae por la misma regla que `srt-bwrap`. Se añadió
`otro_envoltorio_del_runtime_tampoco_es_el_juego`, que lo demuestra con una fixture donde no
existe ningún nombre conocido del runtime.

### Pruebas

- Rust: **753** pasan (1 ignorada). Nueva: `otro_envoltorio_del_runtime_tampoco_es_el_juego`.
- TS: **1024** pasan. Nuevas en `BenchmarksView.test.tsx` (alcance del historial): encabezado
  general, encabezado de juego, texto de confirmación y —la importante— que desde un juego se
  llama a `deleteBenchmarksForGame` y **no** a `deleteAllBenchmarks`.
- Verificado en la app real (Xvfb): con 8 grabaciones de 2 juegos, el encabezado dice
  «Historial de todos los juegos · 8», el botón «Borrar todo el historial» y la confirmación
  «Se eliminarán las 8 grabaciones de los 2 juegos». Se cerró sin borrar (8 intactas).

### Registrado para fase 6 (NO implementado)

En `docs/planificacion-fases.md`: centro general de benchmarks, historial en la ficha de cada
juego, selector de juegos, filtros/búsqueda/ordenación/checkboxes, componentes reutilizables
y acceso a comparaciones e informes desde ambas vistas sin duplicar datos.

### Límite de la validación

Los datos usados en la verificación real fueron **sembrados**, no las grabaciones auténticas
del usuario. La prueba sigue pendiente en CachyOS con su historial real.

## Ampliación del diagnóstico de 5.3 — topología real antes de corregir

Esta intervención **no corrige nada funcional**: amplía el diagnóstico opt-in, añade sus
pruebas y documenta lo auditado. Fase 5.3 sigue ABIERTA.

### Por qué

Las pruebas reales en CachyOS dieron duplicaciones (Eslabong ×3, Tattoo Tycoon ×3, Deskrawl
×2, Palworld ×2) con **854 pruebas en verde**. Al ejecutar el motor real contra los fixtures
existentes salían **un solo grupo** por juego (o ninguno): los fixtures **no reproducen** el
árbol real. Eso descarta que el problema sea falta de pruebas y obliga a mirar el equipo real.

### Defectos demostrados por los datos reales

| # | Defecto | Dónde |
|---|---|---|
| A | Un envoltorio (`pv-adverb`, `systemd-inhibit`) abre sesión | solo es representante quien tiene papel `EjecutableDelJuego`, y ese papel se calcula mirando **solo los procesos de su propio grupo** |
| B | Varios grupos para una misma ejecución | `agrupar_por_padre` sube por padres que estén en `validos`; `parece_juego` descarta `python3`, `wineserver`, `winedevice`… y, al faltar el eslabón, el hijo se vuelve raíz |
| C | Cambio de representante → sesión nueva | la sesión se ancla a `pid@starttime` del representante; si el representante cambia, la clave cambia |
| D | IDs de sesión repetidos | `format!("ses_{}", ahora...)` solo lleva el timestamp al segundo: dos sesiones en el mismo segundo colisionan |
| E | `bajo_proton` miente | `MARCAS_DE_PROTON` incluye `wine`, así que Wine nativo cuenta como Proton (Star Valor) |
| F | Heroic llega al observador con `exec: null` | el adaptador y la conversión al catálogo sí propagan `exec`; el punto de pérdida **no está demostrado** |
| G | `ejecuta_de_verdad=true` no implica el videojuego | el `cwd` de Tattoo Tycoon es `…/pfx/drive_c/windows` |

**A, B, C y D están demostrados en código y en los datos del usuario.** F no: por eso el
diagnóstico ahora lo rastrea en lugar de suponerlo.

### Contrato de identidad (auditado, sin cambiar todavía)

Se separan cinco cosas que hasta ahora se mezclaban:

- **Instancia de proceso**: `pid@starttime`. Distingue dos usos del mismo PID.
- **Instancia de ejecución**: la cadena de procesos de un lanzamiento. **No** es identidad
  inmutable de sesión: un cambio de PPID, un envoltorio que desaparece o un huérfano pueden
  partirla o unirla.
- **Identidad del juego**: título e id de plataforma del catálogo.
- **Representante**: el proceso que se elige para contar la sesión. Puede evolucionar.
- **Sesión**: lo que se persiste y se asocia a grabaciones y benchmarks.

Ninguna raíz distinta implica por sí sola ejecución independiente, ni compartir raíz implica
compartir ejecución. `bajo_proton` tampoco se limita a quitar `wine` de la lista: tiene dos
consumidores con responsabilidades distintas (`podria_ejecutar_un_juego` lo usa como «es un
`.exe` bajo Wine/Proton»; `elegir_principal` como desempate), y hay que separarlas sin romper
Star Valor ni Starsector.

### Qué se añadió al diagnóstico

- **Una línea JSON por registro.** Había un salto de línea literal dentro del objeto del
  catálogo, así que cada pasada escribía JSON partido y los lectores línea a línea perdían
  los registros `paso` (101 en la captura del usuario).
- **`candidatos`**: `ppid`, `inicio`, `identidad_de_instancia`, `ancestros` (cadena
  reconstruida con lo observado, cortada donde falta el eslabón).
- **`descartes`**: `ppid` y motivo, para ver si un eslabón intermedio se perdió en el filtro.
- **`clasificacion`**: papel resultante, elegibilidad, grupo, representante e identidad
  correlacionada. Es la parte **inferida**, marcada como tal.
- **`ciclo_de_sesiones`** y **`recorrido_del_ejecutable`**: recorrido del ejecutable desde el
  adaptador hasta el catálogo efectivo, para localizar dónde se pierde `exec`.

Se mantiene **opt-in** (`THORIAPP_DIAGNOSTICO_PROCESOS`): sin la variable no se anota nada y
la traza del escaneo ni se construye. No se registran líneas de comandos completas, entorno,
credenciales ni configuraciones sensibles.

### Pruebas

Rust **854** pasan (1 ignorada). Nuevas: `cada_registro_es_una_sola_linea_json`,
`la_identidad_de_instancia_usa_pid_y_arranque`, `la_cadena_de_ancestros_no_inventa_eslabones`
y `la_cadena_de_ancestros_corta_ciclos`. TS sin cambios (**1027**). Clippy: solo el aviso
preexistente de `benchmarks.rs`.

### Captura pendiente en CachyOS

```bash
rm -f /tmp/diag_topologia.json
cd /workspace/tauri-app
THORIAPP_DIAGNOSTICO_PROCESOS=/tmp/diag_topologia.json npx tauri dev
# lanzar Eslabong, Palworld y Tattoo Tycoon (uno a uno y luego juntos)
```

Con eso se puede comprobar por fin si los grupos separados comparten ancestro —la
fragmentación del árbol— y en qué paso se pierde el `exec` de Heroic. **No se corrige A–G
hasta tener esa captura.**
