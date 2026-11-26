import { describe, expect, it } from 'vitest';
import { SecuenciaComando, type ContextoComando } from './secuenciaComando';

const ctx = (appid = 730, original = '%command%'): ContextoComando => ({
  appid,
  original,
});

describe('las respuestas viejas no pisan los cambios recientes', () => {
  it('aplica la última respuesta cuando llegan en orden', () => {
    const s = new SecuenciaComando();
    const c = ctx();

    const p1 = s.abrir(c);
    expect(s.aceptar(p1, c)).toBe(true);

    const p2 = s.abrir(c);
    expect(s.aceptar(p2, c)).toBe(true);
    expect(s.ultimaAplicada).toBe(2);
  });

  it('descarta la respuesta que llega tarde aunque sea la primera pedida', () => {
    // El caso real: el usuario pulsa un toggle, luego otro. La segunda petición vuelve
    // antes que la primera. Si se aplicara la primera al llegar, la UI volvería al estado
    // anterior y el segundo cambio se perdería.
    const s = new SecuenciaComando();
    const c = ctx();

    const lenta = s.abrir(c);
    const rapida = s.abrir(c);

    // Vuelve la rápida (la última pedida): se aplica.
    expect(s.aceptar(rapida, c)).toBe(true);
    // Y ahora la lenta: ya no vale.
    expect(s.aceptar(lenta, c)).toBe(false);
  });

  it('no aplica dos veces la misma respuesta', () => {
    const s = new SecuenciaComando();
    const c = ctx();
    const p = s.abrir(c);

    expect(s.aceptar(p, c)).toBe(true);
    expect(s.aceptar(p, c)).toBe(false);
  });

  it('una respuesta de otro juego no se aplica', () => {
    // Cambiar de juego con una petición en vuelo: la respuesta del juego anterior no
    // describe nada de lo que hay ahora en pantalla.
    const s = new SecuenciaComando();
    const viejo = s.abrir(ctx(730));
    const nuevo = s.abrir(ctx(570));

    expect(s.aceptar(viejo, ctx(570))).toBe(false);
    expect(s.aceptar(nuevo, ctx(570))).toBe(true);
  });

  it('una respuesta de otro comando de partida no se aplica', () => {
    // Si se importa otro comando de Steam, lo que venía del análisis anterior ya no
    // corresponde.
    const s = new SecuenciaComando();
    const antes = s.abrir(ctx(730, 'mangohud %command%'));
    const despues = s.abrir(ctx(730, '%command%'));

    expect(s.aceptar(antes, ctx(730, '%command%'))).toBe(false);
    expect(s.aceptar(despues, ctx(730, '%command%'))).toBe(true);
  });

  it('esLaActual distingue la pendiente de las superadas', () => {
    const s = new SecuenciaComando();
    const c = ctx();
    const p1 = s.abrir(c);
    const p2 = s.abrir(c);

    expect(s.esLaActual(p1, c)).toBe(false);
    expect(s.esLaActual(p2, c)).toBe(true);
    // Y si el contexto cambia, ya no es actual aunque sea la última.
    expect(s.esLaActual(p2, ctx(570))).toBe(false);
  });

  it('un viaje de ida y vuelta al mismo juego no revive peticiones viejas', () => {
    // Juego A → B → A. La respuesta pendiente de A (la primera vez) no puede aplicarse
    // ahora, aunque el contexto coincida: es más vieja que la última aplicada.
    const s = new SecuenciaComando();
    const a1 = s.abrir(ctx(730));
    const b = s.abrir(ctx(570));
    expect(s.aceptar(b, ctx(570))).toBe(true);

    const a2 = s.abrir(ctx(730));
    expect(s.aceptar(a1, ctx(730))).toBe(false);
    expect(s.aceptar(a2, ctx(730))).toBe(true);
  });
});
