import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type { OpcionesSteam } from '../lib/types';

/**
 * Compara lo que Thoriapp tiene en el editor con lo que Steam tiene configurado.
 *
 * ## Por qué existe
 *
 * Thoriapp arranca en `%command%` (sin ajustes) y el editor es independiente de Steam. Si
 * el usuario ya tenía `mesa_glthread=true game-performance mangohud %command%` en Steam y
 * graba sin pasar por Thoriapp, la app no tenía forma de saberlo: registraba `%command%`
 * en la prueba y la pantalla mostraba un comando que no era el suyo. Esto lo detecta.
 *
 * ## Qué NO hace
 *
 * - **No sobrescribe nada.** Ni Steam ni el editor: devuelve los dos textos y el usuario
 *   decide si importa. Importar es una acción explícita.
 * - **No escribe en Steam.** Ver `steam_launch` en Rust.
 * - **No afirma nada si no pudo leer.** `null` = no se sabe; no se asume vacío.
 *
 * Se consulta una vez por juego abierto (`appid`), no en bucle: `localconfig.vdf` puede
 * pesar megabytes y no cambia mientras la app está abierta de forma relevante para esto.
 */
export interface DiscrepanciaSteam {
  /** Opciones leídas de Steam (texto exacto). */
  steam: OpcionesSteam;
  /** ¿Difiere de lo que Thoriapp tiene ahora mismo en el editor? */
  difiere: boolean;
}

export function usarOpcionesSteam(
  appid: number | undefined,
  comandoActual: string
): { estado: 'cargando' | 'ok' | 'sin_datos' | 'error'; discrepancia: DiscrepanciaSteam | null } {
  const ultimaPeticion = useRef(0);
  const [estado, setEstado] = useState<'cargando' | 'ok' | 'sin_datos' | 'error'>('cargando');
  const [discrepancia, setDiscrepancia] = useState<DiscrepanciaSteam | null>(null);

  useEffect(() => {
    if (!appid) {
      setEstado('sin_datos');
      setDiscrepancia(null);
      return;
    }

    // Contador de petición: cambiar de juego rápido lanzaba dos lecturas y la que
    // terminaba última ganaba, aunque fuera la del juego anterior. Se descarta cualquier
    // respuesta que no sea de la petición vigente.
    const peticion = ++ultimaPeticion.current;
    const esLaVigente = () => ultimaPeticion.current === peticion;
    setEstado('cargando');
    // Se borra la discrepancia anterior antes de leer: si no, al cambiar de juego se
    // seguía enseñando durante un momento (o para siempre, si la lectura fallaba) el aviso
    // de «Steam tiene otro comando» del juego anterior.
    setDiscrepancia(null);

    (async () => {
      try {
        const opciones = await api.steamLaunchOptions(appid);
        if (!esLaVigente()) return;
        if (!opciones) {
          // No se pudo leer: no se dice nada. Inventar `%command%` aquí es exactamente el
          // fallo que se está corrigiendo.
          setEstado('sin_datos');
          setDiscrepancia(null);
          return;
        }
        setEstado('ok');
        setDiscrepancia({
          steam: opciones,
          difiere: opciones.opciones.trim() !== (comandoActual || '').trim(),
        });
      } catch {
        if (esLaVigente()) {
          setEstado('error');
          setDiscrepancia(null);
        }
      }
    })();

    return () => {
      // Al desmontar, cualquier respuesta en vuelo deja de aplicar.
      ultimaPeticion.current += 1;
    };
  }, [appid, comandoActual]);

  return { estado, discrepancia };
}
