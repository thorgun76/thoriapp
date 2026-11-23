import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { GameCard, coverUrl } from './GameCard';
import { api } from '../lib/api';
import { clearCoverCache } from '../hooks/useLocalCover';

// Sin cleanup el DOM se acumula entre tests y los roles se duplican.
afterEach(cleanup);

beforeEach(() => {
  clearCoverCache();
  // Por defecto el launcher no tiene carátula en disco.
  vi.spyOn(api, 'getLocalCover').mockResolvedValue(null);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('coverUrl', () => {
  it('usa la carátula vertical 600x900 del CDN de Steam', () => {
    expect(coverUrl(1623730)).toBe(
      'https://cdn.cloudflare.steamstatic.com/steam/apps/1623730/library_600x900.jpg'
    );
  });

  it('sin appid no hay carátula (juegos de Lutris/Heroic/HeroNite)', () => {
    expect(coverUrl(0)).toBeNull();
    expect(coverUrl(undefined)).toBeNull();
  });

  it('un appid negativo tampoco genera una URL rota', () => {
    expect(coverUrl(-1)).toBeNull();
  });
});

const img = () => screen.queryByRole('img', { hidden: true });

describe('GameCard', () => {
  it('muestra la carátula del CDN de Steam cuando el launcher no tiene una', async () => {
    render(<GameCard game={{ appid: 1623730, name: 'Palworld' }} onOpen={() => {}} />);

    await waitFor(() => expect(img()).toBeTruthy());
    expect(img()!.getAttribute('src')).toContain('/apps/1623730/library_600x900.jpg');
    expect(screen.getByText('Palworld')).toBeTruthy();
  });

  it('prioriza la carátula que el launcher guarda en disco', async () => {
    // Regresión: los juegos de Lutris no tienen appid de Steam, así que antes
    // salían siempre con el icono genérico aunque el launcher tuviera portada.
    vi.spyOn(api, 'getLocalCover').mockResolvedValue('data:image/jpeg;base64,AAAA');

    render(
      <GameCard
        game={{ appid: 0, name: 'Juego de Lutris', launcher: 'lutris', cover_key: 'mi-juego' }}
        onOpen={() => {}}
      />
    );

    await waitFor(() => expect(img()!.getAttribute('src')).toBe('data:image/jpeg;base64,AAAA'));
    // El 4º argumento es el runner (lo usa Heronite para indexar su arte).
    expect(api.getLocalCover).toHaveBeenCalledWith('lutris', 0, 'mi-juego', undefined);
  });

  it('la carátula local gana a la del CDN de Steam', async () => {
    vi.spyOn(api, 'getLocalCover').mockResolvedValue('data:image/png;base64,BBBB');

    render(<GameCard game={{ appid: 440, name: 'TF2', launcher: 'heroic' }} onOpen={() => {}} />);

    await waitFor(() => expect(img()!.getAttribute('src')).toBe('data:image/png;base64,BBBB'));
    expect(img()!.getAttribute('src')).not.toContain('steamstatic');
  });

  it('si el launcher no tiene carátula y no hay appid, cae al icono de mando', async () => {
    render(
      <GameCard game={{ appid: 0, name: 'Juego de Heroic', launcher: 'heroic' }} onOpen={() => {}} />
    );

    await waitFor(() => expect(screen.queryByRole('img', { hidden: true })).toBeNull());
    expect(screen.getByText('Juego de Heroic')).toBeTruthy();
    expect(screen.getByText('Heroic')).toBeTruthy();
  });

  it('un fallo del comando no rompe la tarjeta', async () => {
    vi.spyOn(api, 'getLocalCover').mockRejectedValue(new Error('disco'));

    render(
      <GameCard game={{ appid: 0, name: 'Sin carátula', launcher: 'heronite' }} onOpen={() => {}} />
    );

    await waitFor(() => expect(screen.getByText('Sin carátula')).toBeTruthy());
    expect(screen.queryByRole('img', { hidden: true })).toBeNull();
  });

  it('si la carátula de Steam falla, cae al icono de mando', async () => {
    render(<GameCard game={{ appid: 999999999, name: 'Juego retirado' }} onOpen={() => {}} />);

    await waitFor(() => expect(img()).toBeTruthy());
    fireEvent.error(img()!);

    expect(screen.queryByRole('img', { hidden: true })).toBeNull();
    expect(screen.getByText('Juego retirado')).toBeTruthy();
  });

  it('consulta la carátula una sola vez para el mismo juego', async () => {
    // La respuesta pesa ~40 KB en base64: repetir el invoke en cada render sería
    // tirar peticiones.
    render(<GameCard game={{ appid: 0, name: 'A', launcher: 'lutris' }} onOpen={() => {}} />);
    render(<GameCard game={{ appid: 0, name: 'A', launcher: 'lutris' }} onOpen={() => {}} />);

    await waitFor(() => expect(api.getLocalCover).toHaveBeenCalledTimes(1));
  });

  it('avisa del juego al hacer clic', async () => {
    const opened: string[] = [];
    render(
      <GameCard game={{ appid: 440, name: 'Team Fortress 2' }} onOpen={(g) => opened.push(g.name)} />
    );

    await act(async () => {});
    fireEvent.click(screen.getByText('Team Fortress 2'));
    expect(opened).toEqual(['Team Fortress 2']);
  });
});

describe('GameCard: copiar el comando guardado', () => {
  it('sin comando guardado no hay botón de copiar', async () => {
    render(<GameCard game={{ appid: 440, name: 'TF2' }} onOpen={() => {}} />);

    await waitFor(() => expect(screen.getByText('TF2')).toBeTruthy());
    expect(screen.queryByLabelText(/Copiar el comando/)).toBeNull();
  });

  it('con comando guardado lo copia sin abrir el juego', async () => {
    const onOpen = vi.fn();
    const onCopyCommand = vi.fn();
    render(
      <GameCard
        game={{ appid: 440, name: 'TF2' }}
        onOpen={onOpen}
        quickCommand="DXVK_HUD=1 %command%"
        onCopyCommand={onCopyCommand}
      />
    );

    await waitFor(() => expect(screen.getByText('TF2')).toBeTruthy());
    fireEvent.click(screen.getByLabelText(/Copiar el comando/));

    expect(onCopyCommand).toHaveBeenCalledWith('DXVK_HUD=1 %command%');
    // Regresión: el clic no debe burbujear a la tarjeta y abrir el juego.
    expect(onOpen).not.toHaveBeenCalled();
    expect(screen.getByText('Copiado')).toBeTruthy();
  });

  it('el teclado tampoco abre el juego', async () => {
    const onOpen = vi.fn();
    const onCopyCommand = vi.fn();
    render(
      <GameCard
        game={{ appid: 440, name: 'TF2' }}
        onOpen={onOpen}
        quickCommand="gamemoderun %command%"
        onCopyCommand={onCopyCommand}
      />
    );

    await waitFor(() => expect(screen.getByText('TF2')).toBeTruthy());
    fireEvent.keyDown(screen.getByLabelText(/Copiar el comando/), { key: 'Enter' });

    expect(onCopyCommand).toHaveBeenCalledWith('gamemoderun %command%');
    expect(onOpen).not.toHaveBeenCalled();
  });
});

describe('GameCard: invitación a optimizar', () => {
  it('avisa de lo que hace la tarjeta al pasar por encima, sin robar el clic', async () => {
    const onOpen = vi.fn();
    render(<GameCard game={{ appid: 440, name: 'TF2' }} onOpen={onOpen} />);

    await waitFor(() => expect(screen.getByText('TF2')).toBeTruthy());

    const cta = screen.getByText('Optimizar');
    // Decorativo: el nombre accesible del botón sigue siendo el del juego y el
    // clic sobre la etiqueta tiene que abrir la tarjeta, no quedarse ahí.
    expect(cta.closest('[aria-hidden="true"]')).toBeTruthy();
    fireEvent.click(cta);
    expect(onOpen).toHaveBeenCalledWith({ appid: 440, name: 'TF2' });
  });

  it('no añade una parada de tabulación extra por juego', async () => {
    render(<GameCard game={{ appid: 440, name: 'TF2' }} onOpen={() => {}} />);

    await waitFor(() => expect(screen.getByText('TF2')).toBeTruthy());
    expect(screen.getAllByRole('button')).toHaveLength(1);
  });
});
