import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { RankingCard } from './RankingCard';
import type { Benchmark } from '../lib/types';

afterEach(cleanup);

function bench(
  id: string,
  n: number,
  { low1 = 50, fps = 60, low01 = 40 } = {}
): Benchmark {
  return {
    id,
    profile_id: '',
    game_name: 'Palworld',
    appid: 1623730,
    timestamp: '2026-11-01T10:00:00',
    duration_seconds: 60,
    avg_fps: fps,
    fps_1_percent_low: low1,
    fps_0_1_percent_low: low01,
    max_fps: 120,
    min_fps: 10,
    frame_time_avg: 16.6,
    frame_time_1_percent_high: 33,
    total_frames: 3600,
    notes: null,
    launch_command: 'mangohud %command%',
    number: n,
  };
}

describe('RankingCard', () => {
  it('no se pinta sin grabaciones', () => {
    const { container } = render(<RankingCard rows={[]} />);
    expect(container.textContent).toBe('');
  });

  it('titula con el número real de grabaciones', () => {
    render(<RankingCard rows={[bench('a', 1), bench('b', 2), bench('c', 3)]} />);
    expect(screen.getByText(/Cuál va mejor de tus 3 grabaciones/)).toBeTruthy();
  });

  it('con una sola grabación no dice «de tus 1 grabaciones»', () => {
    // Gramaticalmente mal y además prometía un ranking que no existe.
    render(<RankingCard rows={[bench('a', 1)]} />);

    expect(screen.queryByText(/1 grabaciones/)).toBeNull();
    expect(screen.queryByText(/Cuál va mejor/)).toBeNull();
    expect(screen.getByText('Tu única grabación de este juego')).toBeTruthy();
  });

  it('con una sola grabación no explica el criterio de orden', () => {
    // No hay orden que explicar; lo que falta es una segunda grabación.
    render(<RankingCard rows={[bench('a', 1)]} />);

    expect(screen.queryByText(/Ordenadas por el 1 % low/)).toBeNull();
    expect(screen.getByText(/Graba otra vez cambiando una sola cosa/)).toBeTruthy();
  });

  it('explica que ordena por 1 % low y no por FPS medios', () => {
    render(<RankingCard rows={[bench('a', 1), bench('b', 2)]} />);
    expect(screen.getByText(/Ordenadas por el 1 % low/)).toBeTruthy();
  });

  it('enseña el 1 % low de cada grabación, no solo los FPS medios', () => {
    render(<RankingCard rows={[bench('a', 1, { low1: 42, fps: 90 })]} />);
    expect(screen.getByText('42 FPS')).toBeTruthy();
    expect(screen.getByText(/90 medios/)).toBeTruthy();
    expect(screen.getByText('1 % low')).toBeTruthy();
  });

  it('marca la ganadora y da la diferencia de las demás frente a ella', () => {
    render(<RankingCard rows={[bench('a', 1, { low1: 40 }), bench('b', 2, { low1: 60 })]} />);
    // El nombre se compone de dos nodos («Mejor · » + «Prueba N»), así que se busca por párrafo.
    expect(screen.getByText((_, el) => el?.tagName === 'P' && el.textContent === 'Mejor · Prueba 2')).toBeTruthy();
    expect(screen.getByText(/referencia/)).toBeTruthy();
    // 40 frente a 60 = -33,3 %
    expect(screen.getByText(/-33,3 %/)).toBeTruthy();
  });

  it('llama a onDelete con la grabación pulsada', () => {
    const onDelete = vi.fn();
    render(<RankingCard rows={[bench('a', 1), bench('b', 2, { low1: 70 })]} onDelete={onDelete} />);

    fireEvent.click(screen.getByLabelText('Borrar Prueba 1'));
    expect(onDelete).toHaveBeenCalledWith(expect.objectContaining({ id: 'a' }));
  });

  it('borra la grabación correcta aunque esté la última', () => {
    const onDelete = vi.fn();
    render(<RankingCard rows={[bench('a', 1), bench('b', 2, { low1: 70 })]} onDelete={onDelete} />);

    fireEvent.click(screen.getByLabelText('Borrar Prueba 2'));
    expect(onDelete).toHaveBeenCalledWith(expect.objectContaining({ id: 'b' }));
  });

  it('no pinta botones de borrar si no se le pasa ninguno', () => {
    render(<RankingCard rows={[bench('a', 1), bench('b', 2)]} />);
    expect(screen.queryByLabelText(/Borrar/)).toBeNull();
  });

  it('ofrece «Borrar todas» con dos o más grabaciones', () => {
    const onDeleteAll = vi.fn();
    render(<RankingCard rows={[bench('a', 1), bench('b', 2)]} onDeleteAll={onDeleteAll} />);

    fireEvent.click(screen.getByText(/Borrar todas/));
    expect(onDeleteAll).toHaveBeenCalledTimes(1);
  });

  it('avisa de que hay 10 por juego cuando se puede borrar', () => {
    render(<RankingCard rows={[bench('a', 1), bench('b', 2)]} onDelete={vi.fn()} />);
    expect(screen.getByText(/como máximo 10 pruebas por juego/)).toBeTruthy();
  });

  it('no enseña el aviso de límite en modo solo lectura', () => {
    render(<RankingCard rows={[bench('a', 1), bench('b', 2)]} />);
    expect(screen.queryByText(/como máximo 10 pruebas por juego/)).toBeNull();
  });

  it('nombra las pruebas con su número, sin inventarlo', () => {
    const sinNumero = { ...bench('z', 1), number: null };
    render(<RankingCard rows={[sinNumero]} />);
    expect(screen.getByText(/Prueba sin número/)).toBeTruthy();
  });
});

