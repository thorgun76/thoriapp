import { describe, it, expect } from 'vitest';
import {
  SIGNIFICANT_PENALTY,
  PRECACHE_ENOUGH_MB,
  shaderVerdict,
  bytesToMb,
  cacheSizeLabel,
  slowFramesRatio,
} from './shaders';
import type { ShaderCache, ShaderStutter } from './types';

const stutter = (penalty: number, over: Partial<ShaderStutter> = {}): ShaderStutter => ({
  window_seconds: 10,
  early_fps_1_percent_low: 30,
  steady_fps_1_percent_low: 60,
  early_pct_over_33ms: 2,
  steady_pct_over_33ms: 0.5,
  stutter_penalty_percent: penalty,
  ...over,
});

const cache = (mb: number, precached = true): ShaderCache => ({
  bytes: Math.round(mb * 1024 * 1024),
  locations: [],
  steam_precached: precached,
});

describe('shaderVerdict', () => {
  it('sin datos no opina', () => {
    expect(shaderVerdict(null).kind).toBe('no-data');
    expect(shaderVerdict(undefined).kind).toBe('no-data');
    expect(shaderVerdict(stutter(50, { window_seconds: 0 })).kind).toBe('no-data');
  });

  it('sin penalización el arranque está limpio', () => {
    const v = shaderVerdict(stutter(0));
    expect(v.kind).toBe('clean');
    expect(v.title).toMatch(/limpio/i);
  });

  it('penalización pequeña no se presenta como problema de shaders', () => {
    const v = shaderVerdict(stutter(SIGNIFICANT_PENALTY - 1));
    expect(v.kind).toBe('unclear');
    expect(v.title).not.toMatch(/shaders/i);
  });

  it('justo en el umbral ya cuenta como shaders', () => {
    expect(shaderVerdict(stutter(SIGNIFICANT_PENALTY)).kind).toBe('shaders');
  });

  it('un arranque muy malo con caché de Steam ya preparada sugiere otra causa', () => {
    const v = shaderVerdict(stutter(90), cache(PRECACHE_ENOUGH_MB + 10));
    expect(v.kind).toBe('shaders');
    expect(v.advice).toMatch(/otra cosa/i);
  });

  it('caché de Steam casi vacía sugiere dejarla llenar', () => {
    const v = shaderVerdict(stutter(90), cache(1));
    expect(v.advice).toMatch(/casi vacía/i);
  });

  it('sin caché de Steam avisa de que las primeras partidas compilan', () => {
    const v = shaderVerdict(stutter(90), cache(0, false));
    expect(v.advice).toMatch(/no tiene caché/i);
  });

  it('el detalle lleva los números reales, no promesas', () => {
    const v = shaderVerdict(stutter(50, { early_fps_1_percent_low: 30, steady_fps_1_percent_low: 60 }));
    expect(v.detail).toContain('30 FPS');
    expect(v.detail).toContain('60 FPS');
    expect(v.detail).toContain('50 %');
  });
});

describe('bytesToMb y cacheSizeLabel', () => {
  it('convierte y no rompe con valores raros', () => {
    expect(bytesToMb(0)).toBe(0);
    expect(bytesToMb(-5)).toBe(0);
    expect(bytesToMb(Number.NaN)).toBe(0);
    expect(bytesToMb(1024 * 1024)).toBe(1);
  });

  it('etiqueta el tamaño en unidades entendibles', () => {
    expect(cacheSizeLabel(0)).toBe('vacía');
    expect(cacheSizeLabel(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(cacheSizeLabel(120 * 1024 * 1024)).toBe('120 MB');
  });
});

describe('slowFramesRatio', () => {
  it('compara frames lentos entre arranque y resto', () => {
    expect(slowFramesRatio(stutter(0, { early_pct_over_33ms: 4, steady_pct_over_33ms: 1 }))).toBe(4);
  });

  it('no divide por cero ni informa de cero contra cero', () => {
    expect(slowFramesRatio(stutter(0, { early_pct_over_33ms: 0, steady_pct_over_33ms: 0 }))).toBeNull();
    expect(slowFramesRatio(stutter(0, { early_pct_over_33ms: 3, steady_pct_over_33ms: 0 }))).toBeNull();
  });
});
