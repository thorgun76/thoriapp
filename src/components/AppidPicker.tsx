import { useEffect, useId, useRef, useState } from 'react';
import { Loader2, Search } from 'lucide-react';
import { api } from '../lib/api';

/**
 * Diálogo para vincular a mano un juego con su ficha de Steam.
 *
 * Existe porque la detección automática (buscar el nombre en la tienda) no puede
 * acertar siempre: hay bibliotecas que guardan "NoManSky_v5.0", nombres
 * traducidos o truncados. En vez de enseñar un nivel de ProtonDB dudoso o dejarlo
 * en «sin datos» para siempre, el usuario busca y elige una vez.
 */
export function AppidPicker({
  initialQuery,
  onPick,
  onClose,
}: {
  initialQuery: string;
  onPick: (appid: number, name: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState(initialQuery);
  const [results, setResults] = useState<{ appid: number; name: string }[]>([]);
  const [loading, setLoading] = useState(false);
  /** Se distingue «no hay resultados» de «aún no se ha buscado». */
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titleId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  /** Descarta respuestas de búsquedas que el usuario ya ha dejado atrás. */
  const reqRef = useRef(0);

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  // Búsqueda con retardo: cada tecla no debe lanzar una petición a Steam.
  useEffect(() => {
    const term = query.trim();
    if (term.length < 2) {
      setResults([]);
      setSearched(false);
      setLoading(false);
      return;
    }
    const token = ++reqRef.current;
    setLoading(true);
    const t = setTimeout(() => {
      api
        .searchStore(term)
        .then((found) => {
          if (token !== reqRef.current) return;
          setResults(found.map((g) => ({ appid: g.appid, name: g.name })));
          setError(null);
        })
        .catch((e) => {
          if (token !== reqRef.current) return;
          setResults([]);
          setError(String(e));
        })
        .finally(() => {
          if (token !== reqRef.current) return;
          setLoading(false);
          setSearched(true);
        });
    }, 300);
    return () => clearTimeout(t);
  }, [query]);

  // Esc cierra: es lo que espera cualquiera en un diálogo.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-black/60"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onClick={(e) => e.stopPropagation()}
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-xl border-2 border-[var(--border)] bg-[var(--surface)] p-5 shadow-2xl"
      >
        <h4 id={titleId} className="mb-1 text-base font-bold text-[var(--text)]">
          Buscar este juego en Steam
        </h4>
        <p className="mb-4 text-sm text-[var(--text-muted)]">
          La app no ha podido identificarlo por su nombre. Elige el juego correcto y se
          recordará para la próxima vez.
        </p>

        <label className="relative mb-3 block">
          <span className="sr-only">Nombre del juego en Steam</span>
          <Search
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[var(--text-muted)]"
            aria-hidden="true"
          />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Nombre del juego"
            className="w-full rounded-lg border border-[var(--border-hover)] bg-[var(--bg)] py-2 pl-9 pr-3 text-sm text-[var(--text)] placeholder:text-[var(--text-dim)] focus-visible:border-[var(--action)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action/40"
          />
        </label>

        <div className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1">
          {loading && (
            <p className="flex items-center gap-2 py-6 text-sm text-[var(--text-muted)]">
              <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
              Buscando en Steam…
            </p>
          )}

          {!loading && error && (
            <p className="py-6 text-sm text-[var(--error)]">
              No se ha podido buscar en Steam: {error}
            </p>
          )}

          {!loading && !error && searched && results.length === 0 && (
            <p className="py-6 text-sm text-[var(--text-muted)]">
              Ningún juego coincide. Prueba con menos palabras o con el título en inglés.
            </p>
          )}

          {!loading && results.length > 0 && (
            <ul className="flex flex-col gap-1">
              {results.map((r) => (
                <li key={r.appid}>
                  <button
                    type="button"
                    onClick={() => onPick(r.appid, r.name)}
                    className="flex w-full cursor-pointer items-center gap-3 rounded-lg border border-transparent px-2 py-2 text-left transition-colors hover:border-[var(--action)]/40 hover:bg-[var(--card-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action"
                  >
                    <img
                      src={`https://cdn.cloudflare.steamstatic.com/steam/apps/${r.appid}/capsule_231x87.jpg`}
                      alt=""
                      loading="lazy"
                      className="h-9 w-24 shrink-0 rounded bg-[var(--card)] object-cover"
                      // Muchos juegos no tienen miniatura: se oculta en vez de
                      // dejar el icono de imagen rota.
                      onError={(e) => {
                        e.currentTarget.style.visibility = 'hidden';
                      }}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-semibold text-[var(--text)]">
                        {r.name}
                      </span>
                      <span className="block text-xs text-[var(--text-muted)]">
                        AppID {r.appid}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="mt-4 flex justify-end">
          <button
            type="button"
            onClick={onClose}
            className="cursor-pointer rounded-lg border border-[var(--border-hover)] px-4 py-2 text-sm font-semibold text-[var(--text)] transition-colors hover:bg-[var(--card-hover)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-action"
          >
            Cancelar
          </button>
        </div>
      </div>
    </div>
  );
}
