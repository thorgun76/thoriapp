import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SavedComparisonDetail, tokenizarComando, diffComandos } from './SavedComparisonDetail';
import type { EvaluacionPersistida } from '../lib/api';
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

/**
 * Evaluación **registrada** con la forma que produce Rust.
 *
 * **Reescrito en C.3.** Antes el estado se derivaba aquí llamando a `interpretar()`, el
 * intérprete de frontend. Ese módulo ya no existe: el motor es la única fuente, y estas
 * pruebas declaran el estado a mano para comprobar qué pinta la vista a partir de él.
 *
 * El valor por defecto (`mejora`) coincide con lo que estas pruebas esperan en la mayoría
 * de los casos; las que necesitan otro estado lo piden explícitamente.
 */
function evaluacionDe(
  c: BenchmarkComparison,
  estado: EvaluacionPersistida['estado'] = 'mejora'
): EvaluacionPersistida {
  return {
    version: 1,
    metodologia: 'v1',
    metodologia_interpretacion: 'v1',
    evaluada_en: '2026-01-02 11:00:00',
    referencia: { id: c.baseline.id, number: c.baseline.number ?? null },
    candidata: { id: c.test.id, number: c.test.number ?? null },
    estado,
    confianza: 'provisional',
    condiciones: {
      margen_porcentaje: 5,
      compatibilidad_metodologica: 'establecida',
    },
    ruido_combinado_porcentaje: c.noise_percent ?? null,
    motivos: [],
    limitaciones: [],
    metricas: [],
  } as EvaluacionPersistida;
}

/**
 * Evaluación con métricas concretas, para probar la presentación de preferencias.
 *
 * El helper `evaluacionDe` devuelve `metricas: []`, que es un caso real (una evaluación
 * sin métricas evaluables) pero no sirve para comprobar colores. Aquí se declaran las
 * métricas con la forma que produce Rust y con el apoyo que el motor escribió.
 */
function evaluacionConMetricas(
  c: BenchmarkComparison,
  filas: { clave: string; preferencia: string | null; apoyo: number }[]
): EvaluacionPersistida {
  const base = evaluacionDe(c);
  const apoyo = (n: number) => ({
    percentil: { muestras: n, valor: 0 },
  });
  return {
    ...base,
    metricas: filas.map((f) => ({
      clave: f.clave,
      direccion: 'mas_es_mejor',
      // C3-24: la elegibilidad y la preferencia son campos distintos.
      elegible: true,
      preferencia: f.preferencia,
      apoyo_referencia: apoyo(f.apoyo),
      apoyo_candidata: apoyo(f.apoyo),
    })),
  } as unknown as EvaluacionPersistida;
}

