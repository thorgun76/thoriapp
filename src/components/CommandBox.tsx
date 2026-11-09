import { AlertCircle, AlertTriangle, Check, Copy, Info, Star } from 'lucide-react';
import type { GameInfo, RuntimeEnv } from '../lib/types';
import { preflight, type PreflightCheck } from '../lib/preflight';
import { tierDescription, tierLabel } from '../lib/format';

/** Clases del badge de ProtonDB según el tier (verde → naranja → rojo). */
export function tierColor(tier: string): string {
  switch (tier.toLowerCase()) {
    case 'platinum':
      return 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-[var(--compatible)]/30';
    case 'gold':
    case 'bronze':
      return 'bg-[var(--caution-bg)] text-[var(--caution)] border-[var(--caution)]/30';
    case 'silver':
      return 'bg-[var(--surface)] text-[var(--text)] border-[var(--border)]';
    default:
      return 'bg-[var(--error-bg)] text-[var(--error)] border-[var(--error)]/30';
  }
}

interface CommandBoxProps {
  gameName: string;
  launchCommand: string;
  activeCount: number;
  /**
   * Qué lleva el comando ahora mismo, para resumirlo en chips debajo del código.
   * `{ value, kind }` con `kind` en `env` (variable), `wrapper` (envoltorio como
   * gamemoderun/mangohud) y `arg` (bandera del juego).
   */
  activeParts?: { value: string; kind: 'env' | 'wrapper' | 'arg' }[];
  copied: boolean;
  expertMode: boolean;
  manualEditing: boolean;
  manualCommand: string;
  /**
   * Problema detectado en el comando manual (vacío si está bien).
   *
   * Un comando manual puede llegar de un perfil importado, así que no se da por
   * bueno: si lleva caracteres que encadenan órdenes o un ejecutable que no es un
   * lanzador conocido, se muestra el motivo y se bloquea copiarlo.
   */
  manualCommandError?: string;
  info: GameInfo;
  /** Estado real de la máquina; `null` mientras carga (no se muestran avisos). */
  env: RuntimeEnv | null;
  /** Launcher del juego (`steam`, `lutris`…) para avisos de sandbox. */
  launcher?: string;
  /**
   * Anti-cheat denegado: **no hay comando**. La caja deja de ofrecer copiar,
   * guardar o editar, y explica por qué. El texto llega ya montado en
   * `launchCommand`/`blockedReason` para no duplicar el criterio de `isDenied`.
   */
  blocked?: boolean;
  /** Motivo del bloqueo (mismo texto que usa el portapapeles). */
  blockedReason?: string;
  onCopy: () => void;
  onSaveProfile: () => void;
  onReset: () => void;
  onToggleManualEditing: () => void;
  onManualCommandChange: (value: string) => void;
}

