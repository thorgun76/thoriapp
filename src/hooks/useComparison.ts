import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type {
  ConfirmacionDeVistaPrevia,
  ErrorDeEvaluacion,
  VariableEvaluada,
} from '../lib/api';
import type { BenchmarkComparison } from '../lib/types';
import { COMPARISONS_CHANGED_EVENT, notifyComparisonsChanged } from './useSavedComparisons';

/**
 * Identidad de un par de grabaciones.
 *
 * Se serializa el par completo en vez de concatenar con un separador: con `id|id`, el par
 * `("a|b", "c")` y el par `("a", "b|c")` darían la misma clave, y una colisión haría creer
 * que una comparación ya está guardada.
 */
export function claveDePar(baselineId: string, testId: string): string {
  return JSON.stringify([baselineId, testId]);
}

/**
 * Identidad de una **selección**: qué par y con qué variable declarada.
 *
 * Cambiar la variable cambia la selección, porque la evaluación es otra. Si no formara
 * parte de la clave, cambiar de variable mantendría una vista previa que ya no
 * corresponde.
 */
export function claveDeSeleccion(
  baselineId: string,
  testId: string,
  variable: VariableEvaluada | null
): string {
  return JSON.stringify([baselineId, testId, variable?.codigo ?? null]);
}

/** Vista previa aceptada, con la selección a la que corresponde. */
interface VistaPreviaAceptada {
  clave: string;
  confirmacion: ConfirmacionDeVistaPrevia;
}

/** Qué se sabe de un guardado cuya respuesta no llegó a verse. */
/**
 * Lo que se puede afirmar del último guardado cuando la respuesta IPC no llegó.
 *
 * - `desconocido`: no se pudo releer, o se releyó y **no** hay registro de ese par.
 * - `recuperado`: se releyó y **no** hay registro del par: el guardado no ocurrió (o no
 *   consta). Es el único caso en que el intento se puede reintentar sin duplicar.
 * - `par_existente`: hay una comparación guardada de **ese par**, pero encontrar el par
 *   **no** demuestra que sea el resultado de este intento. Puede ser una comparación
 *   anterior de las mismas dos pruebas. Se informa de que existe y no se afirma nada más.
 *
 * En ningún caso se declara «recuperada la evaluación exacta»: eso exigiría comparar
 * evaluaciones, que es una regla de dominio de Rust y no se reimplementa aquí.
 */
export type EstadoDelGuardadoPerdido =
  | { tipo: 'desconocido' }
  | { tipo: 'recuperado' }
  | { tipo: 'par_existente' };

/**
 * Compara automáticamente la prueba inicial con la final, y guarda solo lo que el motor
 * ha evaluado.
 *
 * Garantías de esta pieza, que son las que hacen segura la activación:
 *
 *  - **No hay comparación sin evaluación**: lo único que se pide es la vista previa
 *    evaluada, y lo único que se guarda es la confirmación que devolvió. No existe un
 *    estado intermedio con la comparación pero sin evaluación.
 *  - **La selección manda**: cada respuesta lleva la clave de la selección que la pidió.
 *    Una respuesta tardía de otro par (o de otra variable) se descarta, no se pinta.
 *  - **A→B→A** no puede colarse: cada petición incrementa un token, así que la respuesta
 *    de la primera A no se acepta al volver, aunque los identificadores coincidan.
 *  - **Un guardado por gesto**: la protección es síncrona (una referencia), no un
 *    `setState`. Dos clics en el mismo tick no pueden enviar dos guardados.
 *  - **Respuesta perdida ≠ fallo**: si el error es de transporte, se relee la lista y se
 *    busca el par; si el par está, el guardado ocurrió. Si no se puede saber, se conserva
 *    la incertidumbre y no se afirma que no se guardó.
 */
