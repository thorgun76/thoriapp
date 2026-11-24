import { estabilidadDe } from './estabilidad';
import type { Benchmark } from './types';

/**
 * Clasificación de las grabaciones **de un mismo juego**.
 *
 * ## Por qué no se ordena por FPS medios
 *
 * Ordenar por FPS medios premia bajar los ajustes hasta que el promedio sube, aunque el
 * juego vaya a tirones: es justo el caso en que un promedio engaña. La experiencia se
 * parece mucho más al **1 % low** (los FPS del peor 1 % de las imágenes) que al promedio.
 *
 * ## Por qué no se ordena por el índice de estabilidad
 *
 * Porque es un cociente sobre el propio promedio y no premia más FPS: un juego clavado a
 * 30 FPS da casi 100/100 y ganaría siempre frente a otro a 90 FPS con algún tirón.
 *
 * ## El criterio
 *
 * Cascada de desempate, con métricas reales de MangoHud (no hay puntuación inventada):
 *
 *   1. **1 % low** — la que mejor representa «va fluido de verdad».
 *   2. **FPS medios** — entre dos con el mismo 1 % low, más FPS es mejor.
 *   3. **Estabilidad** — desempate final: con los mismos lows, la más pareja.
 *
 * Comparar el 1 % low solo tiene sentido **dentro del mismo juego**: entre juegos distintos
 * los valores absolutos no son comparables. Por eso esto vive en la ficha del juego y no
 * en un ranking global.
 */

/** Datos de una grabación ya preparados para la tabla. */
export interface EntradaRanking {
  bench: Benchmark;
  /** FPS del peor 1 % de las imágenes. Criterio principal. */
  low1: number;
  /** FPS medios. Primer desempate. */
  fps: number;
  /** Índice de estabilidad 0-100, o `null` si no se puede calcular. Desempate final. */
  estabilidad: number | null;
  /** Posición 1-based. */
  puesto: number;
  /** `true` en la primera, solo cuando hay más de una para comparar. */
  ganadora: boolean;
  /** Diferencia de 1 % low frente a la ganadora, en %. 0 para la primera. */
  pctVsGanadora: number;
}

/** Ordena `a` antes que `b` según la cascada de criterios. */
function comparar(a: EntradaRanking, b: EntradaRanking): number {
  // 1 % low, descendente.
  if (b.low1 !== a.low1) return b.low1 - a.low1;
  // FPS medios, descendente.
  if (b.fps !== a.fps) return b.fps - a.fps;
  // Estabilidad, descendente. Sin dato va al final, no al principio: 0 significaría
  // «pésima» cuando en realidad es «no medida».
  const estA = a.estabilidad ?? -1;
  const estB = b.estabilidad ?? -1;
  if (estB !== estA) return estB - estA;
  // A igualdad de todo, la más reciente primero. El desempate es estable y predecible.
  return b.bench.timestamp.localeCompare(a.bench.timestamp);
}

/** Número máximo de grabaciones que se clasifican (el backend conserva 10 por juego). */
export const MAX_RANKING = 10;

/**
 * Clasifica las grabaciones de un juego de mejor a peor.
 *
 * Devuelve como mucho {@link MAX_RANKING} entradas. No ordena por FPS medios a propósito:
 * ver la cabecera del módulo.
 */
export function clasificar(benchmarks: Benchmark[], maximo = MAX_RANKING): EntradaRanking[] {
  const entradas: EntradaRanking[] = benchmarks
    // Una grabación sin 1 % low no se puede clasificar por el criterio principal; se deja
    // fuera en vez de relegarla al final con un 0 que no significa nada.
    .filter((b) => Number.isFinite(b.fps_1_percent_low) && b.fps_1_percent_low > 0)
    .map((bench) => ({
      bench,
      low1: bench.fps_1_percent_low,
      fps: bench.avg_fps,
      estabilidad: estabilidadDe(bench),
      puesto: 0,
      ganadora: false,
      pctVsGanadora: 0,
    }));

  entradas.sort(comparar);
  const recortadas = entradas.slice(0, maximo);
  if (recortadas.length === 0) return [];

  const mejor = recortadas[0];
  const referencia = Math.max(mejor.low1, 0.001);

  return recortadas.map((e, i) => ({
    ...e,
    puesto: i + 1,
    ganadora: i === 0 && recortadas.length > 1,
    pctVsGanadora: ((e.low1 - mejor.low1) / referencia) * 100,
  }));
}

/** Texto del criterio, para no repetirlo en cada interfaz. */
export const CRITERIO_RANKING =
  'Ordenadas por el 1 % low (los FPS del peor 1 % de las imágenes), que es lo que se ' +
  'nota como tirones. A igualdad, más FPS medios y mejor estabilidad.';
