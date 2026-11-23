import { TrendingUp } from 'lucide-react';
import type { Hardware } from '../lib/types';

interface GameSidebarProps {
  hardware: Hardware | null;
  onMeasurePerformance: () => void;
}

/** Columna derecha de la vista de juego: hardware del equipo y acciones rápidas. */
export function GameSidebar({ hardware, onMeasurePerformance }: GameSidebarProps) {
  return (
    <div className="space-y-4">
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
          <button
            onClick={onMeasurePerformance}
            className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
          >
            <TrendingUp className="w-4 h-4" /> Medir y comparar
          </button>
        </div>
      </aside>
    </div>
  );
}
