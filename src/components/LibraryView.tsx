import { useState } from 'react';
import { ArrowDownAZ, ArrowUpAZ, Filter, Gamepad2, Home, RefreshCw, Stethoscope } from 'lucide-react';
import { api } from '../lib/api';
import type { Game, LauncherFilter } from '../lib/types';
import { matchesSearch } from '../lib/format';
import { GameCard } from './GameCard';

const matchesQuery = (g: Game, query: string) => matchesSearch(g.name, query);

/**
 * Orden de la rejilla. Solo alfabético: la biblioteca no trae horas jugadas ni
 * fecha de la última partida, así que ordenar por «recientes» o «más jugados»
 * sería inventarse datos. El nombre se compara sin acentos ni mayúsculas para
 * que «Á» quede junto a «A» y no al final.
 */
export type SortOrder = 'az' | 'za';

export function sortGames(games: Game[], order: SortOrder): Game[] {
  const collator = new Intl.Collator('es', { sensitivity: 'base', numeric: true });
  return [...games].sort((a, b) => {
    const cmp = collator.compare(a.name, b.name);
    return order === 'az' ? cmp : -cmp;
  });
}

/** Juegos de Steam filtrados por texto y launcher. */
export function filterSteamGames(library: Game[], query: string, launcherFilter: LauncherFilter) {
  return library.filter(
    (g) => matchesQuery(g, query) && (launcherFilter === 'all' || launcherFilter === 'steam')
  );
}

/** Juegos de launchers externos (Lutris/Heroic/HeroNite) filtrados. */
export function filterExternalGames(
  externalGames: Game[],
  query: string,
  launcherFilter: LauncherFilter
) {
  return externalGames.filter(
    (g) => matchesQuery(g, query) && (launcherFilter === 'all' || g.launcher === launcherFilter)
  );
}

/** Contadores por launcher, calculados ANTES de filtrar para que no cambien. */
export function countByLauncher(library: Game[], externalGames: Game[]) {
  const count = (launcher: string) => externalGames.filter((g) => g.launcher === launcher).length;
  return {
    steam: library.length,
    lutris: count('lutris'),
    heroic: count('heroic'),
    heronite: count('heronite'),
  };
}

export type LauncherCounts = ReturnType<typeof countByLauncher>;

interface LibraryViewProps {
  games: Game[];
  counts: LauncherCounts;
  launcherFilter: LauncherFilter;
  externalCount: number;
  /** Copias de juegos que ya están en Steam y por eso no se listan. */
  hiddenDuplicates?: number;
  /** Juegos de Heronite desinstalados: siguen en su library.json y se ocultan. */
  heroniteUninstalled?: number;
  showExternalGames: boolean;
  scanningLaunchers: boolean;
  onFilterChange: (filter: LauncherFilter) => void;
  onToggleExternal: () => void;
  onRescan: () => void;
  onOpenGame: (game: Game) => void;
}