export function useComparison(
  antesId?: string,
  despuesId?: string,
  onNotify?: (m: string) => void,
  variable?: VariableEvaluada | null
) {
  const variableElegida = variable ?? null;

  /**
   * `onNotify` en un ref, y no en las dependencias del efecto.
   *
   * [C3-13] El componente real lo recrea en cada render (`const notify = (m) => setToast(m)`).
   * Con el callback en las dependencias, **cualquier** render —incluido el que solo pinta el
   * propio aviso— reiniciaba el efecto: invalidaba la vista previa y repetía el IPC sin que la
   * selección hubiera cambiado. El ref conserva la identidad estable y sigue llamando siempre
   * a la versión más reciente, así que no se pierde nada.
   */
  const onNotifyRef = useRef(onNotify);
  onNotifyRef.current = onNotify;

  /** Última vista previa **aceptada**, con la selección a la que pertenece. */
  const [vistaPrevia, setVistaPrevia] = useState<VistaPreviaAceptada | null>(null);
  const [savedKeys, setSavedKeys] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [guardadoPerdido, setGuardadoPerdido] = useState<EstadoDelGuardadoPerdido | null>(null);

  /**
   * Token de la petición vigente. Es una referencia porque se comprueba **antes** de
   * aceptar la respuesta, sin esperar a un render.
   */
  const tokenVigente = useRef(0);

  /**
   * Token de la **relectura del listado**, independiente del de la vista previa.
   *
   * Dos relecturas pueden estar en vuelo a la vez (una al montar, otra al guardar) y
   * resolver fuera de orden: la más antigua, al llegar tarde, pisaría un listado más
   * reciente y `isSaved` volvería a decir que algo no está guardado cuando sí lo está.
   * Solo la relectura más nueva escribe.
   */
  const tokenListado = useRef(0);

  /** Cerrojo síncrono del guardado: dos clics no pueden entrar los dos. */
  const guardando = useRef(false);

  const claveActual =
    antesId && despuesId ? claveDeSeleccion(antesId, despuesId, variableElegida) : null;

  /** Comparación ya aceptada, si su selección sigue siendo la vigente. */
  const confirmacion =
    vistaPrevia != null && vistaPrevia.clave === claveActual ? vistaPrevia.confirmacion : null;
  const comparison: BenchmarkComparison | null = confirmacion?.comparacion ?? null;

  // Al cambiar la selección, la vista previa deja de valer **en el mismo evento**: no se
  // espera a que llegue la respuesta nueva. Así no hay un instante en que se muestre (ni
  // se pueda guardar) una evaluación de otro par.
  useEffect(() => {
    const token = ++tokenVigente.current;

    if (!antesId || !despuesId) {
      setVistaPrevia(null);
      setGuardadoPerdido(null);
      return;
    }

    setVistaPrevia(null);
    setGuardadoPerdido(null);

    const clavePeticion = claveDeSeleccion(antesId, despuesId, variableElegida);

    (async () => {
      try {
                const result = await api.compareBenchmarksEvaluada(antesId, despuesId, variableElegida);
                // Respuesta obsoleta: la petición de la que venía ya no es la vigente.
        if (token !== tokenVigente.current) {
                    return;
        }
        setVistaPrevia({
          clave: clavePeticion,
          confirmacion: result,
        });
              } catch (e) {
                if (token !== tokenVigente.current) {
                    return;
        }
        onNotifyRef.current?.(`❌ ${mensajeDeErrorDeComparacion(e, 'No se pudo comparar')}`);
      }
    })();

    return () => {
      // Un cambio de selección o un desmontaje invalidan: la respuesta que llegue después
      // no se acepta.
      tokenVigente.current++;
    };
  }, [antesId, despuesId, variableElegida]);

  const refreshSaved = useCallback(async () => {
    const token = ++tokenListado.current;
    try {
      const list = await api.loadComparisons();
      // Una relectura más nueva ya ha resuelto (o está en vuelo): esta no escribe.
      if (token !== tokenListado.current) return list;
      setSavedKeys(
        list.map((item) => claveDePar(item.comparison.baseline.id, item.comparison.test.id))
      );
      return list;
    } catch (e) {
      console.error('Error al leer las pruebas guardadas:', e);
      return null;
    }
  }, []);

  useEffect(() => {
    refreshSaved();
    const onChanged = () => {
      refreshSaved();
    };
    window.addEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
    return () => window.removeEventListener(COMPARISONS_CHANGED_EVENT, onChanged);
  }, [refreshSaved]);

  const isSaved = comparison
    ? savedKeys.includes(claveDePar(comparison.baseline.id, comparison.test.id))
    : false;

  const save = useCallback(async () => {
    // Tres condiciones, todas comprobadas de forma síncrona: hay confirmación vigente, no
    // está ya guardada, y no hay otro guardado en curso.
    if (confirmacion == null || isSaved || guardando.current) return;

    guardando.current = true;
    setSaving(true);
    setGuardadoPerdido(null);
    const claveDelIntento = claveActual;
    const tokenDelIntento = tokenVigente.current;
    const parDelIntento = claveDePar(
      confirmacion.comparacion.baseline.id,
      confirmacion.comparacion.test.id
    );

    try {
      await api.saveComparisonEvaluada(confirmacion, variableElegida);
      // La respuesta puede llegar cuando el usuario ya ha cambiado de selección: en ese
      // caso se refresca la lista (el guardado ocurrió) pero no se toca la vista actual.
      onNotify?.('✓ Guardada en Mis pruebas');
      await refreshSaved();
      if (tokenDelIntento === tokenVigente.current && claveDelIntento === claveActual) {
        notifyComparisonsChanged();
      }
    } catch (e) {
      if (!esErrorDeTransporte(e)) {
        // Error tipado desde el origen: se puede afirmar que no se guardó.
        onNotify?.(`❌ ${mensajeDeErrorDeComparacion(e, 'No se pudo guardar')}`);
        return;
      }

      // Transporte roto: no se sabe si el guardado ocurrió. Se relee la lista y se busca
      // **el par**, que es lo que decide el duplicado. No se usa el id de la comparación
      // como prueba: no se conoce antes de guardar.
      //
      // `aplicarSiSigueVigente`: este fallo puede llegar **después** de que el usuario haya
      // cambiado de selección. El estado de «guardado perdido» describe **este** intento y
      // solo tiene sentido sobre su selección; escribirlo en la selección nueva inventaría
      // una duda sobre un par que nunca se intentó guardar.
      const aplicarSiSigueVigente = (estado: EstadoDelGuardadoPerdido) => {
        if (tokenDelIntento !== tokenVigente.current) return false;
        if (claveDelIntento !== claveActual) return false;
        setGuardadoPerdido(estado);
        return true;
      };

      try {
        // Se relee **por el mismo camino** que el resto del listado: así esta lectura
        // también respeta el token y no puede pisar a una más reciente.
        const lista = await refreshSaved();
        if (lista == null) throw new Error('relectura fallida');
        const guardada = lista.some(
          (item) =>
            claveDePar(item.comparison.baseline.id, item.comparison.test.id) === parDelIntento
        );
        if (guardada) {
          // Existe **una** comparación de este par. No se puede afirmar que sea la de este
          // intento: pudo guardarse antes, o con otra variable. Se informa de lo que
          // consta —hay una guardada— y se deja el intento sin confirmar.
          if (aplicarSiSigueVigente({ tipo: 'par_existente' })) {
            onNotify?.(
              '⚠ Ya hay una comparación guardada de estas dos pruebas, pero no se pudo confirmar si es la de este intento.'
            );
          }
          notifyComparisonsChanged();
        } else {
          // La lista no la tiene... pero el guardado podría seguir en curso y aparecer
          // después. No se afirma que no se guardó.
          if (aplicarSiSigueVigente({ tipo: 'desconocido' })) {
            onNotify?.(
              '⚠ No se pudo confirmar si se guardó. Vuelve a abrir la lista antes de intentarlo de nuevo.'
            );
          }
        }
      } catch {
        if (aplicarSiSigueVigente({ tipo: 'desconocido' })) {
          onNotify?.(
            '⚠ No se pudo confirmar si se guardó, y la lista no se pudo releer. No es seguro repetirlo.'
          );
        }
      }
    } finally {
      guardando.current = false;
      setSaving(false);
    }
  }, [confirmacion, isSaved, variableElegida, onNotify, refreshSaved, claveActual]);

  /** Descarta la vista previa vigente (el usuario cierra la tarjeta). */
  const clear = useCallback(() => {
    tokenVigente.current++;
    setVistaPrevia(null);
    setGuardadoPerdido(null);
  }, []);

  return {
    /** Comparación evaluada vigente, o `null`. Sin evaluación no hay comparación. */
    comparison,
    /** La evaluación que la interfaz debe presentar, o `null`. */
    evaluacion: confirmacion?.evaluacion ?? null,
    /** Lo que hay que devolver al guardar. */
    confirmacion,
    isSaved,
    saving,
    save,
    clear,
    /** Resultado de un guardado cuya respuesta se perdió. `null` = no hubo pérdida. */
    guardadoPerdido,
  };
}

