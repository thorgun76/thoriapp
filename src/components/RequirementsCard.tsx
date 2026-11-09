import { AlertTriangle, Check, Cpu, HardDrive, HelpCircle, MemoryStick, Monitor } from 'lucide-react';
import type { RequirementCheck, RequirementVerdict, RequirementsReport } from '../lib/types';

/** Icono por tipo de requisito (los desconocidos caen en el genérico). */
function iconFor(kind: string) {
  switch (kind) {
    case 'ram':
    case 'vram':
      return MemoryStick;
    case 'storage':
      return HardDrive;
    case 'cpu':
    case 'hardware':
      return Cpu;
    case 'gpu':
      return Monitor;
    default:
      return HelpCircle;
  }
}

const VERDICT_STYLE: Record<
  RequirementVerdict,
  { text: string; bg: string; border: string; Icon: typeof Check; word: string }
> = {
  ok: {
    text: 'text-[var(--compatible)]',
    bg: 'bg-[var(--compatible-bg)]',
    border: 'border-[var(--compatible)]/30',
    Icon: Check,
    word: 'Cumple',
  },
  tight: {
    text: 'text-[var(--caution)]',
    bg: 'bg-[var(--caution-bg)]',
    border: 'border-[var(--caution)]/40',
    Icon: AlertTriangle,
    word: 'Justo',
  },
  fail: {
    text: 'text-[var(--error)]',
    bg: 'bg-[var(--error-bg)]',
    border: 'border-[var(--error)]/40',
    Icon: AlertTriangle,
    word: 'No cumple',
  },
  unknown: {
    text: 'text-[var(--text-muted)]',
    bg: 'bg-[var(--surface)]',
    border: 'border-[var(--border)]',
    Icon: HelpCircle,
    word: 'Sin dato',
  },
};

/** Título del banner según el peor veredicto. */
export function requirementsHeadline(overall: RequirementVerdict): string | null {
  switch (overall) {
    case 'fail':
      return 'Tu equipo no llega al mínimo de Steam';
    case 'tight':
      return 'Llegas al mínimo, pero justo';
    case 'ok':
      return 'Tu equipo supera el mínimo de Steam';
    default:
      return null;
  }
}

function CheckRow({ check }: { check: RequirementCheck }) {
  const style = VERDICT_STYLE[check.verdict];
  const ItemIcon = iconFor(check.kind);
  // La fila `hardware` es la informativa (CPU/GPU sin tabla de equivalencias) y
  // nunca puede tener veredicto: mostrarle un badge "Sin dato" la haría parecer
  // un fallo de la app. Se pinta como nota, sin badge. Las filas `cpu` y `gpu`
  // sí llevan veredicto desde que hay tabla de rendimiento para las dos.
  const informativo = check.kind === 'hardware';

  return (
    <li className="flex items-start gap-2.5 py-2 border-b border-[var(--border)] last:border-0">
      <ItemIcon className="w-4 h-4 mt-0.5 shrink-0 text-[var(--text-muted)]" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="text-xs text-[var(--text-muted)]">{check.required}</span>
          {/* `actual` solo se mostraba para los importes ("16 GB" va dentro de
              `required`), así que en CPU/GPU el modelo del equipo no aparecía. Con
              la tabla de CPUs eso deja un veredicto sin sujeto: "Cumple" ¿con qué? */}
          {!informativo && check.actual && (
            <span className="text-xs text-[var(--text)]">→ {check.actual}</span>
          )}
          {!informativo && (
            <span className={`text-xs font-semibold ${style.text}`}>{style.word}</span>
          )}
        </div>
        {check.detail && (
          <p className="text-[11px] text-[var(--text-muted)] mt-0.5 leading-relaxed">{check.detail}</p>
        )}
      </div>
      {!informativo && <style.Icon className={`w-4 h-4 mt-0.5 shrink-0 ${style.text}`} />}
    </li>
  );
}

