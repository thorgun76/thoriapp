import { useState } from 'react';
import {
  DndContext,
  DragOverlay,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragStartEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { Copy, Star } from 'lucide-react';
import type { Profile } from '../lib/types';
import { SortableProfile } from './SortableProfile';

export interface PerfilesTabProps {
  /** Todos los perfiles guardados, de todos los juegos. Es la colección única. */
  profiles: Profile[];
  /** Comando ya resuelto de cada perfil, por id. */
  profileCommands: Record<string, string>;
  /** Abre un juego por appid, desde la biblioteca de perfiles. */
  onAbrirJuego: (appid: number) => void;
  /**
   * Abre el juego del perfil **y** carga ese perfil.
   *
   * Es una acción distinta de «Abrir juego»: una solo navega, la otra además cambia la
   * configuración. Se ofrecen por separado para que nadie cargue un perfil sin querer.
   */
  onCargarEnJuego: (profile: Profile) => void;
  onCopy: (profile: Profile) => void;
  onExport: (profile: Profile) => void;
  onDelete: (profile: Profile) => void;
  onReorder: (ordered: Profile[]) => void;
  onImportar: () => void;
  onExportarTodos: () => void;
}

/**
 * Biblioteca global de perfiles: la pestaña «Mis perfiles».
 *
 * Aquí solo se **gestionan** perfiles; no se decide qué configuración está cargada en un
 * juego. Eso vive en la vista del juego (`PerfilesDelJuego`), y por eso las acciones de
 * aquí que afectan a un juego abierto se limitan a abrirlo o a pedir su carga.
 *
 * La lista sale siempre de la colección única que llega por props: no hay estado local de
 * perfiles, solo el de la interfaz (cuál está desplegado, cuál se arrastra).
 */
export function PerfilesTab({
  profiles,
  profileCommands,
  onAbrirJuego,
  onCargarEnJuego,
  onCopy,
  onExport,
  onDelete,
  onReorder,
  onImportar,
  onExportarTodos,
}: PerfilesTabProps) {
  const [expandedProfile, setExpandedProfile] = useState<string | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );

  const handleDragStart = (event: DragStartEvent) => setActiveId(event.active.id as string);

  const handleDragEnd = (event: DragEndEvent) => {
    setActiveId(null);
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const oldIndex = profiles.findIndex((p) => p.id === active.id);
    const newIndex = profiles.findIndex((p) => p.id === over.id);
    // Reordenar un índice inexistente (lista que cambió durante el arrastre) no debe
    // mover nada: `arrayMove` con -1 deja la lista en un orden arbitrario.
    if (oldIndex < 0 || newIndex < 0) return;
    onReorder(arrayMove(profiles, oldIndex, newIndex));
  };

  const activeProfile = activeId ? profiles.find((p) => p.id === activeId) : null;

  return (
    <div className="max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <p className="text-xs text-[var(--text-muted)] mb-2">Perfiles</p>
          <h2 className="text-2xl font-bold">Tus configuraciones guardadas</h2>
        </div>
        <div className="flex gap-2">
          <button
            onClick={onImportar}
            className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--action)] text-white hover:bg-[var(--action-hover)] transition-colors flex items-center gap-2"
          >
            <Star className="w-4 h-4" />
            Importar perfil
          </button>
          <button
            onClick={onExportarTodos}
            className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors flex items-center gap-2"
          >
            <Copy className="w-4 h-4" />
            Exportar todos
          </button>
        </div>
      </div>

      {profiles.length === 0 ? (
        <div className="text-center py-16">
          <Star className="w-12 h-12 text-[var(--text-dim)] mx-auto mb-4" />
          <p className="text-[var(--text-muted)] mb-2">No tienes perfiles guardados</p>
          <p className="text-xs text-[var(--text-dim)]">
            Configura un juego y guarda la configuración como perfil
          </p>
        </div>
      ) : (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragStart={handleDragStart}
          onDragEnd={handleDragEnd}
          onDragCancel={() => setActiveId(null)}
        >
          <SortableContext items={profiles.map((p) => p.id)} strategy={verticalListSortingStrategy}>
            <div className="space-y-2">
              {profiles.map((profile) => (
                <SortableProfile
                  key={profile.id}
                  profile={profile}
                  isExpanded={expandedProfile === profile.id}
                  onToggle={() =>
                    setExpandedProfile(expandedProfile === profile.id ? null : profile.id)
                  }
                  onCopy={() => onCopy(profile)}
                  onExport={() => onExport(profile)}
                  onDelete={() => onDelete(profile)}
                  onOpenGame={() => onAbrirJuego(profile.appid)}
                  onLoadInGame={() => onCargarEnJuego(profile)}
                  command={profileCommands[profile.id]}
                />
              ))}
            </div>
          </SortableContext>
          <DragOverlay>
            {activeProfile ? (
              <div className="rounded-xl bg-[var(--surface)] border-2 border-[var(--action)] shadow-2xl shadow-action/20 p-4 cursor-grabbing">
                <div className="grid grid-cols-2 gap-4">
                  <div className="flex items-center gap-2">
                    <h3 className="text-base font-bold text-[var(--text)] truncate">
                      {activeProfile.name}
                    </h3>
                  </div>
                  <p className="text-sm text-[var(--text-muted)] truncate text-right">
                    {activeProfile.game_name}
                  </p>
                </div>
              </div>
            ) : null}
          </DragOverlay>
        </DndContext>
      )}
    </div>
  );
}
