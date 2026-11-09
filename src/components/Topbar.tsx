import { useState } from 'react';
import { Search } from 'lucide-react';
import type { Game, Hardware } from '../lib/types';
import { coverUrl } from './GameCard';

interface TopbarProps {
  query: string;
  suggestions: Game[];
  showSuggestions: boolean;
  gameCount: number;
  /** Juegos que quedan tras filtrar por la búsqueda actual. */
  matchedCount?: number;
  scanningLaunchers: boolean;
  hardware: Hardware | null;
  onQueryChange: (value: string) => void;
  onFocus: () => void;
  onBlur: () => void;
  onPickSuggestion: (game: Game) => void;
}

const vendorLabel = (vendor: string) =>
  vendor === 'Amd' ? 'AMD' : vendor === 'Nvidia' ? 'NVIDIA' : vendor === 'Intel' ? 'Intel' : '';

/**
 * Nombre de GPU sin el ruido del driver.
 *
 * `lspci`/`vulkaninfo` devuelven cosas como «AMD Radeon RX 6800 XT (RADV NAVI21)»
 * o «NVIDIA GeForce RTX 3070/PCIe/SSE2». En la barra cabe el modelo, no la
 * cadena entera: se corta por el paréntesis y se quita el prefijo del fabricante.
 *
 * El paréntesis se quita **solo si no es parte del nombre**: `Intel(R) UHD` lleva
 * uno pegado a la marca, y cortar por el primer `(` dejaría «Intel» a secas.
 */
export const gpuModel = (gpu: string): string =>
  gpu
    // Paréntesis precedido de espacio = anotación del driver, no parte del nombre.
    .replace(/\s+\([^)]*\)\s*$/, '')
    .replace(/\/PCIe\/SSE2$/, '')
    .replace(/\bIntel\(R\)\s*/i, 'Intel ')
    .replace(/^(NVIDIA|AMD|Intel)\s+/i, '')
    .replace(/^Radeon\s+(?=RX|R\d|Vega)/i, '')
    .replace(/^GeForce\s+/i, '')
    .replace(/\s+/g, ' ')
    .trim();

/** Acorta el modelo a lo que cabe en un chip («RX 6800 XT», «RTX 3070»). */
export const shortGpu = (model: string): string =>
  /^(RX|RTX|GTX|Arc|UHD|Iris|Vega|Radeon|HD)\b/i.test(model) ? model : model.split(/\s+/).slice(0, 2).join(' ');

const LAUNCHER_LABEL: Record<string, string> = {
  steam: 'Steam',
  lutris: 'Lutris',
  heroic: 'Heroic',
  heronite: 'Heronite',
};

/**
 * Miniatura de la lista de sugerencias. Si el CDN no tiene la carátula (juegos
 * retirados, herramientas, apps sin arte), cae a un hueco con la inicial en vez
 * de dejar una imagen rota.
 */
function SuggestionCover({ game }: { game: Game }) {
  const [failed, setFailed] = useState(false);
  const src = coverUrl(game.appid);

  if (!src || failed) {
    return (
      <span className="w-8 h-11 shrink-0 rounded bg-[var(--border)] flex items-center justify-center text-[10px] font-bold text-[var(--text-muted)]">
        {game.name.trim().charAt(0).toUpperCase() || '?'}
      </span>
    );
  }

  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className="w-8 h-11 shrink-0 rounded object-cover bg-[var(--border)]"
    />
  );
}

/** Barra superior: buscador con autocompletado y estado del sistema. */
export function Topbar({
  query,
  suggestions,
  showSuggestions,
  gameCount,
  matchedCount,
  scanningLaunchers,
  hardware,
  onQueryChange,
  onFocus,
  onBlur,
  onPickSuggestion,
}: TopbarProps) {
  // Con una búsqueda activa se ve "3 de 120 juegos": el total solo no dice si el
  // filtro ha servido de algo, y el número filtrado solo (lo que hacía antes)
  // parece que la biblioteca encoge al teclear.
  const searching = query.trim().length > 0 && matchedCount !== undefined;
  return (
    <header className="h-14 border-b border-[var(--border)] flex items-center px-6 gap-4 bg-[var(--surface)]">
      <div className="flex-1 relative max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-muted)]" />
        <input
          type="text"
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && suggestions.length > 0) onPickSuggestion(suggestions[0]);
          }}
          onFocus={onFocus}
          onBlur={onBlur}
          placeholder="Buscar juegos para optimizar..."
          className="w-full pl-10 pr-4 py-2 rounded-lg bg-[var(--card)] border border-[var(--border)] focus:border-[var(--action)]/50 focus:outline-none text-sm"
        />
        {showSuggestions && suggestions.length > 0 && (
          <div
            className="absolute top-full left-0 right-0 mt-1 bg-[var(--card)] border border-[var(--border)] rounded-lg shadow-xl overflow-hidden z-50 max-h-72 overflow-y-auto"
            role="listbox"
          >
            {suggestions.map((g) => (
              <button
                key={`${g.launcher ?? 'steam'}-${g.appid || g.name}`}
                onClick={() => onPickSuggestion(g)}
                role="option"
                aria-selected={false}
                className="w-full px-3 py-2 text-left hover:bg-[var(--card-hover)] transition-colors flex items-center gap-3"
              >
                {/* Carátula pequeña: reconocer el juego de un vistazo es más rápido
                    que leer el nombre, sobre todo con sagas de nombres parecidos. */}
                <SuggestionCover game={g} />
                <span className="min-w-0 flex-1">
                  <span className="block text-sm truncate">{g.name}</span>
                  <span className="block text-[11px] text-[var(--text-dim)]">
                    {LAUNCHER_LABEL[g.launcher ?? 'steam'] ?? 'Steam'}
                    {g.appid > 0 ? ` · ${g.appid}` : ''}
                  </span>
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        <span className="text-xs px-2 py-1 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30">
          {searching ? `${matchedCount} de ${gameCount}` : `${gameCount}`} juegos
        </span>
        {scanningLaunchers && (
          <span className="text-xs px-2 py-1 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-[var(--caution)]/30 flex items-center gap-1">
            <div className="w-3 h-3 border-2 border-[var(--caution)]/30 border-t-[var(--caution)] rounded-full animate-spin"></div>
            Escaneando launchers...
          </span>
        )}
        {hardware && (
          /* Antes este badge llevaba un punto verde que sugería «todo correcto»,
             pero ser AMD o NVIDIA no es bueno ni malo: es un dato. Ahora es
             neutro y añade el modelo, que es lo que el usuario reconoce. */
          <span
            className="text-xs px-2 py-1 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)] flex items-center gap-1.5 max-w-[220px]"
            title={hardware.gpu || undefined}
          >
            <span className="shrink-0 text-[var(--text-dim)]">{vendorLabel(hardware.vendor)}</span>
            {shortGpu(gpuModel(hardware.gpu)) && (
              <span className="truncate text-[var(--text)]">{shortGpu(gpuModel(hardware.gpu))}</span>
            )}
          </span>
        )}
      </div>
    </header>
  );
}
