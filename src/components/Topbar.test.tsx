import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import { Topbar, gpuModel, shortGpu } from './Topbar';

afterEach(cleanup);

describe('gpuModel', () => {
  it('quita la anotación del driver entre paréntesis', () => {
    expect(gpuModel('AMD Radeon RX 6800 XT (RADV NAVI21)')).toBe('RX 6800 XT');
  });

  it('quita el sufijo de Vulkan y el fabricante', () => {
    expect(gpuModel('NVIDIA GeForce RTX 3070/PCIe/SSE2')).toBe('RTX 3070');
  });

  it('no confunde el (R) de Intel con una anotación del driver', () => {
    // Cortar por el primer paréntesis dejaría «Intel» a secas.
    expect(gpuModel('Intel(R) UHD Graphics 630')).toBe('UHD Graphics 630');
  });

  it('deja intacto un nombre sin ruido', () => {
    expect(gpuModel('Intel Arc A770 Graphics')).toBe('Arc A770 Graphics');
  });

  it('con un nombre vacío no inventa nada', () => {
    expect(gpuModel('')).toBe('');
  });
});

describe('shortGpu', () => {
  it('deja el modelo si empieza por una familia conocida', () => {
    expect(shortGpu('RX 6800 XT')).toBe('RX 6800 XT');
    expect(shortGpu('Arc A770 Graphics')).toBe('Arc A770 Graphics');
  });

  it('deja entero un modelo de familia conocida', () => {
    expect(shortGpu('UHD Graphics 630')).toBe('UHD Graphics 630');
  });

  it('recorta a dos palabras el resto', () => {
    expect(shortGpu('Mesa zink Vulkan 1.4')).toBe('Mesa zink');
  });
});

describe('Topbar: contador de juegos', () => {
  const props = {
    query: '',
    suggestions: [],
    showSuggestions: false,
    scanningLaunchers: false,
    hardware: null,
    onQueryChange: () => {},
    onFocus: () => {},
    onBlur: () => {},
    onPickSuggestion: () => {},
  };

  it('sin búsqueda muestra el total', () => {
    const { getByText } = render(<Topbar {...props} gameCount={120} matchedCount={120} />);
    expect(getByText('120 juegos')).toBeTruthy();
  });

  it('con búsqueda muestra cuántos coinciden sobre el total', () => {
    // Antes el badge mostraba solo el número filtrado y parecía que la
    // biblioteca encogía al teclear.
    const { getByText } = render(
      <Topbar {...props} query="zelda" gameCount={120} matchedCount={3} />
    );
    expect(getByText('3 de 120 juegos')).toBeTruthy();
  });
});
