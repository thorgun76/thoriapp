# Metodología del resumen de MangoHud (`_summary.csv`)

> **Estado: resultados experimentales, no verificación de la implementación de MangoHud.**
>
> Todo lo de aquí se dedujo contrastando dos grabaciones reales con sus resúmenes. No se ha
> leído el código fuente de MangoHud. Las conclusiones son compatibles con los datos, pero
> podrían no valer para otras versiones. Por eso **Thoriapp no usa nada de esto para
> calcular**: se guarda el resumen original tal cual y se muestra como fuente aparte.

## Qué se guarda y qué no

- **Se guarda**: cada par nombre/valor que escribió MangoHud, sin recalcular ni redondear,
  asociado únicamente a su CSV (`<base>.csv` ↔ `<base>_summary.csv`).
- **No se usa** para sobrescribir las métricas de Thoriapp ni para decidir el ganador de
  una comparación.

## Diferencias observadas entre el resumen y Thoriapp

Grabación `Palworld-Win64-Shipping_2026-11-24_18-19-57` (598 muestras, 60 s):

| Métrica | Summary MangoHud | Thoriapp (10 s descartados) | Thoriapp (sin descartar) |
|---|---|---|---|
| FPS medios | 60.4 | 66.23 | 63.24 |
| 1 % low | 26.4349 | 41.31 | 33.58 |
| 0.1 % low | 17.8632 | 18.29 | 18.38 |

### 1. La media

MangoHud: `1000 / media(frametime)` = **60.3629** → se escribe 60.4 (una décima).

Thoriapp: `media(fps)` = **63.2367**.

No son el mismo estadístico. La media armónica pondera más los fotogramas lentos; la
aritmética, los rápidos. Ninguna es «la correcta» sin decir cuál se usa.

### 2. El descarte de calentamiento

Thoriapp descarta los primeros 10 s (configurable). Explica 63.24 → 66.23 y 33.58 → 41.31.
MangoHud resume la grabación **completa**.

### 3. Los percentiles

Esto es lo que hace que los números no cuadren aunque uno intente compararlos:

- **`0.1% Min FPS` es el máximo absoluto del frametime**, no un percentil.
  Con 598 muestras, el 0.1 % son 0,6 muestras. Comprobado: `55.981 ms → 17.8632`, exacto
  hasta el cuarto decimal.
- **`1% Min FPS` es una media de los ~9-10 peores frametimes.**
  Comprobado: media de los 9 peores `= 37.8991 ms → 26.3858` (resumen: 26.4349).
  Un percentil interpolado al 99 % daría `29.7762 ms → 33.58`, que no coincide.
- **`97% Percentile FPS`** sí es un percentil, con *nearest-rank*: `79.1689`, exacto.

Thoriapp usa percentiles con interpolación lineal sobre el frametime (p99 y p99.9). Son
definiciones distintas con nombres parecidos.

## Unidades de la serie temporal

Verificadas con evidencia interna del CSV, no por convención:

| Columna | Unidad | Cómo se comprobó |
|---|---|---|
| `fps` | 1/s | `frametime × fps = 1000` con error máx. 0,0001 |
| `frametime` | ms | idem |
| `elapsed` | **nanosegundos** | total ≈ 60e9 para una grabación de 60 s |
| `cpu_load`, `gpu_load` | % | rango 36–56 y 62–97 |
| `cpu_temp`, `gpu_temp` | °C | rango 64–69 y 51–60 |
| `gpu_core_clock`, `cpu_mhz`, `gpu_mem_clock` | MHz | rango 204–2475, 4617–4653 |
| `gpu_power` | W | rango 44–99 |
| `gpu_vram_used`, `ram_used`, `swap_used` | GB | 4,03–4,50 VRAM en una GPU de 8 GB; 15,1–15,5 de RAM |
| `ram` (cabecera del sistema) | **KiB** | `32777068 KiB = 31.26 GiB`; en bytes serían 31.26 MiB, incompatible con `ram_used` |

**Muestreo**: 100 ms exactos en las dos grabaciones (delta 0,1000–0,1010 s, mediana 100 ms).
Son ~600 **muestras periódicas**, no fotogramas. `total_frames` es un nombre heredado;
`sample_count` dice lo mismo sin engañar.

## Ceros y sensores

Un cero en todas las muestras **no** significa por sí solo «sensor ausente». Se decide por
métrica:

| Métrica | Ceros | Interpretación |
|---|---|---|
| `cpu_power` | 599/599 y 598/598 | probablemente sensor no disponible |
| `process_rss` | 599/599 y 598/598 | probablemente sensor no disponible |
| `swap_used` | nunca | válido (0 GB sería legítimo) |
| `gpu_load` | nunca | un 0 % sería legítimo |

Se distinguen cinco estados: columna inexistente, valor inválido, no disponible, válido y
disponibilidad incierta. No se convierte un cero válido en ausente ni al revés.

## Correlación no es causa

Observación: 11 muestras de la grabación del 24-11 tienen `gpu_core_clock` = 204 MHz (frente
a ~2400 MHz normales), a los 5,5 s, con fps 23–52. Lo mismo con `gpu_temp` bajo,
`gpu_power` bajo y utilización alta.

**No se puede concluir** que la frecuencia baja causara el tirón: es igual de compatible con
lo contrario —que la carga bajara y por eso la GPU redujera frecuencia—. El CSV no permite
distinguir la dirección. Se guarda el dato y no se diagnostica con él.

El mismo criterio se aplica a shaders, temperaturas, consumo y utilización.