// ── Copiar el comando ────────────────────────────────────────────────────────
//
// El botón está junto al de borrar: son las dos cosas que se hacen con una fila
// (recuperar el comando con el que se hizo la prueba y descartarla). Antes había que
// irse a la pestaña Benchmarks para copiarlo.

describe('RankingCard: copiar el comando', () => {
  it('copia el comando limpio, sin «Comando:» ni la marca de origen', () => {
    const onCopy = vi.fn();
    const b = { ...bench('a', 1), launch_command: 'mangohud %command%' };
    render(<RankingCard rows={[b]} onCopy={onCopy} />);

    fireEvent.click(screen.getByLabelText('Copiar el comando de Prueba 1'));

    // Pegarlo en Steam tiene que funcionar: con prefijo o sufijo rompería el lanzamiento.
    expect(onCopy).toHaveBeenCalledWith('mangohud %command%');
  });

  it('prefiere lo que dice Steam, que es lo que de verdad se ejecutó', () => {
    const onCopy = vi.fn();
    const b = {
      ...bench('a', 1),
      launch_command: '%command%',
      steam_command: 'DXVK_HUD=1 mangohud %command%',
    };
    render(<RankingCard rows={[b]} onCopy={onCopy} />);

    fireEvent.click(screen.getByLabelText('Copiar el comando de Prueba 1'));

    expect(onCopy).toHaveBeenCalledWith('DXVK_HUD=1 mangohud %command%');
  });

  it('no ofrece copiar si no hay comando registrado', () => {
    // Copiar un `%command%` inventado sería peor que no copiar nada.
    const onCopy = vi.fn();
    const sinComando = { ...bench('a', 1), launch_command: '', steam_command: null };
    render(<RankingCard rows={[sinComando]} onCopy={onCopy} />);

    expect(screen.queryByLabelText(/Copiar el comando/)).toBeNull();
  });

  it('no ofrece copiar si Steam se leyó y estaba vacío', () => {
    const onCopy = vi.fn();
    const b = { ...bench('a', 1), steam_command: '' };
    render(<RankingCard rows={[b]} onCopy={onCopy} />);

    expect(screen.queryByLabelText(/Copiar el comando/)).toBeNull();
  });

  it('no pinta el botón si no se le pasa onCopy', () => {
    render(<RankingCard rows={[bench('a', 1)]} />);
    expect(screen.queryByLabelText(/Copiar el comando/)).toBeNull();
  });

  it('cada fila copia su propio comando', () => {
    const onCopy = vi.fn();
    const a = { ...bench('a', 1), launch_command: 'gamemoderun %command%', steam_command: 'gamemoderun %command%' };
    const b = { ...bench('b', 2, { low1: 70 }), launch_command: 'mangohud %command%', steam_command: 'mangohud %command%' };
    render(<RankingCard rows={[a, b]} onCopy={onCopy} />);

    fireEvent.click(screen.getByLabelText('Copiar el comando de Prueba 2'));
    expect(onCopy).toHaveBeenCalledWith('mangohud %command%');
  });
});
