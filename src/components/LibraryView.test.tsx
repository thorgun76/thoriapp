import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { LibraryView } from './LibraryView';

vi.mock('../lib/api', () => ({
  api: { getLocalCover: vi.fn().mockResolvedValue(null) },
}));

afterEach(cleanup);

beforeEach(() => {
  vi.clearAllMocks();
});

function renderLibrary(props: Partial<Parameters<typeof LibraryView>[0]> = {}) {
  return render(
    <LibraryView
      games={[]}
      counts={{ steam: 0, lutris: 0, heroic: 0, heronite: 0 }}
      launcherFilter="all"
      externalCount={0}
      showExternalGames={false}
      scanningLaunchers={false}
      onFilterChange={() => {}}
      onToggleExternal={() => {}}
      onRescan={() => {}}
      onOpenGame={() => {}}
      {...props}
    />
  );
}

describe('LibraryView: avisos de juegos ocultos', () => {
  // Heronite no borra la entrada al desinstalar (guarda las horas jugadas), así que
  // esos juegos se ocultan de la lista. Antes se avisaba con un recuento («199 juegos
  // desinstalados en Heronite no se muestran»): era ruido en la cabecera, un número que
  // no se puede usar para nada. Ya no se dice nada.
  it('no avisa de los juegos de Heronite desinstalados', () => {
    renderLibrary({ heroniteUninstalled: 199 });

    expect(screen.queryByText(/desinstalado/i)).toBeNull();
    expect(screen.queryByText(/Heronite no se muestran/i)).toBeNull();
  });

  it('no dice nada si no hay desinstalados', () => {
    renderLibrary({ heroniteUninstalled: 0 });

    expect(screen.queryByText(/desinstalado/i)).toBeNull();
  });

  it('mantiene el aviso de duplicados de Steam', () => {
    // Ese sí se queda: evita que el usuario crea que la app ha perdido juegos.
    renderLibrary({ hiddenDuplicates: 1, heroniteUninstalled: 4 });

    expect(screen.getByText(/1 juego ya está en Steam/)).toBeTruthy();
    expect(screen.queryByText(/desinstalado/i)).toBeNull();
  });
});

describe('LibraryView: orden de la rejilla', () => {
  const games = [
    { appid: 1, name: 'Zelda' },
    { appid: 2, name: 'Ámbar' },
    { appid: 3, name: 'Celeste' },
    { appid: 4, name: 'Portal 2' },
    { appid: 5, name: 'Portal 10' },
  ];

  const cardNames = () =>
    screen
      .getAllByRole('button')
      .map((b) => b.getAttribute('title'))
      .filter((t): t is string => !!t && games.some((g) => g.name === t));

  it('ordena de la A a la Z sin que los acentos se vayan al final', () => {
    renderLibrary({ games });

    // «Ámbar» debe quedar entre la A y la C, no después de la Z.
    expect(cardNames()).toEqual(['Ámbar', 'Celeste', 'Portal 2', 'Portal 10', 'Zelda']);
  });

  it('y de la Z a la A al pulsar el botón', () => {
    renderLibrary({ games });

    fireEvent.click(screen.getByTitle('Nombre, de la Z a la A'));

    expect(cardNames()).toEqual(['Zelda', 'Portal 10', 'Portal 2', 'Celeste', 'Ámbar']);
  });

  it('con un solo juego no se ofrece ordenar', () => {
    renderLibrary({ games: [{ appid: 1, name: 'Celeste' }] });

    expect(screen.queryByText('Ordenar')).toBeNull();
  });
});
