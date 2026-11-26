import { useEffect, useState } from 'react';
import { api } from '../lib/api';

/**
 * Posición de cada ajuste dentro del catálogo **efectivo** del motor.
 *
 * El catálogo no es `rules.params` concatenado con las recomendaciones: `catalogo_efectivo`
 * (Rust) salta las recomendaciones vacías, `%command%` y las que ya existen en el perfil del
 * juego. El perfil genérico trae `PROTON_LOCAL_SHADER_CACHE=1` y `mangohud`, así que en la
 * práctica siempre se descarta alguna y calcular el índice a mano desplazaba todos los
 * siguientes. El toggle escribía entonces sobre otro ajuste, que es lo que se veía como
 * «este interruptor no hace nada».
 *
 * Aquí solo se guarda lo que responde el motor: ninguna regla se duplica en TypeScript.
 *
 * @param appid Juego abierto. Cambiarlo recarga el mapa.
 * @param extras Textos de recomendación en el orden en que se enseñan. Igual que se le
 *   mandan a `analyze_launch_command`, o el índice no apuntaría al mismo ajuste.
 * @returns Mapa `valor → índice`. Vacío mientras carga; un valor ausente significa que el
 *   motor no conoce esa recomendación y no se debe tocar.
 */
export function useCatalogoDeAjustes(
  appid: number | undefined,
  extras: string[],
): Map<string, number> {
  const [indices, setIndices] = useState<Map<string, number>>(new Map());

  useEffect(() => {
    if (appid === undefined) {
      setIndices(new Map());
      return;
    }
    let vigente = true;
    (async () => {
      try {
        const catalogo = await api.catalogoDeAjustes(appid, extras);
        if (!vigente) return;
        setIndices(new Map(catalogo.map((d) => [d.valor, d.indice])));
      } catch {
        // Sin catálogo no se puede tocar ninguna recomendación, pero la app sigue
        // funcionando con los ajustes del juego: no se propaga como fallo global.
        if (vigente) setIndices(new Map());
      }
    })();
    return () => {
      vigente = false;
    };
    // `extras` entra por contenido: el array se recrea en cada render y volvería a pedir el
    // catálogo en bucle.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [appid, extras.join('\u0000')]);

  return indices;
}
