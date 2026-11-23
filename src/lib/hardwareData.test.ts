import { describe, expect, it } from 'vitest';
import gamesJson from '../../src-tauri/games.json?raw';
import { describeTag } from './recommendations';

/**
 * Guardián del dato de `games.json`.
 *
 * `requires` mal escrito no da error: el tag simplemente nunca está entre los
 * `hwTags` del equipo, así que el ajuste queda oculto para todo el mundo y nadie
 * se entera. Aquí se comprueba contra la lista real de tags de `hw.rs`.
 */

type P = {
  value: string;
  requires?: string[];
  recommended_for?: string[];
  not_recommended_for?: string[];
  evidence?: string;
  source?: string;
};
type G = { name: string; appid: number; params: P[] };

const games = JSON.parse(gamesJson) as G[];
const allParams = games.flatMap((g) => g.params);

/** Etiquetas que `hw.rs::Hardware::tags()` puede emitir. */
const HW_TAGS = new Set([
  'amd',
  'nvidia',
  'intel',
  'ntsync',
  'wayland',
  'game-performance',
  'vram-bajo',
  'vram-medio',
  'vram-alto',
  'cpu-sin-avx2',
  'rt',
]);

describe('games.json: los requisitos de hardware son válidos', () => {
  it('todo `requires` usa una etiqueta que hw.rs emite de verdad', () => {
    const invalidos: string[] = [];
    for (const g of games) {
      for (const p of g.params) {
        for (const tag of p.requires ?? []) {
          if (!HW_TAGS.has(tag)) invalidos.push(`${g.name}: ${p.value} requires=${tag}`);
        }
      }
    }
    expect(invalidos).toEqual([]);
  });

  it('todo `recommended_for` y `not_recommended_for` usa etiquetas válidas', () => {
    const validas = new Set([...HW_TAGS, 'all', 'online']);
    const invalidos: string[] = [];
    for (const g of games) {
      for (const p of g.params) {
        for (const tag of [...(p.recommended_for ?? []), ...(p.not_recommended_for ?? [])]) {
          if (!validas.has(tag)) invalidos.push(`${g.name}: ${p.value} -> ${tag}`);
        }
      }
    }
    expect(invalidos).toEqual([]);
  });

  it('cada etiqueta usada tiene un nombre legible para el usuario', () => {
    // Si no, la tarjeta diría «Necesita vram-bajo».
    for (const g of games) {
      for (const p of g.params) {
        for (const tag of p.requires ?? []) {
          expect(describeTag(tag)).not.toBe(tag);
        }
      }
    }
  });

  it('lo marcado como `documentado` trae fuente', () => {
    const sin: string[] = [];
    for (const g of games) {
      for (const p of g.params) {
        if (p.evidence === 'documentado' && !p.source) sin.push(`${g.name}: ${p.value}`);
      }
    }
    expect(sin).toEqual([]);
  });
});

describe('games.json: los ajustes que dependen de la GPU la declaran', () => {
  it('RADV_* y cualquier ruta de AMD requieren amd', () => {
    // RADV es el driver Vulkan de AMD; en NVIDIA/Intel no existe.
    const culpables = allParams
      .filter((p) => /^RADV_|radeon_icd/.test(p.value))
      .filter((p) => !(p.requires ?? []).includes('amd'))
      .map((p) => p.value);
    expect(culpables).toEqual([]);
  });

  it('lo de NVIDIA requiere nvidia', () => {
    const culpables = allParams
      .filter((p) => /NVAPI|__GLX_VENDOR_LIBRARY_NAME|__NV_PRIME/.test(p.value))
      .filter((p) => !(p.requires ?? []).includes('nvidia'))
      .map((p) => p.value);
    expect(culpables).toEqual([]);
  });

  it('nada exige dos vendors de GPU a la vez', () => {
    // Sería imposible de cumplir: el ajuste no aparecería nunca.
    const imposibles = allParams
      .filter((p) => {
        const r = p.requires ?? [];
        const gpus = r.filter((t) => ['amd', 'nvidia', 'intel'].includes(t));
        return gpus.length > 1;
      })
      .map((p) => p.value);
    expect(imposibles).toEqual([]);
  });
});

describe('games.json: nada que sepamos que ya no hace nada', () => {
  it('no vuelve DXVK_ASYNC', () => {
    // Se eliminó en DXVK 2.0: el commit oficial 922f038 («Remove option for
    // asynchronous shader compilation») deja `asyncPipeCompiler` en `false`
    // ignorando la config. En DXVK 2.x+ la variable es inerte, así que
    // anunciarla como ajuste es vender humo.
    const muertos = allParams.filter((p) => p.value.startsWith('DXVK_ASYNC')).map((p) => p.value);
    expect(muertos).toEqual([]);
  });
});
