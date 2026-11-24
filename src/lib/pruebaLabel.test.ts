import { describe, expect, it } from 'vitest';
import { etiquetasDe, interpretar } from './interpretacion';
import { comandoPegable, etiquetaPrueba, textoComando } from './pruebaLabel';
import type { Benchmark, BenchmarkComparison } from './types';

/**
 * Numeración de pruebas («Prueba N»).
 *
 * El número lo asigna el backend al guardar y no cambia nunca; aquí se comprueba que la
 * interfaz lo usa de forma coherente y que **no inventa** nada cuando falta. Los casos de
 * asignación (consecutivo, no reutilizar tras borrar, contador persistente) se prueban en
 * Rust, que es donde vive el estado; este fichero cubre su presentación.
 */

const bench = (extra: Partial<Benchmark> = {}): Benchmark =>
  ({
    id: 'b1',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-23 15:10:13',
    duration_seconds: 30,
    avg_fps: 74.3,
    fps_1_percent_low: 50,
    fps_0_1_percent_low: 45,
    max_fps: 80,
    min_fps: 30,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 20,
    total_frames: 300,
    notes: null,
    launch_command: '%command%',
    origen_comando: 'instantanea',
    ...extra,
  }) as Benchmark;

describe('etiquetaPrueba', () => {
  it('usa el número de la grabación', () => {
    expect(etiquetaPrueba(bench({ number: 2 }))).toBe('Prueba 2');
  });

  it('dice «Prueba sin número» en grabaciones antiguas, sin inventar uno', () => {
    // Grabación guardada antes de que existiera el campo: no hay cronología fiable.
    expect(etiquetaPrueba(bench({ number: null }))).toBe('Prueba sin número');
    expect(etiquetaPrueba(bench())).toBe('Prueba sin número');
  });

  it('no considera número un 0 o un valor inválido', () => {
    expect(etiquetaPrueba(bench({ number: 0 }))).toBe('Prueba sin número');
    expect(etiquetaPrueba(bench({ number: Number.NaN }))).toBe('Prueba sin número');
  });

  it('no depende del id interno ni de la fecha', () => {
    const a = etiquetaPrueba(bench({ number: 3, id: 'bench_20260101', timestamp: '2026-01-01 10:00:00' }));
    const b = etiquetaPrueba(bench({ number: 3, id: 'otro', timestamp: '2026-05-05 10:00:00' }));
    expect(a).toBe(b);
    expect(a).toBe('Prueba 3');
  });
});

describe('etiquetasDe', () => {
  it('traduce las dos grabaciones de una comparación', () => {
    const et = etiquetasDe({ baseline: bench({ number: 1 }), test: bench({ number: 2 }) });
    expect(et).toEqual({ antes: 'Prueba 1', despues: 'Prueba 2' });
  });

  it('mantiene Antes/Después en comparaciones antiguas sin número', () => {
    const et = etiquetasDe({ baseline: bench(), test: bench() });
    expect(et).toEqual({ antes: 'Antes', despues: 'Después' });
  });

  it('mezcla: una con número y otra sin él', () => {
    const et = etiquetasDe({ baseline: bench(), test: bench({ number: 5 }) });
    expect(et).toEqual({ antes: 'Antes', despues: 'Prueba 5' });
  });
});

/**
 * Comparación con la forma que consume `interpretar`.
 *
 * Se rellena igual que en `interpretacion.test.ts`: el intérprete deriva los cambios de
 * los propios benchmarks, no de campos precalculados, así que los `*_change_percent` van
 * a cero a propósito para no sugerir que se usan.
 */
const comparacion = (baseline: Benchmark, test: Benchmark, noise = 5): BenchmarkComparison =>
  ({
    baseline,
    test,
    fps_change_percent: 0,
    fps_1_percent_change_percent: 0,
    fps_0_1_percent_change_percent: 0,
    frame_time_change_percent: 0,
    conclusion: '',
    noise_percent: noise,
    within_noise: false,
  }) as unknown as BenchmarkComparison;

