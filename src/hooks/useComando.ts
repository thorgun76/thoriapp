/**
 * Estado de edición del comando de lanzamiento.
 *
 * ## Qué problema resuelve
 *
 * El comando de un juego tiene tres versiones a la vez y hasta la fase 2 se mezclaban:
 *
 * - **el original importado** (lo que había en Steam), que no debe cambiar nunca;
 * - **el comando actual**, que sale de lo que el motor confirmó la última vez;
 * - **lo que el usuario está pidiendo ahora**, que son los toggles, y que puede no haberse
 *   podido aplicar todavía.
 *
 * Mezclarlos es lo que producía el fallo original: al importar se pasaba a «modo manual»,
 * la línea mostrada dejaba de venir de los toggles y los toggles seguían moviéndose sin
 * efecto. Aquí el texto siempre es **lo que el motor confirmó**, y los toggles son
 * peticiones que se confirman o se rechazan, nunca un texto alternativo.
 *
 * ## Quién manda
 *
 * Rust. Este archivo no sabe qué es un wrapper, ni en qué orden van, ni qué significa
 * «ambigua»: pide `analizar`, `estado_de_ajustes` y `serializar` y pinta lo que le dan. Lo
 * único que decide es **cuándo** se pide y **qué petición es la buena** cuando llegan
 * desordenadas (`SecuenciaComando`).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../lib/api';
import type {
  AnalisisComando,
  CambioAjuste,
  ErrorMotor,
  EstadoAjuste,
  ResultadoSerializacion,
} from '../lib/motorComando';
import { estaEncendido, esAccionable as esAccionableSegunMotor } from '../lib/motorComando';
import { SecuenciaComando, type ContextoComando, type Peticion } from '../lib/secuenciaComando';
import type { Param } from '../lib/types';

/**
 * De dónde sale el comando que se está viendo.
 *
 * - `inicial`: el juego se acaba de abrir, aún no hay nada. No se pinta como «sin ajustes»
 *   porque eso afirmaría algo que no se ha comprobado.
 * - `propuesta`: Thoriapp genera el comando a partir de los ajustes del catálogo.
 * - `importado`: se trajo un texto externo (Steam, portapapeles, perfil) y el motor lo
 *   analizó. Los ajustes reconocidos quedan editables y el resto se conserva.
 * - `original_restaurado`: se volvió al texto importado tras haberlo modificado.
 */
export type Procedencia =
  | 'inicial'
  | 'propuesta'
  | 'importado'
  | 'original_restaurado'
  /** Viene de un perfil guardado que el usuario cargó. */
  | 'perfil';

/** Una revisión confirmada por el motor. Todo lo que se pinta sale de aquí. */
export interface RevisionComando {
  /** El comando, tal y como lo confirmó el motor. */
  comando: string;
  /** Análisis del comando. `null` mientras no hay análisis. */
  analisis: AnalisisComando | null;
  /** Estado de cada ajuste del catálogo, en el orden del catálogo. */
  estados: EstadoAjuste[];
  /**
   * El texto del que se partió al importar. **Inmutable.** `null` si no se importó nada.
   *
   * Es lo que devuelve «Restaurar comando original» y contra lo que se mide qué ha
   * cambiado el usuario.
   */
  originalImportado: string | null;
  procedencia: Procedencia;
}

/** Un problema del motor, ya traducido a algo que se puede enseñar. */
export interface FalloMotor {
  /** Qué pasó, en una frase. */
  mensaje: string;
  /**
   * Qué se puede hacer.
   *
   * Se calcula en Rust (`ErrorAnalisis`), no aquí: el frontend no tiene por qué saber qué
   * combinaciones de segmentos son inseguras.
   */
  detalle?: string;
}

