import { AnimatePresence, motion } from 'framer-motion';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { ChevronDown, Copy, GripVertical } from 'lucide-react';
import type { Profile } from '../lib/types';

/** Props de la tarjeta ordenable. */
interface SortableProfileProps {
  profile: Profile;
  isExpanded: boolean;
  onToggle: () => void;
  onCopy: () => void;
  onExport: () => void;
  onDelete: () => void;
  command: string | undefined;
}

/** Tarjeta ordenable de un perfil guardado (drag & drop). */
export function SortableProfile({ profile, isExpanded, onToggle, onCopy, onExport, onDelete, command }: SortableProfileProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: profile.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 1000 : 'auto',
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`rounded-xl bg-[var(--surface)] border overflow-hidden transition-colors duration-200 ${
        isExpanded ? 'border-[var(--action)]' : 'border-[var(--border)]'
      }`}
    >
      {/* Header del perfil (siempre visible) */}
      <div className="flex items-center">
        {/* Handle de arrastre */}
        <div
          {...attributes}
          {...listeners}
          className="p-2 cursor-grab active:cursor-grabbing hover:bg-[var(--card-hover)] transition-colors touch-none"
        >
          <GripVertical className="w-4 h-4 text-[var(--text-muted)]" />
        </div>

        {/* Contenido del perfil */}
        <button
          onClick={onToggle}
          className="flex-1 p-4 flex items-center gap-4 hover:bg-[var(--card-hover)] transition-colors text-left"
        >
          <div className="flex-1 min-w-0 grid grid-cols-2 gap-4">
            <div className="flex items-center gap-2">
              <h3 className="text-base font-bold text-[var(--text)] truncate">
                {profile.name}
              </h3>
              {profile.expert_mode && (
                <span className="text-[10px] px-2 py-0.5 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30 shrink-0">
                  Experto
                </span>
              )}
            </div>
            <p className="text-sm text-[var(--text-muted)] truncate text-right">
              {profile.game_name}
            </p>
          </div>
          <ChevronDown
            className={`w-5 h-5 text-[var(--text-muted)] transition-transform duration-200 shrink-0 ${
              isExpanded ? 'rotate-180' : ''
            }`}
          />
        </button>
      </div>

      {/* Contenido expandido (animado) */}
      <AnimatePresence initial={false}>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15, ease: 'easeOut' }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-4 space-y-3 border-t border-[var(--border)]">
              {/* Fecha y notas */}
              <div className="pt-3 flex items-center gap-3 text-xs text-[var(--text-dim)]">
                <span>Creado: {new Date(profile.created_at).toLocaleDateString('es-ES')}</span>
                {profile.notes && (
                  <>
                    <span>·</span>
                    <span className="truncate">{profile.notes}</span>
                  </>
                )}
              </div>

              {/* Comando con botón copiar */}
              {command && (
                <div className="flex items-center gap-2">
                  <div className="flex-1 p-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] min-w-0">
                    <code className="text-xs text-[var(--compatible)] font-mono break-all">
                      {command}
                    </code>
                  </div>
                  <button
                    onClick={onCopy}
                    className="px-3 py-2 rounded-lg text-xs font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors flex items-center gap-1 shrink-0"
                  >
                    <Copy className="w-3 h-3" />
                    Copiar
                  </button>
                </div>
              )}

              {/* Botones de acción */}
              <div className="flex gap-2 justify-end pt-1">
                <button
                  onClick={onExport}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
                >
                  Exportar
                </button>
                <button
                  onClick={onDelete}
                  className="px-3 py-1.5 rounded-lg text-xs font-semibold bg-[var(--error)]/10 text-[var(--error)] border border-[var(--error)]/30 hover:bg-[var(--error)]/20 transition-colors"
                >
                  Eliminar
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

