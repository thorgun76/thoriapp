import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type {
  CommunityRecommendation,
  Game,
  GameEngine,
  GameInfo,
  GameRules,
} from '../lib/types';

interface UseGameDetailsOptions {
  /** Tags de hardware (getter, para no recrear callbacks). */
  getHwTags: () => string[];
}

/**
 * Datos del juego abierto: reglas, info de ProtonDB, motor y recomendaciones.
 *
 * Antes `openGame()` escribía en 8 estados distintos de App.tsx. Aquí viven
 * juntos, y existe `openToken` para descartar respuestas de un juego que el
 * usuario ya abandonó (evita que un clic rápido deje datos cruzados).
 */
export function useGameDetails({ getHwTags }: UseGameDetailsOptions) {
  const [current, setCurrent] = useState<Game | null>(null);
  const [rules, setRules] = useState<GameRules | null>(null);
  const [info, setInfo] = useState<GameInfo | null>(null);
  const [engine, setEngine] = useState<GameEngine | null>(null);
  const [engineRecommendations, setEngineRecommendations] = useState<string[]>([]);
  const [communityRecommendations, setCommunityRecommendations] = useState<CommunityRecommendation[]>([]);
  const [loading, setLoading] = useState(false);
  /** Mensaje si no se pudieron cargar las reglas (sin ellas no hay nada que mostrar). */
  const [error, setError] = useState<string | null>(null);

  const openTokenRef = useRef(0);
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const openGame = useCallback(
    async (g: Game) => {
      const token = ++openTokenRef.current;

      // Reset inmediato de los datos del juego anterior
      setCurrent(g);
      setRules(null);
      setInfo(null);
      setEngine(null);
      setEngineRecommendations([]);
      setCommunityRecommendations([]);
      setLoading(true);
      setError(null);

      try {
        // appid 0 = juego de Lutris/Heroic/HeroNite → reglas genéricas
        const appidToUse = g.appid === 0 ? 0 : g.appid;

        const [r, i, eng] = await Promise.all([
          api.getRules(appidToUse),
          // game_info consulta Steam y ProtonDB por red. Si cualquiera falla
          // (sin internet, timeout, juego retirado de la tienda) se sigue
          // adelante con datos vacíos: sin esto, `info` quedaba en null y la
          // vista del juego no se pintaba nunca. Es lo que pasaba al abrir un
          // juego del buscador que no está en la biblioteca.
          g.appid === 0
            ? Promise.resolve({
                name: g.name,
                native: g.launcher === 'lutris' || g.launcher === 'heronite',
                errors: [],
              } as GameInfo)
            : api.gameInfo(g.appid).catch((e) => {
                console.warn('No se pudo consultar Steam/ProtonDB:', e);
                // `native` queda sin definir (= desconocido), no null.
                return { name: g.name, errors: [String(e)] } as GameInfo;
              }),
          api.detectEngine(appidToUse, g.name).catch(() => null),
        ]);

        // Motor y comunidad se pueden pedir ya, en paralelo
        const [engineRecs, communityRecs] = await Promise.all([
          eng ? api.getEngineRecommendations(eng) : Promise.resolve<string[]>([]),
          api.getCommunityRecommendations(getHwTags()).catch((e) => {
            console.warn('No se pudieron cargar las recomendaciones de la comunidad:', e);
            return [] as CommunityRecommendation[];
          }),
        ]);

        if (!mountedRef.current || token !== openTokenRef.current) return;

        setRules(r);
        setInfo(i);
        setEngine(eng);
        setEngineRecommendations(engineRecs);
        setCommunityRecommendations(communityRecs);
      } catch (e) {
        console.error(e);
        if (mountedRef.current && token === openTokenRef.current) {
          setError(typeof e === 'string' ? e : 'No se pudieron cargar los datos del juego');
        }
      } finally {
        if (mountedRef.current && token === openTokenRef.current) setLoading(false);
      }
    },
    [getHwTags]
  );

  /** Cierra el juego abierto (volver a la biblioteca). */
  const closeGame = useCallback(() => {
    openTokenRef.current++;
    setError(null);
    setCurrent(null);
    setRules(null);
    setInfo(null);
    setEngine(null);
    setEngineRecommendations([]);
    setCommunityRecommendations([]);
  }, []);

  return {
    current,
    rules,
    info,
    engine,
    engineRecommendations,
    communityRecommendations,
    loading,
    error,
    openGame,
    closeGame,
  };
}
