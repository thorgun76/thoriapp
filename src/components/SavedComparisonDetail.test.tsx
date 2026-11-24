import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SavedComparisonDetail, tokenizarComando, diffComandos } from './SavedComparisonDetail';
import { interpretar, etiquetasDe, titularDelCambio, mejorObservada } from '../lib/interpretacion';
import type { Benchmark, BenchmarkComparison } from '../lib/types';

afterEach(cleanup);

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: '7DaysToDie',
    appid: 251570,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 100,
    fps_1_percent_low: 70,
    fps_0_1_percent_low: 60,
    max_fps: 140,
    min_fps: 40,
    frame_time_avg: 10,
    frame_time_1_percent_high: 14.3,
    total_frames: 600,
    notes: null,
    launch_command: '%command%',
    spikes_per_min: 2,
    pct_over_33ms: 1,
    jitter_ms: 1.5,
    worst_frame_ms: 25,
    warmup_seconds: 10,
    noise_percent: 5,
    log_path: '/x/a.csv',
    ...over,
  };
}

function cmp(
  antes: Partial<Benchmark> = {},
  despues: Partial<Benchmark> = {},
  noise: number | null = 5
): BenchmarkComparison {
  return {
    baseline: bench(antes),
    test: bench(despues),
    fps_change_percent: 0,
    fps_1_percent_change_percent: 0,
    fps_0_1_percent_change_percent: 0,
    frame_time_change_percent: 0,
    conclusion: '',
    noise_percent: noise,
    within_noise: false,
  } as BenchmarkComparison;
}

/**
 * Abre la sección «Configuración comparada».
 *
 * Desde el rediseño VS está plegada por defecto (con los dos comandos completos ocupaba
 * más alto que el propio resultado), así que cualquier aserción sobre comandos tiene que
 * desplegarla primero. Es también la prueba de que el plegado funciona.
 */
function abrirConfiguracion() {
  const btn = screen.getByRole('button', { name: /Configuración comparada/i });
  if (btn.getAttribute('aria-expanded') !== 'true') fireEvent.click(btn);
}

function renderDetail(c: BenchmarkComparison, handlers: Partial<{ onDelete: () => void; onCopy: (t: string) => void }> = {}) {
  return render(
    <SavedComparisonDetail
      comparison={c}
      onBack={() => {}}
      onDelete={handlers.onDelete ?? (() => {})}
      onCopy={handlers.onCopy ?? (() => {})}
    />
  );
}

/* ─────────────────────────── Diferencias de comandos ─────────────────────────── */

describe('tokenizarComando', () => {
  it('respeta comillas dobles con espacios', () => {
    expect(tokenizarComando('WINEDLLOVERRIDES="d3d11=n" %command%')).toEqual([
      'WINEDLLOVERRIDES=d3d11=n',
    ]);
  });

  it('respeta comillas simples', () => {
    expect(tokenizarComando("--name 'mi juego' %command%")).toEqual(['--name', 'mi juego']);
  });

  it('no parte una ruta entrecomillada', () => {
    expect(tokenizarComando('PROTON_PATH="/home/u/Mis Juegos/x" %command%')).toEqual([
      'PROTON_PATH=/home/u/Mis Juegos/x',
    ]);
  });

  it('descarta el marcador %command%', () => {
    expect(tokenizarComando('%command%')).toEqual([]);
  });

  it('comando vacío no rompe', () => {
    expect(tokenizarComando('')).toEqual([]);
    expect(tokenizarComando('   ')).toEqual([]);
  });

  it('escapa caracteres con barra invertida', () => {
    expect(tokenizarComando('--a\\ b %command%')).toEqual(['--a b']);
  });
});

describe('diffComandos', () => {
  it('comandos idénticos → sin cambios', () => {
    expect(diffComandos('%command%', '%command%').iguales).toBe(true);
  });

  it('dos vacíos NO son «iguales»: no hay dato', () => {
    // Antes esto devolvía `iguales: true` y la pantalla afirmaba «sin cambios» sobre
    // pruebas que nunca registraron el comando. Es el origen del fallo de Palworld.
    const d = diffComandos('', '');
    expect(d.iguales).toBe(false);
    expect(d.comparable).toBe(false);
  });

  it('un comando ausente tampoco es comparable', () => {
    expect(diffComandos('', 'mangohud %command%').comparable).toBe(false);
    expect(diffComandos('mangohud %command%', '').comparable).toBe(false);
  });

  it('detecta añadidos y eliminados', () => {
    const d = diffComandos('gamemoderun %command%', 'gamemoderun mangohud %command%');
    expect(d.añadidos).toEqual(['mangohud']);
    expect(d.eliminados).toEqual([]);
    expect(d.iguales).toBe(false);
  });

  it('un cambio de valor aparece como un añadido y un eliminado', () => {
    // No se intenta adivinar "parámetro modificado": con shell no es seguro.
    const d = diffComandos('DXVK_HUD=fps %command%', 'DXVK_HUD=full %command%');
    expect(d.añadidos).toEqual(['DXVK_HUD=full']);
    expect(d.eliminados).toEqual(['DXVK_HUD=fps']);
  });
});

