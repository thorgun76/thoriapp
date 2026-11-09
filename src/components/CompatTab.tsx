import { AlertTriangle, ExternalLink, ShieldAlert, ShieldCheck, ShieldQuestion } from 'lucide-react';
import { openExternal } from '../lib/links';
import { tierDescription, tierLabel } from '../lib/format';
import { antiCheatsLabel, antiCheatVerdict, type AcSeverity } from '../lib/anticheat';
import { RequirementsCard } from './RequirementsCard';
import { ProtonPulseCard } from './ProtonPulseCard';
import type {
  AntiCheatInfo,
  Game,
  GameEngine,
  GameInfo,
  GameRules,
  ProtonPulseSummary,
  RequirementsReport,
} from '../lib/types';

interface CompatTabProps {
  current: Game;
  rules: GameRules;
  info: GameInfo;
  engine: GameEngine | null;
  /** Anti-cheat del juego; `null` si no hay dato. */
  antiCheat: AntiCheatInfo | null;
  /** Reportes de Proton Pulse filtrados por la GPU del equipo; `null` = sin datos. */
  protonPulse?: ProtonPulseSummary | null;
  protonPulseLoading?: boolean;
  /** Copia al portapapeles las opciones de lanzamiento de un reporte ajeno. */
  onCopyLaunchOptions?: (options: string) => void;
  /** Requisitos de Steam comparados con esta máquina; `null` mientras carga. */
  requirements?: RequirementsReport | null;
  requirementsLoading?: boolean;
  requirementsError?: string | null;
}

const AC_STYLES: Record<AcSeverity, { box: string; text: string; Icon: typeof ShieldCheck }> = {
  ok: {
    box: 'bg-[var(--compatible-bg)] border-[var(--compatible)]/30',
    text: 'text-[var(--compatible)]',
    Icon: ShieldCheck,
  },
  caution: {
    box: 'bg-[var(--caution-bg)] border-[var(--caution)]/40',
    text: 'text-[var(--caution)]',
    Icon: AlertTriangle,
  },
  blocked: {
    box: 'bg-[var(--error-bg)] border-[var(--error)]/40',
    text: 'text-[var(--error)]',
    Icon: ShieldAlert,
  },
  unknown: {
    box: 'bg-[var(--surface)] border-[var(--border)]',
    text: 'text-[var(--text-muted)]',
    Icon: ShieldQuestion,
  },
};

/** Aviso de anti-cheat: lo primero que hay que saber antes de configurar nada. */
function AntiCheatCard({ info }: { info: AntiCheatInfo | null }) {
  const verdict = antiCheatVerdict(info);
  const { box, text, Icon } = AC_STYLES[verdict.severity];
  const cheats = antiCheatsLabel(info);

  return (
    <article className={`p-5 rounded-xl border ${box}`}>
      <div className="flex items-start gap-3">
        <Icon className={`w-5 h-5 mt-0.5 shrink-0 ${text}`} />
        <div className="min-w-0">
          <h3 className="text-base font-bold text-[var(--text)]">Anti-cheat</h3>
          <p className={`text-sm font-semibold ${text}`}>{verdict.label}</p>
          {cheats && <p className="text-xs text-[var(--text-muted)] mt-1">Usa: {cheats}</p>}
          <p className="text-sm text-[var(--text-muted)] mt-2 leading-relaxed">{verdict.detail}</p>
          {verdict.advice && (
            <p className="text-sm text-[var(--text)] mt-2 leading-relaxed">{verdict.advice}</p>
          )}
          {info?.note && (
            <p className="text-xs text-[var(--text-muted)] mt-2 italic leading-relaxed">
              Último cambio conocido: {info.note}
            </p>
          )}
          {info?.reference && (
            <button
              onClick={() => openExternal(info.reference!)}
              className="mt-3 flex items-center gap-2 text-xs font-semibold text-[var(--action)] hover:underline"
            >
              <ExternalLink className="w-3.5 h-3.5" /> Ver la fuente
            </button>
          )}
        </div>
      </div>
    </article>
  );
}