/**
 * ¿El fallo puede ser de transporte, es decir, podría el guardado haber ocurrido?
 *
 * Los errores del contrato (`ErrorDeEvaluacion`) llevan `tipo` y **afirman** un resultado
 * del origen: si llegó un rechazo, el guardado no ocurrió. Un error de invocación (IPC
 * caído, ventana cerrada, timeout) no dice nada sobre el disco.
 */
function esErrorDeTransporte(e: unknown): boolean {
  return !esErrorDeEvaluacion(e);
}

function esErrorDeEvaluacion(e: unknown): e is ErrorDeEvaluacion {
  if (typeof e !== 'object' || e == null) return false;
  const tipo = (e as { tipo?: unknown }).tipo;
  return (
    tipo === 'confirmacion_rechazada' ||
    tipo === 'grabacion_inexistente' ||
    tipo === 'identificador_ambiguo' ||
    tipo === 'comparacion_ya_existente' ||
    tipo === 'lectura_fallida' ||
    tipo === 'escritura_fallida'
  );
}

/**
 * Mensaje para el usuario a partir del **discriminante**, no del texto libre.
 *
 * Un `confirmacion_rechazada` por obsolescencia no es un error del usuario: es que los
 * datos cambiaron, y lo que toca es repetir la vista previa.
 */
