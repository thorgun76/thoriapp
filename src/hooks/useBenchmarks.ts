import { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { ContextoDeCaptura } from '../lib/api';
import type { Benchmark, OrigenComando, RecordedSetup } from '../lib/types';
import { notifyComparisonsChanged } from './useSavedComparisons';

/**
 * ¿Son el mismo juego dos nombres?
 *
 * Se comparan normalizados (minúsculas, sin ruta ni extensión) porque el nombre del CSV
 * sale del propio fichero de MangoHud y el de la ficha puede traer formato distinto.
 */
export function MismoJuego(a: string, b: string): boolean {
  const norm = (s: string) =>
    s
      .trim()
      .toLowerCase()
      .replace(/\.[a-z0-9]+$/, '')
      .replace(/\.(x86_64|x86|exe|bin)$/, '');
  return norm(a) === norm(b);
}

/** Grabaciones que se conservan de cada juego. Debe coincidir con el backend. */
export const MAX_BENCHMARKS_PER_GAME = 10;

interface UseBenchmarksOptions {
  currentAppid?: number;
  currentGameName?: string;
  currentLaunchCommand?: string;
  /**
   * Comando configurado para el juego de una grabación que **no** es el que está abierto.
   *
   * Llegar a «Pruebas» desde la barra lateral no abre ningún juego; si el usuario graba
   * así, la grabación sí dice de qué juego es (el nombre, del propio fichero, y el AppID
   * cuando el logro lo trae) y Thoriapp puede recuperar el comando verdadero en vez de
   * guardar la prueba sin él.
   *
   * El AppID llega a 0 cuando el CSV no lo trae: el nombre es entonces la única identidad,
   * por eso se pasan las dos cosas. Devuelve `null` si no lo sabe; no se inventa nada.
   */
  comandoParaJuego?: (juego: { appid: number; gameName: string }) => Promise<string | null>;
  /**
   * Opciones de lanzamiento que Steam tiene configuradas para un juego.
   *
   * Se leen **al guardar** la prueba, no al grabar: Steam no deja saber cuándo cambió el
   * usuario el texto, así que la lectura más cercana a la grabación es la última hecha
   * antes de guardarla. Es lo que de verdad se ejecutó al entrar al juego, y por eso es el
   * dato que dice con qué argumento se hizo la prueba (el comando del editor puede
   * perfectamente no haber llegado nunca a Steam).
   *
   * Devuelve `null` si no se pudo leer: entonces la prueba queda sin dato, no con `''`.
   *
   * Se pasa también el nombre porque el AppID de un CSV de MangoHud es 0: el backend
   * resuelve el juego por nombre contra la biblioteca de Steam.
   */
  steamParaJuego?: (juego: { appid: number; gameName: string }) => Promise<string | null>;
  /**
   * Contexto de captura para la instantánea histórica (subfase 5.2).
   *
   * Se pide **por juego de la grabación**, no «lo que haya abierto»: el CSV puede ser de un
   * juego distinto del que el usuario está configurando. Si la interfaz no puede decir que
   * el contexto es de ESE juego, devuelve `correspondeAlJuego: false` y la configuración se
   * registra como desconocida.
   *
   * Si no se pasa, la instantánea se guarda igual pero sin configuración: el hueco queda
   * visible como «desconocido», nunca relleno con datos de otro sitio.
   */
  contextoParaJuego?: (juego: {
    appid: number;
    gameName: string;
  }) => Promise<ContextoDeCaptura | null>;
  onNotify?: (message: string) => void;
}

/**
 * Las grabaciones de MangoHud: cargarlas, analizar un log nuevo y borrarlas.
 *
 * El backend conserva las últimas MAX_BENCHMARKS_PER_GAME grabaciones de cada juego, así
 * que aquí solo se mantiene la lista tal cual llega de Rust (de la más antigua a la más
 * reciente). El recorte no se replica aquí: si algún día cambia en Rust, cambia solo allí.
 */
export function useBenchmarks({
  currentAppid,
  currentGameName,
  currentLaunchCommand,
  comandoParaJuego,
  steamParaJuego,
  contextoParaJuego,
  onNotify,
}: UseBenchmarksOptions) {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [analyzing, setAnalyzing] = useState(false);
  /**
   * Motivo por el que no se pudo leer el historial, o `null`.
   *
   * «No hay pruebas» y «no se pudieron leer las pruebas» son afirmaciones distintas: la
   * primera se pinta como una pantalla vacía; la segunda tiene que verse, porque debajo
   * hay un archivo con datos que no se han perdido.
   */
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      // `0` **no** es «no hay juego»: es un juego sin AppID de Steam, que es el caso de
      // todas las grabaciones de MangoHud (`analyze_log` deja `appid` a 0 porque el CSV no
      // lo trae). Con `!currentAppid` la ventana de un juego sin AppID pedía el historial
      // **completo** y luego lo enseñaba sin filtrar; lo correcto es filtrar por nombre.
      // Solo `undefined` significa «no hay ningún juego abierto».
      const hayJuego = currentAppid !== undefined && currentAppid !== null;
      const list = hayJuego
        ? await api.loadBenchmarksForGame({ appid: currentAppid, gameName: currentGameName })
        : await api.loadBenchmarks();
      setBenchmarks(list);
      setError(null);
    } catch (e) {
      console.error('Error al cargar benchmarks:', e);
      setError(String(e));
    }
  }, [currentAppid, currentGameName]);

  useEffect(() => {
    load();
  }, [load]);

  /**
   * Analiza el log de MangoHud y lo guarda.
   * `warmupSeconds` descarta los primeros segundos de la grabación (arranque y carga).
   * Devuelve `false` si había un duplicado o hubo error, para que la vista sepa
   * si limpiar el campo de notas.
   */
  const analyze = useCallback(
    async (
      logPath: string,
      notes: string,
      warmupSeconds: number,
      /**
       * Configuración que había al grabar. Si no se pasa, se usa el comando actual; es lo
       * correcto cuando el usuario acaba de analizar el fichero a mano.
       */
      setup?: RecordedSetup | null
    ): Promise<boolean> => {
      if (!logPath.trim()) {
        onNotify?.('Ingresa la ruta del log de MangoHud');
        return false;
      }

      setAnalyzing(true);
      try {
        const benchmark = await api.analyzeMangohudLog(logPath, warmupSeconds);

        // Se compara la RUTA del CSV, no las métricas. Comparar números daba falsos
        // positivos: dos grabaciones distintas en la misma zona del juego producen cifras
        // casi idénticas y la segunda se rechazaba como "duplicado".
        const duplicate = await api.isLogAlreadySaved(logPath);
        if (duplicate) {
          onNotify?.('⚠️ Esa grabación ya está guardada en el historial');
          return false;
        }

        // El log a veces no trae el nombre del juego: se usa el que tiene abierto la app.
        let gameName = benchmark.game_name;
        const knownName = currentGameName && currentGameName !== 'Desconocido';
        if ((!gameName || gameName === 'Desconocido') && knownName) {
          gameName = currentGameName;
        }

        // Comando registrado para ESTA grabación.
        //
        // Prioridad: la instantánea tomada al detectar la grabación (`setup.command`). Si
        // no hay instantánea, se usa el comando configurado ahora, que es lo único que se
        // sabe: puede no ser el de la grabación, pero es mejor que nada y la interfaz lo
        // etiqueta como «comando registrado», no como «comando ejecutado».
        //
        // Nunca se inventa `%command%`: ese texto significa «sin ajustes activos», así que
        // ponerlo por defecto convertiría «no lo sé» en «no había nada configurado», que
        // es justo el error que mostraba una prueba de Palworld con
        // `mesa_glthread=true game-performance mangohud %command%` como `%command%`.
        // Una instantánea **vacía no gana** a un comando conocido. `??` no servía: deja
        // pasar `''` (la instantánea se tomó con el perfil aún sin cargar, `rules` nulo,
        // o un juego sin reglas), y entonces se guardaba «sin comando» cuando la app sí
        // sabía cuál era. Ese hueco es exactamente el que después se pintaba como
        // `%command%`, dando a entender «sin ajustes» en una grabación que se hizo con
        // `mesa_glthread=true game-performance mangohud %command%`.
        const appidGrabacion = currentAppid || benchmark.appid || 0;

        const deInstantanea = (setup?.command ?? '').trim();
        let actual = (currentLaunchCommand ?? '').trim();

        // Si la grabación no es del juego abierto, el comando del editor no vale: se
        // pregunta por el juego real. Es lo que pasa al grabar desde «Pruebas» sin haber
        // abierto la ficha.
        //
        // Ojo con identificar ese juego: `benchmark.appid` viene **siempre a 0** al analizar
        // un CSV (MangoHud no escribe el AppID en el fichero). Así que la identidad real es
        // el nombre del juego, que sí viaja en los logros. Comparar solo por appid hacía
        // que esta recuperación no se ejecutara nunca.
        const mismoJuego =
          !!appidGrabacion &&
          appidGrabacion === currentAppid &&
          (!currentGameName ||
            !benchmark.game_name ||
            benchmark.game_name === 'Desconocido' ||
            MismoJuego(benchmark.game_name, currentGameName));

        if (!deInstantanea && !mismoJuego && comandoParaJuego) {
          try {
            actual = (
              (await comandoParaJuego({
                appid: appidGrabacion,
                gameName: benchmark.game_name,
              })) ?? ''
            ).trim();
          } catch {
            actual = '';
          }
        }

        const comandoRegistrado = deInstantanea || actual;
        // De dónde salió el texto, para el registro de la grabación. Sin esto, cuando una
        // prueba sale con `%command%` no hay forma de saber si fue porque de verdad no
        // había ajustes o porque el dato se perdió por el camino.
        const origenComando: OrigenComando = deInstantanea
          ? 'instantanea'
          : actual
            ? 'comando_actual'
            : 'desconocido';

        // Lo que Steam tiene configurado de verdad. Se lee aquí, junto al guardado, porque
        // es el único sitio donde se conoce el AppID de la grabación (el CSV no lo trae).
        //
        // Es lo que responde a «¿con qué argumento hice esta prueba?»: el comando del
        // editor puede no haber llegado nunca a Steam (se copia a mano), y cuando la
        // instantánea sale vacía el editor devuelve `%command%`, que no dice nada. Si la
        // lectura falla, la prueba se guarda sin dato: `null` no es `''`.
        let steamCommand: string | null = null;
        try {
          steamCommand = steamParaJuego
            ? await steamParaJuego({ appid: appidGrabacion, gameName })
            : await api
                .steamLaunchOptionsParaJuego({ appid: appidGrabacion, gameName })
                .then((o: { opciones: string } | null) => o?.opciones ?? null);
        } catch {
          steamCommand = null;
        }

        // Contexto de configuración para la instantánea. Un fallo aquí no impide
        // la prueba: la instantánea queda con la configuración en desconocido, que es un
        // estado que la interfaz sabe mostrar.
        let contexto: ContextoDeCaptura | null = null;
        try {
          contexto = contextoParaJuego
            ? await contextoParaJuego({ appid: appidGrabacion, gameName: benchmark.game_name })
            : null;
        } catch {
          contexto = null;
        }

        // Pista para el observador de sesiones: el juego del que trata esta grabación. No
        // es evidencia de identidad —el AppID del CSV no existe—, solo dice qué juego
        // acaba de grabarse, para poder ponerle nombre a una sesión sin identificar.
        // Un fallo aquí no puede impedir guardar la prueba.
        try {
          await api.fijarPistaDeJuego(benchmark.game_name ?? gameName ?? null);
        } catch {
          // Sin pista el backend sigue funcionando; solo pierde ese desempate.
        }

        await api.saveBenchmark({
          benchmark,
          profileId: '',
          gameName,
          appid: appidGrabacion,
          launchCommand: comandoRegistrado,
          origenComando,
          steamCommand,
          notes: notes || null,
          contexto,
        });

        // Aviso cuando el comando no vino de la instantánea de la grabación: el usuario
        // debe saber que ese dato es el de ahora, no necesariamente el de la prueba.
        const avisoOrigen =
          origenComando === 'instantanea'
            ? ''
            : origenComando === 'comando_actual'
              ? ' · Comando tomado del actual, no de la grabación'
              : ' · Sin comando registrado';

        // El backend conserva las últimas MAX_BENCHMARKS_PER_GAME grabaciones de cada
        // juego; el mensaje solo explica en qué punto de la serie queda esta.
        const count = benchmarks.length;
        // Se dice de dónde salió el comando cuando no fue de la instantánea: es la
        // diferencia entre «esta prueba se grabó así» y «el comando se leyó al guardar».
        if (origenComando !== 'instantanea') {
          console.info(
            `[benchmark] comando registrado desde ${origenComando}: "${comandoRegistrado}"`
          );
        }
        if (count === 0) {
          onNotify?.(
            `✓ Guardada como punto de partida (${gameName}). Cambia algo y graba otra vez para comparar${avisoOrigen}`
          );
        } else if (count >= MAX_BENCHMARKS_PER_GAME) {
          onNotify?.(
            `✓ Guardada. Ya tenías ${MAX_BENCHMARKS_PER_GAME}: se borró la más antigua de ${gameName}${avisoOrigen}`
          );
        } else {
          onNotify?.(
            `✓ Guardada (${count + 1} de ${MAX_BENCHMARKS_PER_GAME}). Se compara sola con la anterior${avisoOrigen}`
          );
        }

        await load();
        return true;
      } catch (e) {
        onNotify?.(`❌ Error: ${e}`);
        return false;
      } finally {
        setAnalyzing(false);
      }
    },
    [benchmarks, currentAppid, currentGameName, currentLaunchCommand, comandoParaJuego, steamParaJuego, load, onNotify]
  );

  const remove = useCallback(
    async (id: string) => {
      try {
        await api.deleteBenchmark(id);
        onNotify?.('✓ Benchmark eliminado');
        await load();
      } catch (e) {
        onNotify?.(`❌ Error al eliminar: ${e}`);
      }
    },
    [load, onNotify]
  );

  const removeAll = useCallback(async () => {
    try {
      await api.deleteAllBenchmarks();
      onNotify?.(`✓ Se eliminaron ${benchmarks.length} benchmarks`);
      await load();
      // El backend purga las comparaciones guardadas que usaban estas pruebas. Sin esto,
      // «Mis pruebas» seguiría enseñando informes que ya no están en disco.
      notifyComparisonsChanged();
    } catch (e) {
      onNotify?.(`❌ Error al eliminar: ${e}`);
    }
  }, [benchmarks.length, load, onNotify]);

  /**
   * Borra **solo** el historial del juego abierto.
   *
   * `removeAll` vacía la lista entera, que desde la ficha de un juego se llevaría por
   * delante las pruebas de todos los demás. Aquí se pide al backend que filtre por juego.
   */
  const removeAllForGame = useCallback(async () => {
    if (!currentAppid && !currentGameName) return;
    try {
      const borradas = await api.deleteBenchmarksForGame({
        appid: currentAppid ?? 0,
        gameName: currentGameName ?? '',
      });
      onNotify?.(
        borradas ? `✓ Se eliminaron ${borradas} pruebas de este juego` : 'No había pruebas que borrar'
      );
      await load();
      // Igual que arriba: la purga de comparaciones huérfanas ocurre en el backend.
      if (borradas) notifyComparisonsChanged();
    } catch (e) {
      onNotify?.(`❌ Error al eliminar: ${e}`);
    }
  }, [currentAppid, currentGameName, load, onNotify]);

  return { benchmarks, analyzing, load, analyze, remove, removeAll, removeAllForGame, error };
}
