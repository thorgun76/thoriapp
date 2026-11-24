import { describe, expect, it } from 'vitest';
import {
  claveJuego,
  comandoManualDe,
  editandoAMano,
  restablecerJuego,
} from './comandoManual';

// Las opciones que Steam tenía configuradas para Palworld.
const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';

const palworld = { appid: 1623730, name: 'Palworld', launcher: 'steam' };
const elden = { appid: 1245620, name: 'Elden Ring', launcher: 'steam' };
const lutris1 = { appid: 0, name: 'GTA San Andreas', launcher: 'lutris' };
const lutris2 = { appid: 0, name: 'Celeste', launcher: 'lutris' };

describe('claveJuego', () => {
  it('usa el AppID cuando lo hay', () => {
    expect(claveJuego(palworld)).toBe('steam:1623730');
  });

  it('dos juegos de Steam nunca comparten clave', () => {
    expect(claveJuego(palworld)).not.toBe(claveJuego(elden));
  });

  it('sin AppID usa lanzador y nombre', () => {
    expect(claveJuego(lutris1)).toBe('lutris:GTA San Andreas');
  });

  it('sin AppID dos juegos distintos tampoco comparten clave', () => {
    expect(claveJuego(lutris1)).not.toBe(claveJuego(lutris2));
  });

  it('sin juego devuelve vacío', () => {
    expect(claveJuego(null)).toBe('');
    expect(claveJuego(undefined)).toBe('');
  });
});

/**
 * Regresión del fallo reportado: «exporté el argumento de Palworld y ahora aparece en
 * todos los juegos, y Restablecer no cambia nada».
 *
 * Causa: `manualCommand` era un único estado global, no por juego, y `onReset` solo vaciaba
 * los ajustes opcionales sin tocar el editor manual.
 */
describe('el comando manual es de cada juego', () => {
  it('el comando traído de Steam para Palworld NO aparece en otros juegos', () => {
    const comandos = { [claveJuego(palworld)]: PALWORLD };

    expect(comandoManualDe(comandos, claveJuego(palworld))).toBe(PALWORLD);
    expect(comandoManualDe(comandos, claveJuego(elden))).toBe('');
    expect(comandoManualDe(comandos, claveJuego(lutris1))).toBe('');
  });

  it('activar el editor en un juego no lo activa en los demás', () => {
    const activos = { [claveJuego(palworld)]: true };

    expect(editandoAMano(activos, claveJuego(palworld))).toBe(true);
    expect(editandoAMano(activos, claveJuego(elden))).toBe(false);
    expect(editandoAMano(activos, claveJuego(lutris1))).toBe(false);
  });

  it('por defecto no hay nada escrito ni editor activo', () => {
    expect(comandoManualDe({}, 'steam:1')).toBe('');
    expect(editandoAMano({}, 'steam:1')).toBe(false);
  });
});

describe('restablecer vuelve a %command%', () => {
  it('desactiva el editor manual y borra el texto del juego', () => {
    const clave = claveJuego(palworld);
    const { comandos, activos } = restablecerJuego(
      { [clave]: PALWORLD },
      { [clave]: true },
      clave
    );

    // Esto es lo que no pasaba antes: el editor se quedaba activo con el texto de Steam.
    expect(editandoAMano(activos, clave)).toBe(false);
    expect(comandoManualDe(comandos, clave)).toBe('');
  });

  it('restablecer un juego no toca el comando de otro', () => {
    const claveP = claveJuego(palworld);
    const claveE = claveJuego(elden);
    const { comandos, activos } = restablecerJuego(
      { [claveP]: PALWORLD, [claveE]: 'gamemoderun %command%' },
      { [claveP]: true, [claveE]: true },
      claveP
    );

    expect(comandoManualDe(comandos, claveE)).toBe('gamemoderun %command%');
    expect(editandoAMano(activos, claveE)).toBe(true);
  });

  it('restablecer dos veces seguidas deja el mismo estado', () => {
    const clave = claveJuego(palworld);
    const una = restablecerJuego({ [clave]: PALWORLD }, { [clave]: true }, clave);
    const dos = restablecerJuego(una.comandos, una.activos, clave);
    expect(dos).toEqual(una);
  });

  it('restablecer un juego sin nada escrito no rompe nada', () => {
    const clave = claveJuego(palworld);
    const { comandos, activos } = restablecerJuego({}, {}, clave);
    expect(comandoManualDe(comandos, clave)).toBe('');
    expect(editandoAMano(activos, clave)).toBe(false);
  });
});
