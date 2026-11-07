import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
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
  it('avisa de los juegos de Heronite desinstalados', () => {
    // Heronite no borra la entrada al desinstalar (guarda las horas jugadas),
    // así que se ocultan aquí. Sin este aviso el usuario cree que la app ha
    // perdido juegos.
    renderLibrary({ heroniteUninstalled: 3 });

    expect(screen.getByText('3 juegos desinstalados en Heronite no se muestran')).toBeTruthy();
  });

  it('en singular concuerda con el número', () => {
    renderLibrary({ heroniteUninstalled: 1 });

    expect(screen.getByText('1 juego desinstalado en Heronite no se muestra')).toBeTruthy();
  });

  it('no dice nada si no hay desinstalados', () => {
    renderLibrary({ heroniteUninstalled: 0 });

    expect(screen.queryByText(/desinstalado/)).toBeNull();
  });

  it('muestra a la vez el aviso de duplicados de Steam y el de Heronite', () => {
    renderLibrary({ hiddenDuplicates: 1, heroniteUninstalled: 4 });

    expect(screen.getByText(/1 juego ya está en Steam/)).toBeTruthy();
    expect(screen.getByText('4 juegos desinstalados en Heronite no se muestran')).toBeTruthy();
  });
});
