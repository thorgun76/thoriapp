import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { AvisoSteamDesincronizado } from './AvisoSteamDesincronizado';

afterEach(cleanup);

const PALWORLD = 'mesa_glthread=true game-performance mangohud %command%';
const renderAviso = (props: Partial<React.ComponentProps<typeof AvisoSteamDesincronizado>> = {}) =>
  render(
    <AvisoSteamDesincronizado
      steam={{ opciones: PALWORLD, cuenta: '123' }}
      comandoActual="%command%"
      onImportar={() => {}}
      {...props}
    />
  );

describe('AvisoSteamDesincronizado', () => {
  it('muestra el comando de Steam completo y sin tocar', () => {
    renderAviso();
    // Los wrappers y la variable de entorno siguen ahí, en el mismo orden.
    expect(screen.getByText(PALWORLD)).toBeTruthy();
  });

  it('explica la consecuencia real, no una advertencia genérica', () => {
    renderAviso();
    expect(
      screen.getByText(/la prueba registrará el comando de arriba, no el de Steam/i)
    ).toBeTruthy();
  });

  it('deja claro que Thoriapp no escribe en Steam', () => {
    // Con ajustes activos es cuando importa: el usuario podría creer que «Traer a
    // Thoriapp» o guardar el perfil también cambia Steam.
    renderAviso({ comandoActual: 'gamemoderun %command%' });
    expect(screen.getByText(/Thoriapp no modifica Steam por su cuenta/i)).toBeTruthy();
  });

  it('«Traer a Thoriapp» llama al importador', () => {
    const onImportar = vi.fn();
    renderAviso({ onImportar });
    fireEvent.click(screen.getByRole('button', { name: /Traer a Thoriapp/i }));
    expect(onImportar).toHaveBeenCalledTimes(1);
  });

  it('si Steam está vacío no se ofrece importar nada', () => {
    renderAviso({ steam: { opciones: '', cuenta: '123' } });
    expect(screen.queryByRole('button', { name: /Traer a Thoriapp/i })).toBeNull();
    expect(screen.getByText(/vacío: Steam no añade nada/i)).toBeTruthy();
  });

  it('dice qué comando se está usando ahora mismo', () => {
    renderAviso();
    expect(screen.getByText(/Thoriapp no añade ningún ajuste ahora mismo/i)).toBeTruthy();
  });

  it('con ajustes activos en Thoriapp, avisa en vez de afirmar lo contrario', () => {
    renderAviso({ comandoActual: 'gamemoderun %command%' });
    expect(screen.getByText(/Thoriapp no modifica Steam por su cuenta/i)).toBeTruthy();
  });

  it('permite copiar el comando de Thoriapp para pegarlo en Steam', () => {
    const onRevisar = vi.fn();
    renderAviso({ onRevisar });
    fireEvent.click(screen.getByRole('button', { name: /Copiar el de Thoriapp/i }));
    expect(onRevisar).toHaveBeenCalledTimes(1);
  });

  it('no afirma que el comando de Steam sea mejor que el de Thoriapp', () => {
    const { container } = renderAviso();
    const t = container.textContent ?? '';
    expect(t).not.toMatch(/mejor|recomendad|deber[ií]as usar/i);
  });
});