function renderDetail(
  c: BenchmarkComparison,
  handlers: Partial<{ onDelete: () => void; onCopy: (t: string) => void }> = {},
  evaluacion: EvaluacionPersistida | null = evaluacionDe(c)
) {
  return render(
    <SavedComparisonDetail
      comparison={c}
      evaluacion={evaluacion}
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
    // Se registra `sin_cambio`, que es lo que el motor diría de dos grabaciones iguales.
    const c = cmp();
    const { container } = renderDetail(c, {}, evaluacionDe(c, 'sin_cambio'));
    const texto = container.textContent ?? '';
    const posResultado = texto.indexOf('Sin cambio');
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
    expect(texto).toContain('La estabilidad de FPS es un índice propio de');
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
    // E: el rótulo «Margen estimado» ya no existe: el ruido combinado y el margen
    // aplicado son dos datos distintos y cada uno lleva su nombre.
    expect(screen.queryByText(/Ruido combinado/)).toBeNull();

    fireEvent.click(boton);
    expect(boton.getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByText(/Ruido combinado/)).toBeTruthy();
    expect(screen.getByText(/Margen aplicado/)).toBeTruthy();
  });

  it('las barras tienen etiqueta accesible, no solo color', () => {
    renderDetail(cmp({ avg_fps: 100 }, { avg_fps: 140 }));
    const barras = screen.getAllByRole('img');
    expect(barras.length).toBeGreaterThan(0);
    expect(barras[0].getAttribute('aria-label')).toMatch(/Antes: /);
  });

  it('datos con FPS cero no rompen la pantalla', () => {
    const c = cmp({ avg_fps: 0 }, { avg_fps: 0 });
    // El motor no puede evaluar sin FPS: el registro guardado dice eso, y la vista lo
    // presenta tal cual, sin deducirlo de las cifras.
    const { container } = renderDetail(c, {}, evaluacionDe(c, 'datos_insuficientes'));
    // Con FPS cero la evaluación registrada es «datos insuficientes», no un veredicto.
    expect(container.textContent).toContain('Datos insuficientes');
    // Y no se inventa una ganadora.
    expect(container.textContent).not.toMatch(/rinde mejor|rinde peor/);
  });

  it('percentiles ausentes no se inventan', () => {
    const { container } = renderDetail(
      cmp({ fps_1_percent_low: 0, fps_0_1_percent_low: 0 }, { fps_1_percent_low: 0, fps_0_1_percent_low: 0 }, 20)
    );
    // Percentil 0 es un dato medido, no una ausencia: no se inventa ni se convierte en
    // «sin datos». Solo se comprueba que no aparezca una cifra fabricada.
    expect(container.textContent).toContain('0,1 % low');
    expect(container.textContent).toMatch(/Sin cambio|Peor|Mejor/);
  });

  it('grabaciones guardadas sin noise_percent siguen funcionando', () => {
    // Compatibilidad: comparaciones antiguas no tienen margen guardado.
    const c = cmp({}, {}, null);
    renderDetail(c, {}, evaluacionDe(c, 'sin_cambio'));
    // Sin margen guardado se dice que no consta; no se asume un margen del 3 %.
    const texto = document.body.textContent ?? '';
    expect(texto).toMatch(/margen|Sin cambio/i);
    expect(texto).not.toMatch(/margen del 3/);
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
    renderDetail(EMPEORA(), {}, evaluacionDe(EMPEORA(), 'empeora'));
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('Mejor resultado observado');
    expect(texto).toContain('Empeora');
    // La insignia va con el mejor resultado, que aquí es la prueba de referencia.
    expect(screen.getByText(/Mejor resultado$/i)).toBeTruthy();
    expect(texto).not.toContain('Gana «Prueba 1»');
  });

  it('mejora: corona a la Prueba 2', () => {
    // Con `estado: 'mejora'` el modelo solo puede favorecer a la candidata, que es la
    // Prueba 2. La corona sale de ahí, no de mirar las cifras.
    renderDetail(MEJORA(), {}, evaluacionDe(MEJORA(), 'mejora'));
    expect(document.body.textContent).toContain('Mejor resultado observado: Prueba 2');
  });

  it('un estado mixto no da corona aunque los FPS medios suban', () => {
    // Promedio arriba, lows abajo; el motor lo evaluó como mixto. Una métrica mejor no
    // es una ganadora.
    const c = cmp(
      { number: 1, avg_fps: 60, fps_1_percent_low: 50, fps_0_1_percent_low: 45 },
      { number: 2, avg_fps: 80, fps_1_percent_low: 30, fps_0_1_percent_low: 20 }
    );
    renderDetail(c, {}, evaluacionDe(c, 'mixto'));
    const texto = document.body.textContent ?? '';
    expect(texto).toContain('Resultado mixto');
    expect(texto).not.toContain('Mejor resultado observado');
  });

  it('sin cambio no hay corona', () => {
    const c = SIN_CAMBIO();
    renderDetail(c, {}, evaluacionDe(c, 'sin_cambio'));
    expect(document.body.textContent).not.toContain('Mejor resultado observado');
  });

  it('datos insuficientes no hay corona', () => {
    const c = cmp({ number: 1, avg_fps: 0 }, { number: 2, avg_fps: 0 });
    renderDetail(c, {}, evaluacionDe(c, 'datos_insuficientes'));
    expect(document.body.textContent).not.toContain('Mejor resultado observado');
  });

  it('sin corona no aparece la insignia «Mejor resultado» en la pantalla', () => {
    const c = SIN_CAMBIO();
    renderDetail(c, {}, evaluacionDe(c, 'sin_cambio'));
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
    const estados = [
      [EMPEORA(), 'empeora'],
      [MEJORA(), 'mejora'],
      [SIN_CAMBIO(), 'sin_cambio'],
    ] as const;
    for (const [c, estado] of estados) {
      const { container, unmount } = renderDetail(c, {}, evaluacionDe(c, estado));
      expect(container.textContent ?? '').not.toMatch(/Vuelve a «/);
      unmount();
    }
  });

  it('con una grabación de 60 s el aviso del 0,1 % low no aparece', () => {
    // El aviso depende de la duración **medida**, no de ninguna evaluación: 60 s no es una
    // grabación corta, así que no hay nada que advertir.
    renderDetail(EMPEORA());
    expect(document.body.textContent).not.toMatch(/puede variar mucho con grabaciones cortas/gi);
  });

  it('una grabación corta sí muestra el aviso del 0,1 % low, una sola vez', () => {
    // 12 s por prueba: por debajo del umbral de grabación corta.
    const c = cmp(
      { duration_seconds: 12, total_frames: 720 },
      { duration_seconds: 12, total_frames: 720 },
      20
    );
    renderDetail(c);
    const texto = document.body.textContent ?? '';
    const veces = (texto.match(/puede variar mucho con grabaciones cortas/gi) ?? []).length;
    expect(veces).toBe(1);
    expect(texto).not.toMatch(/es poco fiable aquí/);
    expect(texto).not.toMatch(/Tómalo como orientativo/);
  });

  it('los estados sin ganadora no pintan «Peor» agresivo', () => {
    const c = SIN_CAMBIO();
    renderDetail(c, {}, evaluacionDe(c, 'sin_cambio'));
    const texto = document.body.textContent ?? '';
    expect(texto).not.toContain('rinde peor');
  });

  it('el título del veredicto es el registrado, sin recomponerlo con las cifras', () => {
    const c = EMPEORA();
    renderDetail(c, {}, evaluacionDe(c, 'empeora'));
    expect(document.body.textContent).toContain('Empeora');
  });

  it('muestra la confianza registrada como chip', () => {
    // El chip sale de la evaluación guardada, no de ninguna interpretación.
    const c = EMPEORA();
    renderDetail(c, {}, evaluacionDe(c, 'empeora'));
    expect(screen.getByText('Provisional')).toBeTruthy();
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
    renderDetail(EMPEORA(), {}, evaluacionDe(EMPEORA(), 'empeora'));

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
    renderDetail(c, {}, evaluacionDe(c, 'empeora')); // solo un lado lleva insignia

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

  // C3-21: el color deja de salir del signo del cambio y sale de la preferencia que
  // estableció Rust. Bajar el tiempo por imagen es mejorar, y eso **tiene** que verse
  // verde con el motor de acuerdo; el caso contrario (bajar los FPS) tiene que verse en
  // error aunque el signo sea positivo al revés. Las dos pruebas fijan la preferencia a
  // mano, así que no dependen de ningún umbral calculado en React.
  it('las métricas que mejoran al bajar llevan la preferencia del motor', () => {
    const c = cmp({ frame_time_avg: 20 }, { frame_time_avg: 16 });
    renderDetail(c, {}, evaluacionConMetricas(c, [
      { clave: 'frame_time_avg', preferencia: 'favor_de_candidata', apoyo: 9000 },
    ]));
    const cambio = celdas(/Tiempo por imagen/i)[2];
    // El -20 % es mejora porque el motor lo dijo, no porque el signo sea negativo.
    expect(cambio.className).toContain('font-semibold');
    expect(cambio.className).not.toContain('text-[var(--error)]');
    // La columna de evaluación nombra la grabación, no el código interno.
    const juicio = celdas(/Tiempo por imagen/i)[3];
    expect(juicio.textContent).toMatch(/Favorece a/);
    expect(juicio.textContent).not.toMatch(/favor_de_/);
    expect(juicio.className).toContain('compatible');
  });

  it('sin preferencia del motor, una mejora aritmética no se presenta como mejora', () => {
    // Contrapunto de la prueba anterior: mismo -20 %, pero el motor no juzgó la métrica.
    // Afirmar «mejora» aquí sería una preferencia inventada en React.
    const c = cmp({ frame_time_avg: 20 }, { frame_time_avg: 16 });
    renderDetail(c, {}, evaluacionConMetricas(c, [
      { clave: 'frame_time_avg', preferencia: null, apoyo: 9000 },
    ]));
    const celdasFila = celdas(/Tiempo por imagen/i);
    expect(celdasFila[2].className).not.toContain('compatible');
    expect(celdasFila[2].className).not.toContain('text-[var(--error)]');
    expect(celdasFila[2].className).not.toContain('font-semibold');
    expect(celdasFila[3].textContent).not.toMatch(/Favorece a/);
  });

  it('una métrica sin datos en una de las dos grabaciones muestra «—»', () => {
    renderDetail(
      cmp({ fps_0_1_percent_low: 0, avg_fps: 0 }, { fps_0_1_percent_low: 60, avg_fps: 100 })
    );
    // Sin FPS medios válidos la estabilidad no se puede calcular: no se inventa un 0.
    expect(celda(/Estabilidad/i, 0)).toBe('—');
  });

  it('el aviso del 0,1 % low aparece una sola vez en toda la pantalla', () => {
    // Grabación corta: es el único caso en que el aviso sale.
    renderDetail(cmp({ duration_seconds: 12, total_frames: 720 }, { duration_seconds: 12, total_frames: 720 }));
    const pantalla = document.body.textContent ?? '';
    expect(pantalla.match(/puede variar mucho con grabaciones cortas/gi)).toHaveLength(1);
  });

  it('con una grabación larga el aviso no aparece', () => {
    renderDetail(cmp());
    const pantalla = document.body.textContent ?? '';
    expect(pantalla.match(/puede variar mucho con grabaciones cortas/gi)).toBeNull();
  });

  it('la tabla mantiene la cabecera con los nombres de cada prueba', () => {
    renderDetail(cmp());
    const t = tabla();
    const cabeceras = Array.from(t.querySelectorAll('th')).map((th) => th.textContent);
    expect(cabeceras).toContain('Métrica');
    expect(cabeceras).toContain('Cambio');
  });
});




