import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { Benchmark, MuestrasTemporales } from '../lib/types';
import {
  GraficoTemporal, indicesDePicos, picosRecortados, reducirConservandoExtremos, rutaDeSerie,
  topeY, umbralDePico,
} from './GraficoTemporal';
import { SIN_DATOS_TEMPORALES } from '../lib/umbrales';
import { ANTES, DESPUES } from '../lib/etiquetas';

afterEach(cleanup);

function bench(over: Partial<Benchmark> = {}): Benchmark {
  return {
    id: 'b',
    profile_id: '',
    game_name: 'Juego',
    appid: 1,
    timestamp: '2026-11-02 14:43:52',
    duration_seconds: 60,
    avg_fps: 60,
    fps_1_percent_low: 40,
    fps_0_1_percent_low: 30,
    max_fps: 90,
    min_fps: 20,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 25,
    total_frames: 600,
    notes: null,
    launch_command: '',
    warmup_seconds: 0,
    ...over,
  };
}

const serie = (n: number, base = 16): MuestrasTemporales => ({
  frame_time_ms: Array.from({ length: n }, (_, i) => base + (i % 3)),
  elapsed_s: Array.from({ length: n }, (_, i) => i * 0.1),
});

describe('GraficoTemporal: pruebas antiguas sin serie', () => {
  it('muestra un mensaje discreto y ningún gráfico', () => {
    render(<GraficoTemporal comparacion={{ baseline: bench(), test: bench({ id: 'c' }) }} />);
    expect(screen.getByTestId('sin-datos-temporales')).toBeDefined();
    expect(screen.getByText(SIN_DATOS_TEMPORALES)).toBeDefined();
    expect(screen.queryByTestId('grafico-temporal')).toBeNull();
  });

  it('el mensaje no promete que los datos vayan a aparecer solos', () => {
    // Es importante no dar a entender que la app va a rellenar los datos.
    expect(SIN_DATOS_TEMPORALES).not.toMatch(/calcul|estimar|inferir/i);
  });

  it('C3-26: no atribuye la ausencia a una procedencia que el dato no prueba', () => {
    // El texto anterior afirmaba «se guardó antes de que Thoriapp almacenara los tiempos».
    // Faltar la serie también ocurre con registros importados o con logs sin eje temporal,
    // así que esa causa era una invención. El mensaje solo puede decir que no consta.
    expect(SIN_DATOS_TEMPORALES).not.toMatch(/antes de que/i);
    expect(SIN_DATOS_TEMPORALES).toMatch(/no tiene guardad/i);
  });

  it('una serie vacía también se trata como ausencia de datos', () => {
    const vacia: MuestrasTemporales = { frame_time_ms: [], elapsed_s: [] };
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: vacia }),
          test: bench({ id: 'c', muestras_temporales: vacia }),
        }}
      />
    );
    expect(screen.getByTestId('sin-datos-temporales')).toBeDefined();
  });
});

