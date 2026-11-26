import { useEffect, useState } from 'react';
import { Home, Star, TrendingUp, MonitorDot } from 'lucide-react';
import type { SidebarView, EstadoObservador } from '../lib/types';
import logo from '../assets/thoriapp-logo.png';

interface SidebarProps {
  sidebarView: SidebarView;
  /** true si hay un juego abierto (la biblioteca muestra la vista de juego). */
  hasCurrentGame: boolean;
  savedTestsCount: number;
  profilesCount: number;
  distro: string | undefined;
  onNavigate: (view: SidebarView) => void;
  /** Estado del observador de procesos, si la app ya lo ha consultado. */
  observador?: EstadoObservador | null;
  /** Activar o desactivar la observación (el observador es de solo lectura). */
  onCambiarObservador?: (activo: boolean) => void;
}

interface NavItem {
  view: SidebarView;
  label: string;
  icon: typeof Home;
  badge?: number;
}

/** Menú lateral: navegación y pie con el entorno detectado. */
export function Sidebar({
  sidebarView,
  hasCurrentGame,
  savedTestsCount,
  profilesCount,
  distro,
  onNavigate,
  observador,
  onCambiarObservador,
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
          <img
            src={logo}
            alt="Thoriapp"
            className="w-10 h-10 rounded-xl object-contain"
            draggable={false}
          />
          <div>
            <h1 className="text-base font-bold text-white">Thoriapp</h1>
            <p className="text-xs text-[var(--text-muted)]">Mide · Compara · Optimiza</p>
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
                  type="button"
                  onClick={() => onNavigate(view)}
                  className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors ${
                    active
                      ? 'bg-action/10 text-[var(--action)] border border-action/20'
                      : 'text-[var(--text-muted)] hover:bg-[var(--card)] hover:text-white'
                  }`}
                >
                  <Icon className="w-4 h-4" />
                  <span>{label}</span>
                  {badge !== undefined && badge > 0 && (
                    <span className="ml-auto text-xs bg-action/20 text-[var(--action)] px-2 py-0.5 rounded-full">
                      {badge}
                    </span>
                  )}
                </button>
              );
            })}
          </nav>
        </div>

      </div>

      {/* Footer */}
      <div className="p-4 border-t border-[var(--border)]">
        {/* Observador de procesos: un interruptor y su estado medido, nada más. Es la
            única forma de apagarlo, y apagado no se lee ni una vez `/proc`. */}
        <ObservadorDeProcesos observador={observador} onCambiar={onCambiarObservador} />
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-full bg-gradient-to-br from-[var(--action)] to-purple-600 flex items-center justify-center text-white text-sm font-bold">
            L
          </div>
          <div>
            <p className="text-sm font-semibold text-white">Tu entorno Linux</p>
            <p className="text-xs text-[var(--text-muted)]">{distro || 'Cargando...'}</p>
          </div>
        </div>
      </div>
    </aside>
  );
}

/**
 * Texto del estado del observador.
 *
 * Todo lo que se decide aquí viene ya decidido por Rust: qué juegos se ven y con qué nombre
 * comercial. React no adivina nombres ni sustituye el juego observado por el seleccionado en
 * la interfaz, que no es evidencia de nada.
 */
function textoEstado(o: EstadoObservador): string {
  if (!o.activo) {
    return 'Observador desactivado';
  }
  if (o.estado === 'sin_visibilidad') {
    return 'Sin visibilidad de procesos';
  }
  if (o.estado !== 'observando') {
    return 'Buscando un juego…';
  }
  const juegos = o.juegos ?? [];
  if (juegos.length === 0) {
    return 'Buscando un juego…';
  }
  // Varios juegos a la vez: se dicen todos, sin elegir uno y callar el resto.
  if (juegos.length > 1) {
    const nombres = juegos.map((j) => j.nombre ?? 'juego sin identificar');
    return `Varios juegos en ejecución: ${nombres.join(', ')}`;
  }
  const unico = juegos[0];
  if (unico.nombre) {
    return `${unico.nombre} en ejecución`;
  }
  // Se vio un juego pero no se pudo poner nombre: se dice tal cual. Solo aquí, cuando de
  // verdad no se puede identificar.
  return 'Juego detectado · Identidad desconocida';
}

/**
 * Interruptor del observador de procesos.
 *
 * Es de solo lectura: mira `/proc` para saber si un juego está corriendo. Cuando no puede
 * ver nada (contenedor, permisos) lo dice tal cual, en vez de fingir que no hay juego
 * abierto.
 */
function ObservadorDeProcesos({
  observador,
  onCambiar,
}: {
  observador?: EstadoObservador | null;
  onCambiar?: (activo: boolean) => void;
}) {
  // El valor inicial es `false`: mientras Rust no diga lo contrario, el observador no está
  // encendido. Arrancar en `true` enseñaba el interruptor activo en la primera pintura,
  // antes de que llegara el estado real, y parecía que se activaba solo.
  const [valor, setValor] = useState(observador?.activo ?? false);

  useEffect(() => {
    if (observador) {
      setValor(observador.activo);
    }
  }, [observador]);

  const alternar = () => {
    const nuevo = !valor;
    setValor(nuevo);
    onCambiar?.(nuevo);
  };

  return (
    <div className="mb-3">
      <div className="flex items-center justify-between gap-2">
        <span className="flex items-center gap-1.5 text-xs font-medium text-[var(--text-muted)]">
          <MonitorDot className="w-3.5 h-3.5" />
          Observar procesos
        </span>
        {/* Mismo patrón que el resto de interruptores de la app (`OptimizeTab`): carril
            `h-6 w-11` y círculo `h-4 w-4` centrado con flex, desplazado con
            `translate-x-1` / `translate-x-6`. La versión anterior usaba un carril más
            estrecho que el desplazamiento y el círculo se salía por el borde derecho. */}
        <button
          type="button"
          role="switch"
          aria-checked={valor}
          aria-label="Observar procesos de juegos"
          onClick={alternar}
          className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
            valor ? 'bg-[#38C9A3]' : 'bg-[#353D4D]'
          }`}
        >
          <span
            className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
              valor ? 'translate-x-6' : 'translate-x-1'
            }`}
          />
        </button>
      </div>
      <p className="mt-1 text-[11px] text-[var(--text-dim)] leading-snug">
        {observador ? textoEstado(observador) : 'Sin datos'}
      </p>
    </div>
  );
}
