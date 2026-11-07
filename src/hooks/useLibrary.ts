import { useCallback, useEffect, useRef, useState } from 'react';
import { api, type RawExternalGame } from '../lib/api';
import type { Game } from '../lib/types';
import { clearCoverCache } from './useLocalCover';
import { dedupeAgainstSteam } from '../lib/dedupe';

/** Convierte lo que devuelve Rust en el `Game` que usa la UI. */
export function toGame(g: RawExternalGame): Game {
  return {
    // Los juegos externos suelen no tener AppID de Steam, pero Rust sí lo rellena
    // cuando el juego de Lutris/Heroic/Heronite es en realidad de Steam: sin esto
    // se perdía la carátula del CDN para esos casos.
    appid: g.appid ?? 0,
    name: g.name,
    installed: true,
    launcher: g.launcher,
    install_path: g.install_path,
    runner: g.runner,
    cover_key: g.cover_key ?? undefined,
  };
}

/** Resumen tipo "3 de lutris, 1 de heroic" para los avisos. */
export function summarizeLaunchers(games: Game[]): string {
  const counts = games.reduce<Record<string, number>>((acc, game) => {
    const launcher = game.launcher || 'unknown';
    acc[launcher] = (acc[launcher] || 0) + 1;
    return acc;
  }, {});
  return Object.entries(counts)
    .map(([launcher, count]) => `${count} de ${launcher}`)
    .join(', ');
}

export interface ScanExternalResult {
  games: Game[];
  summary: string;
  /** Juegos de Heronite ocultos por estar desinstalados. */
  heroniteUninstalled: number;
}

interface UseLibraryOptions {
  getHwTags: () => string[];
  /** Avisos opcionales (toast). Se llama solo en el escaneo manual. */
  onNotify?: (message: string) => void;
}

const SEARCH_DEBOUNCE_MS = 500;
const MIN_QUERY_LENGTH = 2;

/**
 * Biblioteca de juegos: escaneo de Steam al arrancar, escaneo bajo demanda de
 * Lutris/Heroic/HeroNite, y búsqueda en la Steam Store con debounce.
 *
 * `getHwTags` se pasa como getter (no como valor) para que los callbacks no se
 * recreen cada vez que cambia el hardware y no se disparen efectos de más.
 */