function mensajeDeErrorDeComparacion(e: unknown, prefijo: string): string {
  if (!esErrorDeEvaluacion(e)) {
    return typeof e === 'string' ? `${prefijo}: ${e}` : `${prefijo}: ${String(e)}`;
  }
  switch (e.tipo) {
    case 'confirmacion_rechazada':
      if (e.motivo === 'entradas_cambiadas' || e.motivo === 'evaluacion_no_corresponde') {
        return 'La vista previa ya no corresponde a los datos actuales. Vuelve a comparar antes de guardar; no se guardó nada.';
      }
      return `${prefijo}: la confirmación no coincide con el recálculo (${e.motivo}). No se guardó nada.`;
    case 'grabacion_inexistente':
      return `${prefijo}: alguna de las dos grabaciones ya no existe.`;
    case 'identificador_ambiguo':
      return `${prefijo}: el identificador corresponde a varias grabaciones, así que no se puede saber cuál evaluar.`;
    case 'comparacion_ya_existente':
      return 'Esta comparación ya está guardada en Mis pruebas.';
    case 'lectura_fallida':
      return `${prefijo}: no se pudo leer ${e.archivo}.`;
    case 'escritura_fallida':
      return `${prefijo}: no se pudo escribir. ${e.mensaje}`;
  }
}

/**
 * ¿Se puede ofrecer el botón de guardar?
 *
 * Con el resultado en duda no se ofrece. Cubre dos casos distintos:
 *
 * - `desconocido`: no consta si se guardó. Repetir podría chocar con el duplicado.
 * - `par_existente`: hay una guardada de estas dos pruebas. No se sabe si es la de este
 *   intento, así que repetir añadiría una segunda comparación del mismo par sin que el
 *   usuario haya podido distinguirlas.
 */
export function puedeGuardar(estado: {
  confirmacion: ConfirmacionDeVistaPrevia | null;
  isSaved: boolean;
  saving: boolean;
  guardadoPerdido: EstadoDelGuardadoPerdido | null;
}): boolean {
  if (estado.confirmacion == null) return false;
  if (estado.saving) return false;
  if (estado.guardadoPerdido != null) return false;
  return !estado.isSaved;
}
