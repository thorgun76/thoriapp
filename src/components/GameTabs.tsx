import { ShieldCheck, Zap, type LucideIcon } from 'lucide-react';
import type { View } from '../lib/types';

/** Las dos pestañas que existen dentro de un juego. */
export type GameTab = Extract<View, 'compatibility' | 'recommendations'>;

interface TabDef {
  id: GameTab;
  label: string;
  hint: string;
  Icon: LucideIcon;
}

/**
 * Definición de las pestañas, en el orden en que se pintan.
 *
 * Se declaran aquí y no dentro del componente para que el test pueda comprobar
 * que cada pestaña existe sin depender del marcado.
 */
export const GAME_TABS: TabDef[] = [
  {
    id: 'compatibility',
    label: 'Compatibilidad',
    hint: 'Requisitos, anti-cheat y ProtonDB',
    Icon: ShieldCheck,
  },
  {
    id: 'recommendations',
    label: 'Optimización',
    hint: 'Rendimiento, FPS y ajustes avanzados',
    Icon: Zap,
  },
];

/**
 * Selector de pestaña de la vista de juego.
 *
 * Son dos tarjetas grandes (icono en un cuadro de color + título + resumen) en
 * vez de pestañas finas: es la decisión que más cambia lo que se ve debajo, y
 * como pestaña de texto competía en peso con el comando de lanzamiento.
 *
 * Siguen siendo pestañas (`role="tab"` + `aria-selected`), no enlaces: cambian el
 * contenido de la misma pantalla. La activa se marca con tres señales a la vez
 * (fondo teñido, borde de 2 px a color y título en blanco) para que no dependa
 * solo del color.
 */
export function GameTabs({
  view,
  onSelect,
}: {
  /** Pestaña activa. */
  view: View;
  onSelect: (tab: GameTab) => void;
}) {
  return (
    <div
      className="mb-6 grid grid-cols-1 sm:grid-cols-2 gap-4"
      role="tablist"
      aria-label="Secciones del juego"
    >
      {GAME_TABS.map(({ id, label, hint, Icon }) => {
        const active = view === id;
        return (
          <button
            key={id}
            role="tab"
            type="button"
            aria-selected={active}
            onClick={() => onSelect(id)}
            className={`group flex items-center gap-4 p-5 rounded-2xl border-2 text-left cursor-pointer transition-colors duration-200 motion-reduce:transition-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-action focus-visible:ring-offset-[var(--bg)] ${
              active
                ? 'bg-gradient-to-b from-action/15 to-action/[0.06] border-action shadow-[0_0_28px_-16px_var(--action)]'
                : 'bg-[var(--surface)] border-[var(--border)] hover:border-action/40 hover:bg-[var(--card-hover)]'
            }`}
          >
            <span
              className={`flex items-center justify-center w-16 h-16 rounded-2xl shrink-0 transition-colors duration-200 motion-reduce:transition-none ${
                active
                  ? 'bg-action text-[var(--bg)]'
                  : 'bg-action/15 text-[var(--action)] group-hover:bg-action/25'
              }`}
            >
              <Icon className="w-8 h-8" aria-hidden="true" />
            </span>
            <span className="min-w-0">
              <span
                className={`block text-xl font-bold leading-tight ${
                  active ? 'text-white' : 'text-[var(--text)]'
                }`}
              >
                {label}
              </span>
              <span className="block text-xs text-[var(--text-muted)] mt-1">{hint}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}