/**
 * Regresiones H3, H4, H7, H8 y H9 [C.3]: lo que se comprueba es el **valor** y el
 * **comportamiento**, no la presencia de una etiqueta explicativa.
 */
describe('estabilidad y preferencia no se derivan de las cifras', () => {
  it('H3: el índice de estabilidad es el suyo, no el 1 % low', () => {
    // 100 FPS con 70 / 60 → (2*0.7 + 0.6)/3*100 = 66.7
    // Con 90 / 80 → (2*0.9 + 0.8)/3*100 = 86.7
    renderDetail(
      cmp(
        { avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 },
        { avg_fps: 100, fps_1_percent_low: 90, fps_0_1_percent_low: 80 }
      )
    );
    const pantalla = document.body.textContent ?? '';
    // El índice se redondea a entero en la interfaz: 66.7 → 67 y 86.7 → 87.
    expect(pantalla).toContain('67');
    expect(pantalla).toContain('87');
    // …y el 1 % low se sigue viendo como 1 % low, con su propio número.
    expect(pantalla).toContain('70.0');
    expect(pantalla).toContain('90.0');
  });

  it('H8: la tarjeta de estabilidad no lleva chip Mejor/Peor', () => {
    // Mejora clarísima de estabilidad: si hubiera chip, saldría aquí.
    renderDetail(
      cmp(
        { avg_fps: 100, fps_1_percent_low: 30, fps_0_1_percent_low: 20 },
        { avg_fps: 100, fps_1_percent_low: 95, fps_0_1_percent_low: 90 }
      )
    );
    // La tarjeta de estabilidad se localiza por su encabezado.
    const tarjeta = screen.getByRole('heading', { name: 'Estabilidad' }).closest('div')!
      .parentElement!;
    expect(tarjeta.textContent).not.toMatch(/\b(Mejor|Peor)\b/);
    expect(tarjeta.textContent).not.toMatch(/a favor de/i);
  });

  it('H7: el índice de estabilidad no se presenta como fluidez alcanzada', () => {
    // 30 FPS constantes: el índice da 100, que **no** significa «va fluido».
    renderDetail(
      cmp(
        { avg_fps: 30, fps_1_percent_low: 30, fps_0_1_percent_low: 30 },
        { avg_fps: 30, fps_1_percent_low: 30, fps_0_1_percent_low: 30 }
      )
    );
    const pantalla = document.body.textContent ?? '';
    // El índice llega a su máximo…
    expect(pantalla).toContain('100');
    // …y precisamente por eso se niega la lectura de fluidez: la única mención permitida
    // es la que **desmiente** esa inferencia (la nota del índice), no una afirmación.
    const menciones = pantalla.match(/va fluido|es fluido|fluido/gi) ?? [];
    expect(menciones).toHaveLength(1);
    expect(pantalla).toMatch(/no «va fluido»/);
  });

  it('H4: ninguna fila histórica obtiene preferencia por el signo', () => {
    // Histórica (sin evaluación) con todas las cifras subiendo: aun así, sin preferencias.
    renderDetail(
      cmp({ avg_fps: 60, fps_1_percent_low: 40 }, { avg_fps: 90, fps_1_percent_low: 80 }),
      {},
      null
    );
    const tablas = screen.getAllByRole('table');
    const tabla = tablas[tablas.length - 1]!;
    const texto = tabla.textContent ?? '';
    expect(texto).not.toMatch(/a favor de la nueva|a favor de la anterior/i);
    expect(texto).not.toMatch(/sin preferencia establecida/i);
  });

  it('H4: tampoco en el panel principal de una histórica', () => {
    renderDetail(cmp({ avg_fps: 60 }, { avg_fps: 90 }), {}, null);
    const pantalla = document.body.textContent ?? '';
    // Sin evaluación no hay corona ni «mejor resultado observado» que nombre un ganador.
    expect(pantalla).not.toMatch(/Mejor resultado observado/i);
    expect(pantalla).not.toMatch(/Mejor resultado(?!$)/i);
  });
});
