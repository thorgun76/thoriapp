import { useState } from 'react';
import type { Benchmark } from '../lib/types';
import { SIN_DATOS_TEMPORALES } from '../lib/umbrales';
import { ANTES, DESPUES } from '../lib/interpretacion';

/**
 * Gráfico del frametime (ms por muestra) de las dos grabaciones.
 *
 * ## Qué dibuja exactamente
 *
 * Cada punto es el frametime **medio de una muestra** del log de MangoHud, no un frame
 * individual. Con `log_interval=100` hay una muestra cada 100 ms, así que el gráfico es
 * una serie de ~300-600 puntos: se ven las tendencias y los picos que duran décimas de
 * segundo, pero no un micro-tirón de 2 ms.
 *
 * ## Lo que este gráfico NO hace
 *
 * - **No une las dos series.** Antes y Después son dos trazos separados, uno al lado del
 *   otro, no superpuestos. Están en ejes distintos porque son dos partidas distintas: no
 *   comparten una línea temporal y dibujarlas encima sugeriría que el segundo 12 de una es
 *   el mismo momento que el segundo 12 de la otra. No lo es.
 * - **No marca causas.** Un pico se señala como pico; no se dice que sea por shaders, por
 *   otra GPU o por la IA del juego, porque el log no lo dice.
 * - **No suaviza.** Una media móvil escondería justo lo que se busca.
 *
 * Los colores son los mismos que en el resto de la aplicación (cian = Antes, ámbar =
 * Después) y la forma del trazo además de la etiqueta identifica cada serie, para que no
 * dependa del color.
 */

/** Serie con los datos que se pueden dibujar. */
interface Serie {
  nombre: string;
  color: string;
  /** Guion largo para distinguir sin depender del color. */
  dash?: string;
  valores: number[];
  /** Tiempo en segundos de cada muestra; vacío si el log no lo traía. */
  tiempos: number[];
  /** Muestras del log (lo que `total_frames` contiene de verdad). */
  muestras: number;
}

const COLOR_ANTES = '#38bdf8';
const COLOR_DESPUES = '#f59e0b';

/** Un pico se define como un valor por encima del doble de la mediana. */
export function umbralDePico(valores: number[]): number {
  if (valores.length === 0) return Infinity;
  const orden = [...valores].sort((a, b) => a - b);
  const mediana = orden[Math.floor(orden.length / 2)];
  return mediana * 2;
}

/** Índices de los valores que superan `umbral`. */
export function indicesDePicos(valores: number[], umbral: number): number[] {
  return valores.reduce<number[]>((acc, v, i) => (v > umbral ? [...acc, i] : acc), []);
}


/**
 * Escala vertical del gráfico.
 *
 * - `detalle` recorta los picos extremos para que el «cuerpo» del trazo ocupe el alto
 *   disponible: un solo tirón de 300 ms aplastaba los 60 s buenos en una línea plana.
 * - `picos` usa el máximo real, para ver hasta dónde llegó el peor momento.
 *
 * En los dos casos los picos recortados se MARCAN: recortar sin avisar sería mentir sobre
 * la forma de la serie.
 */
export type Escala = 'detalle' | 'picos';

/** Percentil simple (interpolación lineal) sobre una copia ordenada. */
export function percentil(valores: number[], p: number): number {
  if (valores.length === 0) return 0;
  const orden = [...valores].sort((a, b) => a - b);
  const pos = (orden.length - 1) * Math.min(1, Math.max(0, p));
  const bajo = Math.floor(pos);
  const alto = Math.ceil(pos);
  if (bajo === alto) return orden[bajo];
  return orden[bajo] + (orden[alto] - orden[bajo]) * (pos - bajo);
}

/**
 * Tope del eje Y para cada escala.
 *
 * En `detalle` se usa el percentil 99 con un 10 % de aire, pero nunca por debajo del doble
 * de la mediana: si no hay picos, el tope se queda en el máximo real y no se recorta nada.
 */
export function topeY(valores: number[], escala: Escala): number {
  if (valores.length === 0) return 1;
  const max = Math.max(...valores);
  if (escala === 'picos') return max * 1.05;
  const mediana = percentil(valores, 0.5);
  const cuerpo = Math.max(percentil(valores, 0.99) * 1.1, mediana * 2);
  return Math.max(cuerpo, 1) >= max ? max * 1.05 : cuerpo;
}

/** Índices de las muestras que quedan fuera del tope (picos recortados). */
export function picosRecortados(valores: number[], tope: number): number[] {
  return valores.reduce<number[]>((acc, v, i) => (v > tope ? [...acc, i] : acc), []);
}

/**
 * Reduce la serie a `maximo` puntos conservando SIEMPRE los mínimos y máximos de cada
 * tramo, que son los picos que el usuario viene a ver. Una media móvil los borraría.
 *
 * Cada punto resultante recuerda su índice original, para poder marcar los picos.
 */
