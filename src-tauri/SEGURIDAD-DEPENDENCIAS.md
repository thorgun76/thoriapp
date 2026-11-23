# Dependencias y avisos de seguridad

Notas sobre lo que `cargo audit` reporta y por qué. Ver también `npm run audit`.

## Cómo comprobarlo

```bash
cargo install cargo-audit     # solo la primera vez
npm run audit                 # JS de producción + Rust
```

`cargo audit` devuelve error solo si encuentra **vulnerabilidades**. Las
advertencias de "unmaintained" y "unsound" aparecen como `warning` y no rompen
la comprobación. En CI corre igual (`.github/workflows/verify.yml`).

## Estado actual

Ninguna vulnerabilidad conocida (`error`). Hay dos advertencias, ambas en la
pila GTK de Tauri y ninguna en código de Thoriapp:

| ID | Crate | Tipo | Por qué no aplica |
|----|-------|------|-------------------|
| RUSTSEC-2024-0429 | `glib` 0.18.5 | unsound | Afecta a los impls de `Iterator`/`DoubleEndedIterator` sobre `glib::VariantStrIter`. Thoriapp no usa GVariant: GTK solo dibuja la ventana. |
| RUSTSEC-2024-0370 | `proc-macro-error` 1.0.4 | unmaintained | Dependencia de compilación de `glib-macros`. No se incluye en el binario final. |

Cadena de dependencia, para que quede claro que no se puede arreglar aquí:

```
tauri → muda / tauri-runtime-wry → tao → gtk 0.18 → glib 0.18 → glib-macros → proc-macro-error
```

Subir a `glib` 0.19+ depende de que Tauri cambie su dependencia de `gtk`.

**Al actualizar Tauri:** volver a ejecutar `npm run audit:rust`. Si Tauri ya usa
`gtk` 0.19 o superior, estas dos entradas desaparecen solas y este archivo se
puede acortar.

## Historial

- **2026-11:** `rusqlite` 0.32 → 0.41 corrigió RUSTSEC-2026-0459 (`libsqlite3-sys`
  con un CVE de SQLite). Lo usaba `launchers.rs` para leer la base de datos de
  Lutris; la API usada (`Connection`, `OpenFlags`, `prepare`, `query_map`) no
  cambió, así que la actualización no requirió tocar código.
