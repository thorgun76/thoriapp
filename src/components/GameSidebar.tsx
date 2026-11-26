import { ListChecks } from 'lucide-react';
import type { Hardware } from '../lib/types';
import type { PerfilesDelJuegoProps } from './PerfilesDelJuego';
import { PerfilesDelJuego } from './PerfilesDelJuego';

interface GameSidebarProps {
  hardware: Hardware | null;
  /**
   * Perfiles de este juego. Va **arriba** de todo: es lo primero con lo que se interactúa
   * al abrir un juego que ya tiene configuraciones guardadas.
   */
  perfiles: Omit<PerfilesDelJuegoProps, 'onGestionar'> & { onGestionar: () => void };
  /** Ya no se pinta: «Pruebas del juego» lleva a Benchmarks con el juego abierto. */
  onMeasurePerformance?: () => void;
  /**
   * Abre la ventana con el historial de pruebas **de este juego**.
   *
   * Es la única acción rápida. Antes había también «Medir y comparar», que abría
   * Benchmarks: la ventana nueva ya tiene su propio botón para eso, así que dos botones
   * para lo mismo sobraban.
   */
  onShowGameTests: () => void;
}

/**
 * Columna derecha de la vista de juego.
 *
 * Orden: perfiles del juego, hardware, acciones rápidas. Los perfiles van primero porque
 * son lo accionable; el hardware es informativo y las acciones son el final natural.
 */
export function GameSidebar({
  hardware,
  perfiles,
  onMeasurePerformance: _onMeasurePerformance,
  onShowGameTests,
}: GameSidebarProps) {
  return (
    <div className="space-y-4">
      {/* Perfiles del juego */}
      <PerfilesDelJuego {...perfiles} />

      {/* Hardware Profile */}
      {hardware && (
        <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
          <h3 className="text-lg font-bold mb-4 text-[var(--text)]">Tu hardware</h3>
          <div className="space-y-2">
            <div className="flex items-start gap-3 text-sm">
              <span className="text-[var(--text-muted)] min-w-[70px]">Sistema</span>
              <strong className="text-[var(--text)] text-right flex-1">{hardware.distro}</strong>
            </div>
            <div className="flex items-start gap-3 text-sm">
              <span className="text-[var(--text-muted)] min-w-[70px]">Gráficos</span>
              <strong className="text-[var(--text)] text-right flex-1 break-words">{hardware.gpu}</strong>
            </div>
            <div className="flex items-start gap-3 text-sm">
              <span className="text-[var(--text-muted)] min-w-[70px]">CPU</span>
              <strong className="text-[var(--text)] text-right flex-1 break-words">{hardware.cpu}</strong>
            </div>
            <div className="flex items-start gap-3 text-sm">
              <span className="text-[var(--text-muted)] min-w-[70px]">RAM</span>
              <strong className="text-[var(--text)] text-right flex-1">
                {hardware.ram_gb.toFixed(1)} GB
              </strong>
            </div>
            <div className="flex items-start gap-3 text-sm">
              <span className="text-[var(--text-muted)] min-w-[70px]">Kernel</span>
              <strong className="text-[var(--text)] text-right flex-1">{hardware.kernel}</strong>
            </div>
            {hardware.ntsync && (
              <div className="flex items-start gap-3 text-sm">
                <span className="text-[var(--text-muted)] min-w-[70px]">NTSYNC</span>
                <strong className="text-[var(--compatible)] text-right flex-1">✓ Disponible</strong>
              </div>
            )}
          </div>
        </aside>
      )}

      {/* Quick Actions */}
      <aside className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <h3 className="text-sm font-bold mb-3 text-[var(--text)]">Acciones rápidas</h3>
        <div className="space-y-2">
          {/* Solo queda una acción: «Medir y comparar» se quitó porque la ventana
              «Pruebas del juego» ya lleva a Benchmarks con el juego abierto, y tener las
              dos aquí era dos botones para lo mismo. `onMeasurePerformance` sigue
              aceptándose (la app lo pasa) pero no se pinta. */}
          <button
            onClick={onShowGameTests}
            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
          >
            <ListChecks className="w-4 h-4" /> Pruebas del juego
          </button>
        </div>
      </aside>
    </div>
  );
}
