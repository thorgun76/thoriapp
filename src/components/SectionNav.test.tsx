import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SectionNav } from './SectionNav';

afterEach(cleanup);

const sections = [
  { id: 'sec-uno', label: 'Uno' },
  { id: 'sec-dos', label: 'Dos' },
];

describe('SectionNav', () => {
  it('no se pinta con menos de dos secciones (un índice de un solo sitio sobra)', () => {
    const { container } = render(<SectionNav sections={[sections[0]]} />);

    expect(container.innerHTML).toBe('');
  });

  it('pinta una entrada por sección', () => {
    render(<SectionNav sections={sections} />);

    expect(screen.getByRole('navigation', { name: /Secciones de esta pestaña/ })).toBeTruthy();
    expect(screen.getByText('Uno')).toBeTruthy();
    expect(screen.getByText('Dos')).toBeTruthy();
  });

  it('al pulsar una entrada salta a su bloque', () => {
    const target = document.createElement('div');
    target.id = 'sec-dos';
    target.scrollIntoView = vi.fn();
    document.body.appendChild(target);

    render(<SectionNav sections={sections} />);
    fireEvent.click(screen.getByText('Dos'));

    expect(target.scrollIntoView).toHaveBeenCalled();
    target.remove();
  });

  it('si el bloque no existe no falla', () => {
    // Los ids dependen de lo que se pinte (p. ej. Proton Pulse solo con datos).
    render(<SectionNav sections={sections} />);

    expect(() => fireEvent.click(screen.getByText('Uno'))).not.toThrow();
  });
});
