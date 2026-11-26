# 🎮 Thoriapp — Optimizador de juegos para Linux

Aplicación de escritorio (Tauri 2 + React/TypeScript + Rust) para preparar, registrar y
comparar configuraciones de ejecución de juegos en Linux.

> **Versión**: 1.2.0
> **Estado**: fases 1–5.3 cerradas; 5.4, 6 y 7 pendientes. El observador de ejecución es
> **experimental, opcional y apagado por defecto**.
> Ver [Limitaciones conocidas](#-limitaciones-conocidas).

## 📦 Instalación

```bash
cd tauri-app
chmod +x install.sh
./install.sh
npm run tauri dev
```

Para compilar el paquete distribuible:

```bash
npm run tauri build
```

## ✨ Qué hace

### Motor de comandos en Rust

El comando de lanzamiento lo compone **Rust**, no el frontend: precedencia de envoltorios,
marcador `%command%`, validación de argumentos y revisión del motor. La interfaz muestra el
comando resultante junto a la procedencia de cada parte, para que siempre se sepa qué se va a
ejecutar y por qué.

**Conservación de argumentos desconocidos**: los argumentos que Thoriapp no interpreta se
conservan literalmente y en su orden. Nada se descarta de forma silenciosa al reconstruir el
comando.

### Perfiles y restauración

Perfiles por juego con guardado, aplicación y **restauración** al estado anterior. Se comprueba
la integridad antes de aplicar; un dato ilegible se trata como ausente, nunca como un valor
inventado.

### Benchmarks e instantáneas de configuración

- Importación de grabaciones de **MangoHud** (CSV y resumen) con ventana temporal.
- **Instantáneas de configuración**: al guardar una prueba se congela la configuración con la
  que se hizo, con la procedencia de cada dato. A partir de ahí la prueba es histórica:
  renombrar un perfil o cambiar Steam **no** la modifica.
- Historial y comparaciones **aislados por juego**: no se mezclan juegos distintos.
- `ejecucion_verificada` queda vacío a propósito: tener Proton configurado no demuestra que la
  prueba se hiciera con Proton.

### Observador de ejecución — experimental y opcional

Detecta qué juego está corriendo leyendo `/proc` y correlacionándolo con el catálogo de
lanzadores. **Está apagado por defecto**: mientras no se active, no se lee `/proc`.

- Identidad **observada** y **comercial** por separado. Sin evidencia suficiente el resultado
  es «identidad desconocida», no una suposición.
- Asociación de grabaciones a sesiones con niveles de confianza, y **asignación manual** con
  prioridad sobre cualquier inferencia.
- No es necesario para registrar, importar, guardar o comparar benchmarks.

## 🎮 Compatibilidad de lanzadores

Comprobada **en el equipo real** (CachyOS), no solo con fixtures:

| Lanzador | Juego | Resultado |
|---|---|---|
| Steam | Deskrawl, Palworld | Detección individual correcta |
| Heroic | Tattoo Tycoon | Detección individual correcta |
| Heronite | Eslabong | Detección individual correcta |
| Lutris (nativo) | Starsector | Nombre comercial correcto, estable 120 s |
| Lutris (Wine) | Star Valor | Detección individual correcta |

- **Simultaneidad validada**: Steam + Steam (Palworld + Deskrawl) y Lutris + Steam
  (Starsector + Deskrawl).
- Cierre de los seis juegos correcto; reiniciar Thoriapp no deja sesiones fantasma.

## ⚠️ Limitaciones conocidas

- **Eslabong (Heronite) + Tattoo Tycoon (Heroic) simultáneos**: la detección es **incompleta**
  en ambos órdenes de lanzamiento. La causa exacta no está demostrada y la incidencia queda
  aplazada; no bloquea la fase 5.4. Reabrirla exige una captura nueva del diagnóstico con el
  observador activo en ese escenario: los registros existentes no bastan.
- El observador es **experimental**. La fase 5.3 se cerró con esta limitación declarada.
- La detección depende del catálogo de lanzadores: un juego que no esté en él y no aporte
  evidencia propia se queda como identidad desconocida.
- Una sesión observada **no acredita** la configuración realmente ejecutada.

## 🚀 Uso básico

1. Abre la app y elige juego.
2. Revisa las recomendaciones (compatibilidad y optimización están separadas).
3. Guarda un perfil con el comando que quieras usar.
4. Lanza el juego y graba con MangoHud.
5. Importa la grabación, guarda la prueba y compárala con las anteriores.

## 📋 Estructura del proyecto

- `src-tauri/` — backend Rust: motor de comandos, perfiles, benchmarks, observador.
  - `src/engine.rs`, `src/rules.rs` — motor y reglas.
  - `src/benchmarks.rs`, `src/mangohud.rs` — pruebas e importación.
  - `src/sessions*.rs`, `src/proceso_linux.rs`, `src/evidencia_proceso.rs` — observador.
  - `src/fixtures_procesos.rs` — topologías reales usadas por las pruebas.
- `src/` — frontend React/TypeScript.
- `tests/fixtures/mangohud/` — grabaciones reales usadas como fixtures (sin datos personales).
- `docs/` — documentación de arquitectura, decisiones y planificación.

## 🧪 Comprobar el proyecto

```bash
# Tipos y tests del frontend
npx tsc --noEmit
npx vitest run

# Rust
cd src-tauri
export PATH="$HOME/.cargo/bin:$PATH"   # el cargo de apt es antiguo para Tauri 2
cargo test --bins
cargo clippy --bins
```

El único test ignorado es `anticheat::tests::manual_descarga_y_busca_appids_reales`: toca la
red real y se lanza a mano con `--ignored`.

## 📚 Documentación

- `STATUS.md` — estado por fase, decisiones y correcciones con su evidencia.
- `docs/sesiones-de-ejecucion.md` — contrato del observador y qué está garantizado.
- `docs/validacion-real.md` — protocolo de comprobación con juegos reales en el equipo.
- `docs/planificacion-fases.md` — frontera entre fases.
- `CHANGELOG.md` — cambios acumulados desde `v1.1.0`.

## 🗺️ Roadmap

| Fase | Estado |
|---|---|
| 1–4 — Motor, catálogo, perfiles, integridad | Cerradas |
| 5.1 — Almacenamiento e integridad | Cerrada |
| 5.2 — Instantáneas de configuración | Cerrada |
| 5.3 — Captura temprana y evidencias de ejecución | **Cerrada** con la limitación de arriba |
| 5.4 — Calidad de comparación | Pendiente |
| 6 — Rediseño de la experiencia de usuario | Pendiente |
| 7 — Posterior a la fase 6 | Pendiente |

## 📄 Licencia

Ver `LICENSE`.