/* ─────────────────────────── Pantalla de detalle ─────────────────────────── */

describe('SavedComparisonDetail: jerarquía', () => {
  it('el resultado va ANTES que la configuración', () => {
    const { container } = renderDetail(cmp());
    const texto = container.textContent ?? '';
    const posResultado = texto.indexOf('Sin diferencia concluyente');
    const posConfig = texto.indexOf('Configuración comparada');
    expect(posResultado).toBeGreaterThan(-1);
    expect(posConfig).toBeGreaterThan(-1);
    expect(posResultado).toBeLessThan(posConfig);
  });

  it('muestra el nombre legible del juego', () => {
    renderDetail(cmp({ game_name: 'Palworld-Win64-Shipping' }, { game_name: 'Palworld-Win64-Shipping' }));
    expect(screen.getByRole('heading', { level: 2 }).textContent).toBe('Palworld');
  });

  it('usa "Antes" y "Después", no "Inicial"/"Final"', () => {
    const { container } = renderDetail(cmp({ avg_fps: 100 }, { avg_fps: 140 }));
    const texto = container.textContent ?? '';
    expect(texto).toContain('Antes');
    expect(texto).toContain('Después');
    expect(texto).not.toMatch(/\bInicial\b|\bFinal\b/);
  });

  it('no dice "Fluidez" en la vista principal', () => {
    const { container } = renderDetail(cmp());
    expect(container.textContent).not.toMatch(/fluidez/i);
  });

  it('la estabilidad se muestra sobre 100 y se explica como índice propio', () => {
    const { container } = renderDetail(cmp());
    const texto = container.textContent ?? '';
    expect(texto).toContain('Estabilidad de FPS');
    expect(texto).toContain('índice propio de Thoriapp');
    expect(texto).toContain('no una métrica estándar');
  });

  it('avisa de que un 100 no significa fluido', () => {
    const { container } = renderDetail(cmp());
    expect(container.textContent).toMatch(/30 FPS/);
  });
});

describe('SavedComparisonDetail: comandos', () => {
  it('comandos idénticos muestran una sola tarjeta con "Sin cambios"', () => {
    renderDetail(cmp({ launch_command: '%command%' }, { launch_command: '%command%' }));
    abrirConfiguracion();
    // `%command%` en las dos: registrado como "sin parámetros en las dos".
    expect(screen.getAllByText('Sin parámetros en las dos').length).toBeGreaterThan(0);
  });

  it('dos comandos largos idénticos dicen «Sin cambios»', () => {
    const c = 'mesa_glthread=true game-performance mangohud %command%';
    renderDetail(cmp({ launch_command: c }, { launch_command: c }));
    abrirConfiguracion();
    expect(screen.getAllByText('Sin cambios').length).toBeGreaterThan(0);
  });

  it('con comandos iguales avisa de que no hay cambio al que atribuir la mejora', () => {
    const { container } = renderDetail(
      cmp({ launch_command: 'mangohud %command%' }, { launch_command: 'mangohud %command%' })
    );
    abrirConfiguracion();
    expect(container.textContent).toMatch(/no hay ningún cambio de configuración al que atribuir/i);
  });

  it('explica el marcador %command%', () => {
    const { container } = renderDetail(cmp());
    abrirConfiguracion();
    expect(container.textContent).toMatch(/marcador de Steam/);
  });

  it('comandos distintos muestran el diff y no atribuyen causalidad', () => {
    const { container } = renderDetail(
      cmp({ launch_command: '%command%' }, { launch_command: 'gamemoderun mangohud %command%' })
    );
    abrirConfiguracion();
    const texto = container.textContent ?? '';
    expect(texto).toContain('Comando registrado al iniciar cada prueba');
    expect(texto).toContain('+ mangohud');
    expect(texto).toMatch(/no demuestra que sea la causa/);
  });

  it('el botón Copiar copia el comando', () => {
    const onCopy = vi.fn();
    renderDetail(cmp({ launch_command: 'mangohud %command%' }, { launch_command: 'mangohud %command%' }), { onCopy });
    abrirConfiguracion();
    // Los botones de copiar de los participantes del VS comparten etiqueta: se busca el
    // de la tarjeta de configuración, que es el que copia el comando completo.
    const botones = screen.getAllByRole('button', { name: /Copiar/i });
    fireEvent.click(botones[botones.length - 1]);
    expect(onCopy).toHaveBeenCalledWith('mangohud %command%');
  });
});