export function useLibrary({ onNotify }: UseLibraryOptions) {
  const [library, setLibrary] = useState<Game[]>([]);
  const [externalGames, setExternalGames] = useState<Game[]>([]);
  /** Copias externas de juegos que ya están en Steam, ocultas en la vista. */
  const [hiddenDuplicates, setHiddenDuplicates] = useState(0);
  /** Juegos de Heronite desinstalados (siguen en su library.json). */
  const [heroniteUninstalled, setHeroniteUninstalled] = useState(0);
  const [scanningLaunchers, setScanningLaunchers] = useState(false);

  // Búsqueda en la store
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<Game[]>([]);
  const [showSug, setShowSug] = useState(false);
  const searchTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  /**
   * Biblioteca de Steam actual, sin pasar por el estado de React.
   *
   * `scanExternal` la necesita para ocultar duplicados, pero si dependiera de
   * `library` cambiaría de identidad en cada escaneo y `loadInitial` (que hace
   * `setLibrary`) también: el efecto de arranque en App.tsx volvería a
   * ejecutarse y se entraría en un bucle infinito de escaneos.
   */
  const steamLibraryRef = useRef<Game[]>([]);

  // Evita setState tras desmontar
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
    };
  }, []);

  /**
   * Escanea launchers externos. Devuelve el resultado; no muestra avisos.
   *
   * `steamLibrary` se pasa explícitamente porque en `loadInitial` se llama justo
   * después de `setLibrary`, y el estado de este render todavía tiene la
   * biblioteca de Steam vacía. Por eso mismo el callback no depende de `library`
   * (ver `steamLibraryRef`): así su identidad es estable y no realimenta el
   * efecto de arranque.
   */
  const scanExternal = useCallback(async (steamLibrary?: Game[]): Promise<ScanExternalResult> => {
    setScanningLaunchers(true);
    try {
      const raw = await api.scanExternalLaunchers();
      const scanned = raw.map(toGame);
      // Steam manda: si el juego ya está en Steam, no se repite desde otro
      // launcher. Se usa la `library` del momento del escaneo.
      const games = dedupeAgainstSteam(steamLibrary ?? steamLibraryRef.current, scanned);
      // Cuántos juegos de Heronite se ocultaron por estar desinstalados. Solo
      // informativo: la UI lo dice para que no parezca que faltan juegos.
      const uninstalled = await api.heroniteUninstalledCount().catch(() => 0);
      if (mountedRef.current) {
        setExternalGames(games);
        setHiddenDuplicates(scanned.length - games.length);
        setHeroniteUninstalled(uninstalled);
      }
      return { games, summary: summarizeLaunchers(games), heroniteUninstalled: uninstalled };
    } finally {
      if (mountedRef.current) setScanningLaunchers(false);
    }
    // Sin dependencias: usa `steamLibraryRef`, así mantiene una identidad estable.
  }, []);

  /** Re-escaneo manual (botón "Re-escanear"), con avisos de resultado. */
  const rescanExternal = useCallback(async () => {
    try {
      // El usuario pudo cambiar una portada desde el launcher: se olvidan las
      // carátulas cacheadas para que el re-escaneo traiga las nuevas.
      clearCoverCache();
      const { games, summary, heroniteUninstalled } = await scanExternal();
      if (!onNotify) return;
      // Se menciona aparte: el usuario los sigue viendo en Heronite y si no se
      // explica, parece que Thoriapp los ha perdido.
      const avisoHeronite =
        heroniteUninstalled > 0
          ? ` · ${heroniteUninstalled} desinstalado${heroniteUninstalled === 1 ? '' : 's'} de Heronite oculto${heroniteUninstalled === 1 ? '' : 's'}`
          : '';
      if (games.length > 0) {
        onNotify(`✓ Re-escaneo completado: ${games.length} juegos (${summary})${avisoHeronite}`);
      } else {
        onNotify('ℹ️ No se encontraron juegos en launchers externos');
      }
    } catch (e) {
      console.error('Error re-escaneando launchers:', e);
      onNotify?.('❌ Error al escanear launchers');
    }
  }, [scanExternal, onNotify]);

  /**
   * Carga inicial: biblioteca de Steam y, después, launchers externos.
   * Orden deliberado: el `get_rules` previo en el arranque da tiempo al backend
   * Rust a cargar games.json (ver App.tsx).
   */
  const loadInitial = useCallback(async () => {
    try {
      const lib = await api.scanLibrary();
      if (!mountedRef.current) return;
      steamLibraryRef.current = lib;
      setLibrary(lib);

      try {
        const { games, summary } = await scanExternal();
        if (games.length > 0) {
          onNotify?.(`✓ Detectados ${games.length} juegos externos (${summary})`);
        }
      } catch (e) {
        // Un fallo de launchers externos no debe tumbar la biblioteca de Steam
        console.warn('Error cargando juegos de launchers externos:', e);
      }
    } catch (e) {
      console.error(e);
    }
  }, [scanExternal, onNotify]);

  /** Búsqueda inmediata en la Steam Store. */
  const search = useCallback(async (term: string) => {
    if (!term.trim() || term.length < MIN_QUERY_LENGTH) return;
    try {
      const results = await api.searchStore(term);
      if (!mountedRef.current) return;
      setSuggestions(results);
      setShowSug(true);
    } catch (e) {
      console.error(e);
    }
  }, []);

  /** Cambio en el input de búsqueda: actualiza el texto y aplica debounce. */
  const handleQueryChange = useCallback(
    (value: string) => {
      setQuery(value);
      if (searchTimeoutRef.current) clearTimeout(searchTimeoutRef.current);
      if (value.length < MIN_QUERY_LENGTH) {
        setSuggestions([]);
        setShowSug(false);
        return;
      }
      searchTimeoutRef.current = setTimeout(() => search(value), SEARCH_DEBOUNCE_MS);
    },
    [search]
  );

  return {
    // datos
    library,
    externalGames,
    /** Cuántas copias de juegos ya presentes en Steam se han ocultado. */
    hiddenDuplicates,
    heroniteUninstalled,
    scanningLaunchers,
    // búsqueda
    query,
    suggestions,
    showSug,
    setShowSug,
    search,
    /** Limpia el input y el desplegable (al abrir un juego). */
    clearSearch: () => {
      setQuery('');
      setSuggestions([]);
      setShowSug(false);
    },
    handleQueryChange,
    // acciones
    loadInitial,
    rescanExternal,
  };
}
