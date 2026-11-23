import { describe, expect, it } from 'vitest';
import { describeTag, missingReason, recommendationStatus } from './recommendations';
import type { Param } from './types';

const param = (over: Partial<Param> = {}): Param => ({
  kind: 'env',
  value: 'X=1',
  title: '',
  why: '',
  impact: 'low',
  requires: [],
  default_on: false,
  evidence: 'docs',
  proton_only: false,
  ...over,
});

describe('missingReason', () => {
  it('devuelve null cuando el parámetro se puede aplicar', () => {
    expect(missingReason(param(), { isNative: false, hwTags: [], forceProton: false })).toBeNull();
  });

  it('bloquea parámetros de Proton en un juego nativo sin forzar Proton', () => {
    const reason = missingReason(param({ proton_only: true }), {
      isNative: true,
      hwTags: [],
      forceProton: false,
    });
    expect(reason).toMatch(/Proton/i);
  });

  it('permite parámetros de Proton en un juego nativo si se fuerza Proton', () => {
    expect(
      missingReason(param({ proton_only: true }), { isNative: true, hwTags: [], forceProton: true })
    ).toBeNull();
  });

  it('permite parámetros de Proton en un juego no nativo', () => {
    expect(
      missingReason(param({ proton_only: true }), { isNative: false, hwTags: [], forceProton: false })
    ).toBeNull();
  });

  it('nombra el requisito que falta', () => {
    const reason = missingReason(param({ requires: ['mesa_ge', 'ntsync'] }), {
      isNative: false,
      hwTags: ['mesa_ge'],
      forceProton: false,
    });
    expect(reason).toBe('Necesita soporte NTSync');
  });

  it('con todos los requisitos presentes no hay motivo', () => {
    expect(
      missingReason(param({ requires: ['mesa_ge'] }), {
        isNative: false,
        hwTags: ['mesa_ge'],
        forceProton: false,
      })
    ).toBeNull();
  });

  it('el juego nativo manda sobre los requisitos', () => {
    // Si es nativo y el parámetro es proton_only, el motivo es ese, no el requisito.
    const reason = missingReason(param({ proton_only: true, requires: ['ntsync'] }), {
      isNative: true,
      hwTags: [],
      forceProton: false,
    });
    expect(reason).toMatch(/Proton/i);
  });
});

describe('recommendationStatus', () => {
  it('proton_only sin forzar Proton no se recomienda', () => {
    expect(recommendationStatus(param({ proton_only: true }), { hwTags: [], forceProton: false })).toBe(
      'not_recommended'
    );
  });

  it('"all" en recommended_for siempre se recomienda', () => {
    expect(
      recommendationStatus(param({ recommended_for: ['all'] }), { hwTags: [], forceProton: false })
    ).toBe('recommended');
  });

  it('se recomienda si coincide algún tag de hardware', () => {
    expect(
      recommendationStatus(param({ recommended_for: ['amd', 'nvidia'] }), {
        hwTags: ['nvidia'],
        forceProton: false,
      })
    ).toBe('recommended');
  });

  it('es neutral si recommended_for no incluye el hardware', () => {
    expect(
      recommendationStatus(param({ recommended_for: ['amd'] }), { hwTags: ['nvidia'], forceProton: false })
    ).toBe('neutral');
  });

  it('not_recommended_for gana sobre recommended_for', () => {
    expect(
      recommendationStatus(
        param({ recommended_for: ['all'], not_recommended_for: ['nvidia'] }),
        { hwTags: ['nvidia'], forceProton: false }
      )
    ).toBe('not_applicable');
  });

  it('devuelve not_available si falta un requisito', () => {
    expect(
      recommendationStatus(param({ requires: ['mesa_ge'] }), { hwTags: [], forceProton: false })
    ).toBe('not_available');
  });

  it('sin ninguna pista de compatibilidad es neutral', () => {
    expect(recommendationStatus(param(), { hwTags: ['nvidia'], forceProton: false })).toBe('neutral');
  });
});

describe('describeTag: los motivos se leen en castellano', () => {
  it('traduce los tags de hardware a algo que se entiende', () => {
    // Antes salía el tag crudo en la tarjeta: «Necesita amd», «Necesita vram-bajo».
    expect(describeTag('amd')).toBe('una GPU AMD');
    expect(describeTag('nvidia')).toBe('una GPU NVIDIA');
    expect(describeTag('rt')).toBe('una GPU con ray tracing');
  });

  it('un tag desconocido se deja tal cual en vez de inventarse un nombre', () => {
    expect(describeTag('algo-nuevo')).toBe('algo-nuevo');
  });

  it('el motivo de missingReason usa el nombre legible', () => {
    const reason = missingReason(param({ requires: ['nvidia'] }), {
      isNative: false,
      hwTags: ['amd'],
      forceProton: false,
    });
    expect(reason).toBe('Necesita una GPU NVIDIA');
  });

  it('todas las etiquetas que emite hw.rs tienen nombre legible', () => {
    // Si hw.rs añade un tag nuevo sin traducción, saldría el identificador crudo.
    const emitidos = [
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
    ];
    for (const tag of emitidos) {
      expect(describeTag(tag)).not.toBe(tag);
    }
  });
});
