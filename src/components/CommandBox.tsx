import {
  AlertCircle,
  AlertTriangle,
  Check,
  Copy,
  Info,
  Pencil,
  RotateCcw,
  Star,
  Terminal,
} from 'lucide-react';
import { useState } from 'react';
import type { GameInfo, RuntimeEnv } from '../lib/types';
import { preflight, type PreflightCheck } from '../lib/preflight';
import { AppidPicker } from './AppidPicker';
import { GameBadges } from './GameBadges';

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
  /**
   * AppID de Steam, solo para enlazar la etiqueta de ProtonDB con el análisis del
   * juego. Sin él la etiqueta se pinta igual, pero sin acción.
   */
  appid?: number;
  /**
   * Vincula el juego con un AppID de Steam elegido a mano.
   *
   * Sin esta función la etiqueta «sin datos» no se puede pulsar; con ella, la
   * app ofrece buscar el juego cuando no lo reconoce por su nombre.
   */
  onLinkAppid?: (appid: number) => void;
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
  appid,
  onLinkAppid,
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
  // Diálogo para vincular el juego a mano con su ficha de Steam. Solo aplica a
  // juegos sin AppID cuya detección por nombre ha fallado.
  const [picking, setPicking] = useState(false);

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
      <section className="mb-8 p-6 rounded-2xl bg-[var(--error-bg)] border border-error/50 shadow-[0_0_44px_-24px_var(--error)]">
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
    <section className="mb-8 p-6 rounded-2xl bg-[var(--surface)] border border-action/40 shadow-[0_0_56px_-26px_var(--action)]">
      {/* Encabezado: qué juego y en qué estado. El título manda (era el mismo
          tamaño que el de una tarjeta de la biblioteca) y los avisos de
          compatibilidad se agrupan a la derecha, fuera del flujo del texto. */}
      <div className="flex items-start justify-between gap-4 mb-5">
        <div className="min-w-0">
          <p className="flex items-center gap-2 text-[11px] text-[var(--action)] font-bold uppercase tracking-[0.14em] mb-2">
            <Terminal className="w-4 h-4" />
            Comando de lanzamiento
          </p>
          <h2 className="text-3xl font-bold tracking-tight mb-2">{gameName}</h2>
          <p className="text-sm text-[var(--text-muted)] leading-relaxed max-w-2xl">
            {launchCommand === '%command%'
              ? 'Punto de partida sin modificaciones. Activa los ajustes opcionales para realizar pruebas.'
              : `${activeCount} ajuste(s) activo(s). Prueba cada cambio por separado.`}
          </p>
        </div>
        <GameBadges
          native={info.native}
          tier={info.tier}
          appid={appid}
          name={info.name}
          onLink={onLinkAppid ? () => setPicking(true) : undefined}
        />
      </div>

      {/* Vincular el juego con su ficha de Steam cuando no se reconoce por nombre */}
      {picking && onLinkAppid && (
        <AppidPicker
          initialQuery={gameName}
          onClose={() => setPicking(false)}
          onPick={(id) => {
            setPicking(false);
            onLinkAppid(id);
          }}
        />
      )}

      {/* Codebox grande y prominente */}
      <div className="p-4 rounded-xl bg-[var(--bg)] border border-[var(--border-hover)] mb-4">
        <code className="text-[15px] text-[var(--compatible)] font-mono break-all leading-relaxed">
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

      <div className="flex flex-col sm:flex-row gap-3">
        <button
          onClick={onCopy}
          disabled={!!manualCommandError}
          title={manualCommandError || undefined}
          className={`flex-1 flex items-center justify-center gap-2.5 px-6 py-4 rounded-xl font-bold text-base transition-all ${
            manualCommandError
              ? 'bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] cursor-not-allowed'
              : copied
                ? 'bg-[var(--compatible)] text-[var(--bg)]'
                : 'bg-gradient-to-b from-[var(--action-hover)] to-[var(--action)] text-[var(--bg)] shadow-lg shadow-action/30 hover:to-[var(--action-hover)]'
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
          className="px-5 py-4 rounded-xl text-sm font-semibold bg-action/10 text-[var(--action)] border border-action/30 hover:bg-action/20 transition-colors flex items-center justify-center gap-2"
        >
          <Star className="w-4 h-4" />
          Guardar perfil
        </button>
        <button
          onClick={onReset}
          className="px-5 py-4 rounded-xl text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:text-[var(--text)] transition-colors flex items-center justify-center gap-2"
        >
          <RotateCcw className="w-4 h-4" />
          Restablecer
        </button>
      </div>

      {/* Fase 4: Editor manual del comando (solo en modo experto) */}
      {expertMode && (
        <div className="mt-4 p-4 rounded-xl bg-[var(--card)] border border-[var(--border)]">
          <div className="flex items-start justify-between gap-4 mb-3">
            <div className="min-w-0">
              <p className="flex items-center gap-2 text-sm font-semibold text-[var(--text)]">
                <Pencil className="w-4 h-4 text-[var(--text-muted)]" />
                Editar comando manualmente
              </p>
              <p className="text-xs text-[var(--text-muted)] mt-1">
                Modifica el comando directamente. Los cambios manuales sobrescriben las selecciones automáticas.
              </p>
            </div>
            <button
              onClick={onToggleManualEditing}
              className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                manualEditing ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
              }`}
              role="switch"
              aria-checked={manualEditing}
              // El interruptor no lleva texto: sin nombre accesible sería un botón
              // anónimo en el orden de tabulación.
              aria-label="Editar comando manualmente"
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
  env: 'bg-action/10 text-[var(--action)] border-action/25',
  wrapper: 'bg-[var(--caution-bg)] text-[var(--caution)] border-caution/25',
  arg: 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-compatible/25',
};

const PART_TITLES: Record<'env' | 'wrapper' | 'arg', string> = {
  env: 'Variable de entorno: se aplica a todos los procesos del juego',
  wrapper: 'Programa que envuelve el lanzamiento (overlay, gestor de energía…)',
  arg: 'Argumento propio del juego',
};

/** Colores e icono de cada nivel de aviso. */
const SEVERITY_STYLES = {  error: {
    Icon: AlertCircle,
    box: 'bg-[var(--error-bg)] border-error/40',
    text: 'text-[var(--error)]',
  },
  warn: {
    Icon: AlertTriangle,
    box: 'bg-[var(--caution-bg)] border-caution/40',
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