export function reducirConservandoExtremos(
  valores: number[],
  maximo = 400
): { indice: number; valor: number }[] {
  const n = valores.length;
  if (n <= maximo) return valores.map((valor, indice) => ({ indice, valor }));

  const salida: { indice: number; valor: number }[] = [];
  // Cada tramo aporta su mínimo y su máximo, en orden temporal.
  const tramo = Math.ceil(n / Math.ceil(maximo / 2));
  for (let i = 0; i < n; i += tramo) {
    const fin = Math.min(n, i + tramo);
    if (fin - i === 1) {
      salida.push({ indice: i, valor: valores[i] });
      continue;
    }
    let iMin = i;
    let iMax = i;
    for (let j = i + 1; j < fin; j++) {
      if (valores[j] < valores[iMin]) iMin = j;
      if (valores[j] > valores[iMax]) iMax = j;
    }
    const primero = Math.min(iMin, iMax);
    const segundo = Math.max(iMin, iMax);
    salida.push({ indice: primero, valor: valores[primero] });
    salida.push({ indice: segundo, valor: valores[segundo] });
  }
  return salida;
}

function serieDe(b: Benchmark, nombre: string, color: string): Serie | null {
  const m = b.muestras_temporales;
  if (!m || m.frame_time_ms.length === 0) return null;
  return {
    nombre,
    color,
    valores: m.frame_time_ms,
    tiempos: m.elapsed_s ?? [],
    muestras: b.sample_count ?? b.total_frames,
  };
}

/**
 * Ruta `d` de un trazo, con la geometría ya normalizada.
 *
 * Si hay tiempos reales se usan para el eje X. Si no, se usa el índice de la muestra y el
 * eje se etiqueta «muestra», no «segundos»: poner segundos sin tenerlos sería inventar un
 * eje que el log no dio.
 */
export function rutaDeSerie(
  serie: Serie,
  ancho: number,
  alto: number,
  maxY: number,
  maxX: number,
  puntos?: { indice: number; valor: number }[]
): string {
  const n = serie.valores.length;
  if (n === 0 || maxY <= 0 || maxX <= 0) return '';
  const total = Math.max(1, n - 1);
  const x = (i: number) =>
    serie.tiempos.length === n ? (serie.tiempos[i] / maxX) * ancho : (i / total) * ancho;
  const y = (v: number) => alto - (Math.max(0, Math.min(v, maxY)) / maxY) * alto;
  // Con la serie reducida se dibuja lo que queda; sin reducir, todas las muestras.
  const lista = puntos ?? serie.valores.map((valor, indice) => ({ indice, valor }));
  return lista
    .map((p, k) => `${k === 0 ? 'M' : 'L'}${x(p.indice).toFixed(2)},${y(p.valor).toFixed(2)}`)
    .join(' ');
}

/** Ancho y alto del área de dibujo en unidades del `viewBox`. */
const ANCHO = 600;
const ALTO = 180;

