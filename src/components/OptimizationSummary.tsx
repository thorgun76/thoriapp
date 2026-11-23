import { Activity, Gamepad2, MessageSquare } from 'lucide-react';
import type { ImpactSummary } from '../lib/gameView';

/** Color del badge de estado según el peor impacto de los ajustes activos. */
const TONE_CLASS: Record<ImpactSummary['tone'], string> = {
  danger: 'bg-[var(--error-bg)] text-[var(--error)] border-error/30',
  caution: 'bg-[var(--caution-bg)] text-[var(--caution)] border-caution/30',
  ok: 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-compatible/30',
  none: 'bg-[var(--card)] text-[var(--text-muted)] border-[var(--border)]',
};

/**
 * Resumen del perfil de optimización del juego abierto.
 *
 * Va a todo el ancho, debajo de las pestañas: es contexto (qué se está tocando
 * y con qué respaldo) y no una acción, así que no compite con el comando ni se
 * mete en la columna estrecha de la derecha.
 */
export function OptimizationSummary({
  summary,
  impacts,
  engine,
  appid,
  reports,
}: {
  /** Texto del perfil base (`rules.summary`). */
  summary: string;
  /** Estado de los ajustes activos (ver `summarizeImpacts`). */
  impacts: ImpactSummary;
  /** Motor del juego, si se conoce. */
  engine?: string | null;
  /** AppID de Steam; los juegos de otros launchers no tienen. */
  appid?: number;
  /** Número de reportes de ProtonDB disponibles, si los hay. */
  reports?: number | null;
}) {
  return (
    <section
      aria-label="Perfil de optimización"
      className="mb-6 p-5 rounded-2xl bg-[var(--surface)] border border-[var(--border)]"
    >
      <div className="flex items-start gap-4">
        <span className="hidden sm:flex items-center justify-center w-12 h-12 rounded-xl bg-action/15 text-[var(--action)] shrink-0">
          <Activity className="w-6 h-6" />
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[11px] font-bold uppercase tracking-[0.12em] text-[var(--text-muted)] mb-2">
            Perfil de optimización · Linux
          </p>
          <p className="text-sm text-[var(--text)] leading-relaxed mb-3">{summary}</p>
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={`flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-lg font-semibold border ${TONE_CLASS[impacts.tone]}`}
            >
              <Activity className="w-3.5 h-3.5" />
              {impacts.label}
            </span>
            {engine && (
              <span className="flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-lg bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                <Gamepad2 className="w-3.5 h-3.5" />
                {engine}
              </span>
            )}
            {!!appid && (
              <span className="text-xs px-2.5 py-1 rounded-lg bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                Steam AppID {appid}
              </span>
            )}
            {!!reports && (
              <span className="flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-lg bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                <MessageSquare className="w-3.5 h-3.5" />
                {reports} reportes
              </span>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
