import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

const mocks = vi.hoisted(() => ({ searchStore: vi.fn() }));
vi.mock('../lib/api', () => ({ api: mocks }));

import { AppidPicker } from './AppidPicker';

describe('AppidPicker', () => {
  // Sin esto cada test suma su DOM al anterior y los roles se duplican.
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.searchStore.mockResolvedValue([
      { appid: 275850, name: "No Man's Sky" },
      { appid: 999, name: "No Man's Sky: Beyond" },
    ]);
  });

  it('busca lo que el usuario escribe y deja elegir un resultado', async () => {
    const onPick = vi.fn();
    render(<AppidPicker initialQuery="No Man" onPick={onPick} onClose={() => {}} />);

    // El campo llega relleno con el nombre del juego y con el foco puesto.
    const input = screen.getByRole('textbox') as HTMLInputElement;
    expect(input.value).toBe('No Man');

    // Los resultados aparecen tras el retardo de la búsqueda.
    const first = await screen.findByText("No Man's Sky");
    await waitFor(() => expect(mocks.searchStore).toHaveBeenCalledWith('No Man'));

    fireEvent.click(first);
    expect(onPick).toHaveBeenCalledWith(275850, "No Man's Sky");
  });

  it('avisa cuando la búsqueda no da nada', async () => {
    mocks.searchStore.mockResolvedValue([]);
    render(<AppidPicker initialQuery="asdfqwer" onPick={() => {}} onClose={() => {}} />);

    expect(await screen.findByText(/Ningún juego coincide/i)).toBeTruthy();
  });

  it('no busca con menos de dos letras', async () => {
    render(<AppidPicker initialQuery="a" onPick={() => {}} onClose={() => {}} />);

    await new Promise((r) => setTimeout(r, 350));
    expect(mocks.searchStore).not.toHaveBeenCalled();
  });

  it('explica el error si la búsqueda falla', async () => {
    mocks.searchStore.mockRejectedValue('sin red');
    render(<AppidPicker initialQuery="No Man" onPick={() => {}} onClose={() => {}} />);

    expect(await screen.findByText(/No se ha podido buscar en Steam/i)).toBeTruthy();
  });

  it('se cierra con Escape y con Cancelar', async () => {
    const onClose = vi.fn();
    render(<AppidPicker initialQuery="No Man" onPick={() => {}} onClose={onClose} />);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: /Cancelar/i }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
