import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Benchmark, BenchmarkComparison } from '../lib/types';
import { SavedComparisonDetail, diffComandos, tokenizarComando } from './SavedComparisonDetail';

afterEach(cleanup);

/**
 * Regresión del fallo de Palworld.
 *
 * Una comparación guardada mostraba `%command%` cuando el comando real era
 * `mesa_glthread=true game-performance mangohud %command%`. Había tres causas
 * encadenadas:
 *
 * 1. El guardado manual (`analyzeLog`) no pasaba la instantánea del comando, así que se
 *    usaba el comando "actual" en el momento de guardar.
 * 2. `launchLine()` devuelve `%command%` cuando no hay ajustes activos, y ese valor se
 *    guardaba como si fuera una observación. Es legítimo («sin parámetros»), pero se
 *    confundía con «no registrado».
 * 3. `diffComandos('', '')` devolvía `iguales: true`, así que dos pruebas sin comando
 *    mostraban «sin cambios». Y dos comandos con los mismos tokens en distinto orden
 *    también.
 */

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
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
    warmup_seconds: 0,
    ...over,
  };
}

function cmp(antes: Partial<Benchmark>, despues: Partial<Benchmark>): BenchmarkComparison {
  return {
    baseline: bench({ id: 'a', ...antes }),
    test: bench({ id: 'z', ...despues }),
    fps_change_percent: 0,
    fps_1_percent_change_percent: 0,
    fps_0_1_percent_change_percent: 0,
    frame_time_change_percent: 0,
    conclusion: '',
    noise_percent: 5,
    within_noise: false,
  } as BenchmarkComparison;
}

const propsVista = { onBack: () => {}, onDelete: () => {}, onCopy: () => {} };

/**
 * Monta la vista con «Configuración comparada» ya abierta.
 *
 * Desde el rediseño VS esa sección está plegada por defecto, así que las aserciones sobre
 * comandos tienen que desplegarla. Se hace aquí y no en cada test para no repetirlo.
 */
function renderDetail(c: BenchmarkComparison) {
  const r = render(<SavedComparisonDetail comparison={c} {...propsVista} />);
  const btn = screen.getByRole('button', { name: /Configuración comparada/i });
  if (btn.getAttribute('aria-expanded') !== 'true') fireEvent.click(btn);
  return r;
}

const textoDe = (c: BenchmarkComparison) => {
  const { container, unmount } = renderDetail(c);
  const t = container.textContent ?? '';
  unmount();
  return t;
};

const COMANDO_PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

describe('el comando se muestra completo y sin tocar', () => {
  it('conserva variables de entorno y wrappers en el orden original', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toContain(COMANDO_PALWORLD);
    // No se reordena ni se recorta: la variable de entorno va primero, como se escribió.
    expect(t.indexOf('mesa_glthread=true')).toBeLessThan(t.indexOf('game-performance'));
    expect(t.indexOf('game-performance')).toBeLessThan(t.indexOf('mangohud'));
  });

  it('conserva comillas y espacios internos', () => {
    const c = `DXVK_HUD="fps,frametimes" mangohud --dlsym %command% -novid`;
    const t = textoDe(cmp({ launch_command: c }, { launch_command: 'mangohud %command%' }));
    expect(t).toContain('DXVK_HUD="fps,frametimes"');
    expect(t).toContain('-novid');
  });

  it('la caja de cada prueba muestra su propio comando, no el mismo repetido', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toContain('%command%');
    expect(t).toContain(COMANDO_PALWORLD);
  });

  it('no elimina mangohud ni game-performance del texto mostrado', () => {
    const t = textoDe(cmp({ launch_command: 'gamemoderun %command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toContain('mangohud');
    expect(t).toContain('game-performance');
  });
});

describe('comparación de comandos', () => {
  it('dos comandos distintos muestran los dos completos', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toMatch(/El comando registrado cambió/);
    // No se afirma «sin cambios» cuando sí los hay: es el fallo original.
    expect(t).not.toContain('Sin cambios');
  });

  it('dos comandos idénticos dicen «sin cambios» una sola vez', () => {
    const t = textoDe(cmp({ launch_command: COMANDO_PALWORLD }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toContain('Sin cambios');
    expect(t).not.toMatch(/El comando registrado cambió/);
  });

  it('%command% contra %command% se describe como «sin parámetros», no como cambio', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: '%command%' }));
    expect(t).toContain('Sin parámetros en las dos');
  });

  it('una diferencia de comando NO se presenta como causa de la diferencia de FPS', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toMatch(/no demuestra que sea la causa/);
    expect(t).not.toMatch(/gracias a|debido a|l[a|o]s? .*caus(ó|a) la mejora/i);
  });

  it('«%command%» es un valor registrado, no un hueco: no dice «no registrado»', () => {
    const t = textoDe(cmp({ launch_command: '%command%' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).not.toContain('Comando no registrado');
  });
});