/**
 * Requisitos de Steam frente a la máquina real.
 *
 * Va antes que anti-cheat en la pestaña Compatibilidad: si el equipo no llega al
 * mínimo, no tiene sentido discutir Proton. Nunca dice "cumples" cuando falta el
 * dato — un falso positivo aquí cuesta una compra.
 *
 * `loading` y `error` se muestran como estados propios y no como "sin datos":
 * "todavía no lo sé" y "Steam no lo publica" no son lo mismo.
 */
export function RequirementsCard({
  report,
  loading,
  error,
}: {
  report: RequirementsReport | null;
  loading?: boolean;
  error?: string | null;
}) {
  const headline = report ? requirementsHeadline(report.overall) : null;
  const overall = report ? VERDICT_STYLE[report.overall] : VERDICT_STYLE.unknown;
  // Cuántos campos de sistema publica Steam en el tier mínimo de verdad.
  const publicados = report ? report.raw.filter((r) => r.tier === 'minimum').length : 0;

  if (loading) {
    return (
      <article className="p-5 rounded-xl border bg-[var(--surface)] border-[var(--border)]">
        <h3 className="text-base font-bold text-[var(--text)]">Requisitos del sistema</h3>
        <p className="text-sm text-[var(--text-muted)] mt-1">Comprobando contra tu hardware…</p>
      </article>
    );
  }

  if (error) {
    return (
      <article className="p-5 rounded-xl border bg-[var(--surface)] border-[var(--border)]">
        <h3 className="text-base font-bold text-[var(--text)]">Requisitos del sistema</h3>
        <p className="text-sm text-[var(--text-muted)] mt-1">
          No se pudieron leer los requisitos: {error}
        </p>
      </article>
    );
  }

  if (!report || report.checks.length === 0) {
    return (
      <article className="p-5 rounded-xl border bg-[var(--surface)] border-[var(--border)]">
        <div className="flex items-start gap-3">
          <HelpCircle className="w-5 h-5 mt-0.5 shrink-0 text-[var(--text-muted)]" />
          <div>
            <h3 className="text-base font-bold text-[var(--text)]">Requisitos del sistema</h3>
            <p className="text-sm text-[var(--text-muted)] mt-1">
              No se han podido leer requisitos de sistema para este juego. Puede que Steam no
              los publique o que falle la conexión con la tienda.
            </p>
          </div>
        </div>
      </article>
    );
  }

  return (
    <article className={`p-5 rounded-xl border ${overall.bg} ${overall.border}`}>
      <div className="flex items-start gap-3">
        <overall.Icon className={`w-5 h-5 mt-0.5 shrink-0 ${overall.text}`} />
        <div className="min-w-0 flex-1">
          <h3 className="text-base font-bold text-[var(--text)]">Requisitos del sistema</h3>
          {headline && <p className={`text-sm font-semibold ${overall.text}`}>{headline}</p>}

          <ul className="mt-3">
            {report.checks.map((c, i) => (
              <CheckRow key={`${c.kind}-${c.tier}-${i}`} check={c} />
            ))}
          </ul>

          {report.margin_note && (
            <p className="text-[11px] text-[var(--text-muted)] mt-3 leading-relaxed">
              {report.margin_note}
            </p>
          )}

          {/* Puede que Steam no publique más campos. Se dice para no confundirlo
              con un fallo de la app, pero se cuenta sobre `raw` (lo que publica
              Steam), no sobre `checks`: desde que se filtra el ruido (OS,
              DirectX, red…), un juego con 7 campos puede mostrar solo 3 filas y
              el aviso mentiría. */}
          {publicados > 0 && publicados <= 3 && (
            <p className="text-[11px] text-[var(--text-muted)] mt-2 leading-relaxed">
              Steam solo publica {publicados} {publicados === 1 ? 'dato' : 'datos'} de sistema
              para este juego; el resto no está disponible en la tienda.
            </p>
          )}
        </div>
      </div>
    </article>
  );
}
