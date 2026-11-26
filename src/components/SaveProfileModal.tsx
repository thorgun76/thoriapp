import { AnimatePresence, motion } from 'framer-motion';

interface SaveProfileModalProps {
  open: boolean;
  name: string;
  notes: string;
  onNameChange: (value: string) => void;
  onNotesChange: (value: string) => void;
  onCancel: () => void;
  /**
   * Crea un perfil nuevo con el nombre escrito. Nunca sobrescribe: si ya existe uno con
   * ese nombre, lo dice `onSaveError`.
   */
  onSave: () => void;
  /**
   * Perfil cargado, si lo hay. Con él se ofrece actualizarlo en el sitio en vez de crear
   * otro, que es lo que se quiere el 90 % de las veces.
   */
  perfilCargado?: string | null;
  /** Actualiza el perfil cargado. Solo se ofrece si hay uno. */
  onUpdate?: () => void;
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
  perfilCargado = null,
  onUpdate,
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
            {perfilCargado && (
              <p className="text-xs text-[var(--text-muted)] mb-4 -mt-2 leading-snug">
                Hay un perfil cargado: <strong>{perfilCargado}</strong>. Puedes actualizarlo o
                guardar esta configuración como uno nuevo.
              </p>
            )}
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
                {/* «Actualizar» solo aparece con un perfil cargado: sin él no hay nada que
                    actualizar, y crearlo a espaldas del usuario es justo lo que no debe
                    pasar. */}
                {perfilCargado && onUpdate && (
                  <button
                    onClick={onUpdate}
                    className="flex-1 px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors"
                  >
                    Actualizar «{perfilCargado}»
                  </button>
                )}
                <button
                  onClick={onSave}
                  disabled={!name.trim()}
                  className={`flex-1 px-4 py-2 rounded-lg text-sm font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                    perfilCargado
                      ? 'bg-[var(--bg)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)]'
                      : 'bg-[var(--action)] text-white hover:bg-[var(--action-hover)]'
                  }`}
                >
                  {perfilCargado ? 'Guardar como nuevo' : 'Guardar'}
                </button>
              </div>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