/** Pestaña "Compatibilidad": datos de ProtonDB, nativo/Proton y cuello de botella. */
export function CompatTab({
  current,
  rules,
  info,
  engine,
  antiCheat,
  protonPulse = null,
  protonPulseLoading = false,
  onCopyLaunchOptions,
  requirements = null,
  requirementsLoading = false,
  requirementsError = null,
}: CompatTabProps) {
  return (
    <div className="space-y-4">
                        {/* Antes que anti-cheat y antes que ProtonDB: si el equipo no
                            llega al mínimo, lo demás es secundario. */}
                        <RequirementsCard
                          report={requirements}
                          loading={requirementsLoading}
                          error={requirementsError}
                        />

                        {/* Va antes de los ajustes: si el anti-cheat bloquea, no sirven de nada. */}
                        <AntiCheatCard info={antiCheat} />

                        {/* Después del anti-cheat (si bloquea, estos reportes no
                            importan) pero antes del bloque de información: aquí sí
                            hay hardware concreto del que hablar. */}
                        <ProtonPulseCard
                          summary={protonPulse}
                          loading={protonPulseLoading}
                          onCopyOptions={(o) => onCopyLaunchOptions?.(o)}
                        />

                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <h3 className="text-base font-bold mb-3 text-[var(--text)]">Información del juego</h3>
                          <div className="space-y-3">
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">Nombre</span>
                              <strong className="text-[var(--text)]">{info.name || current.name}</strong>
                            </div>
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">AppID</span>
                              <strong className="text-[var(--text)]">{current.appid}</strong>
                            </div>
                            <div className="flex justify-between text-sm">
                              <span className="text-[var(--text-muted)]">Ejecución</span>
                              <strong className={info.native ? 'text-[var(--compatible)]' : 'text-[var(--caution)]'}>
                                {info.native ? 'Linux nativo' : 'Requiere Proton'}
                              </strong>
                            </div>
                            {engine && (
                              <div className="flex justify-between text-sm">
                                <span className="text-[var(--text-muted)]">API gráfica</span>
                                <strong className="text-[var(--text)]">{engine.api.toUpperCase()}</strong>
                              </div>
                            )}
                          </div>
                        </article>

                        {info.tier && (
                          <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <h3 className="text-base font-bold mb-3 text-[var(--text)]">Compatibilidad ProtonDB</h3>
                            <div className="space-y-3">
                              <div className="flex justify-between text-sm">
                                <span className="text-[var(--text-muted)]">Tier</span>
                                <strong className={`font-bold ${
                                  info.tier.toLowerCase() === 'platinum' ? 'text-[var(--compatible)]' :
                                  info.tier.toLowerCase() === 'gold' ? 'text-[var(--caution)]' :
                                  info.tier.toLowerCase() === 'silver' ? 'text-[var(--text)]' :
                                  info.tier.toLowerCase() === 'bronze' ? 'text-[var(--caution)]' :
                                  'text-[var(--error)]'
                                }`}>
                                  {tierLabel(info.tier)}
                                </strong>
                              </div>
                              {/* Un tier solo no dice si hay que tocar algo: se explica. */}
                              {tierDescription(info.tier) && (
                                <p className="text-xs text-[var(--text-muted)] leading-relaxed">
                                  {tierDescription(info.tier)}
                                </p>
                              )}
                              {info.tier.toLowerCase() === 'borked' && (
                                <p className="flex items-start gap-2 text-xs text-[var(--caution)] leading-relaxed">
                                  <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
                                  Antes de invertir tiempo: ProtonDB recoge que este juego no arranca.
                                  Los ajustes de esta app no lo van a arreglar por sí solos.
                                </p>
                              )}
                              {info.total && (
                                <div className="flex justify-between text-sm">
                                  <span className="text-[var(--text-muted)]">Reportes</span>
                                  <strong className="text-[var(--text)]">{info.total}</strong>
                                </div>
                              )}
                              {info.confidence && (
                                <div className="flex justify-between text-sm">
                                  <span className="text-[var(--text-muted)]">Confianza</span>
                                  <strong className="text-[var(--text)]">
                                    {info.confidence.toLowerCase() === 'strong' ? 'Alta' :
                                     info.confidence.toLowerCase() === 'moderate' ? 'Moderada' :
                                     info.confidence.toLowerCase() === 'weak' ? 'Baja' :
                                     info.confidence}
                                  </strong>
                                </div>
                              )}
                            </div>
                            <button
                              onClick={() => openExternal(`https://www.protondb.com/app/${current.appid}`)}
                              className="mt-4 w-full flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors"
                            >
                              <ExternalLink className="w-4 h-4" /> Ver en ProtonDB
                            </button>
                          </article>
                        )}

                        <article className="p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <h3 className="text-base font-bold mb-3 text-[var(--text)]">Cuello de botella</h3>
                          <p className="text-sm text-[var(--text-muted)] mb-3">{rules.bottleneck}</p>
                          <p className="text-sm text-[var(--text)]">{rules.summary}</p>
                        </article>
    </div>
  );
}