describe('cambios de configuración posteriores', () => {
  it('cada prueba conserva su instantánea aunque la comparación se haga después', () => {
    // Los dos benchmarks llevan su propio comando: la comparación no los reconstruye
    // desde una configuración "actual", que aquí no existe.
    const t = textoDe(
      cmp(
        { launch_command: 'gamemoderun mangohud %command%' },
        { launch_command: COMANDO_PALWORLD }
      )
    );
    expect(t).toContain('gamemoderun mangohud %command%');
    expect(t).toContain(COMANDO_PALWORLD);
  });
});

describe('pruebas antiguas sin comando', () => {
  it('muestra «Comando no registrado» y no afirma nada', () => {
    const t = textoDe(cmp({ launch_command: '' }, { launch_command: '' }));
    expect(t).toContain('Comando no registrado');
    expect(t).toMatch(/No se puede decir si cambió algo/);
    // Lo importante: NO se asume %command% ni «sin cambios».
    expect(t).not.toContain('Sin cambios');
    expect(t).not.toContain('Sin parámetros en las dos');
  });

  it('si solo falta uno, se identifica cuál', () => {
    const t = textoDe(cmp({ launch_command: '' }, { launch_command: COMANDO_PALWORLD }));
    expect(t).toContain('Comando no registrado');
    expect(t).toMatch(/«Antes» no guardó el comando/);
    // El que sí está registrado se sigue viendo.
    expect(t).toContain(COMANDO_PALWORLD);
  });

  it('no se inventa un comando para la prueba antigua', () => {
    const t = textoDe(cmp({ launch_command: '' }, { launch_command: COMANDO_PALWORLD }));
    // La caja vacía está vacía: no se rellena con %command%.
    expect(t).not.toMatch(/%command%\s*mangohud\s*game-performance/);
  });
});

describe('diffComandos: igualdad por texto completo', () => {
  it('dos vacíos no son iguales', () => {
    const d = diffComandos('', '');
    expect(d.iguales).toBe(false);
    expect(d.comparable).toBe(false);
  });

  it('un vacío no es comparable aunque el otro exista', () => {
    expect(diffComandos('', 'mangohud %command%').comparable).toBe(false);
    expect(diffComandos('mangohud %command%', '   ').comparable).toBe(false);
  });

  it('texto idéntico tras recortar espacios es igual', () => {
    const d = diffComandos('  mangohud %command%  ', 'mangohud %command%');
    expect(d.iguales).toBe(true);
    expect(d.comparable).toBe(true);
  });

  it('los mismos tokens en distinto orden NO son iguales', () => {
    // Antes `diffComandos` comparaba tokens y esto era «sin cambios».
    const d = diffComandos('--foo=1 --bar=2 %command%', '--bar=2 --foo=1 %command%');
    expect(d.iguales).toBe(false);
    expect(d.comparable).toBe(true);
  });

  it('un wrappers de más se detecta', () => {
    const d = diffComandos('%command%', COMANDO_PALWORLD);
    expect(d.iguales).toBe(false);
    expect(d.añadidos).toContain('mesa_glthread=true');
    expect(d.añadidos).toContain('game-performance');
  });

  it('el caso del fallo: vacío contra comando real no dice «iguales»', () => {
    // El vacío es lo que quedaba en las pruebas antiguas; el comando real es el de
    // Palworld. Antes esto podía acabar en «sin cambios».
    expect(diffComandos('', COMANDO_PALWORLD).iguales).toBe(false);
  });
});

describe('tokenizarComando respeta comillas', () => {
  it('agrupa lo entrecomillado', () => {
    expect(tokenizarComando('VAR="a b" mangohud %command%')).toEqual(['VAR=a b', 'mangohud']);
  });

  it('respeta comillas simples', () => {
    expect(tokenizarComando("PROTON_LOG='x y' %command%")).toEqual(['PROTON_LOG=x y']);
  });

  it('no se usa para decidir igualdad: es solo para resaltar', () => {
    // Documenta la separación: tokenizar pierde el orden y las comillas.
    const d = diffComandos('VAR="a b" %command%', 'VAR="a  b" %command%');
    expect(d.iguales).toBe(false);
  });
});
