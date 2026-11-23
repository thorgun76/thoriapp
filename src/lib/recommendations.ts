import type { Param } from './types';

/**
 * Nombre legible de una etiqueta de hardware. El tag interno («amd», «rt») no
 * dice nada a quien lee la tarjeta; antes salía literal: «Necesita vram-bajo».
 */
export function describeTag(tag: string): string {
  const labels: Record<string, string> = {
    amd: 'una GPU AMD',
    nvidia: 'una GPU NVIDIA',
    intel: 'una GPU Intel',
    ntsync: 'soporte NTSync',
    wayland: 'una sesión Wayland',
    'game-performance': 'game-performance disponible',
    'vram-bajo': 'menos de 4 GB de VRAM',
    'vram-medio': 'menos de 8 GB de VRAM',
    'vram-alto': '8 GB o más de VRAM',
    'cpu-sin-avx2': 'una CPU sin AVX2',
    rt: 'una GPU con ray tracing',
  };
  return labels[tag] ?? tag;
}

export type RecommendationStatus = 'recommended' | 'neutral' | 'not_applicable' | 'not_available' | 'not_recommended';

/**
 * ¿Por qué NO se puede aplicar este parámetro? Devuelve el motivo o `null`.
 * Antes vivía en App.tsx con acceso directo a `info`/`hw`/`forceProton`.
 */
export function missingReason(
  p: Param,
  opts: { isNative: boolean; hwTags: string[]; forceProton: boolean }
): string | null {
  if (opts.isNative && p.proton_only && !opts.forceProton) {
    return 'Solo aplica con Proton; este juego es nativo de Linux';
  }
  const required = p.requires.find((t) => !opts.hwTags.includes(t));
  if (required) return `Necesita ${describeTag(required)}`;
  return null;
}

/**
 * Estado de un parámetro según el hardware y el juego abierto.
 * Réplica exacta de la lógica anterior, ahora pura y testeable.
 */
export function recommendationStatus(
  p: Param,
  opts: { hwTags: string[]; forceProton: boolean }
): RecommendationStatus {
  const { hwTags, forceProton } = opts;

  if (p.proton_only && !forceProton) return 'not_recommended';

  if (p.not_recommended_for && p.not_recommended_for.length > 0) {
    for (const tag of p.not_recommended_for) {
      if (hwTags.includes(tag)) return 'not_applicable';
    }
  }

  if (p.recommended_for && p.recommended_for.length > 0) {
    if (p.recommended_for.includes('all')) return 'recommended';
    for (const tag of p.recommended_for) {
      if (hwTags.includes(tag)) return 'recommended';
    }
    return 'neutral';
  }

  if (p.requires && p.requires.length > 0) {
    for (const req of p.requires) {
      if (!hwTags.includes(req)) return 'not_available';
    }
  }

  return 'neutral';
}