/** Texto discreto para las pruebas guardadas antes de que existiera este campo. */
export function GraficoTemporal({
  comparacion,
  etiquetas,
}: {
  comparacion: { baseline: Benchmark; test: Benchmark };
  /**
   * Nombres de las dos series: «Prueba 1» / «Prueba 2», o «Antes» / «Después» en las
   * grabaciones anteriores a la numeración. La leyenda y las etiquetas de accesibilidad
   * usan esto, no las constantes, para que el gráfico diga lo mismo que el resto.
   */
  etiquetas?: { antes: string; despues: string };
}) {
  const et = etiquetas ?? { antes: ANTES, despues: DESPUES };
  const [escala, setEscala] = useState<Escala>('detalle');
  const antes = serieDe(comparacion.baseline, et.antes, COLOR_ANTES);
  const despues = serieDe(comparacion.test, et.despues, COLOR_DESPUES);

  if (!antes && !despues) {
    return (
      <p className="mt-3 text-[11px] text-[var(--text-muted)] leading-relaxed" data-testid="sin-datos-temporales">
        {SIN_DATOS_TEMPORALES}
      </p>
    );
  }

  const series = [antes, despues].filter((s): s is Serie => s !== null);

  // Misma escala Y para los dos trazos: si cada uno se escalara solo, el que va peor
  // parecería igual de plano y la comparación sería engañosa.
  const todos = series.flatMap((s) => s.valores);
  const maxY = topeY(todos, escala);
  const maxX = Math.max(
    1,
    ...series.map((s) => (s.tiempos.length ? s.tiempos[s.tiempos.length - 1] : s.valores.length - 1))
  );

  // Un solo umbral para las dos series, por el mismo motivo que la escala.
  const umbral = umbralDePico(todos);

  const ejeX = series.every((s) => s.tiempos.length === s.valores.length) ? 's' : 'muestra';

  return (
    <figure className="mt-4" data-testid="grafico-temporal">
      {/* Cabecera + selector de escala. La explicación larga desaparece: el gráfico ya
          lleva su leyenda y el detalle técnico ya se explica una vez en la tabla. */}
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <figcaption className="text-xs font-semibold text-[var(--text)]">
          Tiempo por muestra (ms) <span className="font-normal text-[var(--text-dim)]">· más bajo es mejor</span>
        </figcaption>
        <div
          className="flex items-center rounded-lg border border-[var(--border)] bg-[var(--bg)] p-0.5"
          role="group"
          aria-label="Escala vertical del gráfico"
        >
          {([
            { id: 'detalle' as Escala, texto: 'Detalle habitual' },
            { id: 'picos' as Escala, texto: 'Todos los picos' },
          ]).map((op) => (
            <button
              key={op.id}
              type="button"
              onClick={() => setEscala(op.id)}
              aria-pressed={escala === op.id}
              className={`px-2.5 py-1 rounded-md text-[11px] font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] ${
                escala === op.id
                  ? 'bg-[var(--surface)] text-[var(--text)] shadow-sm'
                  : 'text-[var(--text-muted)] hover:text-[var(--text)]'
              }`}
            >
              {op.texto}
            </button>
          ))}
        </div>
      </div>

      <div className="space-y-3">
        {series.map((s) => {
          const puntos = reducirConservandoExtremos(s.valores);
          const d = rutaDeSerie(s, ANCHO, ALTO, maxY, maxX, puntos);
          const picos = indicesDePicos(s.valores, umbral);
          const recortados = picosRecortados(s.valores, maxY);
          const idPico = `pico-${s.nombre}`;
          const posX = (i: number) =>
            s.tiempos.length === s.valores.length
              ? (s.tiempos[i] / maxX) * ANCHO
              : (i / Math.max(1, s.valores.length - 1)) * ANCHO;
          return (
            <div key={s.nombre}>
              <div className="flex items-center justify-between text-[11px] mb-1 gap-2">
                <span className="font-bold truncate" style={{ color: s.color }}>
                  {/* La forma además del color. */}
                  <span aria-hidden="true">{s.nombre === et.antes ? '― ' : '┄ '}</span>
                  {s.nombre}
                </span>
                <span className="text-[var(--text-dim)] font-mono whitespace-nowrap">
                  {s.valores.length} puntos
                  {picos.length > 0 && ` · ${picos.length} pico${picos.length === 1 ? '' : 's'}`}
                  {recortados.length > 0 && ` · ${recortados.length} fuera de escala`}
                </span>
              </div>
              <svg
                viewBox={`0 0 ${ANCHO} ${ALTO}`}
                className="w-full h-28 rounded-lg bg-[var(--bg)] border border-[var(--border)]"
                role="img"
                aria-label={`Tiempo por muestra de «${s.nombre}»: ${s.valores.length} puntos, ${picos.length} picos por encima del doble de la mediana${
                  recortados.length > 0 ? `, ${recortados.length} por encima del tope del eje` : ''
                }`}
                preserveAspectRatio="none"
              >
                {/* Guía de la mediana, común a las dos series. */}
                <line
                  x1="0"
                  x2={ANCHO}
                  y1={ALTO - (Math.min(umbral / 2, maxY) / maxY) * ALTO}
                  y2={ALTO - (Math.min(umbral / 2, maxY) / maxY) * ALTO}
                  stroke="var(--border)"
                  strokeWidth="1"
                  strokeDasharray="4 4"
                />
                <path
                  d={d}
                  fill="none"
                  stroke={s.color}
                  strokeWidth="1.5"
                  strokeDasharray={s.nombre === et.despues ? '4 3' : undefined}
                  vectorEffect="non-scaling-stroke"
                  data-testid={idPico}
                />
                {picos.map((i) => (
                  <circle
                    key={i}
                    cx={posX(i)}
                    cy={ALTO - (Math.min(s.valores[i], maxY) / maxY) * ALTO}
                    r="2.5"
                    fill="var(--caution)"
                  />
                ))}
                {/* Picos que se salen del tope: se dibujan pegados al borde superior y
                    en otro color, para no hacer creer que llegaron justo hasta ahí. */}
                {recortados.map((i) => (
                  <polygon
                    key={`c-${i}`}
                    points={`${posX(i)},0 ${posX(i) - 4},7 ${posX(i) + 4},7`}
                    fill="var(--error)"
                    data-testid={`recortado-${s.nombre}`}
                  />
                ))}
              </svg>
            </div>
          );
        })}
      </div>

      <div className="flex justify-between text-[10px] text-[var(--text-dim)] mt-1 font-mono">
        <span>0 {ejeX}</span>
        <span>
          {maxX.toFixed(ejeX === 's' ? 1 : 0)} {ejeX}
        </span>
      </div>

      {umbral !== Infinity && (
        <p className="mt-2 text-[10px] text-[var(--text-dim)] leading-relaxed">
          Los puntos naranjas son muestras con más del doble de frametime que la mediana
          (la línea de puntos). Se marcan como picos; el log no dice por qué ocurren.
        </p>
      )}
    </figure>
  );
}
