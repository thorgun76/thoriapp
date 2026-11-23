import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type {
  AntiCheatInfo,
  CommunityRecommendation,
  Game,
  GameEngine,
  GameInfo,
  GameRules,
  ProtonPulseSummary,
  RequirementsReport,
} from '../lib/types';
import { resolveAppId } from '../lib/appid';
import { getOverride, setOverride } from '../lib/appidOverride';

interface UseGameDetailsOptions {
  /** Tags de hardware (getter, para no recrear callbacks). */
  getHwTags: () => string[];
  /**
   * GPU del equipo (`hardware.gpu`), como getter por el mismo motivo.
   *
   * No basta con los tags: decidir si recomendar `VKD3D_CONFIG=dxr` exige el
   * MODELO (¿tiene ray tracing utilizable?), y el tag `nvidia` lo tienen igual una
   * GTX 1660 Ti y una RTX 4070.
   */
  getGpuName?: () => string | null;
}

/** appid 0 = juego de Lutris/Heroic/Heronite sin AppID de Steam: no hay ficha
 * que consultar, ni requisitos, ni motor. Se centraliza para no repetir `=== 0`. */
function appidDe(g: Game): number {
  return g.appid === 0 ? 0 : g.appid;
}

/**
 * Info de Steam/ProtonDB de un juego que no trae AppID (Lutris, Heroic, HeroNite).
 *
 * Se busca el AppID por nombre en la tienda y solo se acepta una coincidencia
 * exacta (ver `lib/appid`). Antes esta rama daba el juego por nativo de Linux
 * (`launcher === 'lutris'`) sin consultar nada: Lutris también mueve juegos de
 * Windows con Wine, así que la etiqueta «Linux nativo» era una suposición, y el
 * nivel de ProtonDB no se pedía nunca aunque el juego estuviera en Platino.
 *
 * Sin coincidencia, `native` queda desconocido (no `true`) y `appid` sin definir.
 */
async function externalGameInfo(g: Game): Promise<GameInfo> {
  const plain: GameInfo = { name: g.name, errors: [] };
  // Vínculo manual (el usuario ya dijo qué juego es): manda sobre la búsqueda.
  const manual = getOverride(g);
  if (manual !== null) {
    try {
      return { ...(await api.gameInfo(manual)), appid: manual };
    } catch (e) {
      console.warn('No se pudo consultar el AppID vinculado a mano:', e);
      return { ...plain, appid: manual };
    }
  }
  try {
    const { appid, tried, found } = await resolveAppId(g.name, api.searchStore);
    // Rastro para poder diagnosticar desde la consola: qué nombre buscó la app
    // y qué devolvió la tienda. Sin esto solo se veía «sin datos» sin motivo.
    console.info(
      '[ProtonDB] AppID de «%s» → %s (búsquedas: %s; tienda: %s)',
      g.name,
      appid ?? 'sin coincidencia',
      tried.join(' / '),
      found.join(' | ') || 'nada'
    );
    if (appid === null) return plain;
    return { ...(await api.gameInfo(appid)), appid };
  } catch (e) {
    console.warn('No se pudo resolver el AppID de Steam:', e);
    return plain;
  }
}

/**
 * Datos del juego abierto: reglas, info de ProtonDB, motor y recomendaciones.
 *
 * Antes `openGame()` escribía en 8 estados distintos de App.tsx. Aquí viven
 * juntos, y existe `openToken` para descartar respuestas de un juego que el
 * usuario ya abandonó (evita que un clic rápido deje datos cruzados).
 */