export interface EstadoComando {
  /** La última revisión confirmada. Nunca cambia a medias. */
  revision: RevisionComando;
  /** Hay una petición en vuelo. Los controles se pueden seguir usando (se encolan). */
  cargando: boolean;
  /** El último fallo del motor, o `null`. Un fallo **no** descarta la revisión anterior. */
  fallo: FalloMotor | null;
  /**
   * Ajustes pedidos que aún no ha confirmado el motor (`null` = ninguno en vuelo).
   *
   * La interfaz lo usa para pintar el toggle **al instante**, en vez de esperar al IPC y
   * enseñar el estado viejo durante todo el viaje (que se veía como pestañeo).
   */
  ajustesPendientes: Set<number> | null;
  /** Pide cambiar un ajuste. Se puede llamar varias veces seguidas. */
  fijarAjuste: (indice: number, activo: boolean) => void;
  /** Pide fijar varios ajustes a la vez (importar, restaurar, limpiar). */
  fijarAjustes: (activos: Set<number>) => void;
  /** Analiza un texto externo y lo adopta como comando actual, conservando el original. */
  importar: (texto: string) => Promise<void>;
  /** Vuelve exactamente al comando importado, con sus ajustes originales. */
  restaurarImportado: () => Promise<void>;
  /**
   * Carga el comando de un perfil, **sin** tocar la instantánea original importada.
   *
   * Devuelve el comando que el motor confirmó, o `null` si falló. En ese caso la revisión
   * anterior sigue intacta: la sustitución solo se confirma al final, así que un error del
   * motor no deja el editor con un comando a medias.
   */
  cargarPerfil: (texto: string) => Promise<string | null>;
  /** Vuelve al comando propuesto (ningún ajuste encendido). */
  limpiarAjustes: () => Promise<void>;
  /**
   * Quita un argumento conservado de la tarjeta de argumentos.
   *
   * Solo lo intenta; el motor decide si es seguro. Si no lo es, el comando no cambia y se
   * deja constancia, en vez de borrar algo que el usuario no esperaba.
   */
  quitarSegmento: (indice: number) => Promise<void>;
  /** Copia del original importado, para mostrarla o guardarla. */
  originalImportado: string | null;
  /** Último comando confirmado. Atajo de `revision.comando`. */
  comando: string;
}

/**
 * Estado de edición del comando de un juego.
 *
 * @param appid Juego abierto. Cambiarlo reinicia todo: no se arrastra nada del anterior.
 * @param params Catálogo de ajustes del juego (`rules.params`). Es solo para saber qué
 *   índices existen; el significado de cada uno lo pone Rust.
 * @param comandoInicial Texto del que partir al abrir el juego (un perfil guardado, el que
 *   Steam tenía, o `%command%`). Si cambia, se reanaliza.
 * @param preparado `false` mientras las reglas del juego aún cargan. Sin ellas no se pide
 *   nada: analizar contra un catálogo vacío diría «no reconozco nada» y borraría los
 *   ajustes que en realidad sí lo son.
 */
