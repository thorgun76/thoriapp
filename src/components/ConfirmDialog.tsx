import { useState } from 'react';
import type { ConfirmRequest } from '../hooks/useConfirm';

/**
 * Cuadro de confirmación con el estilo de la app.
 *
 * Se usa en lugar de `window.confirm()` y `window.prompt()`, que en Tauri abren
 * el diálogo nativo del sistema y rompen la estética de la ventana.
 */
export function ConfirmDialog({
  state,
  onClose,
}: {
  state: ConfirmRequest;
  onClose: () => void;
}) {
  // Solo se usa cuando el cuadro pide un valor (sustituye a prompt()).
  const [value, setValue] = useState(state.input?.defaultValue ?? '');

  const confirm = () => {
    const { input, onConfirm } = state;
    const current = value.trim();
    onClose();
    if (input) {
      if (current) input.onSubmit(current);
      return;
    }
    onConfirm?.();
  };

  const primaryClass =
    state.tone === 'primary'
      ? 'bg-[var(--action)] hover:bg-[var(--action-hover)] focus-visible:ring-2 focus-visible:ring-[var(--action)]'
      : 'bg-[var(--error)] hover:opacity-90 focus-visible:ring-2 focus-visible:ring-[var(--error)]';

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        onClick={(e) => e.stopPropagation()}
        className="w-full max-w-sm p-5 rounded-xl bg-[var(--surface)] border-2 border-[var(--border)] shadow-2xl"
      >
        <h4 id="confirm-title" className="text-base font-bold text-[var(--text)] mb-2">
          {state.title}
        </h4>
        <p className="text-sm text-[var(--text-muted)] mb-5">{state.message}</p>

        {state.input && (
          <label className="block mb-5">
            <span className="block text-xs font-semibold text-[var(--text-muted)] mb-1.5">
              {state.input.label}
            </span>
            <input
              type="text"
              autoFocus
              value={value}
              placeholder={state.input.placeholder}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') confirm();
              }}
              className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm text-[var(--text)] font-mono focus:outline-none focus:border-[var(--action)]/60 focus:ring-2 focus:ring-[var(--action)]/20"
            />
          </label>
        )}

        <div className="flex justify-end gap-2">
          <button
            type="button"
            autoFocus={!state.input}
            onClick={onClose}
            className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--bg)] text-[var(--text)] border border-[var(--border)] hover:border-[var(--action)]/50 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--action)]"
          >
            Cancelar
          </button>
          <button
            type="button"
            onClick={confirm}
            className={`px-4 py-2 rounded-lg text-sm font-semibold text-white transition-colors focus-visible:outline-none ${primaryClass}`}
          >
            {state.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Aviso breve en la esquina inferior derecha. Es el mismo toast que usan
 * Pruebas y Mis pruebas, para que todos los avisos se vean igual.
 */
export function Toast({ message }: { message: string }) {
  return (
    <div
      role="status"
      className="fixed bottom-6 right-6 z-50 px-5 py-3 rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-xl shadow-[var(--action)]/20 text-sm font-semibold text-white"
    >
      {message}
    </div>
  );
}