export function useGameDetails({ getHwTags, getGpuName }: UseGameDetailsOptions) {
  const [current, setCurrent] = useState<Game | null>(null);
  const [rules, setRules] = useState<GameRules | null>(null);
  const [info, setInfo] = useState<GameInfo | null>(null);
  /** Anti-cheat del juego (AreWeAntiCheatYet). `null` = sin datos o aún cargando. */
  const [antiCheat, setAntiCheat] = useState<AntiCheatInfo | null>(null);
  /** Requisitos de Steam frente a esta máquina. `null` = sin datos. */
  const [requirements, setRequirements] = useState<RequirementsReport | null>(null);
  /** Reportes de Proton Pulse filtrados por esta GPU. `null` = sin datos. */
  const [protonPulse, setProtonPulse] = useState<ProtonPulseSummary | null>(null);
  const [protonPulseLoading, setProtonPulseLoading] = useState(false);
  const [requirementsLoading, setRequirementsLoading] = useState(false);
  const [requirementsError, setRequirementsError] = useState<string | null>(null);
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
      setAntiCheat(null);
      setProtonPulse(null);
      setProtonPulseLoading(appidDe(g) !== 0);
      setRequirements(null);
      setRequirementsError(null);
      setRequirementsLoading(appidDe(g) !== 0);
      setEngine(null);
      setEngineRecommendations([]);
      setCommunityRecommendations([]);
      setLoading(true);
      setError(null);

      try {
        // appid 0 = juego de Lutris/Heroic/HeroNite → reglas genéricas
        const appidToUse = appidDe(g);

        const [r, i, eng] = await Promise.all([
          api.getRules(appidToUse),
          // game_info consulta Steam y ProtonDB por red. Si cualquiera falla
          // (sin internet, timeout, juego retirado de la tienda) se sigue
          // adelante con datos vacíos: sin esto, `info` quedaba en null y la
          // vista del juego no se pintaba nunca. Es lo que pasaba al abrir un
          // juego del buscador que no está en la biblioteca.
          g.appid === 0
            ? externalGameInfo(g)
            : api.gameInfo(g.appid).catch((e) => {
                console.warn('No se pudo consultar Steam/ProtonDB:', e);
                // `native` queda sin definir (= desconocido), no null.
                return { name: g.name, errors: [String(e)] } as GameInfo;
              }),
          api.detectEngine(appidToUse, g.name).catch(() => null),
        ]);

        // Motor y comunidad se pueden pedir ya, en paralelo
        const [engineRecs, communityRecs, ac] = await Promise.all([
          eng ? api.getEngineRecommendations(eng, getGpuName?.() ?? null) : Promise.resolve<string[]>([]),
          api.getCommunityRecommendations(getHwTags()).catch((e) => {
            console.warn('No se pudieron cargar las recomendaciones de la comunidad:', e);
            return [] as CommunityRecommendation[];
          }),
          // El anti-cheat decide si el juego arranca siquiera: si falla, se avisa en la
          // UI como «sin datos» en vez de dar el juego por bueno.
          g.appid === 0
            ? Promise.resolve(null)
            : api
                .getAnticheatInfo(g.appid)
                .then((r) => r.info)
                .catch((e) => {
                  console.warn('No se pudo consultar el anti-cheat:', e);
                  return null;
                }),
        ]);

        if (!mountedRef.current || token !== openTokenRef.current) return;

        setRules(r);
        setInfo(i);
        setEngine(eng);
        setEngineRecommendations(engineRecs);
        setCommunityRecommendations(communityRecs);
        setAntiCheat(ac);

        // Requisitos aparte y sin `await`: es lo que más tarda (Steam + disco) y
        // no debe retrasar la vista. Si falla, la tarjeta dice por qué.
        // Requisitos y Proton Pulse aparte y sin `await`: los dos van por red y
        // ninguna vista debe esperar a ellos para pintarse.
        if (appidToUse !== 0) {
          api
            .getProtonPulseReports(appidToUse, getGpuName?.() ?? null)
            .then((sum) => {
              if (mountedRef.current && token === openTokenRef.current) setProtonPulse(sum);
            })
            .catch((e) => {
              // Sin reportes no se pierde nada: la tarjeta simplemente no sale.
              console.warn('No se pudieron leer los reportes de Proton Pulse:', e);
            })
            .finally(() => {
              if (mountedRef.current && token === openTokenRef.current) {
                setProtonPulseLoading(false);
              }
            });
        }

        if (appidToUse !== 0) {
          api
            .getRequirements(appidToUse, g.install_path ?? null)
            .then((rep) => {
              if (mountedRef.current && token === openTokenRef.current) setRequirements(rep);
            })
            .catch((e) => {
              console.warn('No se pudieron leer los requisitos:', e);
              if (mountedRef.current && token === openTokenRef.current) {
                setRequirementsError(
                  typeof e === 'string' ? e : 'sin conexión con Steam'
                );
              }
            })
            .finally(() => {
              if (mountedRef.current && token === openTokenRef.current) {
                setRequirementsLoading(false);
              }
            });
        }
      } catch (e) {
        console.error(e);
        if (mountedRef.current && token === openTokenRef.current) {
          setError(typeof e === 'string' ? e : 'No se pudieron cargar los datos del juego');
        }
      } finally {
        if (mountedRef.current && token === openTokenRef.current) setLoading(false);
      }
    },
    [getHwTags, getGpuName]
  );

  /** Cierra el juego abierto (volver a la biblioteca). */
  const closeGame = useCallback(() => {
    openTokenRef.current++;
    setError(null);
    setCurrent(null);
    setRules(null);
    setInfo(null);
    setAntiCheat(null);
    setProtonPulse(null);
    setProtonPulseLoading(false);
    setRequirements(null);
    setRequirementsError(null);
    setRequirementsLoading(false);
    setEngine(null);
    setEngineRecommendations([]);
    setCommunityRecommendations([]);
  }, []);

  /**
   * Vincula a mano el juego abierto con un AppID de Steam.
   *
   * Es la red de seguridad de la detección automática: cuando la biblioteca
   * guarda un nombre que no se puede emparejar con la tienda ("NoManSky_v5.0"),
   * el usuario elige el juego una vez y se recuerda. No recarga reglas ni motor:
   * el comando no cambia, solo la ficha de ProtonDB/requisitos.
   */
  const linkAppid = useCallback(async (g: Game, appid: number) => {
    const token = ++openTokenRef.current;
    setOverride(g, appid);
    try {
      const fresh = await api.gameInfo(appid);
      if (!mountedRef.current || token !== openTokenRef.current) return;
      setInfo({ ...fresh, appid });
    } catch (e) {
      console.warn('No se pudo consultar el AppID vinculado:', e);
      if (!mountedRef.current || token !== openTokenRef.current) return;
      setInfo({ name: g.name, appid, errors: [String(e)] });
    }
  }, []);

  return {
    current,
    rules,
    info,
    antiCheat,
    requirements,
    requirementsLoading,
    requirementsError,
    protonPulse,
    protonPulseLoading,
    engine,
    engineRecommendations,
    communityRecommendations,
    loading,
    error,
    openGame,
    closeGame,
    /** Vincula a mano este juego con un AppID de Steam y recarga sus datos. */
    linkAppid,
  };
}