describe('GraficoTemporal: con datos reales', () => {
  it('dibuja una serie por grabación, en trazos separados', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: serie(60) }),
          test: bench({ id: 'c', muestras_temporales: serie(60, 12) }),
        }}
      />
    );
    expect(screen.getByTestId('grafico-temporal')).toBeDefined();
    expect(screen.queryByTestId('sin-datos-temporales')).toBeNull();
    // Dos `path`, uno por serie: no hay un trazo que mezcle las dos grabaciones.
    const paths = screen.getAllByTestId(/^pico-/);
    expect(paths).toHaveLength(2);
    expect(paths[0].getAttribute('stroke')).not.toBe(paths[1].getAttribute('stroke'));
  });

  it('la serie «Después» es discontinua, para no depender solo del color', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: serie(30) }),
          test: bench({ id: 'c', muestras_temporales: serie(30, 12) }),
        }}
      />
    );
    const [antes, despues] = screen.getAllByTestId(/^pico-/);
    expect(antes.getAttribute('stroke-dasharray')).toBeNull();
    expect(despues.getAttribute('stroke-dasharray')).not.toBeNull();
  });

  it('usa la misma escala Y para las dos series', () => {
    // Una grabación con frametimes 4 veces mayores: si cada serie se escalara sola, la
    // mala parecería igual de plana que la buena.
    const lenta = serie(10, 64);
    const rapida = serie(10, 16);
    const { container } = render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: rapida }),
          test: bench({ id: 'c', muestras_temporales: lenta }),
        }}
      />
    );
    const paths = Array.from(container.querySelectorAll('path'));
    // El trazo rápido queda arriba (y pequeño) y el lento abajo (y alto) en el viewBox.
    const ys = paths.map((p) => p.getAttribute('d') ?? '');
    expect(ys[0]).not.toBe(ys[1]);
    // Mismo viewBox → misma escala.
    const svgs = container.querySelectorAll('svg');
    for (const svg of Array.from(svgs)) {
      expect(svg.getAttribute('viewBox')).toBe('0 0 600 180');
    }
  });

  it('con tiempos reales el eje X son segundos', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: serie(100) }),
          test: bench({ id: 'c', muestras_temporales: serie(100, 12) }),
        }}
      />
    );
    // El eje se rotula con el número y la unidad («0 s … 9.9 s»), en un `div` aparte.
    const texto = screen.getByTestId('grafico-temporal').textContent ?? '';
    // El eje X aparece como «0 s» al principio y «9.9 s» al final (sin espacios): con
    // tiempos reales no se dice «muestra» en el eje.
    // El eje X se rotula «0 s … 9.9 s». Se comprueba el rótulo renderizado.
    expect(texto).toContain('0 s');
    expect(texto).toContain('9.9 s');
  });

  it('sin columna elapsed, el eje X se llama «muestra», no «segundos»', () => {
    const sinTiempos: MuestrasTemporales = {
      frame_time_ms: Array.from({ length: 50 }, () => 16),
      elapsed_s: [],
    };
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: sinTiempos }),
          test: bench({ id: 'c', muestras_temporales: sinTiempos }),
        }}
      />
    );
    const texto = screen.getByTestId('grafico-temporal').textContent ?? '';
    expect(texto).toMatch(/muestra/);
    // No se inventa un eje de segundos.
    expect(texto).not.toMatch(/\b0 s\b/);
  });

  it('identifica picos sin atribuirles una causa', () => {
    const conPico: MuestrasTemporales = {
      frame_time_ms: [16, 16, 16, 16, 200, 16, 16],
      elapsed_s: [0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
    };
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: conPico }),
          test: bench({ id: 'c', muestras_temporales: serie(7) }),
        }}
      />
    );
    const texto = screen.getByTestId('grafico-temporal').textContent ?? '';
    expect(texto).toMatch(/pico/i);
    // Prohibido culpar a nada: el log no lo dice.
    expect(texto).not.toMatch(/shader|compilaci|GPU|CPU|porque|debido a/i);
  });

  it('con una sola serie dibuja solo esa, sin romperse', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: serie(20) }),
          test: bench({ id: 'c' }),
        }}
      />
    );
    expect(screen.getByTestId('grafico-temporal')).toBeDefined();
    expect(screen.getAllByTestId(/^pico-/)).toHaveLength(1);
  });

  it('los nombres de las series son los mismos del resto de la app', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: serie(20) }),
          test: bench({ id: 'c', muestras_temporales: serie(20, 12) }),
        }}
      />
    );
    const texto = screen.getByTestId('grafico-temporal').textContent ?? '';
    expect(texto).toContain(ANTES);
    expect(texto).toContain(DESPUES);
  });
});

describe('umbralDePico e indicesDePicos', () => {
  it('el doble de la mediana', () => {
    expect(umbralDePico([10, 10, 10, 10, 50])).toBe(20);
  });

  it('sin valores no hay umbral alcanzable', () => {
    expect(umbralDePico([])).toBe(Infinity);
    expect(indicesDePicos([], Infinity)).toEqual([]);
  });

  it('señala solo los valores por encima', () => {
    expect(indicesDePicos([10, 25, 10, 40], 20)).toEqual([1, 3]);
  });

  it('una serie plana no tiene picos', () => {
    expect(indicesDePicos([16, 16, 16], umbralDePico([16, 16, 16]))).toEqual([]);
  });
});