/** El comando de lanzamiento: elemento protagonista de la vista de juego. */
export function CommandBox({
  gameName,
  launchCommand,
  activeCount,
  activeParts = [],
  copied,
  expertMode,
  manualEditing,
  manualCommand,
  manualCommandError = '',
  info,
  env,
  launcher,
  blocked = false,
  blockedReason = '',
  onCopy,
  onSaveProfile,
  onReset,
  onToggleManualEditing,
  onManualCommandChange,
}: CommandBoxProps) {
  // Revisión previa: los problemas que se pagan con un "el juego no arranca",
  // detectados antes de copiar el comando al lanzador. Con el juego bloqueado no
  // se revisa nada: no hay comando que revisar y los avisos serían ruido.
  const checks = blocked
    ? []
    : preflight(launchCommand, env, {
        native: info.native === true,
        launcher,
        manual: manualEditing,
      });

  if (blocked) {
    return (
      <section className="mb-8 p-6 rounded-2xl bg-[var(--error-bg)] border-2 border-[var(--error)]/50">
        <div className="flex items-start gap-3">
          <AlertCircle className="w-6 h-6 mt-0.5 shrink-0 text-[var(--error)]" />
          <div className="min-w-0">
            <p className="text-xs text-[var(--error)] font-bold uppercase tracking-wider mb-2">
              Sin comando de lanzamiento
            </p>
            <h2 className="text-2xl font-bold mb-1">{gameName}</h2>
            <p className="text-sm text-[var(--text)] leading-relaxed">{blockedReason}</p>
            <p className="text-xs text-[var(--text-muted)] mt-3 leading-relaxed">
              Ningún parámetro de esta app cambia una decisión del desarrollador. Si el dato
              cambia, la lista de anti-cheat se refresca sola una vez al día.
            </p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="mb-8 p-6 rounded-2xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-lg shadow-[var(--action)]/10">
      <div className="flex items-start justify-between mb-4">
        <div>
          <p className="text-xs text-[var(--action)] font-bold uppercase tracking-wider mb-2">
            Comando de lanzamiento
          </p>
          <h2 className="text-2xl font-bold mb-1">{gameName}</h2>
          <p className="text-sm text-[var(--text-muted)]">
            {launchCommand === '%command%'
              ? 'Punto de partida sin modificaciones. Activa los ajustes opcionales para realizar pruebas.'
              : `${activeCount} ajuste(s) activo(s). Prueba cada cambio por separado.`}
          </p>
        </div>
        <div className="flex flex-col gap-2">
          {info.native === true && (
            <span className="text-xs px-3 py-1 rounded-full bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30 font-semibold">
              ● Linux nativo
            </span>
          )}
          {info.tier && (
            <span
              className={`text-xs px-3 py-1 rounded-full border font-semibold ${
                tierColor(info.tier)
              }`}
              title={tierDescription(info.tier) ?? undefined}
            >
              ProtonDB: {tierLabel(info.tier)}
            </span>
          )}
        </div>
      </div>

      {/* Codebox grande y prominente */}
      <div className="p-5 rounded-xl bg-[var(--bg)] border-2 border-[var(--border)] mb-4">
        <code className="text-base text-[var(--compatible)] font-mono break-all leading-relaxed">
          {launchCommand}
        </code>
      </div>

      {/* Resumen de lo que lleva el comando. El código de arriba es la verdad,
          pero leer una cadena larga de variables no dice de un vistazo qué se
          está metiendo en el juego. Los ajustes del juego (`config`) no salen
          aquí a propósito: no forman parte del comando. */}
      {activeParts.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5 mb-4">
          <span className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mr-1">
            Incluye
          </span>
          {activeParts.map((part, i) => (
            <span
              key={`${part.kind}-${part.value}-${i}`}
              className={`text-[11px] px-2 py-1 rounded-md font-mono border ${PART_STYLES[part.kind]}`}
              title={PART_TITLES[part.kind]}
            >
              {part.value}
            </span>
          ))}
        </div>
      )}

      {/* Comprobaciones sobre el sistema real (wrappers, capa Vulkan, sandbox…) */}
      {checks.length > 0 && (
        <ul className="mb-4 space-y-2">
          {checks.map((check) => (
            <PreflightRow key={check.id} check={check} />
          ))}
        </ul>
      )}
      {env && checks.length === 0 && (
        <p className="mb-4 text-xs text-[var(--text-muted)] flex items-center gap-1.5">
          <Check className="w-3.5 h-3.5 text-[var(--compatible)]" />
          Todo lo que usa este comando está instalado en tu sistema.
        </p>
      )}

      <div className="flex gap-3">
        <button
          onClick={onCopy}
          disabled={!!manualCommandError}
          title={manualCommandError || undefined}
          className={`flex-1 flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-bold text-base transition-all ${
            manualCommandError
              ? 'bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] cursor-not-allowed'
              : copied
                ? 'bg-[var(--compatible)] text-[var(--bg)]'
                : 'bg-[var(--action)] text-white hover:bg-[var(--action-hover)]'
          }`}
        >
          {manualCommandError ? (
            <>
              <AlertCircle className="w-5 h-5" /> Revisa el comando
            </>
          ) : copied ? (
            <>
              <Check className="w-5 h-5" /> ¡Copiado!
            </>
          ) : (
            <>
              <Copy className="w-5 h-5" /> Copiar comando
            </>
          )}
        </button>
        <button
          onClick={onSaveProfile}
          className="px-4 py-3 rounded-xl text-sm font-semibold bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30 hover:bg-[var(--action)]/20 transition-colors flex items-center gap-2"
        >
          <Star className="w-4 h-4" />
          Guardar perfil
        </button>
        <button
          onClick={onReset}
          className="px-4 py-3 rounded-xl text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
        >
          Restablecer
        </button>
      </div>

      {/* Fase 4: Editor manual del comando (solo en modo experto) */}
      {expertMode && (
        <div className="mt-4 p-4 rounded-xl bg-[var(--card)] border border-[var(--border)]">
          <div className="flex items-center justify-between mb-3">
            <div>
              <p className="text-sm font-semibold text-[var(--text)]">Editar comando manualmente</p>
              <p className="text-xs text-[var(--text-muted)] mt-1">
                Modifica el comando directamente. Los cambios manuales sobrescriben las selecciones automáticas.
              </p>
            </div>
            <button
              onClick={onToggleManualEditing}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                manualEditing ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
              }`}
              role="switch"
              aria-checked={manualEditing}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
                  manualEditing ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
          {manualEditing && (
            <>
              <textarea
                value={manualCommand}
                onChange={(e) => onManualCommandChange(e.target.value)}
                aria-invalid={!!manualCommandError}
                aria-describedby={manualCommandError ? 'manual-command-error' : undefined}
                className={`w-full p-3 rounded-lg bg-[var(--bg)] border text-sm text-[var(--compatible)] font-mono focus:outline-none resize-none ${
                  manualCommandError
                    ? 'border-[var(--error)]'
                    : 'border-[var(--border)] focus:border-[var(--action)]'
                }`}
                rows={3}
                placeholder="%command%"
              />
              {manualCommandError && (
                <p
                  id="manual-command-error"
                  role="alert"
                  className="mt-2 text-xs text-[var(--error)] flex items-start gap-1.5"
                >
                  <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
                  <span>{manualCommandError}</span>
                </p>
              )}
            </>
          )}
        </div>
      )}
    </section>
  );
}

/** Color de los chips del resumen, por tipo de pieza del comando. */
const PART_STYLES: Record<'env' | 'wrapper' | 'arg', string> = {
  env: 'bg-[var(--action)]/10 text-[var(--action)] border-[var(--action)]/25',
  wrapper: 'bg-[var(--caution-bg)] text-[var(--caution)] border-[var(--caution)]/25',
  arg: 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-[var(--compatible)]/25',
};

const PART_TITLES: Record<'env' | 'wrapper' | 'arg', string> = {
  env: 'Variable de entorno: se aplica a todos los procesos del juego',
  wrapper: 'Programa que envuelve el lanzamiento (overlay, gestor de energía…)',
  arg: 'Argumento propio del juego',
};

/** Colores e icono de cada nivel de aviso. */
const SEVERITY_STYLES = {  error: {
    Icon: AlertCircle,
    box: 'bg-[var(--error-bg)] border-[var(--error)]/40',
    text: 'text-[var(--error)]',
  },
  warn: {
    Icon: AlertTriangle,
    box: 'bg-[var(--caution-bg)] border-[var(--caution)]/40',
    text: 'text-[var(--caution)]',
  },
  info: {
    Icon: Info,
    box: 'bg-[var(--surface)] border-[var(--border)]',
    text: 'text-[var(--text-muted)]',
  },
} as const;

function PreflightRow({ check }: { check: PreflightCheck }) {
  const { Icon, box, text } = SEVERITY_STYLES[check.severity];

  return (
    <li className={`flex items-start gap-3 p-3 rounded-xl border ${box}`}>
      <Icon className={`w-4 h-4 mt-0.5 shrink-0 ${text}`} />
      <div className="min-w-0">
        <p className={`text-sm font-semibold ${text}`}>{check.title}</p>
        <p className="text-xs text-[var(--text-muted)] mt-0.5">{check.detail}</p>
        {check.hint && (
          <p className="text-xs text-[var(--text-muted)] mt-1 opacity-80">{check.hint}</p>
        )}
      </div>
    </li>
  );
}
