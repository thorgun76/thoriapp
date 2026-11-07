import { Search } from 'lucide-react';
import type { Game, Hardware } from '../lib/types';

interface TopbarProps {
  query: string;
  suggestions: Game[];
  showSuggestions: boolean;
  gameCount: number;
  scanningLaunchers: boolean;
  hardware: Hardware | null;
  onQueryChange: (value: string) => void;
  onFocus: () => void;
  onBlur: () => void;
  onPickSuggestion: (game: Game) => void;
}

const vendorLabel = (vendor: string) =>
  vendor === 'Amd' ? 'AMD' : vendor === 'Nvidia' ? 'NVIDIA' : vendor === 'Intel' ? 'Intel' : 'Linux';

/** Barra superior: buscador con autocompletado y estado del sistema. */
export function Topbar({
  query,
  suggestions,
  showSuggestions,
  gameCount,
  scanningLaunchers,
  hardware,
  onQueryChange,
  onFocus,
  onBlur,
  onPickSuggestion,
}: TopbarProps) {
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
          <div className="absolute top-full left-0 right-0 mt-1 bg-[var(--card)] border border-[var(--border)] rounded-lg shadow-xl overflow-hidden z-50 max-h-72 overflow-y-auto">
            {suggestions.map((g, i) => (
              <button
                key={i}
                onClick={() => onPickSuggestion(g)}
                className="w-full px-4 py-2.5 text-left hover:bg-[var(--card-hover)] transition-colors flex justify-between items-center"
              >
                <span className="text-sm">{g.name}</span>
                <span className="text-xs text-[var(--text-muted)]">AppID {g.appid}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        <span className="text-xs px-2 py-1 rounded bg-[var(--action)]/10 text-[var(--action)] border border-[var(--action)]/30">
          {gameCount} juegos
        </span>
        {scanningLaunchers && (
          <span className="text-xs px-2 py-1 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-[var(--caution)]/30 flex items-center gap-1">
            <div className="w-3 h-3 border-2 border-[var(--caution)]/30 border-t-[var(--caution)] rounded-full animate-spin"></div>
            Escaneando launchers...
          </span>
        )}
        {hardware && (
          <span className="text-xs px-2 py-1 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-[var(--compatible)]/30 flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-[var(--compatible)]"></span>
            {vendorLabel(hardware.vendor)}
          </span>
        )}
      </div>
    </header>
  );
}