describe('rutaDeSerie', () => {
  const s = (valores: number[], tiempos: number[]): Parameters<typeof rutaDeSerie>[0] => ({
    nombre: 'x',
    color: '#fff',
    valores,
    tiempos,
    muestras: valores.length,
  });

  it('sin datos no produce ruta', () => {
    expect(rutaDeSerie(s([], []), 100, 50, 10, 10)).toBe('');
  });

  it('empieza en M y continúa con L', () => {
    const d = rutaDeSerie(s([10, 20], [0, 1]), 100, 50, 20, 1);
    expect(d.startsWith('M')).toBe(true);
    expect(d).toContain('L');
    expect(d.split(' ')).toHaveLength(2);
  });

  it('el valor máximo toca el borde superior', () => {
    const d = rutaDeSerie(s([10], [0]), 100, 50, 10, 1);
    // y = alto - (v/maxY)*alto = 0
    expect(d).toBe('M0.00,0.00');
  });

  it('no dibuja fuera del área con valores desbocados', () => {
    const d = rutaDeSerie(s([999], [0]), 100, 50, 10, 1);
    expect(d).toBe('M0.00,0.00');
  });
});

describe('GraficoTemporal: selector de escala y recortes', () => {
  const conPicos: MuestrasTemporales = {
    frame_time_ms: [...Array.from({ length: 50 }, () => 16), 400, ...Array.from({ length: 20 }, () => 17)],
    elapsed_s: Array.from({ length: 71 }, (_, i) => i * 0.1),
  };

  it('ofrece las dos escalas y arranca en «Detalle habitual»', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: conPicos }),
          test: bench({ id: 'c', muestras_temporales: serie(71) }),
        }}
      />
    );
    const detalle = screen.getByRole('button', { name: 'Detalle habitual' });
    const picos = screen.getByRole('button', { name: 'Todos los picos' });
    expect(detalle.getAttribute('aria-pressed')).toBe('true');
    expect(picos.getAttribute('aria-pressed')).toBe('false');
  });

  it('al pasar a «Todos los picos» deja de recortar', () => {
    render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: conPicos }),
          test: bench({ id: 'c', muestras_temporales: serie(71) }),
        }}
      />
    );
    // En detalle el tirón de 400 ms se sale del tope y se marca.
    expect(screen.getAllByTestId(/^recortado-/).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: 'Todos los picos' }));
    expect(screen.queryAllByTestId(/^recortado-/)).toHaveLength(0);
  });

  it('la escala es compartida por las dos series dentro de cada modo', () => {
    const { container } = render(
      <GraficoTemporal
        comparacion={{
          baseline: bench({ muestras_temporales: conPicos }),
          test: bench({ id: 'c', muestras_temporales: serie(71, 40) }),
        }}
      />
    );
    const viewBoxs = Array.from(container.querySelectorAll('svg')).map((s) => s.getAttribute('viewBox'));
    expect(viewBoxs).toHaveLength(2);
    expect(viewBoxs[0]).toBe(viewBoxs[1]);
  });
});

describe('GraficoTemporal: funciones de recorte', () => {
  it('topeY en «picos» usa el máximo real', () => {
    expect(topeY([10, 20, 500], 'picos')).toBeCloseTo(525, 5);
  });

  it('topeY en «detalle» recorta un pico aislado y deja aire al cuerpo', () => {
    const valores = [...Array.from({ length: 100 }, () => 16), 900];
    const tope = topeY(valores, 'detalle');
    expect(tope).toBeLessThan(900);
    // Margen mínimo: el doble de la mediana, para que el cuerpo del trazo no quede plano.
    expect(tope).toBeGreaterThanOrEqual(16 * 2);
    expect(picosRecortados(valores, tope)).toEqual([100]);
  });

  it('sin picos, «detalle» no recorta nada', () => {
    const valores = Array.from({ length: 50 }, (_, i) => 16 + (i % 3));
    expect(picosRecortados(valores, topeY(valores, 'detalle'))).toEqual([]);
  });

  it('reducir conserva los extremos de cada tramo', () => {
    const valores = Array.from({ length: 1000 }, _ => 16);
    valores[123] = 500; // pico escondido en medio
    valores[777] = 2; // mínimo escondido
    const reducida = reducirConservandoExtremos(valores, 100);
    const indices = reducida.map((p) => p.indice);
    expect(indices).toContain(123);
    expect(indices).toContain(777);
    expect(reducida.length).toBeLessThanOrEqual(110);
    // Se mantiene el orden temporal.
    expect([...indices].sort((a, b) => a - b)).toEqual(indices);
  });

  it('una serie corta no se reduce', () => {
    const valores = [1, 2, 3];
    expect(reducirConservandoExtremos(valores, 400)).toEqual([
      { indice: 0, valor: 1 },
      { indice: 1, valor: 2 },
      { indice: 2, valor: 3 },
    ]);
  });
});
