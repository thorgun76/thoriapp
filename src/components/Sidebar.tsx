import { Home, Settings, Star, TrendingUp } from 'lucide-react';
import type { SidebarView } from '../lib/types';

interface SidebarProps {
  sidebarView: SidebarView;
  /** true si hay un juego abierto (la biblioteca muestra la vista de juego). */
  hasCurrentGame: boolean;
  savedTestsCount: number;
  profilesCount: number;
  expertMode: boolean;
  distro: string | undefined;
  onNavigate: (view: SidebarView) => void;
  onToggleExpertMode: () => void;
}

interface NavItem {
  view: SidebarView;
  label: string;
  icon: typeof Home;
  badge?: number;
}

/** Menú lateral: navegación, conmutador de modo y pie con el entorno detectado. */
export function Sidebar({
  sidebarView,
  hasCurrentGame,
  savedTestsCount,
  profilesCount,
  expertMode,
  distro,
  onNavigate,
  onToggleExpertMode,
}: SidebarProps) {
  const items: NavItem[] = [
    { view: 'library', label: 'Mi biblioteca', icon: Home },
    { view: 'tests', label: 'Mis pruebas', icon: TrendingUp, badge: savedTestsCount },
    { view: 'profiles', label: 'Perfiles', icon: Star, badge: profilesCount },
    { view: 'benchmarks', label: 'Benchmarks', icon: TrendingUp },
  ];

  return (
    <aside className="w-64 bg-[var(--surface)] border-r border-[var(--border)] flex flex-col">
      {/* Brand */}
      <div className="p-5 border-b border-[var(--border)]">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-[var(--action)] to-purple-600 flex items-center justify-center text-white font-bold text-lg">
            ✦
          </div>
          <div>
            <h1 className="text-base font-bold text-white">Thoriapp</h1>
            <p className="text-xs text-[var(--text-muted)]">Optimización Linux</p>
          </div>
        </div>
      </div>

      {/* Navigation */}
      <div className="flex-1 p-3 space-y-6">
        <div>
          <p className="text-[10px] font-semibold text-[var(--text-dim)] uppercase tracking-wider px-3 mb-2">
            Espacio de trabajo
          </p>
          <nav className="space-y-1">
            {items.map(({ view, label, icon: Icon, badge }) => {
              const active =
                view === 'library'
                  ? sidebarView === 'library' && !hasCurrentGame
                  : sidebarView === view;

              return (
                <button
                  key={view}
                  onClick={() => onNavigate(view)}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                    active
                      ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/20'
                      : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                  }`}
                >
                  <Icon className="w-4 h-4" />
                  <span>{label}</span>
                  {badge !== undefined && badge > 0 && (
                    <span className="ml-auto text-xs bg-[var(--action)]/20 text-[var(--action)] px-2 py-0.5 rounded-full">
                      {badge}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>
        </div>

        <div>
          <p className="text-[10px] font-semibold text-[var(--text-dim)] uppercase tracking-wider px-3 mb-2">
            Experiencia
          </p>
          <nav className="space-y-1">
            <div className="w-full flex items-center justify-between px-3 py-2.5 rounded-lg text-sm">
              <div
                className={`flex items-center gap-3 ${
                  expertMode ? 'text-[var(--action)]' : 'text-[var(--text-muted)]'
                }`}
              >
                <Settings className="w-4 h-4" />
                <span>{expertMode ? 'Modo experto' : 'Modo guiado'}</span>
              </div>
              <button
                onClick={onToggleExpertMode}
                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                  expertMode ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                }`}
                role="switch"
                aria-checked={expertMode}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white shadow-lg transition-transform duration-200 ease-in-out ${
                    expertMode ? 'translate-x-6' : 'translate-x-1'
                  }`}
                />
              </button>
            </div>
            <p className="text-[10px] text-[var(--text-dim)] px-3 mt-1">
              {expertMode ? 'Todos los parámetros y detalles técnicos' : 'Solo ajustes esenciales'}
            </p>
          </nav>
        </div>
      </div>

      {/* Footer */}
      <div className="p-4 border-t border-[var(--border)]">
        <div className="flex items-center gap-3 mb-2">
          <div className="w-8 h-8 rounded-full bg-gradient-to-br from-[var(--action)] to-purple-600 flex items-center justify-center text-white text-sm font-bold">
            L
          </div>
          <div>
            <p className="text-sm font-semibold text-white">Tu entorno Linux</p>
            <p className="text-xs text-[var(--text-muted)]">{distro || 'Cargando...'}</p>
          </div>
        </div>
        <p className="text-[10px] text-[var(--text-dim)]">Optimizador honesto para Linux</p>
      </div>
    </aside>
  );
}