describe('SavedComparisonDetail: acciones y accesibilidad', () => {
  it('el botón Eliminar pide confirmación al padre', () => {
    const onDelete = vi.fn();
    renderDetail(cmp(), { onDelete });
    fireEvent.click(screen.getByRole('button', { name: /Eliminar prueba/i }));
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it('con números, «métricas avanzadas» no dice Antes/Después en ninguna parte', () => {
    // Regresión: la tabla de medidas y el gráfico temporal se quedaron fuera del cambio
    // a «Prueba N» y seguían rotulando «Antes»/«Después».
    const c = cmp({ number: 1 }, { number: 2 });
    renderDetail(c);

    fireEvent.click(screen.getByRole('button', { name: /Ver métricas avanzadas/i }));

    const detalles = document.getElementById('detalles-tecnicos');
    expect(detalles).toBeTruthy();
    const texto = detalles!.textContent ?? '';
    expect(texto).toContain('Prueba 1');
    expect(texto).toContain('Prueba 2');
    expect(texto).not.toContain('Antes');
    expect(texto).not.toContain('Después');
    // Solo como rótulo: en mayúscula inicial. En minúscula puede ser prosa legítima
    // («esta prueba se guardó antes de que…»), que no identifica ninguna serie.
    expect(texto).not.toMatch(/\bAntes\b|\bDespués\b/);
    expect(texto).not.toMatch(/\(antes\)|\(después\)/i);
  });

  it('las métricas avanzadas usan Antes/Después en grabaciones sin número', () => {
    renderDetail(cmp());
    fireEvent.click(screen.getByRole('button', { name: /Ver métricas avanzadas/i }));
    const texto = document.getElementById('detalles-tecnicos')!.textContent ?? '';
    expect(texto).toContain('Antes');
    expect(texto).toContain('Después');
  });

  it('las métricas avanzadas están plegadas y se abren con teclado', () => {
    renderDetail(cmp());
    const boton = screen.getByRole('button', { name: /Ver métricas avanzadas/i });
    expect(boton.getAttribute('aria-expanded')).toBe('false');
    expect(screen.queryByText(/Margen estimado/)).toBeNull();

    fireEvent.click(boton);
    expect(boton.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText(/Margen estimado/)).toBeTruthy();
  });

  it('las barras tienen etiqueta accesible, no solo color', () => {
    renderDetail(cmp({ avg_fps: 100 }, { avg_fps: 140 }));
    const barras = screen.getAllByRole('img');
    expect(barras.length).toBeGreaterThan(0);
    expect(barras[0].getAttribute('aria-label')).toMatch(/Antes: /);
  });

  it('datos con FPS cero no rompen la pantalla', () => {
    renderDetail(cmp({ avg_fps: 0 }, { avg_fps: 0 }));
    // Aparece como titular del resultado y también como explicación sin ganadora.
    expect(screen.getAllByText(/No hay datos suficientes/).length).toBeGreaterThan(0);
  });

  it('percentiles ausentes no se inventan', () => {
    const { container } = renderDetail(
      cmp({ fps_1_percent_low: 0, fps_0_1_percent_low: 0 }, { fps_1_percent_low: 0, fps_0_1_percent_low: 0 }, 20)
    );
    expect(container.textContent).toMatch(/Sin datos|No hay datos para comparar/);
  });

  it('grabaciones guardadas sin noise_percent siguen funcionando', () => {
    // Compatibilidad: comparaciones antiguas no tienen margen guardado.
    renderDetail(cmp({}, {}, null));
    expect(screen.getByText(/Sin diferencia concluyente|No hay datos suficientes/)).toBeTruthy();
  });
});

/* ─────────────────────────── Panel VS ─────────────────────────── */

describe('SavedComparisonDetail: panel VS', () => {
  // Cada caso con los números que de verdad llevan al estado buscado. Los criterios son
  // los del intérprete; aquí solo se comprueba qué se pinta a partir de ellos.
  const MEJORA = () =>
    cmp({ number: 1, avg_fps: 74.3, fps_1_percent_low: 50, fps_0_1_percent_low: 45 },
        { number: 2, avg_fps: 100.7, fps_1_percent_low: 70, fps_0_1_percent_low: 62 });
  const EMPEORA = () =>
    cmp({ number: 1, avg_fps: 75, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
        { number: 2, avg_fps: 68.5, fps_1_percent_low: 39.4, fps_0_1_percent_low: 22.4 });
  const SIN_CAMBIO = () =>
    cmp({ number: 1, avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
        { number: 2, avg_fps: 100.3, fps_1_percent_low: 70.2, fps_0_1_percent_low: 60.2 });

  it('muestra las dos pruebas con sus FPS y el VS', () => {
    renderDetail(EMPEORA());
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('VS');
    expect(texto).toContain('Prueba 1');
    expect(texto).toContain('Prueba 2');
    expect(texto).toContain('75.0');
    expect(texto).toContain('68.5');
  });

  it('empeora: corona a la Prueba 1 y dice que la Prueba 2 rinde peor', () => {
    renderDetail(EMPEORA());
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('Prueba 2 rinde peor');
    expect(texto).toContain('Mejor resultado observado');
    // La insignia va con el mejor resultado, que aquí es la prueba de referencia.
    expect(screen.getByText(/Mejor resultado$/i)).toBeTruthy();
    expect(texto).not.toContain('Gana «Prueba 1»');
  });

  it('mejora: corona a la Prueba 2', () => {
    const r = interpretar(MEJORA(), etiquetasDe(MEJORA()));
    expect(r.estado).toBe('mejora');
    expect(mejorObservada(r, etiquetasDe(MEJORA()))?.nombre).toBe('Prueba 2');
  });

  it('más FPS medios con peores lows NO da corona (respeta el estado mixto)', () => {
    // Promedio arriba, 1 % low y 0,1 % low claramente abajo: el intérprete decide.
    const c = cmp(
      { number: 1, avg_fps: 60, fps_1_percent_low: 50, fps_0_1_percent_low: 45 },
      { number: 2, avg_fps: 80, fps_1_percent_low: 30, fps_0_1_percent_low: 20 }
    );
    const r = interpretar(c, etiquetasDe(c));
    expect(r.estado).toBe('mixto');
    expect(mejorObservada(r, etiquetasDe(c))).toBeNull();
  });

  it('sin diferencia concluyente no hay corona', () => {
    const c = SIN_CAMBIO();
    const r = interpretar(c, etiquetasDe(c));
    expect(r.estado).toBe('sin_cambio');
    expect(mejorObservada(r, etiquetasDe(c))).toBeNull();
  });

  it('datos insuficientes no hay corona', () => {
    const c = cmp({ number: 1, avg_fps: 0 }, { number: 2, avg_fps: 0 });
    const r = interpretar(c, etiquetasDe(c));
    expect(mejorObservada(r, etiquetasDe(c))).toBeNull();
  });

  it('sin corona no aparece la insignia «Mejor resultado» en la pantalla', () => {
    renderDetail(SIN_CAMBIO());
    expect(screen.queryByText(/^Mejor resultado$/i)).toBeNull();
  });

  it('la insignia dice «Mejor resultado», no «Ganadora»', () => {
    // «Ganadora» sugería una victoria confirmada; el intérprete solo puede afirmar cuál
    // fue el mejor resultado observado en una única sesión.
    renderDetail(EMPEORA());
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('Mejor resultado');
    expect(texto).not.toMatch(/ganadora/i);
  });

  it('no repite una recomendación al pie del resultado', () => {
    // El titular y «Mejor resultado observado» ya dicen lo que pasó; el consejo final
    // («Vuelve a Prueba 1: …») se eliminó del intérprete y de las dos pantallas.
    for (const c of [EMPEORA(), MEJORA(), SIN_CAMBIO()]) {
      const { container, unmount } = renderDetail(c);
      expect(container.textContent ?? '').not.toMatch(/Vuelve a «/);
      unmount();
    }
  });

  it('el aviso del 0,1 % low aparece una sola vez y en la caja técnica', () => {
    // Antes salía dos veces con el mismo contenido: en la lista de avisos del veredicto
    // (`low01.aviso`) y en la explicación técnica de abajo. Ahora solo en la segunda.
    const c = EMPEORA();
    const r = interpretar(c, etiquetasDe(c));
    // El fixture es una grabación corta, así que el intérprete sí emite el aviso.
    expect(r.low01.aviso).toBeTruthy();

    renderDetail(c);
    const texto = document.body.textContent ?? '';

    // El texto del intérprete ya no se pinta: era la copia duplicada.
    expect(texto).not.toContain(r.low01.aviso!);
    // Queda una sola mención del 0,1 % low como advertencia, con el texto pedido.
    const veces = (texto.match(/puede variar mucho con grabaciones cortas/gi) ?? []).length;
    expect(veces).toBe(1);
    expect(texto).not.toMatch(/es poco fiable aquí/);
    expect(texto).not.toMatch(/Tómalo como orientativo/);
  });

  it('los estados sin ganadora no pintan «Peor» agresivo', () => {
    renderDetail(SIN_CAMBIO());
    const texto = document.body.textContent ?? '';
    expect(texto).not.toContain('rinde peor');
  });

  it('el titular habla del cambio, no de quién gana', () => {
    const c = EMPEORA();
    const r = interpretar(c, etiquetasDe(c));
    expect(titularDelCambio(r, etiquetasDe(c))).toBe('Prueba 2 rinde peor');
  });

  it('conserva «Resultado provisional»', () => {
    renderDetail(EMPEORA());
    expect(screen.getByText(/Resultado provisional/)).toBeTruthy();
  });

  it('el nombre de cada prueba lleva su comando en un tooltip', () => {
    renderDetail(
      cmp(
        { number: 1, launch_command: 'mangohud %command%' },
        { number: 2, launch_command: 'PROTON_ENABLE_NVAPI=1 mangohud %command%' }
      )
    );
    const tips = screen.getAllByRole('tooltip').map((t) => t.textContent ?? '');
    expect(tips.some((t) => t.includes('mangohud %command%'))).toBe(true);
    expect(tips.some((t) => t.includes('PROTON_ENABLE_NVAPI=1'))).toBe(true);
  });

  it('sin comando registrado lo dice, sin inventar %command%', () => {
    renderDetail(cmp({ number: 1, launch_command: '' }, { number: 2, launch_command: '' }));
    const tips = screen.getAllByRole('tooltip').map((t) => t.textContent ?? '');
    expect(tips.some((t) => t.includes('Comando no registrado'))).toBe(true);
  });

  it('la insignia «Mejor resultado» va en la misma fila que el nombre, no encima', () => {
    // Si la insignia va en su propio bloque, empuja el marcador de FPS hacia abajo y los
    // dos números quedan a distinta altura: el duelo se ve descuadrado.
    renderDetail(EMPEORA());

    const insignia = screen.getByText(/^Mejor resultado$/i).parentElement!;
    const filaNombre = insignia.parentElement!;

    // La fila que contiene la insignia contiene también el nombre de la prueba.
    expect(filaNombre.textContent).toContain('Prueba 1');
    // Y no contiene el marcador de FPS: está en otra fila, ya alineada por min-h.
    expect(filaNombre.textContent).not.toMatch(/\bFPS\b/);
  });

  it('las dos filas de nombre son idénticas en altura, con o sin insignia', () => {
    // La garantía de alineación es que la fila del nombre mide lo mismo en los dos lados
    // aunque solo uno lleve insignia; si difirieran, los FPS quedarían descuadrados.
    const c = EMPEORA();
    const r = interpretar(c, etiquetasDe(c));
    expect(r.ganador).toBe('antes'); // solo un lado lleva insignia
    renderDetail(c);

    // El botón va dentro del span de `PruebaN`; la fila del nombre es su abuelo.
    const filas = ['Prueba 1', 'Prueba 2'].map(
      (n) => screen.getByRole('button', { name: n }).parentElement!.parentElement!
    );
    expect(filas[0].className).toBe(filas[1].className);
    expect(filas[0].className).toContain('min-h-');
  });

  it('las pruebas sin número siguen diciendo Antes/Después en el VS', () => {
    renderDetail(cmp({ avg_fps: 75 }, { avg_fps: 68.5 }));
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('Antes');
    expect(texto).toContain('Después');
  });
});

describe('SavedComparisonDetail: configuración comparada plegable', () => {
  it('empieza cerrada y no enseña los comandos', () => {
    renderDetail(cmp({ launch_command: 'mangohud %command%' }, { launch_command: 'DXVK_HUD=1 %command%' }));
    const btn = screen.getByRole('button', { name: /Configuración comparada/i });
    expect(btn.getAttribute('aria-expanded')).toBe('false');
    // El comando sí aparece en el tooltip del participante (ahí se consulta a propósito);
    // lo que no debe existir es la sección desplegada de configuración.
    expect(document.getElementById('configuracion-comparada')).toBeNull();
  });

  it('al abrirla aparecen los dos comandos completos', () => {
    renderDetail(cmp({ launch_command: 'mangohud %command%' }, { launch_command: 'DXVK_HUD=1 %command%' }));
    abrirConfiguracion();
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('mangohud %command%');
    expect(texto).toContain('DXVK_HUD=1 %command%');
  });
});

describe('SavedComparisonDetail: tabla única de métricas', () => {
  /** Devuelve la tabla de métricas (la que tiene cabecera «Cambio»). */
  function tabla() {
    return screen.getByRole('table', { name: /Todas las métricas/i });
  }

  /** Fila del cuerpo de la tabla cuya cabecera de fila contiene `metricLabel`. */
  function fila(metricLabel: RegExp) {
    const filas = Array.from(tabla().querySelectorAll('tbody tr')).filter((f) =>
      metricLabel.test(f.textContent ?? '')
    );
    expect(filas).toHaveLength(1);
    return filas[0]!;
  }

  /** Columnas de datos (los `<td>`), sin el `<th scope="row">` de la métrica. */
  function celdas(metricLabel: RegExp) {
    return Array.from(fila(metricLabel).querySelectorAll('td'));
  }

  function celda(metricLabel: RegExp, col: number) {
    return celdas(metricLabel)[col]?.textContent ?? '';
  }

  it('hay una sola tabla con todas las métricas, no una caja por métrica', () => {
    renderDetail(cmp());
    expect(screen.getAllByRole('table')).toHaveLength(1);
    const t = tabla();
    expect(t.querySelectorAll('tbody tr')).toHaveLength(5);
  });

  it('la diferencia de estabilidad va en puntos, no en porcentaje', () => {
    // 55 → 68 es +13 puntos; en % sería +23,6 % y sería confuso.
    renderDetail(
      cmp(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
        { avg_fps: 100, fps_1_percent_low: 90, fps_0_1_percent_low: 80 }
      )
    );
    const cambio = celda(/Estabilidad/i, 2);
    expect(cambio).toMatch(/pts/);
    expect(cambio).not.toMatch(/%/);
  });

  it('el tiempo por imagen se compara en %, con signo negativo al bajar', () => {
    renderDetail(cmp({ frame_time_avg: 20 }, { frame_time_avg: 16 }));
    expect(celda(/Tiempo por imagen/i, 2)).toBe('-20.0 %');
  });

  it('las métricas que mejoran al bajar no se pintan de error', () => {
    renderDetail(cmp({ frame_time_avg: 20 }, { frame_time_avg: 16 }));
    const cambio = celdas(/Tiempo por imagen/i)[2];
    expect(cambio.className).toContain('compatible');
    expect(cambio.className).not.toContain('text-[var(--error)]');
  });

  it('el valor que gana se resalta en cada fila', () => {
    renderDetail(cmp({ avg_fps: 60 }, { avg_fps: 80 }));
    const c = celdas(/FPS medios/i);
    expect(c[0].className).not.toContain('font-semibold'); // antes: 60, pierde
    expect(c[1].className).toContain('font-semibold'); // después: 80, gana
  });

  it('una métrica sin datos en una de las dos grabaciones muestra «—»', () => {
    renderDetail(
      cmp({ fps_0_1_percent_low: 0, avg_fps: 0 }, { fps_0_1_percent_low: 60, avg_fps: 100 })
    );
    // Sin FPS medios válidos la estabilidad no se puede calcular: no se inventa un 0.
    expect(celda(/Estabilidad/i, 0)).toBe('—');
  });

  it('el aviso del 0,1 % low aparece una sola vez en toda la pantalla', () => {
    renderDetail(cmp());
    const pantalla = document.body.textContent ?? '';
    expect(pantalla.match(/puede variar mucho con grabaciones cortas/gi)).toHaveLength(1);
  });

  it('la tabla mantiene la cabecera con los nombres de cada prueba', () => {
    renderDetail(cmp());
    const t = tabla();
    const cabeceras = Array.from(t.querySelectorAll('th')).map((th) => th.textContent);
    expect(cabeceras).toContain('Métrica');
    expect(cabeceras).toContain('Cambio');
  });
});



