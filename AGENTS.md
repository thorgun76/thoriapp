# Thoriapp

Optimizador de juegos para Linux: Rust + React + TypeScript + Tauri 2.

Aquí se fija **cómo se trabaja**; qué hace cada módulo está en la documentación y el código.

## Arquitectura

- **Rust es la fuente de verdad** de la lógica de negocio: comandos, perfiles, integridad,
  benchmarks, identidad de ejecuciones. React muestra y pide; no decide.
- No dupliques lógica entre frontend y backend: si la regla existe en Rust, el frontend la
  consume.
- Separa dominio, infraestructura e interfaz: ficheros, `/proc`, Steam y lanzadores no se
  filtran al dominio.
- Respeta los contratos existentes: `Procedencia`, `OrigenAppId`, firmas de comandos Tauri y
  formatos en disco.
- Preserva el comportamiento actual. Cambiarlo es una decisión de producto: se autoriza.
- No introduzcas abstracciones que no resuelvan un problema presente.

## Reglas funcionales

Decisiones aprobadas; no se relajan sin autorización.

- **Comandos**: importación y restauración no destructivas; se preservan los argumentos
  desconocidos, su orden, las comillas y los wrappers.
- **Steam**: no se modifica automáticamente. `localconfig.vdf` se lee, nunca se escribe.
- **Perfiles y benchmarks**: son independientes. Los snapshots históricos son **inmutables**:
  no se reconstruyen con el estado actual.
- **Datos**: distingue **declarado**, **inferido** y **observado**; una inferencia no es un
  hecho verificado. No inventes métricas ni conviertas un dato ausente en cero. Más FPS no es
  siempre mejor.
- **Observador de sesiones**: experimental, opcional y **apagado por defecto**; apagado, no
  lee `/proc`. Los benchmarks no dependen de él. Se preservan las correcciones de topología,
  identidad y continuidad: se agrupa por instancia, no por nombre, y la pista del frontend no
  es evidencia de identidad.

## Documentación

Consulta por secciones. **No leas `STATUS.md` completo por rutina**: localiza la parte que
necesites.

| Fichero | Contenido |
|---|---|
| `STATUS.md` | Estado, correcciones con su evidencia y deuda técnica. |
| `docs/ARCHITECTURE.md` | Capas, flujos, fuente de verdad por dominio, contratos persistidos. |
| `docs/CODE_MAP.md` | Dónde está cada funcionalidad: símbolos, IPC y tests. |
| `docs/IPC_MAP.md` | Índice completo de comandos IPC → método de `api.ts`. |
| `docs/planificacion-fases.md` | Roadmap y frontera entre fases. |
| `docs/sesiones-de-ejecucion.md` | Arquitectura del observador y garantías. |
| `docs/validacion-real.md` | Protocolo con juegos reales en el equipo. |
| `docs/mangohud-summary.md` | Interpretación de métricas. |
| `README.md` / `CHANGELOG.md` | Uso e historial de versiones. |

Este fichero no repite el roadmap ni el historial.

## Navegación

Antes de un `rg` amplio o de leer un módulo entero, `docs/CODE_MAP.md` dice qué fichero y qué
símbolo. Si el mapa y el código discrepan, manda el código: corrige el mapa en el mismo cambio
en que renombres, muevas o añadas rutas, símbolos, comandos IPC o tests.

Eficiencia (sin que cueste corrección, seguridad ni fiabilidad):

- Ajusta la profundidad del análisis al alcance y al riesgo real de la tarea.
- Si conoces el símbolo, ve a él directamente; no busques lo que ya sabes dónde está.
- Consulta solo las secciones documentales que necesites.
- Evita exploraciones amplias y leer módulos grandes de principio a fin.
- No repitas búsquedas, lecturas ni comprobaciones ya hechas sin justificación.
- Reutiliza la evidencia válida mientras sus condiciones no cambien.
- Tests focalizados durante el desarrollo; suite completa al cerrar, según `## Validaciones`.
- Informes concisos: no repitas instrucciones ni evidencia ya presentada.

## Validaciones

La referencia es `.github/workflows/verify.yml`. Si cambias un comando allí, cámbialo aquí.

```bash
# Desde la raíz del proyecto (donde está package.json)
npm ci --include=dev        # si faltan devDependencies
npx tsc --noEmit
npx vitest run
npm run build               # genera dist/, que Rust necesita para compilar
npm audit --omit=dev
git diff --check
```

```bash
# Desde src-tauri/ (donde está Cargo.toml)
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --locked --bins
cargo clippy --locked --bins -- -D warnings
cargo audit --file Cargo.lock
```

- `--locked` detecta un `Cargo.lock` desincronizado.
- `dist/` tiene que existir de verdad: `tauri-build` entra en pánico sin él. No se versiona.
- El CI usa `--bins`. El código de pruebas tiene deuda de Clippy registrada en `STATUS.md`: no
  la amplíes.
- Node: la versión de `.node-version`. No la repitas a mano.
- **No cambies una dependencia para callar un aviso** sin analizarlo y justificarlo.

Tres planos que no se sustituyen: **pruebas con fixtures** (rápidas, no demuestran nada del
equipo), **diagnósticos estáticos** (informan, no demuestran) y **validación real** (la única
que prueba la detección con juegos reales). Un fixture en verde no es «validado en el equipo».

## Metodología

Antes de una fase: consulta roadmap y estado, inspecciona la arquitectura y propón **alcance,
diseño, riesgos y criterios de aceptación**. **Espera autorización**: una propuesta registrada
no es permiso para implementarla.

Durante: pruebas específicas del área tocada y sin ampliar el alcance por iniciativa propia.

Al cerrar: validación completa, **limitaciones documentadas** y estado actualizado.

Después: **no avances a la fase siguiente**. Para y espera.

### Limitación vigente

La detección **simultánea** de Eslabong (Heronite) y Tattoo Tycoon (Heroic) es incompleta en
ambos órdenes. La causa no está demostrada y la incidencia está aplazada: no bloquea la
siguiente fase. No se declara resuelta ni se reabre sin una captura nueva del diagnóstico.
