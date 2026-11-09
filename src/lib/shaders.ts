// Lógica pura del diagnóstico de shaders: decide qué contar y con qué palabras.
//
// El backend mide y devuelve números; aquí se traducen a una conclusión. Separarlo
// permite testear todos los casos (incluido el "no hay datos") sin depender de que esta
// máquina tenga Steam, MangoHud o el juego instalado.

import type { ShaderCache, ShaderStutter } from './types';

/** A partir de qué empeoramiento (%) el arranque es un problema de shaders y no ruido. */
export const SIGNIFICANT_PENALTY = 20;

/** Cuánto tiene que haber en la caché para decir que el juego "ya está precargado". */
export const PRECACHE_ENOUGH_MB = 50;

export type ShaderVerdictKind = 'no-data' | 'clean' | 'shaders' | 'unclear';

export interface ShaderVerdict {
  kind: ShaderVerdictKind;
  /** Título corto. */
  title: string;
  /** Qué significa, en una frase. */
  detail: string;
  /** Qué hacer al respecto. */
  advice: string;
}

/**
 * Conclusión sobre el arranque de una grabación.
 *
 * Se pide que el arranque sea peor **y** que la diferencia sea grande: el peor 1 % es
 * una medida ruidosa y con dos grabaciones seguidas del mismo juego se mueve solo. Si
 * solo hubiera que superar el ruido de la grabación (±3 %), casi todo saldría como
 * "problema de shaders" y el aviso perdería sentido.
 */
export function shaderVerdict(
  stutter: ShaderStutter | null | undefined,
  cache?: ShaderCache | null
): ShaderVerdict {
  if (!stutter || !(stutter.window_seconds > 0)) {
    return {
      kind: 'no-data',
      title: 'Sin datos del arranque',
      detail:
        'Esta grabación no permite comparar los primeros segundos con el resto (hacen falta al menos 10 s y frames suficientes en los dos tramos).',
      advice: 'Graba al menos un minuto y vuelve a analizarlo.',
    };
  }

  const penalty = stutter.stutter_penalty_percent;
  const significant = penalty >= SIGNIFICANT_PENALTY;

  if (!significant) {
    return {
      kind: penalty > 0 ? 'unclear' : 'clean',
      title: penalty > 0 ? 'Arranque ligeramente irregular' : 'Arranque limpio',
      detail:
        penalty > 0
          ? `El primer tramo pierde un ${penalty.toFixed(0)} % de fluidez frente al resto. Está por debajo del ${SIGNIFICANT_PENALTY} % a partir del cual se puede hablar de compilación de shaders.`
          : 'Los primeros segundos van igual de suaves que el resto: la compilación de shaders no te está afectando.',
      advice:
        penalty > 0
          ? 'Nada que hacer. Si lo notas al entrar, puede ser la carga del nivel y no los shaders.'
          : 'Puedes precargar menos. Si quieres, desactiva el descarte de calentamiento para incluir el arranque en la media.',
    };
  }

  const precached = cache?.steam_precached === true;
  const plenty = bytesToMb(cache?.bytes ?? 0) >= PRECACHE_ENOUGH_MB;

  const advice = precached
    ? plenty
      ? 'Steam ya tiene la caché de este juego preparada, así que el tirón puede venir de otra cosa: prueba a bajar shaders o texturas a medio, o a limitar el FPS.'
      : 'La caché de Steam existe pero está casi vacía: juega unos minutos más con MangoHud cerrado para que se llene y vuelve a medir.'
    : 'Steam no tiene caché de este juego (suele pasar en juegos sin Steam Cloud de shaders y en lanzadores externos): esas primeras partidas compilan siempre. Juega un rato y repite la grabación; si el arranque ya va fino, el problema era la primera compilación.';

  return {
    kind: 'shaders',
    title: 'Tirones al arrancar por compilación de shaders',
    detail: `En los primeros ${Math.round(stutter.window_seconds)} s el peor 1 % baja a ${stutter.early_fps_1_percent_low.toFixed(0)} FPS, frente a ${stutter.steady_fps_1_percent_low.toFixed(0)} FPS del resto (${penalty.toFixed(0)} % peor).`,
    advice,
  };
}

/** Bytes a MB legibles. 0 → "0 MB". */
export function bytesToMb(bytes: number): number {
  if (!Number.isFinite(bytes) || bytes <= 0) return 0;
  return bytes / (1024 * 1024);
}

/** Texto del tamaño de la caché, sin decimales para los MB grandes. */
export function cacheSizeLabel(bytes: number): string {
  const mb = bytesToMb(bytes);
  if (mb === 0) return 'vacía';
  if (mb < 10) return `${mb.toFixed(1)} MB`;
  return `${Math.round(mb)} MB`;
}

/**
 * Cómo va el conteo de tirones del arranque frente al resto, en % de frames lentos.
 * Devuelve `null` si ninguno de los dos tramos tuvo frames lentos (no hay nada que
 * comparar y decir "0 % igual que 0 %" no aporta).
 */
export function slowFramesRatio(stutter: ShaderStutter): number | null {
  const { early_pct_over_33ms: early, steady_pct_over_33ms: steady } = stutter;
  if (early <= 0 && steady <= 0) return null;
  if (steady <= 0) return null; // dividir por cero no dice nada útil
  return early / steady;
}
