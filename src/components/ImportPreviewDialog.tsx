import { AlertCircle, AlertTriangle, Check, FileJson, ShieldAlert, X } from 'lucide-react';
import type { ImportPreview } from '../lib/types';

interface ImportPreviewDialogProps {
  preview: ImportPreview;
  fileName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Cuadro que muestra qué se va a importar ANTES de guardarlo.
 *
 * Importar era antes una sola llamada: el archivo se escribía en disco y el
 * usuario se enteraba después de lo que traía. Ahora se analiza primero y aquí se
 * ve, perfil a perfil, si se conserva su comando manual o se le quita y por qué.
 */
export function ImportPreviewDialog({
  preview,
  fileName,
  onConfirm,
  onCancel,
}: ImportPreviewDialogProps) {
  const total = preview.profiles.length;
  const nadaQueImportar = total === 0;

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/60"
      onClick={onCancel}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-preview-title"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-2xl max-h-[85vh] flex flex-col rounded-xl bg-[var(--surface)] border-2 border-[var(--border)] shadow-2xl"
      >
        <div className="flex items-start justify-between gap-3 p-5 border-b border-[var(--border)]">
          <div className="min-w-0">
            <h4
              id="import-preview-title"
              className="text-base font-bold text-[var(--text)] flex items-center gap-2"
            >
              <FileJson className="w-4 h-4 text-[var(--action)] shrink-0" />
              Revisa antes de importar
            </h4>
            <p className="text-xs text-[var(--text-muted)] mt-1 truncate" title={fileName}>
              {fileName} · {total} {total === 1 ? 'perfil' : 'perfiles'}
              {preview.duplicates > 0 && ` · ${preview.duplicates} ya guardado(s)`}
            </p>
          </div>
          <button
            type="button"
            onClick={onCancel}
            aria-label="Cerrar"
            className="p-1.5 rounded-lg text-[var(--text-muted)] hover:text-[var(--text)] hover:bg-[var(--card-hover)] transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-5 space-y-3">
          {nadaQueImportar && (
            <p className="text-sm text-[var(--text-muted)] flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5 text-[var(--caution)]" />
              <span>
                {preview.duplicates > 0
                  ? 'Todos los perfiles del archivo ya están guardados. No hay nada que importar.'
                  : 'El archivo no contiene perfiles que importar.'}
              </span>
            </p>
          )}

          {preview.stripped_commands > 0 && (
            <p className="text-xs text-[var(--caution)] bg-[var(--caution-bg)] border border-[var(--caution)]/40 rounded-lg p-3 flex items-start gap-2">
              <ShieldAlert className="w-4 h-4 shrink-0 mt-0.5" />
              <span>
                {preview.stripped_commands === 1
                  ? 'Un perfil trae un comando manual que no se puede verificar.'
                  : `${preview.stripped_commands} perfiles traen un comando manual que no se puede verificar.`}{' '}
                Se importarán igual, pero <strong>sin ese comando</strong>: sus opciones se
                conservan y el comando lo tendrías que escribir tú.
              </span>
            </p>
          )}

          {preview.profiles.map((p) => (
            <div
              key={p.id}
              className="p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]"
            >
              <div className="flex items-baseline justify-between gap-2">
                <p className="text-sm font-semibold text-[var(--text)] truncate">{p.name}</p>
                <p className="text-xs text-[var(--text-dim)] shrink-0">
                  {p.game_name}
                  {p.appid > 0 && ` · ${p.appid}`}
                </p>
              </div>

              {p.enabled_params.length > 0 && (
                <p className="mt-1.5 text-xs font-mono text-[var(--text-muted)] break-words">
                  {p.enabled_params.join(' · ')}
                </p>
              )}

              {p.manual_command ? (
                <div className="mt-2">
                  <p className="text-[11px] text-[var(--text-dim)] mb-1 flex items-center gap-1">
                    {p.keeps_manual_command ? (
                      <>
                        <Check className="w-3 h-3 text-[var(--compatible)]" />
                        Comando manual (se conserva)
                      </>
                    ) : (
                      <>
                        <AlertCircle className="w-3 h-3 text-[var(--error)]" />
                        Comando manual que se va a quitar
                      </>
                    )}
                  </p>
                  <code
                    className={`block px-2 py-1.5 rounded text-xs font-mono break-all ${
                      p.keeps_manual_command
                        ? 'bg-[var(--compatible-bg)] text-[var(--compatible)]'
                        : 'bg-[var(--error-bg)] text-[var(--error)] line-through'
                    }`}
                  >
                    {p.manual_command}
                  </code>
                  {!p.keeps_manual_command && p.manual_command_reason && (
                    <p className="mt-1.5 text-[11px] text-[var(--error)]">
                      {p.manual_command_reason}
                    </p>
                  )}
                </div>
              ) : (
                <p className="mt-2 text-[11px] text-[var(--text-dim)]">
                  Sin comando manual: usa el que genera la app.
                </p>
              )}
            </div>
          ))}
        </div>

        <div className="flex justify-end gap-2 p-5 border-t border-[var(--border)]">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--bg)] text-[var(--text)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
          >
            Cancelar
          </button>
          <button
            type="button"
            autoFocus
            disabled={nadaQueImportar}
            onClick={onConfirm}
            className={`px-4 py-2 rounded-lg text-sm font-semibold text-white transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)] ${
              nadaQueImportar
                ? 'bg-[var(--border)] text-[var(--text-muted)] cursor-not-allowed'
                : 'bg-[var(--action)] hover:bg-[var(--action-hover)]'
            }`}
          >
            {nadaQueImportar
              ? 'Importar'
              : total === 1
                ? 'Importar perfil'
                : `Importar ${total} perfiles`}
          </button>
        </div>
      </div>
    </div>
  );
}
