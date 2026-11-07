import { AnimatePresence, motion } from 'framer-motion';

interface SaveProfileModalProps {
  open: boolean;
  name: string;
  notes: string;
  onNameChange: (value: string) => void;
  onNotesChange: (value: string) => void;
  onCancel: () => void;
  onSave: () => void;
}

/** Modal para nombrar y guardar la configuración actual como perfil. */
export function SaveProfileModal({
  open,
  name,
  notes,
  onNameChange,
  onNotesChange,
  onCancel,
  onSave,
}: SaveProfileModalProps) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
          onClick={onCancel}
        >
          <motion.div
            initial={{ scale: 0.9, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.9, opacity: 0 }}
            className="bg-[var(--surface)] border border-[var(--border)] rounded-2xl max-w-md w-full p-6"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 className="text-lg font-bold mb-4">Guardar perfil</h3>
            <div className="space-y-4">
              <div>
                <label className="text-sm font-semibold text-[var(--text)] mb-2 block">
                  Nombre del perfil
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => onNameChange(e.target.value)}
                  placeholder="Ej: Competitivo, Calidad visual, etc."
                  className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm focus:outline-none focus:border-[var(--action)]"
                  autoFocus
                />
              </div>
              <div>
                <label className="text-sm font-semibold text-[var(--text)] mb-2 block">
                  Notas (opcional)
                </label>
                <textarea
                  value={notes}
                  onChange={(e) => onNotesChange(e.target.value)}
                  placeholder="Describe esta configuración..."
                  className="w-full px-3 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] text-sm focus:outline-none focus:border-[var(--action)] resize-none"
                  rows={3}
                />
              </div>
              <div className="flex gap-3">
                <button
                  onClick={onCancel}
                  className="flex-1 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                >
                  Cancelar
                </button>
                <button
                  onClick={onSave}
                  disabled={!name.trim()}
                  className="flex-1 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  Guardar
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
