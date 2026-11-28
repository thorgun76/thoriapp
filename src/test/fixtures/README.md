# Fixtures de pruebas

## `comparaciones_transporte.json`

### Qué es

La respuesta de **transporte** de `load_comparisons`: lo que el frontend recibe de Rust por
IPC. Cada elemento es un `ComparacionLeida`, la comparación guardada **con
`estado_de_la_evaluacion` ya calculado por el dominio**.

### Procedencia: datos sintéticos

Las siete entradas son **sintéticas**. Nombres `SINTETICO …`, fechas de 2026, comandos de
Steam genéricos y rutas `/sintetico/SINT-*.csv`. **No contienen** grabaciones reales, rutas
del equipo, nombres de juegos del usuario ni ningún dato personal. Se pueden versionar.

### Método de generación

**No está escrita a mano y la prueba no clasifica nada.** Se obtuvo con el lector y el
serializador **reales** de Rust:

1. Test temporal en `src-tauri/src/benchmarks.rs` que llamaba a `try_load_saved_data_de`
   sobre la fixture de disco, aplicaba la misma clasificación que `load_comparisons`
   (`ComparacionLeida`, `serde(flatten)` + `estado_de_la_evaluacion`) y serializaba
   `Vec<ComparacionLeida>` a JSON.
2. Ejecución con `cargo test --locked --bins`.
3. **Test retirado** después. No se modificó código de producción.

La primera versión de la suite replicaba la clasificación en TypeScript y **lo hacía mal**:
permitía que un registro con `evaluacion` presente fuese tratado como ilegible. Usar la
salida real elimina esa posibilidad: si Rust cambia la clasificación, la fixture se regenera
y las pruebas cambian con él.

### Las siete entradas y sus dos diferencias respecto al disco

La fixture de disco de origen tiene **6 comparaciones**; aquí hay **7**. Comparación campo a
campo, ignorando `estado_de_la_evaluacion` (que solo existe en el transporte):

| Entrada | Respecto al disco | `estado_de_la_evaluacion` |
|---|---|---|
| `SINT-GUARDADA-0` (COMPLETA_MEJORA) | idéntica | `soportada` |
| `SINT-GUARDADA-1` (COMPLETA_EMPEORA) | idéntica | `soportada` |
| `SINT-GUARDADA-2` (PARCIAL_PERDIDA) | idéntica | `soportada` |
| `SINT-HISTORICA` | idéntica | `no_registrada` |
| `SINT-MALFORMADA` | **difiere en `evaluacion`** | `malformada` |
| `SINT-NO-SOPORTADA` | idéntica | `no_soportada` |
| `SINT-GUARDADA-PN` (PARCIAL_CON_NARRATIVA) | **derivada, no existe en disco** | `soportada` |

**Diferencia 1 — `SINT-MALFORMADA`.** En disco `evaluacion` es un objeto con `condiciones`
corrupto (un string donde debía ir un objeto). En el transporte llega **`evaluacion: null`**,
porque Rust deserializa como `EvaluacionPersistida`, falla, aparta el registro en
`evaluacion_ilegible` y deja `evaluacion` en `None`. Es la diferencia que **solo el lector
real produce** y la que una clasificación manual podía perder.

**Diferencia 2 — `SINT-GUARDADA-PN` (derivada).** Copia de `SINT-GUARDADA-2` con `id`,
`game_name` y `comparison.conclusion` cambiados. La evaluación estructurada queda **idéntica**,
así que el escenario sigue siendo el parcial real. Hace falta porque ninguna entrada de origen
tiene a la vez **narrativa persistida y evaluación soportada**, y la regresión J exige ese caso.

### Cómo regenerarla si cambia el contrato IPC

1. Añadir a `src-tauri/src/benchmarks.rs` un test que lea la fixture de disco con
   `try_load_saved_data_de`, aplique la clasificación de `load_comparisons` y serialice
   `Vec<ComparacionLeida>`; escribir a la ruta indicada por una variable de entorno.
2. `cargo test --locked --bins <nombre_del_test>`.
3. Si se regenera la fixture de disco, repetir la comparación campo a campo de la tabla de
   arriba y actualizar este documento.
4. **Retirar el test temporal** y comprobar con `git diff` que no queda rastro.

### Estado en el repositorio

**Este archivo no está rastreado todavía** (`git status` → `?? src/test/`). No forma parte
del árbol versionado, así que **no está disponible en un clon limpio ni en CI hasta que se
añada al staging**. La comprobación de que la suite pasa sin el paquete externo
`validacion-c3` se hizo en este contenedor, con el archivo presente en el directorio de
trabajo; eso describe el comportamiento del checkout actual, **no** el de un clon limpio
antes de versionarlo.

Se incluirá en el futuro staging autorizado junto con el resto de la ronda.

## Nota sobre portabilidad

La prueba la importa con ruta relativa (`../test/fixtures/…`), dentro de `src/`. Ninguna
prueba depende de `validacion-c3` ni de rutas absolutas de este contenedor.
