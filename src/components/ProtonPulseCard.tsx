import { Activity, Copy, Users } from 'lucide-react';
import type { ProtonPulseSummary } from '../lib/types';

/**
 * Reportes reales de Proton Pulse para ESTE juego y filtrados por la GPU del equipo.
 *
 * Es distinto de la tabla genérica de la comunidad: allí dice «RADV suele ir bien en
 * AMD» sin saber qué juego; aquí son usuarios que lanzaron este juego con una tarjeta
 * del mismo vendor, con su versión de Proton y sus opciones de lanzamiento.
 *
 * Reglas de presentación:
 *   - Sin datos (`null`) no se pinta nada: una tarjeta vacía se lee como un fallo.
 *   - Nunca se dice «funciona en tu GPU» con un reporte suelto: se dan los números.
 *   - Las opciones de otros se muestran tal cual, con un botón para copiarlas, y se
 *     avisa de que son de otra persona: no son una recomendación de la app.
 */

const RATING_STYLE: Record<string, string> = {
  platinum: 'text-[var(--compatible)]',
  gold: 'text-[var(--compatible)]',
  silver: 'text-[var(--text)]',
  bronze: 'text-[var(--caution)]',
  borked: 'text-[var(--error)]',
};

const RATING_WORD: Record<string, string> = {
  platinum: 'Platino',
  gold: 'Oro',
  silver: 'Plata',
  bronze: 'Bronce',
  borked: 'No arranca',
};

const VENDOR_WORD: Record<string, string> = {
  amd: 'AMD',
  nvidia: 'NVIDIA',
  intel: 'Intel',
};

/** Cuántos reportes se listan: los demás sobran y alargan la tarjeta. */
const MAX_SHOWN = 4;

function ratingWord(rating: string): string {
  return RATING_WORD[rating.toLowerCase()] ?? rating;
}

/** Frase del veredicto. Sin un mínimo de reportes no se afirma nada. */
export function protonPulseHeadline(summary: ProtonPulseSummary): string {
  const vendor = summary.vendor ? VENDOR_WORD[summary.vendor] ?? summary.vendor : null;
  const quien = vendor ? `en ${vendor}` : 'en general';

  if (summary.matching < 2) {
    return `Solo ${summary.matching} reporte ${quien}: tómalo como anécdota, no como dato.`;
  }
  if (summary.borked > summary.matching / 2) {
    return `Más de la mitad de los reportes ${quien} dicen que no arranca.`;
  }
  if (summary.good === summary.matching) {
    return `${summary.matching} reportes ${quien}, todos entre platino y oro.`;
  }
  return `${summary.good} de ${summary.matching} reportes ${quien} están entre platino y oro.`;
}

export function ProtonPulseCard({
  summary,
  loading,
  onCopyOptions,
}: {
  summary: ProtonPulseSummary | null;
  loading?: boolean;
  onCopyOptions: (options: string) => void;
}) {
  if (loading) {
    return (
      <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <h3 className="text-base font-bold text-[var(--text)]">Reportes de Proton Pulse</h3>
        <p className="text-sm text-[var(--text-muted)] mt-1">Buscando reportes de tu GPU…</p>
      </article>
    );
  }

  // Sin reportes no se inventa una tarjeta: muchos juegos no están en Proton Pulse
  // y eso no es un problema que haya que anunciar en cada ficha.
  if (!summary) return null;

  const shown = summary.reports.slice(0, MAX_SHOWN);
  const vendorWord = summary.vendor ? VENDOR_WORD[summary.vendor] ?? summary.vendor : null;

  return (
    <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
      <div className="flex items-start gap-3">
        <Users className="w-5 h-5 mt-0.5 shrink-0 text-[var(--action)]" />
        <div className="min-w-0 flex-1">
          <h3 className="text-base font-bold text-[var(--text)]">Reportes de Proton Pulse</h3>
          <p className="text-sm font-semibold text-[var(--action)] mt-0.5">
            {protonPulseHeadline(summary)}
          </p>
          <p className="text-xs text-[var(--text-muted)] mt-1 leading-relaxed">
            {summary.total} reportes del juego en total
            {vendorWord ? `, ${summary.matching} con tarjeta ${vendorWord}` : ''}
            {summary.same_family > 0 && vendorWord
              ? ` · ${summary.same_family} de la misma generación`
              : ''}
            {summary.borked > 0 ? ` · ${summary.borked} sin arrancar` : ''}
          </p>

          <ul className="mt-3 space-y-3">
            {shown.map((r, i) => (
              <li key={`${r.gpu}-${i}`} className="text-xs">
                <div className="flex items-start gap-2">
                  <Activity
                    className={`w-3.5 h-3.5 mt-0.5 shrink-0 ${
                      RATING_STYLE[r.rating.toLowerCase()] ?? 'text-[var(--text-muted)]'
                    }`}
                  />
                  <div className="min-w-0 flex-1">
                    <p className="text-[var(--text)] leading-relaxed">
                      <span
                        className={`font-semibold ${
                          RATING_STYLE[r.rating.toLowerCase()] ?? 'text-[var(--text-muted)]'
                        }`}
                      >
                        {ratingWord(r.rating)}
                      </span>{' '}
                      · {r.gpu}
                      {r.proton_version && (
                        <span className="text-[var(--text-muted)]"> · Proton {r.proton_version}</span>
                      )}
                    </p>
                    {r.launch_options && (
                      <div className="mt-1 flex items-start gap-2">
                        <code className="flex-1 min-w-0 break-all font-mono text-[11px] text-[var(--compatible)] bg-[var(--bg)] rounded px-2 py-1 border border-[var(--border)]">
                          {r.launch_options}
                        </code>
                        <button
                          onClick={() => onCopyOptions(r.launch_options!)}
                          title="Copiar estas opciones de lanzamiento"
                          className="shrink-0 p-1 rounded text-[var(--text-muted)] hover:text-[var(--action)] hover:bg-[var(--card-hover)]"
                        >
                          <Copy className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              </li>
            ))}
          </ul>

          <p className="text-[11px] text-[var(--text-muted)] mt-3 leading-relaxed">
            Reportes de otros usuarios en Proton Pulse, no una recomendación de esta app: cada
            tarjeta rinde distinto y las opciones de lanzamiento son suyas.
          </p>
        </div>
      </div>
    </article>
  );
}
