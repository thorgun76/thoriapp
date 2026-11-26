import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

const CARGAR = vi.fn();

vi.mock('./api', () => ({
  api: {
    deleteProfile: vi.fn(),
    getCommunityRecommendations: () => Promise.resolve([]),
    getRules: () => Promise.resolve({ params: [] }),
    detectEngine: () => Promise.resolve(null),
    getEngineRecommendations: () => Promise.resolve([]),
    saveProfile: vi.fn(),
    updateProfile: vi.fn(),
    reorderProfiles: vi.fn(),
    exportProfile: vi.fn(),
    exportAllProfiles: vi.fn(),
    importProfileFromText: vi.fn(),
    previewImportProfile: vi.fn(),
    loadProfiles: () => CARGAR(),
  },
}));

import { useProfiles } from '../hooks/useProfiles';

afterEach(() => {
  vi.clearAllMocks();
});

const perfil = (id: string, appid: number, name: string) => ({
  id,
  name,
  appid,
  game_name: 'Juego',
  enabled_params: [],
  selected_community_recs: [],
  selected_engine_recs: [],
  force_proton: false,
  expert_mode: false,
  use_manual_command: false,
  command: '%command%',
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
});

const TODOS = [
  perfil('a', 1623730, 'Palworld uno'),
  perfil('b', 251570, '7DTD uno'),
  perfil('c', 1623730, 'Palworld dos'),
];

/** Monta el hook con la biblioteca completa cargada. */
async function montar() {
  CARGAR.mockResolvedValue(TODOS);
  const r = renderHook(() =>
    useProfiles({
      getHwTags: () => [],
      getGpuName: () => null,
    })
  );
  // El hook no se carga solo: lo dispara quien lo usa, como hace `App`.
  await act(async () => {
    await r.result.current.loadAll();
  });
  await waitFor(() => expect(r.result.current.profiles).toHaveLength(3));
  return r;
}

describe('perfiles de un juego', () => {
  it('filtra por appid exacto, sin colar los de otro juego', async () => {
    const { result } = await montar();

    const dePalworld = result.current.profilesForGame(1623730);
    // Es el fallo que se comía una configuración: enseñar en Palworld un perfil de 7DTD
    // y cargarlo cambiaría el juego equivocado.
    expect(dePalworld.map((p) => p.name)).toEqual(['Palworld uno', 'Palworld dos']);
  });

  it('un appid desconocido no devuelve nada, no todo', async () => {
    const { result } = await montar();
    expect(result.current.profilesForGame(999)).toEqual([]);
  });

  it('sin juego abierto no hay perfiles de juego', async () => {
    const { result } = await montar();
    expect(result.current.profilesForGame(undefined)).toEqual([]);
  });

  it('los perfiles del juego salen de la MISMA colección que la biblioteca', async () => {
    const { result } = await montar();

    // Sin una segunda lista: lo que se borra de la colección desaparece también del juego.
    expect(result.current.profilesForGame(1623730)).toHaveLength(2);
    expect(result.current.profiles).toHaveLength(3);

    const dePalworld = result.current.profilesForGame(1623730);
    for (const p of dePalworld) {
      expect(result.current.profiles.find((todos) => todos.id === p.id)).toBeDefined();
    }
  });

  it('una sola lectura de perfiles: no hay segunda petición por juego', async () => {
    const { result } = await montar();
    const llamadasTrasMontar = CARGAR.mock.calls.length;

    result.current.profilesForGame(1623730);
    result.current.profilesForGame(251570);

    // Derivar no vuelve a preguntar a Rust: dos fuentes se desincronizan.
    expect(CARGAR).toHaveBeenCalledTimes(llamadasTrasMontar);
  });
});
