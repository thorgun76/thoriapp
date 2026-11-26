import { useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../lib/api';
import type { OpcionesSteam } from '../lib/types';

/**
 * Lo que Steam tiene configurado para un juego.
 *
 * Es **lo último leído**, no «el comando del usuario»: Steam puede cambiar por fuera de
 * Thoriapp, y Thoriapp puede cambiar sin tocar Steam.
 */
export interface LecturaSteam {
  estado: 'cargando' | 'ok' | 'sin_datos' | 'error';
  /** Texto exacto leído de Steam. `null` si no se pudo leer. */
  opciones: OpcionesSteam | null;
}

/**
 * Relación entre lo que hay en el editor y lo que hay en Steam.
 *
 * Se calcula, no se guarda: depende de tres cosas que cambian por su cuenta.
 */
export interface ComparacionSteam {
  /**
   * ¿Difiere el comando actual de Thoriapp de lo que tiene Steam?
   *
   * `null` cuando no se puede afirmar nada (no se pudo leer Steam).
   */
  difiere: boolean | null;
  /**
   * ¿Difiere el comando actual de Thoriapp de **lo que se importó** al abrir el juego?
   *
   * Es distinto de `difiere`: Steam puede seguir teniendo lo mismo que al importar, y el
   * usuario haber cambiado ajustes en Thoriapp. Eso es lo normal y no hay que avisar.
   */
  difiereDeLoImportado: boolean;
}

/**
 * Los comandos que hay que no mezclar nunca.
 *
 * Son **dos** entradas, más la lectura que el propio hook mantiene:
 *
 * - `importado`: lo que Steam tenía al importar. Inmutable; es la referencia de la
 *   importación y el original del análisis del motor.
 * - `actual`: lo que Thoriapp propone ahora mismo.
 * - El tercero, «lo último leído de Steam», no se pasa: lo devuelve el hook en `opciones`.
 *   Si se pasara habría dos copias del mismo dato y una podría quedarse vieja.
 */
export interface TresComandos {
  /** Lo que Steam tenía al importar. `null` si no se importó nada. */
  importado: string | null;
  /** Lo que Thoriapp propone ahora mismo (toggles + argumentos conservados). */
  actual: string;
}

/**
 * Lee las opciones de lanzamiento que Steam tiene para un juego.
 *
 * ## Por qué la lectura NO depende del comando
 *
 * Antes el efecto dependía de `comandoActual`, así que **cada vez que el usuario tocaba un
 * toggle se volvía a leer `localconfig.vdf`**. Además de leer un archivo de megabytes por
 * una pulsación, el resultado de esa lectura se comparaba con el comando recién cambiado y
 * el aviso de «Steam tiene otro comando» parpadeaba: tocabas un toggle y parecía que Steam
 * se había desincronizado, cuando el que había cambiado era Thoriapp.
 *
 * Ahora la lectura depende solo de `appid`. La comparación se calcula aparte, con
 * `useMemo`, y no provoca ninguna lectura.
 *
 * ## Qué NO hace
 *
 * - **No sobrescribe nada.** Ni Steam ni el editor: devuelve los textos y el usuario
 *   decide si importa. Importar es una acción explícita.
 * - **No escribe en Steam.** Ver `steam_launch` en Rust.
 * - **No afirma nada si no pudo leer.** `null` = no se sabe; no se asume vacío.
 */
export function usarOpcionesSteam(
  appid: number | undefined,
  comandos: TresComandos,
): LecturaSteam & { comparacion: ComparacionSteam } {
  const ultimaPeticion = useRef(0);
  const [estado, setEstado] = useState<LecturaSteam['estado']>('cargando');
  const [opciones, setOpciones] = useState<OpcionesSteam | null>(null);
  // Solo para que el lint vea que `appid` es lo único que dispara la lectura.
  const [appidLeido, setAppidLeido] = useState<number | undefined>(undefined);

  useEffect(() => {
    if (!appid) {
      setEstado('sin_datos');
      setOpciones(null);
      setAppidLeido(undefined);
      return;
    }

    // Contador de petición: cambiar de juego rápido lanzaba dos lecturas y la que
    // terminaba última ganaba, aunque fuera la del juego anterior.
    const peticion = ++ultimaPeticion.current;
    const esLaVigente = () => ultimaPeticion.current === peticion;
    setEstado('cargando');
    // Se borra lo anterior antes de leer: si no, al cambiar de juego se seguía enseñando
    // durante un momento (o para siempre, si la lectura fallaba) el comando del anterior.
    setOpciones(null);

    (async () => {
      try {
        const leidas = await api.steamLaunchOptions(appid);
        if (!esLaVigente()) return;
        if (!leidas) {
          // No se pudo leer: no se dice nada. Inventar `%command%` aquí es exactamente el
          // fallo que se está corrigiendo.
          setEstado('sin_datos');
          setOpciones(null);
          return;
        }
        setEstado('ok');
        setOpciones(leidas);
        setAppidLeido(appid);
      } catch {
        if (esLaVigente()) {
          setEstado('error');
          setOpciones(null);
        }
      }
    })();

    return () => {
      // Al desmontar, cualquier respuesta en vuelo deja de aplicar.
      ultimaPeticion.current += 1;
    };
    // `appid` y nada más: es lo único que cambia lo que Steam tiene guardado.
  }, [appid]);

  const comparacion = useMemo<ComparacionSteam>(() => {
    const steam = opciones?.opciones ?? null;
    // Solo se puede comparar con Steam si la lectura corresponde al juego abierto.
    const lecturaVigente = estado === 'ok' && appidLeido === appid && steam !== null;

    return {
      difiere: lecturaVigente
        ? normalizar(steam) !== normalizar(comandos.actual)
        : null,
      difiereDeLoImportado:
        comandos.importado !== null &&
        normalizar(comandos.importado) !== normalizar(comandos.actual),
    };
  }, [estado, opciones, appidLeido, appid, comandos.importado, comandos.actual]);

  return { estado, opciones, comparacion };
}

/**
 * Compara sin que los espacios de los extremos cuenten como diferencia.
 *
 * No se normaliza nada más: el comando se conserva byte a byte, así que dos textos que
 * solo difieren en espacios intermedios sí son distintos.
 */
function normalizar(texto: string): string {
  return texto.trim();
}