/** Rejilla de juegos instalados con filtros por plataforma. */
export function LibraryView({
  games,
  counts,
  launcherFilter,
  externalCount,
  hiddenDuplicates = 0,
  heroniteUninstalled = 0,
  showExternalGames,
  scanningLaunchers,
  onFilterChange,
  onToggleExternal,
  onRescan,
  onOpenGame,
}: LibraryViewProps) {
  // Diagnóstico de carátulas: muestra en un modal qué archivos hay realmente en
  // cada launcher, para depurar sin adivinar nombres.
  const [diagnostic, setDiagnostic] = useState<string | null>(null);
  const [sortOrder, setSortOrder] = useState<SortOrder>('az');
  const shown = sortGames(games, sortOrder);

  const runDiagnostic = async () => {
    try {
      setDiagnostic(await api.diagnoseCovers());
    } catch (e) {
      setDiagnostic(`Error: ${String(e)}`);
    }
  };

  const filters: { key: LauncherFilter; label: string; activeClass: string }[] = [
    {
      key: 'all',
      label: `Todos (${counts.steam + counts.lutris + counts.heroic + counts.heronite})`,
      activeClass: 'bg-[var(--action)] text-white shadow-lg shadow-[var(--action)]/20',
    },
    {
      key: 'steam',
      label: `Steam (${counts.steam})`,
      activeClass: 'bg-[#1b2838] text-white shadow-lg shadow-[#1b2838]/20',
    },
    {
      key: 'lutris',
      label: `Lutris (${counts.lutris})`,
      activeClass: 'bg-[#f97316] text-white shadow-lg shadow-[#f97316]/20',
    },
    {
      key: 'heroic',
      label: `Heroic (${counts.heroic})`,
      activeClass: 'bg-[#3b82f6] text-white shadow-lg shadow-[#3b82f6]/20',
    },
    {
      key: 'heronite',
      label: `Heronite (${counts.heronite})`,
      activeClass: 'bg-[#a855f7] text-white shadow-lg shadow-[#a855f7]/20',
    },
  ];

  return (
    <div className="max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <p className="text-xs text-[var(--text-muted)] mb-2">Biblioteca</p>
          <h2 className="text-2xl font-bold">Juegos instalados</h2>
          {hiddenDuplicates > 0 && (
            // Transparencia: el usuario ve sus launchers y podría pensar que la
            // app ha perdido juegos. Se explica que es intencionado.
            <p className="text-xs text-[var(--text-dim)] mt-1">
              {hiddenDuplicates === 1
                ? '1 juego ya está en Steam, así que no se repite desde otro launcher'
                : `${hiddenDuplicates} juegos ya están en Steam, así que no se repiten desde otros launchers`}
            </p>
          )}
          {heroniteUninstalled > 0 && (
            // Heronite conserva los juegos desinstalados en su library.json
            // (para no perder las horas jugadas). Se ocultan aquí, pero se dice
            // para que el usuario no crea que faltan.
            <p className="text-xs text-[var(--text-dim)] mt-1">
              {heroniteUninstalled === 1
                ? '1 juego desinstalado en Heronite no se muestra'
                : `${heroniteUninstalled} juegos desinstalados en Heronite no se muestran`}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2">
          {externalCount > 0 && (
            <button
              onClick={onToggleExternal}
              className={`px-4 py-2 rounded-lg text-sm font-semibold transition-colors flex items-center gap-2 ${
                showExternalGames
                  ? 'bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30'
                  : 'bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)]'
              }`}
            >
              <Gamepad2 className="w-4 h-4" />
              {showExternalGames ? 'Ocultar' : 'Mostrar'} launchers externos ({externalCount})
            </button>
          )}
          <button
            onClick={onRescan}
            disabled={scanningLaunchers}
            className="px-4 py-2 rounded-lg text-sm font-semibold bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {scanningLaunchers ? (
              <>
                <div className="w-4 h-4 border-2 border-[var(--text-muted)]/30 border-t-[var(--text-muted)] rounded-full animate-spin"></div>
                Escaneando...
              </>
            ) : (
              <>
                <RefreshCw className="w-4 h-4" />
                Re-escanear
              </>
            )}
          </button>
          <button
            onClick={runDiagnostic}
            title="Ver qué carpetas de carátulas se están leyendo"
            className="px-3 py-2 rounded-lg text-sm bg-[var(--surface)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors flex items-center gap-2"
          >
            <Stethoscope className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* FILTROS POR LAUNCHER */}
      <div className="mb-6 p-4 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <div className="flex items-center gap-2 mb-3">
          <Filter className="w-4 h-4 text-[var(--action)]" />
          <span className="text-sm font-semibold text-[var(--text)]">Filtrar por plataforma</span>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-2">
          {filters.map((f) => (
            <button
              key={f.key}
              onClick={() => onFilterChange(f.key)}
              className={`px-4 py-2.5 rounded-lg text-sm font-semibold transition-all ${
                launcherFilter === f.key
                  ? f.activeClass
                  : 'bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:border-[var(--action)]/50'
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      {/* ORDEN DE LA REJILLA */}
      {games.length > 1 && (
        <div className="flex items-center justify-end gap-2 mb-4 -mt-2">
          <span className="text-xs text-[var(--text-muted)]">Ordenar</span>
          <div className="flex rounded-lg border border-[var(--border)] overflow-hidden">
            {([
              { key: 'az' as const, label: 'A-Z', Icon: ArrowDownAZ, title: 'Nombre, de la A a la Z' },
              { key: 'za' as const, label: 'Z-A', Icon: ArrowUpAZ, title: 'Nombre, de la Z a la A' },
            ]).map(({ key, label, Icon, title }) => (
              <button
                key={key}
                onClick={() => setSortOrder(key)}
                title={title}
                aria-pressed={sortOrder === key}
                className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold transition-colors ${
                  sortOrder === key
                    ? 'bg-[var(--action)]/15 text-[var(--action)]'
                    : 'bg-[var(--surface)] text-[var(--text-muted)] hover:bg-[var(--card-hover)]'
                }`}
              >
                <Icon className="w-3.5 h-3.5" />
                {label}
              </button>
            ))}
          </div>
        </div>
      )}

      {games.length === 0 ? (
        <div className="text-center py-16">
          <Home className="w-12 h-12 text-[var(--text-dim)] mx-auto mb-4" />
          <p className="text-[var(--text-muted)]">No se encontraron juegos</p>
          <p className="text-xs text-[var(--text-dim)] mt-2">
            Usa el buscador para encontrar juegos en Steam
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4">
          {shown.map((g) => (
            <GameCard key={`${g.launcher || 'steam'}-${g.appid || g.name}`} game={g} onOpen={onOpenGame} />
          ))}
        </div>
      )}

      {diagnostic !== null && (
        <div
          className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-6"
          onClick={() => setDiagnostic(null)}
        >
          <div
            className="bg-[var(--surface)] border border-[var(--border)] rounded-xl max-w-3xl w-full max-h-[80vh] flex flex-col"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between p-4 border-b border-[var(--border)]">
              <h3 className="font-semibold">Diagnóstico de carátulas</h3>
              <button
                onClick={() => setDiagnostic(null)}
                className="px-3 py-1 rounded-lg text-sm bg-[var(--card)] border border-[var(--border)] hover:bg-[var(--card-hover)]"
              >
                Cerrar
              </button>
            </div>
            <pre className="p-4 text-xs overflow-auto whitespace-pre-wrap text-[var(--text-muted)] font-mono">
              {diagnostic}
            </pre>
          </div>
        </div>
      )}
    </div>
  );
}
