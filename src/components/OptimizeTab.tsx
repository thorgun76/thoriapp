import { useState } from 'react';
import { AlertTriangle, Check, Cpu, ExternalLink, Info, Search, Terminal, Users, X, Zap } from 'lucide-react';
import { openExternal } from '../lib/links';
import { normalizeForSearch } from '../lib/format';
import { filterParams, impactsPresent } from '../lib/gameView';
import { esSoloProton } from '../lib/preflight';
import { SectionNav } from './SectionNav';
import type { EstadoAjuste } from '../lib/motorComando';
import type { CommunityRecommendation, GameEngine, GameInfo, GameRules, Param } from '../lib/types';
import { TarjetaAjuste } from './TarjetaAjuste';

/**
 * Qué se dice de un ajuste de Proton en un juego nativo.
 *
 * Una sola redacción para todas las tarjetas: si cada bloque dijera una cosa distinta, el
 * usuario tendría que averiguar cuál es la buena.
 */
const MOTIVO_SOLO_PROTON =
  'Solo se aplica con Proton. Este juego corre en Linux nativo, así que aquí no hace nada.';

/**
 * Título legible de una recomendación del motor.
 *
 * El valor técnico va aparte en la tarjeta; esto es el nombre que se lee primero. Se deduce
 * del texto porque las recomendaciones vienen como cadenas suELTAS, sin `title`.
 */
const REC_TITULO_MOTOR = (rec: string): string => {
  if (rec.includes('NTSYNC')) return 'Sincronización de hilos (ntsync)';
  if (rec.includes('LARGE_ADDRESS')) return 'Memoria extendida (>2 GB)';
  if (rec.includes('SHADER_CACHE')) return 'Caché de shaders por juego';
  return 'Recomendación del motor';
};

/**
 * `Param` sintético para una recomendación, que llega como texto suelto.
 *
 * Existe para que las tres listas de recomendaciones (motor, hardware y la sección de
 * Proton) construyan la tarjeta igual. Antes cada bloque armaba su propio objeto y se
 * olvidaban campos distintos: uno ponía `proton_only`, el otro no, y por eso `WINE_*` se
 * colaba como aplicable en un juego nativo.
 */
function paramSintetico(
  valor: string,
  titulo: string,
  descripcion: string,
  evidencia: 'engine' | 'community',
  protonOnly: boolean,
): Param {
  return {
    kind: esSoloProton(valor) ? 'env' : 'arg',
    value: valor,
    title: titulo,
    why: descripcion,
    impact: 'medio',
    requires: [],
    default_on: false,
    evidence: evidencia,
    essential: false,
    proton_only: protonOnly,
  };
}

interface OptimizeTabProps {
  rules: GameRules;
  info: GameInfo;
  engine: GameEngine | null;
  engineRecommendations: string[];
  communityRecommendations: CommunityRecommendation[];
  enabled: boolean[];
  /**
   * Estado de cada ajuste según el motor (`EstadoAjuste`), alineado con `rules.params`.
   *
   * Se pasa para poder pintar los cinco casos (activo, parcial, ambiguo, conflicto) en vez
   * de un booleano. React no deduce ninguno: los recibe ya decididos desde Rust.
   */
  estadosMotor?: EstadoAjuste[];
  /**
   * Índices de ajuste pedidos y aún sin confirmar. Solo esos pintan el punto de progreso.
   */
  enVuelo?: Set<number> | null;
  /** @deprecated Ya no se lee: el estado lo dice el motor (`recomendacionesPuestas`). */
  selectedCommunityRecs?: Set<number>;
  /** @deprecated Ya no se lee: el estado lo dice el motor (`recomendacionesPuestas`). */
  selectedEngineRecs?: Set<number>;
  /**
   * ¿Está puesta esta recomendación en el comando?
   *
   * Lo dice el motor, que es quien sabe si el argumento está en el comando confirmado. Los
   * `Set` de arriba solo recuerdan lo que se marcó para el perfil y no sirven para pintar
   * el estado real: una recomendación importada de Steam está en el comando sin estar en
   * ningún `Set`.
   */
  recomendacionesPuestas?: (texto: string) => boolean;
  forceProton: boolean;
  showUnavailable: boolean;
  onToggleRule: (index: number) => void;
  onToggleCommunityRec: (index: number) => void;
  onToggleEngineRec: (index: number) => void;
  onToggleProton: () => void;
  missing: (p: Param) => string | null;
  getRecommendationStatus: (p: Param) => string;
  impactColor: (impact: string) => string;
  evidenceLabel: (evidence: string) => string;
}

