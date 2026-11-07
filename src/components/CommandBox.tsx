import { Check, Copy, Star } from 'lucide-react';
import type { GameInfo } from '../lib/types';

interface CommandBoxProps {
  gameName: string;
  launchCommand: string;
  activeCount: number;
  copied: boolean;
  expertMode: boolean;
  manualEditing: boolean;
  manualCommand: string;
  info: GameInfo;
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
  copied,
  expertMode,
  manualEditing,
  manualCommand,
  info,
  onCopy,
  onSaveProfile,
  onReset,
  onToggleManualEditing,
  onManualCommandChange,
}: CommandBoxProps) {
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
                info.tier.toLowerCase() === 'platinum'
                  ? 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-[var(--compatible)]/30'
                  : info.tier.toLowerCase() === 'gold'
                  ? 'bg-[var(--caution-bg)] text-[var(--caution)] border-[var(--caution)]/30'
                  : 'bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)]'
              }`}
            >
              ProtonDB: {info.tier}
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

      <div className="flex gap-3">
        <button
          onClick={onCopy}
          className={`flex-1 flex items-center justify-center gap-2 px-6 py-3 rounded-xl font-bold text-base transition-all ${
            copied
              ? 'bg-[var(--compatible)] text-[var(--bg)]'
              : 'bg-[var(--action)] text-white hover:bg-[var(--action-hover)]'
          }`}
        >
          {copied ? (
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
            <textarea
              value={manualCommand}
              onChange={(e) => onManualCommandChange(e.target.value)}
              className="w-full p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--compatible)] font-mono focus:outline-none focus:border-[var(--action)] resize-none"
              rows={3}
              placeholder="%command%"
            />
          )}
        </div>
      )}
    </section>
  );
}