export function usarComando(
  appid: number | undefined,
  params: Param[],
  comandoInicial: string | null,
  preparado: boolean,
  /**
   * Argumentos que la UI ofrece como recomendación (motor y hardware) y que **no** están en
   * `params`. Se mandan al motor para que formen parte del catálogo efectivo: así activarlas
   * es una operación del motor como cualquier otra y el comando de arriba las recoge.
   *
   * Si se quedaran solo en React, marcarlas no cambiaría el comando. Es justo el fallo que
   * reportó el usuario.
   */
  recomendaciones: string[] = [],
): EstadoComando {
  const secuencia = useRef(new SecuenciaComando());
  // El motor recibe esta lista en **cada** llamada (analizar, estados, serializar): los
  // índices del análisis solo valen si el catálogo tiene la misma longitud en las tres.
  const extras = useMemo(
    () => recomendaciones.map((r) => r.trim()).filter(Boolean),
    // Se compara por contenido, no por referencia: la lista se reconstruye en cada render
    // del padre y una dependencia por identidad dispararía análisis en bucle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [recomendaciones.join('\u0000')],
  );
  const [revision, setRevision] = useState<RevisionComando>(VACIA);
  const [cargando, setCargando] = useState(false);
  const [fallo, setFallo] = useState<FalloMotor | null>(null);

  /**
   * Ajustes que el usuario ha pedido y aún no se han confirmado.
   *
   * Es lo que permite pulsar tres toggles seguidos sin que el segundo se calcule sobre un
   * comando que todavía no ha llegado: se acumulan aquí y se manda el conjunto completo.
   */
  const pendientes = useRef<Set<number> | null>(null);
  /** El bucle de aplicación en marcha, si lo hay. Uno a la vez. */
  const bucle = useRef<Promise<void> | null>(null);
  /**
   * Último conjunto **pedido**, aunque ya se haya mandado.
   *
   * Dura toda la vida del editor, no solo la de un bucle: es la base sobre la que se
   * aplica cada pulsación. Si se borrara al aplicar, la siguiente pulsación partiría de
   * `revision.estados`, que todavía no incluye lo pedido pero no confirmado, y el toggle
   * anterior desaparecería. Se resetea solo cuando el motor confirma (ver `aplicar`).
   */
  const pedido = useRef<Set<number> | null>(null);

  /**
   * Lo mismo que `pedido`, pero en estado de React para poder **pintarlo**.
   *
   * Sin esto el toggle no se movía al pulsarlo: la interfaz solo leía `revision.estados`,
   * que es lo confirmado, así que durante el viaje al motor seguía dibujando el estado
   * viejo y el interruptor «saltaba» al volver. Es lo que se veía como pestañeo.
   *
   * Se pinta lo pedido mientras está en vuelo y se limpia cuando el motor confirma (o
   * falla), que es cuando manda lo confirmado otra vez.
   */
  const [ajustesPendientes, setAjustesPendientes] = useState<Set<number> | null>(null);

  const contexto = useMemo<ContextoComando>(
    () => ({ appid: appid ?? 0, original: revision.analisis?.original ?? '' }),
    [appid, revision.analisis],
  );

  /** Guarda una revisión confirmada. Todo pasa por aquí: una sola puerta. */
  /**
   * Última revisión confirmada, además del estado.
   *
   * `sustituir` es `void` porque nadie más necesita el resultado, pero cargar un perfil sí:
   * el llamante quiere saber qué comando confirmó el motor (o si no confirmó ninguno).
   */
  const ultima = useRef<RevisionComando | null>(null);

  const confirmar = useCallback((nueva: RevisionComando) => {
    ultima.current = nueva;
    setRevision(nueva);
    setFallo(null);
  }, []);

  /**
   * Analiza un texto y lo adopta como comando actual.
   *
   * El análisis y los estados llegan en la misma tanda: si se pintaran por separado habría
   * un instante con el comando nuevo y los toggles viejos, y pulsar en ese instante aplica
   * el cambio al comando equivocado.
   */
  const analizarYAdoptar = useCallback(
    async (
      texto: string,
      opciones: { originalImportado?: string | null; procedencia?: Procedencia } = {},
    ): Promise<RevisionComando | null> => {
      if (appid === undefined) return null;
      let analisis: AnalisisComando;
      try {
        analisis = await api.analyzeLaunchCommand(texto, appid, extras);
      } catch (e) {
        setFallo({ mensaje: mensajeDeError(e) });
        return null;
      }

      // Los estados se piden después del análisis y con el MISMO análisis: así no puede
      // colarse un análisis nuevo entre una cosa y la otra.
      const estados = await api.launchCommandStates(analisis, appid, extras);
      return {
        comando: texto,
        analisis,
        estados,
        originalImportado:
          opciones.originalImportado !== undefined
            ? opciones.originalImportado
            : (revision.originalImportado ?? null),
        procedencia: opciones.procedencia ?? revision.procedencia,
      };
    },
    [appid, revision.originalImportado, revision.procedencia, extras],
  );

  /** Manda al motor el conjunto de ajustes pedido y adopta lo que confirme. */
  const aplicar = useCallback(
    async (activos: Set<number>) => {
      if (appid === undefined) return;
      const actual = revision.analisis;
      if (!actual) return;

      const peticion: Peticion = secuencia.current.abrir({
        appid,
        original: actual.original,
      });
      setCargando(true);

      // `cambios_para` vive en Rust: aquí solo se dice «quiero estos». El motor decide
      // qué hay que quitar y qué añadir, y devuelve error si no hay forma segura.
      let cambios: CambioAjuste[];
      let resultado: ResultadoSerializacion;
      try {
        // Mismo `appid` y mismos `extras` que el análisis: así Rust reconstruye **el mismo
        // catálogo** y un índice de recomendación no puede apuntar a otro ajuste.
        cambios = await api.launchCommandChanges(actual, appid, [...activos], extras);
        resultado = await api.serializeLaunchCommand(actual, appid, cambios, extras);
      } catch (e) {
        if (secuencia.current.esLaActual(peticion, contexto)) {
          // El fallo no descarta la revisión anterior: se sigue viendo el último comando
          // bueno y se explica qué pasó. Dejar la pantalla en blanco perdería el trabajo.
          setFallo({ mensaje: mensajeDeError(e) });
          setCargando(false);
        }
        return;
      }

      if (!secuencia.current.aceptar(peticion, contexto)) {
        // Respuesta de una petición superada: no se toca nada.
        return;
      }
      setCargando(false);

      if (!resultado.aplicado || resultado.error) {
        // El motor no pudo aplicar. Se conserva lo último confirmado y se explica.
        setFallo({
          mensaje: 'No se pudo aplicar el cambio.',
          detalle: resultado.error ? describirErrorMotor(resultado.error) : undefined,
        });
        // Vuelve a mandar lo confirmado: lo optimista ya no vale.
        pedido.current = null;
        setAjustesPendientes(null);
        return;
      }

      // El comando cambió: hay que reanalizarlo para que los estados y los argumentos
      // conservados correspondan al texto nuevo.
      const siguiente = await analizarYAdoptar(resultado.comando);
      if (!secuencia.current.esLaActual(peticion, contexto)) return;
      if (siguiente) {
        confirmar(siguiente);
        // Lo confirmado ya refleja lo pedido: la base vuelve a salir de los estados.
        pedido.current = null;
        setAjustesPendientes(null);
      }
    },
    [appid, revision.analisis, contexto, analizarYAdoptar, confirmar, extras],
  );

  /**
   * Punto de entrada de todos los cambios.
   *
   * Serializa las peticiones: si ya hay una en vuelo, solo se actualiza el conjunto pedido
   * y se aplicará al volver. Sin esto, pulsar dos toggles rápido mandaba dos cambios
   * calculados sobre el mismo comando y el segundo pisaba al primero.
   */
  /**
   * Pide un conjunto de ajustes y espera a que se aplique.
   *
   * Todas las peticiones pasan por aquí y hay **un solo bucle**: si ya hay una en marcha,
   * solo se guarda el conjunto pedido y lo recogerá ese bucle. Así pulsar tres toggles
   * seguidos no lanza tres viajes al backend ni deja que el último pise a los anteriores.
   */
  const encolar = useCallback(
    (siguiente: Set<number>): Promise<void> => {
      pendientes.current = siguiente;
      if (bucle.current) return bucle.current;

      const correr = async () => {
        try {
          while (pendientes.current) {
            const objetivos = pendientes.current;
            // Se vacía **antes** de aplicar: lo que llegue mientras tanto es un cambio
            // nuevo que hay que aplicar después, no parte de este.
            pendientes.current = null;
            await aplicar(objetivos);
          }
        } finally {
          bucle.current = null;
        }
      };
      bucle.current = correr();
      return bucle.current;
    },
    [aplicar],
  );

  const fijarAjuste = useCallback(
    (indice: number, activo: boolean) => {
      // La base es **lo que ya hay pedido**, no lo confirmado. Pulsar tres toggles en la
      // misma tanda de React llama tres veces seguidas y las tres leerían el mismo
      // `revision.estados` viejo: sin esto, cada una armaba un conjunto con solo su toggle
      // y el último ganaba. Lo encontró el test de «tres toggles rápidos».
      const base = new Set(pedido.current ?? activosDe(revision.estados));
      if (activo) base.add(indice);
      else base.delete(indice);
      pedido.current = base;
      setAjustesPendientes(base);
      void encolar(base);
    },
    [revision.estados, encolar],
  );

  const fijarAjustes = useCallback(
    (activos: Set<number>) => {
      pedido.current = new Set(activos);
      setAjustesPendientes(new Set(activos));
      void encolar(new Set(activos));
    },
    [encolar],
  );

  /**
   * Sustituye el comando entero (importar, restaurar) **dentro de la secuencia**.
   *
   * Abrir petición aquí no es decorativo: si no, una serialización que ya estaba en vuelo
   * al importar volvía después y aplicaba su resultado encima del comando recién
   * importado. El usuario veía su importación desaparecer un instante después de hacerla.
   */
  const sustituir = useCallback(
    async (
      texto: string,
      opciones: { originalImportado?: string | null; procedencia: Procedencia },
    ) => {
      if (appid === undefined) return;
      const peticion = secuencia.current.abrir({ appid, original: texto });
      // Un cambio pedido antes deja de tener sentido: el comando es otro.
      pedido.current = null;
      pendientes.current = null;
      setAjustesPendientes(null);
      const nuevo = await analizarYAdoptar(texto, opciones);
      if (!nuevo) return;
      if (!secuencia.current.esLaActual(peticion, { appid, original: texto })) return;
      confirmar(nuevo);
    },
    [appid, analizarYAdoptar, confirmar],
  );

  const importar = useCallback(
    async (texto: string) => {
      const recortado = texto.trim();
      if (!recortado) return;
      // El original es el texto **importado**, no el comando ya reescrito por el motor. Si
      // se guardara el reescrito, «Restaurar» devolvería algo distinto de lo importado.
      await sustituir(recortado, {
        originalImportado: recortado,
        procedencia: 'importado',
      });
    },
    [sustituir],
  );

  /**
   * Carga un perfil como comando actual.
   *
   * Se distingue de `importar` en un punto que importa: **no** fija `originalImportado`.
   * Cargar un perfil no es importar de Steam, y pisar el original haría que «Restaurar el
   * comando importado» devolviera el del perfil en lugar de lo que trajo Steam.
   *
   * Es atómica desde fuera: si el motor falla, `analizarYAdoptar` devuelve `null` y no se
   * confirma nada, así que comando, toggles y todo lo demás siguen como estaban.
   */
  const cargarPerfil = useCallback(
    async (texto: string): Promise<string | null> => {
      const recortado = texto.trim();
      if (!recortado) return null;
      setFallo(null);
      // `originalImportado` se omite a propósito: `sustituir` conserva el que hubiera.
      // `sustituir` confirma la revisión solo al final, así que si el motor falla aquí no
      // se ha tocado nada y la revisión anterior sigue mandando.
      await sustituir(recortado, { procedencia: 'perfil' });
      // Se lee la revisión adoptada, no el texto de entrada: el motor puede normalizar
      // (por ejemplo, quitar un `%command%` duplicado) y lo que vale es lo que confirmó.
      const adoptado = ultima.current;
      if (adoptado === null || adoptado.procedencia !== 'perfil') return null;
      if (!adoptado.comando.trim()) return null;
      return adoptado.comando;
    },
    [sustituir, revision.comando],
  );

  const restaurarImportado = useCallback(async () => {
    const texto = revision.originalImportado;
    if (texto === null) return;
    await sustituir(texto, { procedencia: 'original_restaurado' });
  }, [revision.originalImportado, sustituir]);

  const limpiarAjustes = useCallback(async () => {
    // Limpiar no es «volver al original»: deja el comando en lo que Thoriapp propone sin
    // ajustes, que incluye los argumentos propios del juego.
    await encolar(new Set());
  }, [encolar]);

  const quitarSegmento = useCallback(
    async (indice: number) => {
      const actual = revision.analisis;
      if (!actual) return;
      setCargando(true);
      setFallo(null);
      try {
        const resultado = await api.launchCommandRemoveSegment(actual, indice);
        if (!resultado.aplicado) {
          setFallo({
            mensaje: 'Ese argumento no se puede quitar con seguridad.',
            detalle: 'El motor lo conserva porque quitarlo cambiaría el comando de otra forma.',
          });
          return;
        }
        await analizarYAdoptar(resultado.comando, { procedencia: 'propuesta' });
      } catch (e) {
        setFallo({
          mensaje: 'No se pudo quitar el argumento.',
          detalle: describirErrorMotor(e as ErrorMotor),
        });
      } finally {
        setCargando(false);
      }
    },
    [revision.analisis, analizarYAdoptar, extras],
  );

  // Carga inicial y al cambiar de juego o de comando de partida. Las reglas tienen que
  // estar cargadas: analizar con el catálogo vacío marcaría todo como desconocido.
  useEffect(() => {
    if (!preparado || appid === undefined || comandoInicial === null) return;
    let vigente = true;
    setCargando(true);
    setFallo(null);
    (async () => {
      try {
        const analisis = await api.analyzeLaunchCommand(comandoInicial, appid, extras);
        const estados = await api.launchCommandStates(analisis, appid, extras);
        if (!vigente) return;
        setRevision({
          comando: comandoInicial,
          analisis,
          estados,
          originalImportado: null,
          procedencia: 'inicial',
        });
      } catch (e) {
        if (vigente) setFallo({ mensaje: mensajeDeError(e) });
      } finally {
        if (vigente) setCargando(false);
      }
    })();
    return () => {
      vigente = false;
    };
    // `params.length` y no `params`: el array se recrea en cada render y volvería a analizar
    // en bucle. Lo único que cambia de verdad es cuántos ajustes hay (otro juego).
    // `extras` entra por contenido (ya memoizado): si cambia la lista de recomendaciones,
    // el catálogo efectivo es otro y hay que reanalizar para que los índices sigan valiendo.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preparado, appid, comandoInicial, params.length, extras]);

  // Al cambiar de juego se borra todo: el estado de un juego no describe a otro.
  useEffect(() => {
    secuencia.current = new SecuenciaComando();
    pendientes.current = null;
    pedido.current = null;
    bucle.current = null;
    setAjustesPendientes(null);
    setRevision(VACIA);
    setFallo(null);
  }, [appid]);

  return {
    revision,
    cargando,
    ajustesPendientes,
    fallo,
    fijarAjuste,
    fijarAjustes,
    importar,
    restaurarImportado,
    cargarPerfil,
    limpiarAjustes,
    quitarSegmento,
    originalImportado: revision.originalImportado,
    comando: revision.comando,
  };
}

/** Revisión vacía. Es un objeto constante para no provocar renders por identidad. */
const VACIA: RevisionComando = {
  comando: '',
  analisis: null,
  estados: [],
  originalImportado: null,
  procedencia: 'inicial',
};

/**
 * Índices de los ajustes que Thoriapp tiene puestos y **controla**.
 *
 * `parcial` no entra aunque haya una aparición en la línea: esa parte de la línea no la
 * controla Thoriapp, así que pedir «que esté activo» no la cambiaría. Si entrara, pulsar
 * el toggle mandaría un conjunto que el motor no puede cumplir del todo.
 */
export function activosDe(estados: EstadoAjuste[]): number[] {
  return estados.filter((e) => estaEncendido(e)).map((e) => e.indice);
}

/** ¿Se puede pulsar este ajuste para cambiarlo? */
export function esAccionable(estado: EstadoAjuste): boolean {
  return esAccionableSegunMotor(estado);
}

/** Traduce el error de una IPC a una frase. Los errores de Tauri son strings sueltos. */
function mensajeDeError(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'El motor de comandos no respondió.';
}

/**
 * Explica un `ErrorAnalisis` de Rust sin jerga de índices.
 *
 * Se selecciona por `tipo`, que es un código estable; nunca por `mensaje`.
 */
function describirErrorMotor(error: ErrorMotor): string {
  switch (error.tipo) {
    case 'texto_no_coincide':
      return 'El comando ha cambiado mientras se editaba. Se conserva el último estado confirmado; vuelve a intentarlo.';
    case 'span_fuera_de_rango':
    case 'span_en_mitad_de_caracter':
    case 'spans_desordenados':
      return 'La posición del ajuste no encaja con el comando. No se ha modificado nada.';
    case 'indice_desconocido':
      return 'Ese ajuste no pertenece a este juego. No se ha modificado nada.';
    default:
      return 'No se pudo aplicar el cambio.';
  }
}