describe('interpretar con números de prueba', () => {
  it('el ganador se nombra con el número real de la grabación', () => {
    // Sube el promedio Y los peores momentos: mejora clara de la segunda grabación.
    const c = comparacion(
      bench({ number: 1, avg_fps: 74.3, fps_1_percent_low: 50, fps_0_1_percent_low: 45 }),
      bench({ number: 2, avg_fps: 100.7, fps_1_percent_low: 70, fps_0_1_percent_low: 62 })
    );
    const r = interpretar(c, etiquetasDe(c));
    expect(r.ganador).toBe('despues');
    expect(r.titulo).toContain('Prueba 2');
    expect(r.titulo).not.toContain('Después');
  });

  it('el número del ganador corresponde a la grabación que de verdad gana', () => {
    // Aquí empeora: gana la de referencia, que es la Prueba 7.
    const c = comparacion(
      bench({ number: 7, avg_fps: 120, fps_1_percent_low: 90, fps_0_1_percent_low: 85 }),
      bench({ number: 8, avg_fps: 60, fps_1_percent_low: 40, fps_0_1_percent_low: 35 })
    );
    const r = interpretar(c, etiquetasDe(c));
    expect(r.ganador).toBe('antes');
    expect(r.titulo).toContain('Prueba 7');
    expect(r.titulo).not.toContain('Prueba 8');
  });

  it('sin números se sigue diciendo Antes/Después', () => {
    const c = comparacion(
      bench({ avg_fps: 74.3, fps_1_percent_low: 50, fps_0_1_percent_low: 45 }),
      bench({ avg_fps: 100.7, fps_1_percent_low: 70, fps_0_1_percent_low: 62 })
    );
    const r = interpretar(c, etiquetasDe(c));
    expect(r.titulo).toContain('Después');
  });

  it('sin etiquetas explícitas no revienta y deriva las etiquetas del número', () => {
    const c = comparacion(
      bench({ number: 1, avg_fps: 74.3, fps_1_percent_low: 50, fps_0_1_percent_low: 45 }),
      bench({ number: 2, avg_fps: 100.7, fps_1_percent_low: 70, fps_0_1_percent_low: 62 })
    );
    const r = interpretar(c);
    expect(r.titulo).toContain('Prueba 2');
  });

  it('los criterios no cambian: diferencia dentro del margen sigue sin concluir', () => {
    // 0,5 % de diferencia con ruido 5 %: no se declara mejora.
    const c = comparacion(
      bench({ number: 1, avg_fps: 100, fps_1_percent_low: 70, fps_0_1_percent_low: 60 }),
      bench({ number: 2, avg_fps: 100.5, fps_1_percent_low: 70.3, fps_0_1_percent_low: 60.3 })
    );
    const r = interpretar(c, etiquetasDe(c));
    expect(r.ganador).not.toBe('despues');
  });
});

describe('textoComando en el tooltip', () => {
  it('prefiere lo que Steam tenía configurado', () => {
    const b = bench({ steam_command: 'mesa_glthread=true mangohud %command%' });
    expect(textoComando(b)).toBe('Steam: mesa_glthread=true mangohud %command%');
  });

  it('conserva comillas, espacios y variables de entorno tal cual', () => {
    const cmd = `PROTON_ENABLE_NVAPI=1 gamemoderun mangohud %command% -dx11 --arg="valor con espacios"`;
    expect(textoComando(bench({ steam_command: cmd }))).toBe(`Steam: ${cmd}`);
  });

  it('no recorta comandos largos', () => {
    const largo = `A=1 ${'B=2 '.repeat(60)}%command%`;
    const out = textoComando(bench({ steam_command: largo }));
    expect(out).toBe(`Steam: ${largo}`);
    expect(out!.length).toBeGreaterThan(200);
  });

  it('Steam leído y vacío es un dato, no un comando', () => {
    expect(textoComando(bench({ steam_command: '' }))).toBe('Steam: sin opciones de lanzamiento');
  });

  it('cae al comando del editor, marcando su origen', () => {
    const b = bench({ steam_command: null, launch_command: 'mangohud %command%', origen_comando: 'comando_actual' });
    expect(textoComando(b)).toBe('Comando: mangohud %command% (leído al guardar, no al grabar)');
  });

  it('sin ningún dato devuelve null (la UI dirá «Comando no registrado»)', () => {
    expect(textoComando(bench({ steam_command: null, launch_command: '' }))).toBeNull();
    expect(textoComando(null)).toBeNull();
  });

  it('nunca inventa %command% cuando no hay dato', () => {
    expect(textoComando(bench({ steam_command: null, launch_command: '' }))).not.toBe('%command%');
  });
});

/**
 * Comando **pegable**: lo que se lleva el botón de copiar.
 *
 * Tiene que servir para pegarlo tal cual en Steam. Un prefijo («Comando: ») o la marca de
 * origen («(origen desconocido)») romperían el lanzamiento.
 */
describe('comandoPegable', () => {
  it('devuelve el comando del editor sin prefijo ni marca de origen', () => {
    const b = bench({ launch_command: 'mangohud gamemoderun %command%', steam_command: null });
    expect(comandoPegable(b)).toBe('mangohud gamemoderun %command%');
  });

  it('prefiere Steam: es lo que de verdad se ejecutó al entrar al juego', () => {
    const b = bench({ launch_command: '%command%', steam_command: 'DXVK_HUD=1 %command%' });
    expect(comandoPegable(b)).toBe('DXVK_HUD=1 %command%');
  });

  it('no arrastra la marca de origen de las pruebas antiguas', () => {
    const b = bench({
      launch_command: 'mangohud %command%',
      steam_command: null,
      origen_comando: undefined,
    });
    expect(comandoPegable(b)).toBe('mangohud %command%');
    expect(comandoPegable(b)).not.toContain('origen');
  });

  it('devuelve null si no hay comando: nunca inventa %command%', () => {
    expect(comandoPegable(bench({ launch_command: '', steam_command: null }))).toBeNull();
  });

  it('devuelve null si Steam se leyó vacío', () => {
    // Se lanzó sin ajustes; no hay nada que pegar.
    expect(comandoPegable(bench({ launch_command: 'algo %command%', steam_command: '' }))).toBeNull();
  });

  it('devuelve null sin grabación', () => {
    expect(comandoPegable(null)).toBeNull();
  });
});