/** Pestaña "Optimización": parámetros, motor, hardware y recomendaciones por juego. */
export function OptimizeTab({
  rules,
  info,
  engine,
  engineRecommendations,
  communityRecommendations,
  enabled,
  estadosMotor = [],
  enVuelo = null,
  // Sin función se asume «no puesta»: es lo correcto cuando nadie pasa el estado del
  // motor, y así el componente no inventa que algo está activo.
  recomendacionesPuestas = () => false,
  forceProton,
  showUnavailable,
  onToggleRule,
  onToggleCommunityRec,
  onToggleEngineRec,
  onToggleProton,
  missing,
  getRecommendationStatus,
  impactColor,
  evidenceLabel,
}: OptimizeTabProps) {
  // Filtro local de la pestaña: la lista puede llegar a decenas de
  // ajustes y buscar «FSR» a mano es tedioso.
  const [query, setQuery] = useState('');
  const [impact, setImpact] = useState<string | null>(null);

  const filtering = query.trim().length > 0 || impact !== null;

  /** Impacto de un valor, si ese valor es un parámetro del juego. */
  const impactOf = (value: string): string | null =>
    rules.params.find((p) => p.value === value)?.impact ?? null;

  /**
   * Ajustes que la pestaña puede llegar a enseñar con el modo actual.
   *
   * Es el MISMO criterio que aplican las listas (`showUnavailable`, `essential`
   * en modo guiado, `evitar` fuera, `config` incluido). Sin esto, la etiqueta
   * «bajo» existía siempre (sus 7 parámetros son `essential: false`) pero al
   * pulsarla no quedaba nada: un chip que promete algo que no puede dar.
   */
  const reachableParams = rules.params
    .filter((p) => showUnavailable || !missing(p))
    // `evitar` se **ve** en la lista (con su aviso de riesgo), pero no se ofrece como
    // filtro: un chip «evitar» invitaría a buscar justo los ajustes que empeoran el
    // rendimiento. Por eso la etiqueta no existe aunque el ajuste sí se pinte.
    .filter((p) => p.impact !== 'evitar');

  // Los niveles salen de lo que de verdad se puede ver, más los deducibles de las
  // recomendaciones de motor/hardware (si no, esas filas quedarían inalcanzables).
  const impactOptions = impactsPresent(reachableParams, [
    ...engineRecommendations.map(impactOf),
    ...communityRecommendations.map((r) => impactOf(r.parameter)),
  ]);

  const matches = (p: Param) => filterParams([p], query, impact, normalizeForSearch).length > 0;

  /** Parámetros que pasan el filtro de la barra (sin los filtros propios de cada lista). */
  const visibleParams = filterParams(rules.params, query, impact, normalizeForSearch);

  /**
   * Texto que se busca contra filas que no son parámetros (proton, diagnóstico,
   * motor, hardware). El filtro de impacto no aplica: esas filas no tienen uno.
   */
  const textQuery = normalizeForSearch(query.trim());
  const textMatches = (...fields: (string | null | undefined)[]) =>
    textQuery.length === 0 ||
    fields.some((field) => field && normalizeForSearch(field).includes(textQuery));

  /**
   * Qué se ve con el filtro puesto.
   *
   * Con una búsqueda activa desaparece todo lo que no coincide, no solo las
   * listas largas: si solo se filtraran los parámetros, el usuario escribía y la
   * pantalla seguía igual (había que bajar a mirar si algo había cambiado).
   */
  /**
   * Filas que no son parámetros de `rules.params` y que, por tanto, no tienen
   * `impact` propio: Probar con Proton y las recomendaciones de motor/hardware.
   * Con una etiqueta de impacto puesta no pueden coincidir, así que se ocultan
   * (antes solo miraban el texto y se quedaban siempre).
   *
   * MangoHud NO está en este grupo: es un parámetro más de `rules.params` con su
   * propio impacto (`medicion` en games.json), así que se compara como los demás.
   */
  const impactFilterOn = impact !== null;
  const nonParamRowsVisible = !impactFilterOn;

  const showProton =
    info.native === true && nonParamRowsVisible && textMatches('probar con proton', 'proton');

  const impactAllows = (value: string) => (impactFilterOn ? impactOf(value) === impact : true);

  // MangoHud se busca por texto («mangohud», «fps»…) y por su impacto real. Si el
  // juego no lo trae entre sus parámetros, `impactOf` es `null` y con una etiqueta
  // puesta queda oculto: no se puede afirmar que coincida.
  const mangoHudParam = rules.params.find((p) => p.value === 'mangohud');
  const showMangoHud =
    !!mangoHudParam &&
    !missing(mangoHudParam) &&
    textMatches('mangohud', 'diagnóstico', 'herramientas de diagnóstico', 'fps') &&
    impactAllows('mangohud');

  const showEngineRecs = engineRecommendations
    .map((rec, i) => ({ rec, i }))
    .filter(({ rec }) => textMatches(rec, 'motor', engine?.name))
    .filter(({ rec }) => impactAllows(rec));
  const showCommunityRecs = communityRecommendations
    .map((rec, i) => ({ rec, i }))
    // Mismo criterio que la lista: fuera mangohud/gamemoderun (ya salen arriba) y,
    // en modo guiado, solo las marcadas como recomendadas.
    .filter(({ rec }) => rec.parameter !== 'mangohud' && rec.parameter !== 'gamemoderun')
    .filter(({ rec }) => textMatches(rec.parameter, rec.description, 'hardware'))
    .filter(({ rec }) => impactAllows(rec.parameter));
  const showSystemAdjustments = rules.params.filter(
    (p) => (p.value === 'game-performance' || p.value === 'gamemoderun') && matches(p)
  );
  const showGameParams = visibleParams
    .filter((p) => p.value !== 'mangohud' && p.value !== 'gamemoderun' && p.value !== 'game-performance')
    .filter((p) => !p.requires?.includes('game-performance'));
  // Ya no se descarta `impact: 'evitar'`: `RADV_DEBUG=nofastclears` es una herramienta de
  // diagnóstico real y tiene que poder elegirse. Lo que no se hace es venderla como
  // optimización: su tarjeta lleva la etiqueta «Riesgo», su aviso y el texto de por qué.

  /**
   * ¿Se ofrece Proton en este juego?
   *
   * Solo en juegos con build nativo: en uno que ya corre bajo Proton no hay nada que
   * «probar», los ajustes de Proton aplican directamente. Es la misma condición que la
   * tarjeta de «Probar con Proton».
   */
  const juegoNativo = info.native === true;

  /**
   * ¿Los ajustes de Proton están disponibles **ahora**?
   *
   * Tres casos, y solo el último los deja fuera:
   *
   * - El juego ya usa Proton → disponibles sin hacer nada.
   * - Juego nativo con «Probar con Proton» encendido → disponibles: el usuario ha pedido
   *   explícitamente preparar una configuración de Proton.
   * - Juego nativo sin «Probar con Proton» → **no** disponibles. Enseñarlos como tarjetas
   *   pulsables era el fallo de raíz: parecían aplicables y el motor los rechazaba.
   *
   * La aplicabilidad la dice el motor (`proton_only`), no una lista de nombres escrita aquí.
   * Para las recomendaciones, que llegan como texto, se usa el mismo predicado exportado
   * desde `preflight` (que replica los prefijos de Rust).
   */
  const protonDisponible = !juegoNativo || forceProton;

  /** Un parámetro es de Proton si lo dice el motor. */
  const paramEsProton = (p: Param) => p.proton_only === true;

  // Si el ajuste está **puesto** nunca se esconde, aunque no aplique: quitarlo de la vista
  // lo dejaría en el comando sin forma de verlo ni de retirarlo.
  const puestoSegunMotor = (valor: string) => {
    const estado = estadosMotor.find((e) => e.valor === valor);
    return estado?.estado === 'activo' || estado?.estado === 'parcial';
  };

  // --- Parámetros del juego -------------------------------------------------------
  // El reparto es excluyente: un ajuste está en «Proton» o en «normales», nunca en los dos.
  const paramsJuegoProton = showGameParams.filter(paramEsProton);
  const paramsJuegoNormales = showGameParams.filter((p) => !paramEsProton(p));

  // --- Ajustes del sistema --------------------------------------------------------
  const sistemaProton = showSystemAdjustments.filter(paramEsProton);
  const sistemaNormales = showSystemAdjustments.filter((p) => !paramEsProton(p));

  // --- Recomendaciones ------------------------------------------------------------
  // La aplicabilidad la dice el motor. Las recomendaciones de hardware llegan con
  // `proton_only` desde Rust; las de motor son texto suelto y ese dato no viaja, así que
  // para ellas se usa el predicado de `preflight` (el único sitio donde TS tiene una copia,
  // y hay un test en Rust que comprueba que ambas listas coinciden).
  const recMotorProton = showEngineRecs.filter(({ rec }) => esSoloProton(rec));
  const recMotorNormales = showEngineRecs.filter(({ rec }) => !esSoloProton(rec));

  const recHwProton = showCommunityRecs.filter(({ rec }) => rec.proton_only === true);
  const recHwNormales = showCommunityRecs.filter(({ rec }) => rec.proton_only !== true);

  /**
   * Todo lo de Proton en una sola lista, sin duplicados.
   *
   * Se deduplica por texto porque el mismo valor puede llegar a la vez como parámetro del
   * juego y como recomendación (el perfil genérico ya trae `PROTON_LOCAL_SHADER_CACHE=1` y
   * la recomendación de hardware también la ofrece). Dos tarjetas para el mismo ajuste es
   * justo lo que no debe pasar.
   */
  type FilaProton =
    | { tipo: 'param'; p: Param }
    | { tipo: 'motor'; rec: string; i: number }
    | { tipo: 'hardware'; rec: CommunityRecommendation; i: number };

  const filasProton: FilaProton[] = [];
  const valoresProton = new Set<string>();
  const anadirProton = (valor: string, fila: FilaProton) => {
    const clave = valor.trim();
    if (!clave || valoresProton.has(clave)) return;
    valoresProton.add(clave);
    filasProton.push(fila);
  };
  // **Una sola** condición de visibilidad para las cuatro fuentes. Tenerla repetida en cada
  // `forEach` fue justo lo que dejó `paramsJuegoProton` sin comprobar y la tarjeta seguía
  // saliendo en un juego nativo sin modo Proton.
  const seOfrece = (valor: string) => protonDisponible || puestoSegunMotor(valor);

  paramsJuegoProton.forEach((p) => {
    if (seOfrece(p.value)) anadirProton(p.value, { tipo: 'param', p });
  });
  sistemaProton.forEach((p) => {
    if (seOfrece(p.value)) anadirProton(p.value, { tipo: 'param', p });
  });
  recMotorProton.forEach(({ rec, i }) => {
    if (seOfrece(rec)) anadirProton(rec, { tipo: 'motor', rec, i });
  });
  recHwProton.forEach(({ rec, i }) => {
    if (seOfrece(rec.parameter)) anadirProton(rec.parameter, { tipo: 'hardware', rec, i });
  });

  /**
   * El filtro de la barra también manda sobre la sección de Proton: si el usuario busca
   * «mangohud», no tiene por qué seguir viendo `PROTON_*`.
   */
  const filasProtonVisibles = filasProton.filter((f) => {
    if (f.tipo === 'param') return matches(f.p) && (showUnavailable || !missing(f.p)) && impactAllows(f.p.value);
    if (f.tipo === 'motor') return textMatches(f.rec) && impactAllows(f.rec);
    return textMatches(f.rec.parameter, f.rec.description) && impactAllows(f.rec.parameter);
  });

  /**
   * Aviso compacto: quedan argumentos de Proton en el comando y el modo Proton está
   * apagado. No se quita nada solo; se dice y se ofrece retirarlos.
   */
  const protonEnComandoSinModo = !protonDisponible && filasProton.some((f) => {
    const valor = f.tipo === 'param' ? f.p.value : f.tipo === 'motor' ? f.rec : f.rec.parameter;
    return puestoSegunMotor(valor);
  });

  /** Nada que enseñar con el filtro puesto: se dice en vez de dejar la pestaña vacía. */
  const nothingMatches =
    filtering &&
    sistemaNormales.length === 0 &&
    paramsJuegoNormales.length === 0 &&
    recMotorNormales.length === 0 &&
    recHwNormales.length === 0 &&
    filasProtonVisibles.length === 0 &&
    !showMangoHud &&
    !showProton;

  const sections = [
    ...(juegoNativo ? [{ id: 'sec-proton', label: 'Proton' }] : []),
    ...(filasProtonVisibles.length > 0
      ? [{ id: 'sec-proton-ajustes', label: 'Proton' }]
      : []),
    { id: 'sec-diagnostico', label: 'Diagnóstico' },
    { id: 'sec-ajustes', label: 'Ajustes' },
  ];

  return (
    <div className="space-y-4">
      {/* Con un filtro activo las secciones se ocultan, así que el índice sobra. */}
      {!filtering && <SectionNav sections={sections} />}

      {/* FILTRO DE AJUSTES */}
      <div className="p-3 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-dim)]" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar un ajuste (FSR, mangohud, sincronización…)"
            aria-label="Buscar ajuste"
            className="w-full pl-9 pr-9 py-2 rounded-lg bg-[var(--bg)] border border-[var(--border)] focus:border-action/50 focus:outline-none text-sm"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              aria-label="Limpiar la búsqueda"
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded text-[var(--text-dim)] hover:text-[var(--text)]"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Filtro por impacto: solo los niveles que existen en este juego. */}
        <div className="flex flex-wrap items-center gap-1.5 mt-2">
          <span className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mr-1">
            Impacto
          </span>
          {impactOptions.map((level) => (
            <button
              key={level}
              onClick={() => setImpact(impact === level ? null : level)}
              aria-pressed={impact === level}
              className={`text-[10px] px-2 py-0.5 rounded border font-semibold uppercase transition-colors ${
                impact === level
                  ? impactColor(level)
                  : 'bg-[var(--bg)] text-[var(--text-dim)] border-[var(--border)] hover:text-[var(--text)]'
              }`}
            >
              {level}
            </button>
          ))}
          {filtering && (
            <button
              onClick={() => { setQuery(''); setImpact(null); }}
              className="ml-auto text-[10px] text-[var(--action)] hover:underline font-semibold"
            >
              Quitar filtros
            </button>
          )}
        </div>
        {/* Con el filtro puesto solo queda lo que coincide; si no queda nada, se
            dice aquí en vez de enseñar una pestaña vacía. */}
        {nothingMatches && (
          <p className="text-xs text-[var(--text-muted)] mt-2">
            Ningún ajuste coincide con «{query.trim()}». Quita los filtros para ver todos.
          </p>
        )}
      </div>

                        {/* Fase 3: Toggle Proton estilo Discord */}
                        {showProton && (
                          <article id="sec-proton" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                            <div className="flex items-center justify-between">
                              <div className="flex-1">
                                <h3 className="text-base font-bold text-[var(--text)]">Probar con Proton</h3>
                                <p className="text-sm text-[var(--text-muted)] mt-1">
                                  Este juego tiene versión nativa de Linux. Actívalo si quieres comparar con
                                  Proton: aparecerán sus ajustes de compatibilidad.
                                </p>
                              </div>
                              <button
                                onClick={() => onToggleProton()}
                                aria-label="Preparar ajustes para Proton"
                                className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors duration-200 ease-in-out focus:outline-none ${
                                  forceProton ? 'bg-[var(--action)]' : 'bg-[var(--border)]'
                                }`}
                                role="switch"
                                aria-checked={forceProton}
                              >
                                <span
                                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 ease-in-out ${
                                    forceProton ? 'translate-x-6' : 'translate-x-1'
                                  }`}
                                />
                              </button>
                            </div>
                            {/* Advertencia discreta: Thoriapp prepara el comando, pero no
                                toca la configuración de compatibilidad de Steam. Decirlo
                                aquí evita que alguien active Proton, pegue el comando y no
                                entienda por qué el juego sigue arrancando en nativo. */}
                            {forceProton && (
                              <p className="mt-3 flex items-start gap-1.5 text-[11px] text-[var(--caution)] leading-snug">
                                <Info className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
                                Thoriapp no cambia nada en Steam. Para que estos ajustes surtan efecto,
                                selecciona Proton para este juego en Steam (Propiedades → Compatibilidad).
                              </p>
                            )}
                          </article>
                        )}

                        {/* Ajustes específicos de Proton */}
                        {filasProtonVisibles.length > 0 && (
                        <article id="sec-proton-ajustes" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <div className="mb-3">
                            <h3 className="text-base font-bold text-[var(--text)]">Ajustes específicos de Proton</h3>
                            <p className="text-sm text-[var(--text-muted)] mt-1">
                              {protonDisponible
                                ? 'Variables que Proton y Wine leen al arrancar el juego.'
                                : 'Estos ajustes siguen en el comando. No se quitan solos: retíralos tú si no los quieres.'}
                            </p>
                          </div>

                          {/* Aviso compacto: hay Proton en el comando con el modo apagado. */}
                          {protonEnComandoSinModo && (
                            <p className="mb-3 flex items-start gap-1.5 text-[11px] text-[var(--caution)] leading-snug">
                              <Info className="w-3 h-3 shrink-0 mt-0.5" aria-hidden="true" />
                              Hay argumentos de Proton en el comando y el juego corre en Linux nativo:
                              probablemente no surtan efecto. Desactívalos aquí si quieres quitarlos.
                            </p>
                          )}

                          <div className="grid grid-cols-1 gap-2">
                            {filasProtonVisibles.map((fila) => {
                              if (fila.tipo === 'param') {
                                const p = fila.p;
                                const idx = rules.params.indexOf(p);
                                const estado = estadosMotor.find((e) => e.valor === p.value) ?? null;
                                const puesta = estado?.estado === 'activo' || estado?.estado === 'parcial';
                                const bloqueado = !protonDisponible && !puesta;
                                return (
                                  <TarjetaAjuste
                                    key={`param:${p.value}`}
                                    param={p}
                                    indice={idx}
                                    estado={estado}
                                    encendido={puesta}
                                    // Nunca un toggle inerte: si no aplica, o se puede usar o
                                    // se explica el motivo. Aquí solo llegan los que aplican
                                    // o los que ya están puestos.
                                    accionable={!bloqueado && estado?.estado !== 'ambiguo' && (showUnavailable || !missing(p))}
                                    pendiente={enVuelo?.has(idx) ?? false}
                                    requisito={showUnavailable ? null : missing(p)}
                                    motivoNoAccionable={
                                      bloqueado
                                        ? 'Solo se aplica con Proton. Activa «Probar con Proton» para usarlo.'
                                        : !protonDisponible && puesta
                                          ? 'Está en el comando, pero este juego corre en Linux nativo: probablemente no surta efecto.'
                                          : undefined
                                    }
                                    onCambiar={() => onToggleRule(idx)}
                                  />
                                );
                              }
                              if (fila.tipo === 'motor') {
                                const { rec, i } = fila;
                                const estado = estadosMotor.find((e) => e.valor === rec) ?? null;
                                const puesta = recomendacionesPuestas(rec);
                                return (
                                  <TarjetaAjuste
                                    key={`motor:${rec}`}
                                    param={paramSintetico(rec, REC_TITULO_MOTOR(rec), 'Parámetro específico del motor', 'engine', true)}
                                    indice={i}
                                    estado={estado}
                                    encendido={puesta}
                                    accionable={protonDisponible && estado?.estado !== 'ambiguo'}
                                    pendiente={enVuelo?.has(i) ?? false}
                                    motivoNoAccionable={
                                      protonDisponible
                                        ? undefined
                                        : 'Está en el comando, pero este juego corre en Linux nativo: probablemente no surta efecto.'
                                    }
                                    onCambiar={() => onToggleEngineRec(i)}
                                  />
                                );
                              }
                              const { rec, i } = fila;
                              const estado = estadosMotor.find((e) => e.valor === rec.parameter) ?? null;
                              const puesta = recomendacionesPuestas(rec.parameter);
                              return (
                                <TarjetaAjuste
                                  key={`hardware:${rec.parameter}`}
                                  param={paramSintetico(rec.parameter, rec.parameter, rec.description, 'community', true)}
                                  indice={i}
                                  estado={estado}
                                  encendido={puesta}
                                  accionable={protonDisponible && estado?.estado !== 'ambiguo'}
                                  pendiente={enVuelo?.has(i) ?? false}
                                  motivoNoAccionable={
                                    protonDisponible
                                      ? undefined
                                      : 'Está en el comando, pero este juego corre en Linux nativo: probablemente no surta efecto.'
                                  }
                                  onCambiar={() => onToggleCommunityRec(i)}
                                />
                              );
                            })}
                          </div>
                        </article>
                        )}

                        {/* Herramientas de diagnóstico */}
                        {showMangoHud && (
                        <article id="sec-diagnostico" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <div className="flex items-start justify-between mb-4">
                            <div>
                              <h3 className="text-base font-bold text-[var(--text)]">Herramientas de diagnóstico</h3>
                              <p className="text-sm text-[var(--text-muted)] mt-1">
                                Estas herramientas miden el rendimiento, no lo optimizan. Úsalas para verificar si los ajustes funcionan.
                              </p>
                            </div>
                          </div>

                          <div className="grid grid-cols-1 gap-2">
                            {/* MangoHud: misma tarjeta que el resto, para que el estado y
                                el toggle salgan del motor y no de `enabled[idx]`. */}
                            {mangoHudParam && (() => {
                              const idx = rules.params.indexOf(mangoHudParam);
                              const estado = estadosMotor.find((e) => e.valor === mangoHudParam.value) ?? null;
                              return (
                                <TarjetaAjuste
                                  param={mangoHudParam}
                                  indice={idx}
                                  estado={estado}
                                  encendido={estado?.estado === 'activo' || estado?.estado === 'parcial'}
                                  accionable={estado?.estado !== 'ambiguo'}
                                  pendiente={enVuelo?.has(idx) ?? false}
                                  requisito={missing(mangoHudParam)}
                                  onCambiar={() => onToggleRule(idx)}
                                />
                              );
                            })()}
                          </div>
                        </article>
                        )}

                        {/* Ajustes para probar */}
                        <article id="sec-ajustes" className="scroll-mt-4 p-5 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
                          <div className="flex items-start justify-between mb-4">
                            <div>
                              <h3 className="text-base font-bold text-[var(--text)]">Ajustes para probar</h3>
                              <p className="text-sm text-[var(--text-muted)] mt-1">
                                Parámetros de optimización reales. Actívalos individualmente y mide el resultado.
                              </p>
                            </div>
                          </div>

                          {/* Ajustes del sistema */}
                          {(() => {
                            // Los de Proton viven en su propia sección: aquí solo los
                            // ajustes de sistema que no dependen de Proton.
                            const systemAdjustments = sistemaNormales
                              .filter(p => showUnavailable || !missing(p));

                            if (systemAdjustments.length === 0) return null;

                            return (
                              <div className="mb-6">
                                <h4 className="text-sm font-semibold text-[var(--caution)] mb-3 flex items-center gap-2">
                                  <Cpu className="w-4 h-4" />
                                  Ajustes del sistema
                                </h4>
                                <div className={`grid gap-2 ${systemAdjustments.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                  {systemAdjustments.map((p) => {
                                    const idx = rules.params.indexOf(p);
                                    const isMissing = missing(p);
                                    const estado = estadosMotor.find((e) => e.valor === p.value) ?? null;
                                    const puesta = estado?.estado === 'activo' || estado?.estado === 'parcial';
                                    const soloProton = p.proton_only && !forceProton;
                                    return (
                                      <TarjetaAjuste
                                        key={p.value}
                                        param={p}
                                        indice={idx}
                                        estado={estado}
                                        encendido={puesta}
                                        accionable={!isMissing && !(soloProton && !puesta) && estado?.estado !== 'ambiguo'}
                                        pendiente={enVuelo?.has(idx) ?? false}
                                        requisito={isMissing}
                                        motivoNoAccionable={
                                          soloProton && !puesta
                                            ? 'Solo se aplica con Proton. Activa «Probar con Proton» para usarlo.'
                                            : soloProton && puesta
                                              ? 'Está en el comando, pero este juego corre en Linux nativo: probablemente no surta efecto.'
                                              : undefined
                                        }
                                        onCambiar={() => onToggleRule(idx)}
                                      />
                                    );
                                  })}
                                </div>
                              </div>
                            );
                          })()}

                          {/* Engine Recommendations */}
                          {engineRecommendations.length > 0 && (
                            <div className="mb-6">
                              <h4 className="text-sm font-semibold text-[var(--action)] mb-3 flex items-center gap-2">
                                <Terminal className="w-4 h-4" />
                                Parámetros del motor ({engine?.name})
                              </h4>
                              <div className={`grid gap-2 ${engineRecommendations.length === 1 ? 'grid-cols-1' : 'grid-cols-1 md:grid-cols-2'}`}>
                                {recMotorNormales.map(({ rec, i }) => {
                                  const soloProton = esSoloProton(rec) && !forceProton;
                                  const puesta = recomendacionesPuestas(rec);
                                  const estado = estadosMotor.find((e) => e.valor === rec) ?? null;
                                  return (
                                    <TarjetaAjuste
                                      key={rec}
                                      param={{
                                        kind: esSoloProton(rec) ? 'env' : 'arg',
                                        value: rec,
                                        title: REC_TITULO_MOTOR(rec),
                                        why: rec.includes('NTSYNC')
                                          ? 'Sincronización mejorada de hilos'
                                          : rec.includes('LARGE_ADDRESS')
                                            ? 'Permite usar más de 2GB de RAM'
                                            : 'Parámetro específico del motor',
                                        impact: 'medio',
                                        requires: [],
                                        default_on: false,
                                        evidence: 'engine',
                                        essential: false,
                                        proton_only: esSoloProton(rec),
                                      }}
                                      indice={i}
                                      estado={estado}
                                      encendido={puesta}
                                      accionable={!(soloProton && !puesta) && estado?.estado !== 'ambiguo'}
                                      pendiente={enVuelo?.has(i) ?? false}
                                      motivoNoAccionable={soloProton ? MOTIVO_SOLO_PROTON : undefined}
                                      onCambiar={() => onToggleEngineRec(i)}
                                    />
                                  );
                                })}
                              </div>
                            </div>
                          )}

                          {/* Hardware Recommendations */}
                          {(() => {
                            if (recHwNormales.length === 0) return null;

                            return (
                              <div className="mb-6">
                                <h4 className="text-sm font-semibold text-[var(--compatible)] mb-3 flex items-center gap-2">
                                  <Users className="w-4 h-4" />
                                  Recomendaciones por hardware
                                </h4>
                                {/* Una sola columna: con dos, los argumentos técnicos largos
                                    se partían en varias líneas y la comparación entre
                                    tarjetas dejaba de ser de un vistazo. */}
                                <div className="grid grid-cols-1 gap-2">
                                  {recHwNormales.map(({ rec, i }) => {
                                    // Aquí ya no llegan las de Proton: están en su sección.
                                    const puesta = recomendacionesPuestas(rec.parameter);
                                    const estado = estadosMotor.find((e) => e.valor === rec.parameter) ?? null;
                                    return (
                                      <TarjetaAjuste
                                        key={rec.parameter}
                                        param={paramSintetico(
                                          rec.parameter,
                                          rec.parameter,
                                          rec.description,
                                          'community',
                                          rec.proton_only === true,
                                        )}
                                        indice={i}
                                        estado={estado}
                                        encendido={puesta}
                                        accionable={estado?.estado !== 'ambiguo'}
                                        pendiente={enVuelo?.has(i) ?? false}
                                        onCambiar={() => onToggleCommunityRec(i)}
                                      />
                                    );
                                  })}
                                </div>
                              </div>
                            );
                          })()}

                          {/* Game-specific Recommendations */}
                          {rules.has_rules && (
                            <div>
                              <div className="flex items-center justify-between mb-3">
                                <h4 className="text-sm font-semibold text-[var(--caution)] flex items-center gap-2">
                                  <Zap className="w-4 h-4" />
                                  Parámetros específicos del juego
                                </h4>
                              </div>
                              <div className="space-y-2">
                                {/* Con filtro activo y ningún resultado se dice, en vez
                                    de dejar un hueco que parece un fallo. */}
                                {paramsJuegoNormales
                                  .filter(p => showUnavailable || !missing(p))
                                  .map((p) => {
                                    const idx = rules.params.indexOf(p);
                                    const isMissing = missing(p);
                                    // Por **valor**, no por índice: `estadosMotor` usa
                                    // identidad de texto desde el arreglo del P0, y buscar
                                    // por índice devolvía `null` cuando el catálogo efectivo
                                    // y `rules.params` no coincidían.
                                    const estado = estadosMotor.find((e) => e.valor === p.value) ?? null;
                                    // Un `config` no va al comando: no tiene estado en el motor.
                                    // Los demás sí, y se pintan con la tarjeta del motor para que
                                    // «parcial», «ambiguo» y «conflicto» se vean tal cual.
                                    if (!p.kind || p.kind !== 'config') {
                                      // Un ajuste de Proton no hace nada en un juego nativo:
                                      // `PROTON_USE_NTSYNC=1` solo lo lee el Proton que lanza
                                      // el .exe. Se deja de ofrecer como si fuera aplicable.
                                      //
                                      // No se esconde si ya está en el comando: si el usuario
                                      // lo importó de Steam hay que enseñarlo, explicar que
                                      // puede no surtir efecto y dejar que decida. Quitarlo por
                                      // nuestra cuenta cambiaría su comando sin avisar.
                                      const soloProton = p.proton_only && !forceProton;
                                      const puesta =
                                        estado?.estado === 'activo' || estado?.estado === 'parcial';
                                      const necesitaProton = soloProton && !puesta;
                                      return (
                                        <TarjetaAjuste
                                          // Clave por valor, no por posición: la lista se
                                          // filtra (`showUnavailable`, búsqueda), así que un
                                          // índice reasigna la identidad de la tarjeta y React
                                          // la remonta entera al cambiar de estado. Ese era el
                                          // parpadeo que se veía al pulsar un toggle.
                                          key={p.value}
                                          param={p}
                                          indice={idx}
                                          estado={estado}
                                          encendido={puesta}
                                          accionable={
                                            !isMissing &&
                                            !necesitaProton &&
                                            estado?.estado !== 'ambiguo'
                                          }
                                          pendiente={enVuelo?.has(idx) ?? false}
                                          requisito={isMissing}
                                          motivoNoAccionable={
                                            necesitaProton
                                              ? 'Solo se aplica con Proton. Activa «Probar con Proton» para usarlo.'
                                              : soloProton && puesta
                                                ? 'Está en el comando, pero este juego corre en Linux nativo: probablemente no surta efecto.'
                                                : undefined
                                          }
                                          onCambiar={() => onToggleRule(idx)}
                                        />
                                      );
                                    }
                                    // A partir de aquí, solo `config`: ajuste DENTRO del juego.
                                    // No va al comando, así que se queda el bloque de abajo con su
                                    // etiqueta «En el juego» y sin interruptor.
                                    // Un `config` es un ajuste DENTRO del juego ("usa FSR en
                                    // Calidad"), no un parámetro del lanzador: si se pudiera
                                    // activar, acabaría pegado al comando y lo rompería. Se
                                    // muestra como consejo, sin interruptor.
                                    const esAjusteInterno = p.kind === 'config';
                                    return (
                                      <div
                                        key={p.value}
                                        role={esAjusteInterno || isMissing ? undefined : 'checkbox'}
                                        aria-checked={esAjusteInterno || isMissing ? undefined : !!enabled[idx]}
                                        aria-disabled={esAjusteInterno || isMissing ? undefined : false}
                                        tabIndex={esAjusteInterno || isMissing ? undefined : 0}
                                        onClick={() => {
                                          if (!isMissing && !esAjusteInterno) onToggleRule(idx);
                                        }}
                                        onKeyDown={(e) => {
                                          if (isMissing || esAjusteInterno) return;
                                          if (e.key === ' ' || e.key === 'Enter') {
                                            e.preventDefault();
                                            onToggleRule(idx);
                                          }
                                        }}
                                        className={`w-full text-left p-4 rounded-lg border transition-all ${
                                          esAjusteInterno
                                            ? 'bg-[var(--bg)] border-dashed border-[var(--border)] cursor-default'
                                            : isMissing
                                              ? 'opacity-50 bg-[var(--bg)] border-[var(--border)] cursor-not-allowed'
                                              : enabled[idx]
                                                ? 'bg-[var(--caution-bg)] border-caution/50'
                                                : 'bg-[var(--bg)] border-[var(--border)] hover:border-caution/40'
                                        }`}
                                      >
                                        <div className="flex items-start justify-between gap-3">
                                          <div className="flex-1">
                                            <div className="flex items-center gap-2 mb-2 flex-wrap">
                                              <h5 className="text-sm font-semibold text-[var(--text)]">{p.title}</h5>
                                              <span className={`text-[10px] px-1.5 py-0.5 rounded border ${impactColor(p.impact)}`}>
                                                {p.impact.toUpperCase()}
                                              </span>
                                              {/* Este bloque solo llega para ajustes internos del
                                                  juego («usa FSR en Calidad»), que no van al
                                                  comando: se marcan como tal para que no se
                                                  confundan con una variable del lanzador. */}
                                              <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                                                🎮 En el juego
                                              </span>

                                              {/* Fase 3: Badges de recomendación */}
                                              {(() => {
                                                const status = getRecommendationStatus(p);
                                                if (status === 'recommended') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--compatible-bg)] text-[var(--compatible)] border border-compatible/30 font-semibold">
                                                      ⭐ Recomendado
                                                    </span>
                                                  );
                                                }
                                                if (status === 'not_applicable') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--error-bg)] text-[var(--error)] border border-error/30">
                                                      ℹ️ No aplica
                                                    </span>
                                                  );
                                                }
                                                if (status === 'not_recommended') {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-dim)] border border-[var(--border)]">
                                                      🔒 Proton
                                                    </span>
                                                  );
                                                }
                                                if (p.experimental) {
                                                  return (
                                                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--caution-bg)] text-[var(--caution)] border border-caution/30">
                                                      ⚠️ Experimental
                                                    </span>
                                                  );
                                                }
                                                return null;
                                              })()}

                                              {/* `missing()` deshabilitaba la tarjeta sin decir por qué: el
                                                  usuario veía un gris inerte. El motivo ya estaba calculado. */}
                                              {isMissing && (
                                                <span className="text-[10px] px-1.5 py-0.5 rounded bg-[var(--card)] text-[var(--text-muted)] border border-[var(--border)]">
                                                  🚫 {isMissing}
                                                </span>
                                              )}

                                              {enabled[idx] && <Check className="w-4 h-4 text-[var(--caution)]" />}
                                            </div>
                                            <p className="text-sm text-[var(--text-muted)]">
                                              {p.simple_why || p.why}
                                            </p>
                                            {/* Un ajuste interno se lee siempre, aunque esté
                                                «desactivado»: es un consejo, no un toggle. */}
                                            {(esAjusteInterno || enabled[idx]) && p.warn && (
                                              <p className="text-sm text-[var(--caution)] mt-2 flex items-start gap-2">
                                                <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                                                {p.warn}
                                              </p>
                                            )}
                                            {(esAjusteInterno || enabled[idx]) && p.source && (
                                              <button
                                                onClick={(e) => { e.stopPropagation(); openExternal(p.source!); }}
                                                aria-label={`Abrir la fuente de «${p.title}» en el navegador`}
                                                className="text-sm text-[var(--action)] hover:text-[var(--action-hover)] mt-2 inline-flex items-center gap-1 rounded"
                                              >
                                                <ExternalLink className="w-4 h-4" /> Fuente
                                              </button>
                                            )}

                                            {/* Metadata de validación del ajuste, siempre disponible */}
                                            {enabled[idx] && (p.tested_on || p.validated_date || p.measured_improvement) && (
                                              <div className="mt-3 p-3 rounded-lg bg-[var(--bg)] border border-[var(--border)]">
                                                <p className="text-xs font-semibold text-[var(--text-muted)] mb-2 uppercase tracking-wider">Detalles técnicos</p>
                                                <div className="space-y-1.5 text-xs">
                                                  {p.tested_on && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Probado en:</span>
                                                      <span className="text-[var(--text)]">{p.tested_on}</span>
                                                    </div>
                                                  )}
                                                  {p.validated_date && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Validado:</span>
                                                      <span className="text-[var(--text)]">{p.validated_date}</span>
                                                    </div>
                                                  )}
                                                  {p.measured_improvement && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Mejora:</span>
                                                      <span className="text-[var(--compatible)] font-semibold">{p.measured_improvement}</span>
                                                    </div>
                                                  )}
                                                  {p.evidence && (
                                                    <div className="flex items-start gap-2">
                                                      <span className="text-[var(--text-dim)] min-w-[80px]">Evidencia:</span>
                                                      <span className="text-[var(--text)]">{evidenceLabel(p.evidence)}</span>
                                                    </div>
                                                  )}
                                                </div>
                                              </div>
                                            )}
                                          </div>
                                        </div>
                                      </div>
                                    );
                                  })}
                              </div>
                            </div>
                          )}
                        </article>
    </div>
  );
}
