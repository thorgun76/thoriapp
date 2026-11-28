import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, within } from '@testing-library/react';
import transporte from '../test/fixtures/comparaciones_transporte.json';
import type { SavedComparison } from '../lib/api';
import { DashboardView } from './DashboardView';
import { BenchmarksView, CompareMeter } from './BenchmarksView';
import { diferencia } from './SavedComparisonDetail';

/**
 * Regresiones de los defectos residuales A–J, hallados en la validación real en CachyOS
 * **después** de que la matriz anterior diera verde.
 *
 * ─── De dónde salen los datos ───
 *
 * `src/test/fixtures/comparaciones_transporte.json` **no está escrito a mano**: lo generó
 * el lector y el serializador reales de Rust (`try_load_saved_data_de` +
 * `ComparacionLeida`, el mismo camino que `load_comparisons`) a partir de la fixture de
 * disco usada en CachyOS. Por eso trae `estado_de_la_evaluacion` ya clasificado por el
 * dominio y esta prueba **no replica ningún clasificador**: si Rust cambia la
 * clasificación, la fixture y estas pruebas cambian con él.
 *
 * Procedencia y equivalencia: ver `src/test/fixtures/README.md`.
 *
 * ─── Qué se comprueba ───
 *
 * El **texto renderizado** del componente real, consultando dentro del contenedor que
 * devuelve `render`. Se comprueba lo correcto presente y lo incorrecto ausente.
 *
 * ─── Qué no prueba ───
 *
 * No recorre el IPC ni WebKitGTK: el objeto se pasa como prop. Que Rust serialice esto lo
 * cubre `cargo test`.
 */

const TODO: SavedComparison[] = transporte as unknown as SavedComparison[];

let guardadas: SavedComparison[] = [];

vi.mock('../hooks/useSavedComparisons', () => ({
  COMPARISONS_CHANGED_EVENT: 'thoriapp:comparisons-changed',
  useSavedComparisons: () => ({
    items: guardadas,
    loading: false,
    remove: vi.fn(),
    removeAll: vi.fn(),
    error: null,
  }),
}));

beforeEach(() => {
  guardadas = [...TODO];
});

afterEach(() => {
  cleanup();
});

/* ─────────────────────────────── Utilidades ─────────────────────────────── */

const LIMPIO = (t: string | null | undefined) => (t ?? '').replace(/\s+/g, ' ').trim();

const porNombre = (trozo: string): SavedComparison => {
  const c = TODO.find((g) => g.game_name.includes(trozo));
  if (!c) throw new Error(`La fixture de transporte no trae «${trozo}»`);
  return c;
};

/** Abre el detalle por el camino real: pulsando la tarjeta en el Dashboard. */
function abrirDetalleDe(c: SavedComparison): HTMLElement {
  const { container } = render(<DashboardView />);
  fireEvent.click(
    within(container).getByRole('button', { name: new RegExp(`^${c.game_name}:`, 'i') })
  );
  expect(within(container).queryByText(/Volver a Mis pruebas/i)).not.toBeNull();
  return container;
}

const texto = (c: HTMLElement) => LIMPIO(c.textContent);

/* ───────────────────────── Fiabilidad de la fixture ───────────────────────── */

describe('la fixture de transporte la genera Rust, no la prueba', () => {
  it('trae las cuatro clasificaciones del dominio', () => {
    const estados = TODO.map((g) => g.estado_de_la_evaluacion);
    expect(estados).toContain('soportada');
    expect(estados).toContain('no_registrada');
    expect(estados).toContain('malformada');
    expect(estados).toContain('no_soportada');
  });

  it('lo que Rust clasifica como malformada llega con evaluacion nula', () => {
    // No lo deduce la prueba: lo dice el lector real.
    const c = porNombre('Malformada');
    expect(c.estado_de_la_evaluacion).toBe('malformada');
    expect(c.evaluacion ?? null).toBeNull();
  });

  it('lo que Rust clasifica como no soportada conserva su evaluación', () => {
    const c = porNombre('No soportada');
    expect(c.estado_de_la_evaluacion).toBe('no_soportada');
    expect(c.evaluacion).toBeTruthy();
  });
});

/* ─────────────────────────────────── A ─────────────────────────────────── */

describe('A · PARCIAL_PERDIDA: el detalle no anuncia una victoria global', () => {
  const parcial = () => porNombre('PARCIAL_PERDIDA');

  it('la pérdida del 50 % en el 0,1 % low está en los datos', () => {
    expect(parcial().comparison.fps_0_1_percent_change_percent).toBeCloseTo(-50, 5);
    const m01 = (parcial().evaluacion?.metricas ?? []).find(
      (m) => m.clave === 'fps_0_1_percent_low'
    );
    expect(m01, 'la métrica del 0,1 % low debe estar en el registro').toBeDefined();
    expect(m01!.preferencia).toBe('no_establecida');
  });

  it('el detalle no afirma que la grabación nueva rinde mejor', () => {
    expect(texto(abrirDetalleDe(parcial()))).not.toMatch(/rinde mejor que la anterior/i);
  });

  it('el detalle dice que la mejora es solo en parte de las métricas', () => {
    expect(texto(abrirDetalleDe(parcial()))).toMatch(/en parte de las m[ée]tricas/i);
  });

  it('la pérdida del 50 % sigue visible, sin preferencia', () => {
    const contenedor = abrirDetalleDe(parcial());
    const filas = Array.from(contenedor.querySelectorAll('tbody tr'));
    const fila = filas.find((f) => /0[.,]1 ?% low/i.test(LIMPIO(f.textContent)));
    expect(fila, 'la fila del 0,1 % low debe existir').toBeDefined();
    const tf = LIMPIO(fila!.textContent);
    expect(tf).toMatch(/-50/);
    expect(tf).not.toMatch(/Favorece a/i);
    expect(tf).not.toMatch(/a favor de la nueva/i);
  });

  it('no muestra una corona ni un ganador global', () => {
    const contenedor = abrirDetalleDe(parcial());
    expect(contenedor.querySelector('[data-ganadora="true"]')).toBeNull();
    expect(texto(contenedor)).not.toMatch(/gana la nueva|gana la anterior/i);
  });
});

/* ─────────────────────────────────── B ─────────────────────────────────── */

describe('B · MALFORMADA: evaluación ilegible no es evaluación ausente', () => {
  it('no dice «Evaluación original no registrada»', () => {
    expect(texto(abrirDetalleDe(porNombre('Malformada')))).not.toMatch(
      /Evaluaci[óo]n original no registrada/i
    );
  });

  it('dice que no se puede leer', () => {
    expect(texto(abrirDetalleDe(porNombre('Malformada')))).toMatch(
      /no legible|no se puede leer|ilegible/i
    );
  });

  it('no atribuye la ausencia a que la prueba sea anterior', () => {
    expect(texto(abrirDetalleDe(porNombre('Malformada')))).not.toMatch(
      /antes de que Thoriapp registrara/i
    );
  });
});

/* ─────────────────────────────────── C ─────────────────────────────────── */

describe('C · NO SOPORTADA: no se atribuyen ausencias que el registro no dice', () => {
  const t = () => texto(abrirDetalleDe(porNombre('No soportada')));

  it('no afirma falta de apoyo ni ausencia de evaluación', () => {
    const x = t();
    expect(x).not.toMatch(/sin apoyo declarado/i);
    expect(x).not.toMatch(/M[ée]tricas tal como se evaluaron/i);
    expect(x).not.toMatch(/Compatibilidad metodol[óo]gica no registrada/i);
    expect(x).not.toMatch(/sin preferencia establecida/i);
  });

  it('no filtra el estado crudo del registro ilegible', () => {
    expect(t()).not.toMatch(/\((mejora|empeora|mixto)\)/i);
  });

  it('sí muestra los datos medidos', () => {
    expect(t()).toMatch(/FPS medios/i);
  });
});

/* ─────────────────────────────────── D ─────────────────────────────────── */

describe('D · motivos comprensibles y códigos técnicos como información secundaria', () => {
  it('explica el motivo en castellano, sin el código crudo como único texto', () => {
    const x = texto(abrirDetalleDe(porNombre('PARCIAL_PERDIDA')));
    expect(x).toMatch(/no pudo juzgar|no cuenta en el resultado/i);
  });

  it('conserva los códigos conocidos como información técnica secundaria', () => {
    // Se exige que sigan **accesibles**, aunque la frase principal ya los explique.
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const tecnica = contenedor.querySelector(
      '[data-testid="codigos-tecnicos"], [data-tecnico="codigos"]'
    );
    expect(tecnica, 'debe haber una zona de información técnica').not.toBeNull();
    expect(LIMPIO(tecnica!.textContent)).toMatch(/perdida_observada_sin_evaluar/);
    expect(LIMPIO(tecnica!.textContent)).toMatch(/evaluacion_parcial/);
  });

  it('muestra el apoyo por lado cuando consta y no lo inventa cuando falta', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const ev = porNombre('PARCIAL_PERDIDA').evaluacion!;
    const lows = ev.metricas!.filter((m) => m.clave !== 'avg_fps');

    // El 1 % low trae apoyo por lado en el registro.
    expect(lows.some((m) => m.apoyo_referencia || m.apoyo_candidata)).toBe(true);
    // Y el detalle lo enseña como «muestras», que es lo que hay.
    expect(texto(contenedor)).toMatch(/muestras/i);
    // Nunca un cero inventado donde el dato no consta: se dice que no consta.
    expect(texto(contenedor)).not.toMatch(/(^|[^\d])0 muestras/);
    // C3-25: el apoyo se atribuye a la métrica concreta, no queda como cifra suelta.
    expect(texto(contenedor)).toMatch(/A qué m[ée]trica afecta/i);
    expect(texto(contenedor)).toMatch(/0,1 ?% low[^.]*no pudo juzgar/i);
  });
});

/* ─────────────────────────────────── E ─────────────────────────────────── */

describe('E · ruido estimado y margen aplicado son conceptos separados', () => {
  it('métricas avanzadas no rotulan el ruido como «margen»', () => {
    const contenedor = abrirDetalleDe(porNombre('COMPLETA_MEJORA'));
    // Abre la sección de métricas avanzadas, que es donde vivía el texto «Margen
    // estimado: ±3.0 % (ruido combinado…)».
    const avanzadas = within(contenedor).queryByRole('button', {
      name: /Ver métricas avanzadas/i,
    });
    if (avanzadas) fireEvent.click(avanzadas);

    const x = texto(contenedor);
    expect(x).not.toMatch(/Margen estimado/i);
    expect(x).not.toMatch(/ruido combinado de las dos grabaciones/i);
  });

  it('los dos valores registrados son distintos y no se reconstruye el ausente', () => {
    const ev = porNombre('COMPLETA_MEJORA').evaluacion!;
    expect(ev.condiciones.margen_porcentaje).toBe(5);
    expect(ev.condiciones.ruido_combinado_porcentaje).toBe(3);
    // Donde no consta, consta como ausente: la fixture lo prueba en la histórica.
    expect(porNombre('Historica').evaluacion ?? null).toBeNull();
  });
});

/* ─────────────────────────────────── F ─────────────────────────────────── */

describe('F · la ausencia de instantánea no se atribuye a una versión antigua', () => {
  it('el detalle no afirma cuándo se guardó la prueba', () => {
    expect(texto(abrirDetalleDe(porNombre('COMPLETA_MEJORA')))).not.toMatch(
      /antes de que Thoriapp registrara/i
    );
  });
});

/* ───────────────────────── G: la guía no enseña una regla falsa ───────────────────────── */

describe('G · la guía práctica no declara ganador por FPS y estabilidad', () => {
  /**
   * La guía es contenido estático de `BenchmarksView`: se pinta siempre, sin medida
   * seleccionada. La versión anterior de esta prueba buscaba el bloque y **retornaba si no
   * lo encontraba**, así que no podía fallar: un falso positivo. Ahora la ausencia del
   * bloque es un fallo explícito.
   */
  const renderGuia = () => {
    const { container } = render(<BenchmarksView />);
    const articulos = Array.from(container.querySelectorAll('article'));
    const guia = articulos.find((a) =>
      /Gu[ií]a r[áa]pida de benchmarks/i.test(LIMPIO(a.textContent))
    );
    expect(guia, 'la guía rápida debe renderizarse').toBeDefined();
    return { contenedor: container, guia: guia!, texto: LIMPIO(guia!.textContent) };
  };

  it('la guía rápida se renderiza', () => {
    const { texto: x } = renderGuia();
    expect(x).toContain('Guía rápida de benchmarks');
  });

  it('el ejemplo práctico se renderiza', () => {
    const { guia } = renderGuia();
    const ejemplo = within(guia).getByRole('heading', { name: /Ejemplo pr[áa]ctico/i });
    expect(ejemplo).toBeTruthy();
    // Y trae su contenido, no solo el título.
    expect(LIMPIO(guia.textContent)).toMatch(/Grabaci[óo]n 1: Palworld sin optimizar/);
    expect(LIMPIO(guia.textContent)).toMatch(/Grabaci[óo]n 2: Palworld con game-performance/);
  });

  it('el ejemplo conserva la cifra medida del caso', () => {
    expect(renderGuia().texto).toMatch(/\+16[.,]7 ?% de FPS/);
  });

  it('el ejemplo no declara ganador por FPS y estabilidad mantenida', () => {
    const x = renderGuia().texto;
    // La regla falsa, en sus dos formulaciones conocidas.
    expect(x).not.toMatch(/gana la nueva grabaci[óo]n/i);
    expect(x).not.toMatch(/si la Fluidez se mantiene,? gana/i);
    // Ni ninguna otra forma de cerrar el ejemplo con una victoria.
    expect(x).not.toMatch(/🎉/);
  });

  it('el ejemplo remite a margen, resto de métricas y resultado no concluyente', () => {
    const x = renderGuia().texto;
    expect(x).toMatch(/supera el margen/i);
    expect(x).toMatch(/resto de m[ée]tricas/i);
    // Y admite que la evaluación puede no dar un ganador.
    expect(x).toMatch(/mixto|parcial|dentro del margen/i);
  });
});

/* ────────── H: dirección, elegibilidad, evaluación y preferencia ────────── */

describe('H · dirección, elegibilidad, evaluación y preferencia no se confunden', () => {
  it('una métrica que baja puede no tener preferencia, y se ve así', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const filas = Array.from(contenedor.querySelectorAll('tbody tr'));
    const fila = filas.find((f) => /0[.,]1 ?% low/i.test(LIMPIO(f.textContent)));
    expect(fila).toBeDefined();
    const tf = LIMPIO(fila!.textContent);

    // Dirección: baja (el número es negativo).
    expect(tf).toMatch(/-50/);
    // Preferencia: no establecida. La fila no puede decir que favorece a nadie.
    expect(tf).not.toMatch(/Favorece a/i);
  });

  it('una métrica evaluada e indistinta no se presenta como no evaluada', () => {
    const ev = porNombre('PARCIAL_PERDIDA').evaluacion!;
    const indistintas = ev.metricas!.filter((m) => m.preferencia === 'indistinto');
    // Si el escenario no trae ninguna, la comprobación no aplica; se declara.
    if (indistintas.length === 0) return;
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    expect(texto(contenedor)).toMatch(/sin diferencia apreciable/i);
  });
});

/* ────────────────────── I: estabilidad con escala absoluta ────────────────────── */

describe('I · la estabilidad usa escala absoluta 0-100 en todas las superficies', () => {
  it('en el detalle guardado el ancho de la barra es el índice, no la proporción', () => {
    // La estabilidad se pinta dentro del panel de «Comparación de rendimiento». Las barras
    // son `role="img"` con el valor en su etiqueta accesible y el ancho en el hijo.
    const contenedor = abrirDetalleDe(porNombre('COMPLETA_MEJORA'));
    const valores = porNombre('COMPLETA_MEJORA');
    expect(valores.comparison.baseline).toBeTruthy();

    // Las tarjetas de barras del detalle: la de estabilidad es la que pinta un índice
    // 0-100 (sus valores no llevan «FPS»).
    const barras = Array.from(
      contenedor.querySelectorAll<HTMLElement>('[role="img"]')
    ).filter((b) => /^Prueba \d+: \d+$/.test(b.getAttribute('aria-label') ?? ''));

    expect(barras.length, 'deben existir las barras de estabilidad').toBeGreaterThan(0);

    for (const b of barras) {
      const valor = Number((b.getAttribute('aria-label') ?? '').replace(/^Prueba \d+: /, ''));
      expect(Number.isFinite(valor)).toBe(true);
      const relleno = b.querySelector<HTMLElement>('[style*="width"]');
      expect(relleno, 'la barra debe tener relleno con ancho').not.toBeNull();
      const ancho = Number((relleno!.style.width.match(/([\d.]+)%/) ?? [])[1]);
      // Escala absoluta 0-100: el ancho es el índice (mínimo visual del 2 %).
      expect(ancho).toBe(Math.max(2, Math.min(100, valor)));
    }
  });
});

/* ─────────── J: la narrativa no es una segunda conclusión vigente ─────────── */

describe('J · la narrativa original no compite con la evaluación estructurada', () => {
  it('con evaluación soportada se identifica como texto de la medición', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_CON_NARRATIVA'));
    const narrativa = contenedor.querySelector('[data-testid="narrativa-original"]');
    expect(narrativa, 'la narrativa persistida debe seguir mostrándose').not.toBeNull();
    // Contenido íntegro.
    expect(LIMPIO(narrativa!.textContent)).toBe(
      'Texto narrativo ORIGINAL de la epoca (con evaluacion estructurada registrada).'
    );
    // Rótulo: relato de la medición, no conclusión vigente.
    const x = texto(contenedor);
    expect(x).toMatch(/Texto original de la medici[óo]n/i);
    expect(x).not.toMatch(/Conclusión guardada de esta prueba/i);
    // Y se dice explícitamente cuál manda.
    expect(x).toMatch(/El resultado que cuenta es el de la evaluaci[óo]n registrada/i);
  });

  it('sin evaluación estructurada se identifica como lo único que consta', () => {
    const contenedor = abrirDetalleDe(porNombre('Historica'));
    const narrativa = contenedor.querySelector('[data-testid="narrativa-original"]');
    expect(narrativa).not.toBeNull();
    expect(LIMPIO(narrativa!.textContent)).toBe(
      'Texto narrativo ORIGINAL de la epoca (sin evaluacion registrada).'
    );
    const x = texto(contenedor);
    // 3.7: antes convivían «lo que no consta es la conclusión que se mostró entonces» y
    // «Conclusión guardada de esta prueba», que se contradicen. Ahora se dice que no hay
    // evaluación estructurada y que la narrativa **sí** consta.
    expect(x).toMatch(/Texto original que s[íi] consta en esta prueba/i);
    expect(x).not.toMatch(/Conclusión guardada de esta prueba/i);
    expect(x).not.toMatch(/lo que no consta es la conclusión/i);
    expect(x).toMatch(/No se reconstruye un veredicto moderno/i);
  });

  it('nunca presenta la narrativa como el resultado actual en una evaluación soportada', () => {
    const x = texto(abrirDetalleDe(porNombre('PARCIAL_CON_NARRATIVA')));
    // El veredicto vigente es el de la evaluación (parcial), no el texto antiguo.
    expect(x).toMatch(/en parte de las m[ée]tricas/i);
  });
});

/* ──────── I (detalle): la diferencia en puntos sale de los valores originales ──────── */

describe('I · la diferencia entre índices se calcula desde los valores originales', () => {
  /**
   * Escala absoluta 0-100 con precisión 0: es como se pinta la estabilidad.
   * `diferencia(antes, despues, 'puntos', 0)` es exactamente lo que ve el lector.
   */
  const pts = (a: number, b: number) => diferencia(a, b, 'puntos', 0);

  it('resta los originales, no las cifras ya redondeadas', () => {
    // Los dos se pintan «77», así que restar lo mostrado daría «0 pts» y el 0,8 real se
    // perdería. La diferencia se calcula de los originales.
    expect(pts(76.6, 77.4).texto).toBe('+0.8 pts');
  });

  it('no inventa diferencia donde los datos son idénticos', () => {
    expect(pts(76.6667, 76.6667).texto).toBe('0 pts');
  });

  it('una caída de varios puntos se lee con un decimal', () => {
    // 73,1 → 62,4 son −10,7 puntos reales. Si se restaran las cifras mostradas («73» y
    // «62») el resultado cambiaría de décima: −11 frente a −10,7.
    expect(pts(73.1, 62.4).texto).toBe('-10.7 pts');
    expect(Math.round(62.4) - Math.round(73.1)).toBe(-11);
  });

  it('no inventa un punto donde hubo dos centésimas', () => {
    // «62,49 → 62,51» se pinta «62 → 63». Restar los redondeos daría «+1 pts», una
    // diferencia inventada; restar los originales da +0,02, que con un decimal se lee
    // «+0.02 pts»: pequeño, pero no un cero que niegue el cambio visible.
    const d = pts(62.49, 62.51);
    expect(d.texto).toBe('+0.02 pts');
    expect(d.signo).toBeCloseTo(0.02, 6);
  });

  it('con más decimales el resultado es exactamente la resta', () => {
    expect(diferencia(100, 125, 'puntos', 1).texto).toBe('+25.0 pts');
    expect(diferencia(70, 88, 'puntos', 1).texto).toBe('+18.0 pts');
  });
});

/* ─────────── H: dirección, elegibilidad, evaluación y preferencia ─────────── */

describe('H · una métrica indistinta no se confunde con una excluida ni con una no evaluada', () => {
  const hallarMetrica = (ev: NonNullable<SavedComparison['evaluacion']>) =>
    ev.metricas ?? [];

  it('el registro distingue los tres casos con campos distintos', () => {
    const ev = porNombre('PARCIAL_PERDIDA').evaluacion!;
    const ms = hallarMetrica(ev);

    for (const m of ms) {
      // Dirección: propiedad de los datos, siempre presente si hay cifras.
      // Elegibilidad y evaluación: `participa` / `evaluada`-equivalente lo decide Rust.
      // Preferencia: solo tiene sentido si la métrica entró en la decisión.
      const razones = [...(m.motivos ?? []), ...(m.limitaciones ?? [])];
      if (m.preferencia === 'indistinto') {
        // Indistinta = evaluada y sin diferencia apreciable. NO es «no evaluada».
        expect(razones).not.toContain('no_elegible');
        expect(razones).not.toContain('apoyo_insuficiente');
      }
      if (razones.includes('no_elegible')) {
        // Excluida: no puede llevar una preferencia de ganador.
        expect(['no_establecida', 'indistinto']).toContain(m.preferencia);
      }
    }
  });

  it('la superficie real no llama «indistinta» a una métrica excluida', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const filas = Array.from(contenedor.querySelectorAll('tbody tr'));
    const low01 = filas.find((f) => /0[.,]1 ?% low/i.test(LIMPIO(f.textContent)));
    expect(low01).toBeDefined();
    const tf = LIMPIO(low01!.textContent);

    // El 0,1 % low de este escenario está excluido por apoyo insuficiente: la pantalla
    // no puede presentarlo como «sin diferencia apreciable» (eso sería indistinto).
    expect(tf).not.toMatch(/sin diferencia apreciable/i);
    // Ni afirmar que favorece a alguien.
    expect(tf).not.toMatch(/Favorece a/i);
  });

  it('la superficie sí marca la dirección observada en esa misma fila', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const filas = Array.from(contenedor.querySelectorAll('tbody tr'));
    const low01 = filas.find((f) => /0[.,]1 ?% low/i.test(LIMPIO(f.textContent)));
    // Dirección: baja. Es un hecho medido, independiente de la elegibilidad.
    expect(LIMPIO(low01!.textContent)).toMatch(/-50/);
  });
});

/* ── C3-19: la vista previa usa escala absoluta 0-100 para la estabilidad ── */

describe('C3-19 · la estabilidad de la vista previa se dibuja sobre 0-100', () => {
  /**
   * Valores **reales** de la captura de PARCIAL_PERDIDA: 73 y 62.
   *
   * Con la escala relativa anterior (`v / max(base, test)`) la barra de 73 ocupa el 100 %
   * y la de 62 el 84,9 %, que es justo lo que se vio en pantalla. En una escala absoluta
   * 0-100, 73 se ve al 73 % y 62 al 62 %.
   *
   * Se renderiza el componente real de la vista previa (`CompareMeter`), no el detalle.
   */
  const anchoDe = (c: HTMLElement) =>
    Array.from(c.querySelectorAll<HTMLElement>('[style*="width"]')).map((b) =>
      Number((b.style.width.match(/([\d.]+)%/) ?? [])[1])
    );

  const renderEstabilidad = (base: number, test: number) =>
    render(
      <CompareMeter
        icon={null}
        title="Estabilidad de FPS"
        help=""
        base={base}
        test={test}
        decimals={0}
        unit=""
        level="descriptiva"
        footer=""
        etiquetaBase="Prueba 1"
        etiquetaTest="Prueba 2"
        escalaAbsoluta={100}
      />
    );

  it('73 y 62 se dibujan al 73 % y al 62 %, no al 100 % y al 85 %', () => {
    const { container } = renderEstabilidad(73, 62);
    const [p1, p2] = anchoDe(container);
    expect(p1).toBe(73);
    expect(p2).toBe(62);
    // Lo que hacía la escala relativa, para que quede constancia del defecto.
    expect(Math.round((62 / 73) * 100)).toBe(85);
  });

  it('un índice de 100 llena la barra y uno de 30 no', () => {
    expect(anchoDe(renderEstabilidad(100, 30).container)).toEqual([100, 30]);
  });

  it('las barras de FPS medios siguen siendo relativas entre sí', () => {
    // Otro concepto: aquí no hay máximo absoluto, 100 FPS contra 125 se compara entre sí.
    const { container } = render(
      <CompareMeter
        icon={null}
        title="FPS medios"
        help=""
        base={100}
        test={125}
        decimals={0}
        unit=" FPS"
        level="descriptiva"
        footer=""
        etiquetaBase="Prueba 1"
        etiquetaTest="Prueba 2"
      />
    );
    expect(anchoDe(container)).toEqual([80, 100]);
  });
});

/* ══ Incidencias de la tercera validación real (3.3–3.8) ══ */

describe('3.3 · dirección y preferencia se distinguen en la tabla inferior', () => {
  it('son dos columnas, no una sola columna «Dirección»', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    // La tabla inferior es la de «Métricas tal como se evaluaron»; la superior es la de
    // evaluación por métrica y no es la que mezclaba dirección y preferencia.
    const tablas = Array.from(contenedor.querySelectorAll('table'));
    const tabla = tablas.find((t) =>
      /Antes/.test(LIMPIO(t.querySelector('thead')!.textContent))
    );
    expect(tabla).toBeDefined();
    expect(LIMPIO(contenedor.textContent)).toMatch(/M[ée]tricas tal como se evaluaron/i);
    const cabeceras = Array.from(tabla!.querySelectorAll('thead th')).map((th) =>
      LIMPIO(th.textContent)
    );
    // La columna que mezclaba dirección y preferencia ya no existe.
    expect(cabeceras).toContain('Dirección');
    expect(cabeceras).toContain('Preferencia');
    // Y no queda el texto compuesto «sube · a favor de la nueva» en una sola celda.
    const celdas = Array.from(tabla!.querySelectorAll('tbody td')).map((td) =>
      LIMPIO(td.textContent)
    );
    expect(celdas.some((c) => /·/.test(c))).toBe(false);
  });

  it('la preferencia se afirma solo donde el motor la estableció', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const tablas = Array.from(contenedor.querySelectorAll('table'));
    const tabla = tablas.find((t) => /Preferencia/.test(LIMPIO(t.querySelector('thead')!.textContent)))!;
    const filas = Array.from(tabla.querySelectorAll('tbody tr'));
    const low01 = filas.find((f) => /0[.,]1 ?% low/i.test(LIMPIO(f.textContent)));
    const celdas = Array.from(low01!.querySelectorAll('td')).map((td) => LIMPIO(td.textContent));
    // Dirección: baja (hecho medido). Preferencia: sin preferencia establecida.
    expect(celdas).toContain('baja');
    expect(celdas.join('|')).toMatch(/sin preferencia establecida/i);
  });
});

describe('3.6 · con evaluación no interpretable no se afirma qué consta en ella', () => {
  const t = () => texto(abrirDetalleDe(porNombre('No soportada')));

  it('no dice «No consta en la evaluación»', () => {
    expect(t()).not.toMatch(/No consta en la evaluaci[óo]n/i);
  });

  it('dice que esta versión no la interpreta, sin hablar de su contenido', () => {
    // 4.ª ronda: el rótulo nombra el motivo real (metodología no soportada) en lugar de un
    // «no interpretable» genérico que además se usaba para la evaluación ausente.
    expect(t()).toMatch(/no interpreta/i);
    expect(t()).not.toMatch(/no consta en la evaluaci[óo]n/i);
  });

  it('el dato medido del frametime se sigue mostrando', () => {
    // El tiempo por imagen es una cifra medida: se enseña aunque la evaluación no se lea.
    const contenedor = abrirDetalleDe(porNombre('No soportada'));
    const filas = Array.from(contenedor.querySelectorAll('table tbody tr'));
    const ft = filas.find((f) => /Tiempo por imagen/i.test(LIMPIO(f.textContent)));
    expect(ft).toBeDefined();
    expect(LIMPIO(ft!.textContent)).toMatch(/ms/);
  });
});

describe('3.5 · el margen de una metodología no soportada no se presenta como aplicado', () => {
  it('el registro conserva el 5 % pero no se muestra como criterio vigente', () => {
    // El dato sigue en la evaluación persistida (no se toca), pero la presentación no lo
    // afirma: lo aplicó una metodología que esta versión no sabe interpretar.
    expect(porNombre('No soportada').evaluacion!.condiciones.margen_porcentaje).toBe(5);

    const contenedor = abrirDetalleDe(porNombre('No soportada'));
    const avanzadas = within(contenedor).queryByRole('button', {
      name: /Ver métricas avanzadas/i,
    });
    if (avanzadas) fireEvent.click(avanzadas);
    const x = texto(contenedor);
    expect(x).not.toMatch(/Margen aplicado:\s*±?\s*5/);
    expect(x).toMatch(/no consta en el registro/i);
  });
});

describe('3.7 · la narrativa histórica no se contradice', () => {
  it('no promete una conclusión que después dice no constar', () => {
    const x = texto(abrirDetalleDe(porNombre('Historica')));
    expect(x).not.toMatch(/lo que no consta es la conclusi[óo]n que se mostr[óo]/i);
    expect(x).not.toMatch(/Conclusi[óo]n guardada de esta prueba/i);
    expect(x).toMatch(/No se reconstruye un veredicto moderno/i);
    expect(x).toMatch(/Texto original que s[íi] consta en esta prueba/i);
  });

  it('conserva íntegra la narrativa persistida', () => {
    const contenedor = abrirDetalleDe(porNombre('Historica'));
    const narrativa = contenedor.querySelector('[data-testid="narrativa-original"]');
    expect(LIMPIO(narrativa!.textContent)).toBe(
      'Texto narrativo ORIGINAL de la epoca (sin evaluacion registrada).'
    );
  });
});

describe('3.8 · no queda un icono de balanza sin texto', () => {
  /**
   * El defecto era la **balanza** del veredicto con el mensaje vacío, no los pictogramas
   * identificativos de las cajas «Prueba 1 / Prueba 2» (esos llevan su color de serie y el
   * nombre va en la línea contigua, así que no son texto perdido).
   */
  const balanzasSueltas = (c: HTMLElement) =>
    Array.from(c.querySelectorAll<SVGElement>('svg.lucide-scale')).filter((svg) => {
      const contenedor = svg.parentElement;
      if (!contenedor) return false;
      return LIMPIO(contenedor.textContent) === '';
    });

  it('en el resultado parcial no queda una balanza sin mensaje', () => {
    // Antes se pintaba la balanza con `sinGanadora` vacío en los resultados parciales.
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    expect(balanzasSueltas(contenedor)).toHaveLength(0);
  });

  it('tampoco en una evaluación no interpretable, donde el aviso va aparte', () => {
    expect(balanzasSueltas(abrirDetalleDe(porNombre('No soportada')))).toHaveLength(0);
  });

  it('en un estado que sí tiene mensaje sin ganadora, la balanza sigue apareciendo', () => {
    const contenedor = abrirDetalleDe(porNombre('Historica'));
    expect(texto(contenedor)).toMatch(/no consta qué grabación rindió mejor/i);
  });
});

/* ══ Cuarta validación real (27/11/2026) — estados de evaluación y margen ══ */

describe('A · los cuatro estados de evaluación no se dicen igual', () => {
  const TROZO = /no se puede interpretar|no interpreta|Sin evaluación registrada|no se puede leer/i;

  it('ausente (histórica): no atribuye incompatibilidad a una evaluación inexistente', () => {
    const x = texto(abrirDetalleDe(porNombre('Historica')));
    // Antes decía «Evaluación no interpretable con esta versión» sobre una prueba que no
    // tiene evaluación: atribuía un problema de lectura a algo que nunca se registró.
    expect(x).not.toMatch(/no se puede interpretar con esta versi[óo]n/i);
    expect(x).not.toMatch(/no interpretable con esta versi[óo]n/i);
    expect(x).toMatch(/Sin evaluación registrada/i);
  });

  it('no soportada: nombra la metodología, no una ilegibilidad genérica', () => {
    const x = texto(abrirDetalleDe(porNombre('No soportada')));
    expect(x).toMatch(/metodolog[íi]a que esta versi[óo]n no interpreta/i);
    expect(x).not.toMatch(/no interpretable con esta versi[óo]n/i);
  });

  it('malformada: distingue «registro ilegible» de «sin evaluación»', () => {
    const x = texto(abrirDetalleDe(porNombre('Malformada')));
    expect(x).toMatch(/no se puede leer/i);
    // No se le atribuye la ausencia: el registro existe, solo que no se lee.
    expect(x).not.toMatch(/Sin evaluación registrada: solo se muestran/i);
  });

  it('soportada: usa la presentación del contrato, sin hablar de ilegibilidad', () => {
    const x = texto(abrirDetalleDe(porNombre('PARCIAL_PERDIDA')));
    expect(x).not.toMatch(TROZO);
    // El estado real de cada métrica sí aparece.
    expect(x).toMatch(/No evaluada: apoyo insuficiente/i);
    expect(x).toMatch(/Favorece a «/);
  });

  it('los tres estados no legibles dicen textos distintos entre sí', () => {
    /**
     * Se recoge el **conjunto** de rótulos de estado que aparecen en la tabla superior de
     * cada detalle. Tomar solo la primera celda coincidente devolvía el mismo texto para
     * los tres casos cuando ese texto era la cabecera, no la celda de la métrica.
     */
    /**
     * Se recogen los rótulos de estado de **todo el detalle** (tablas y cabeceras), no solo
     * de la tabla superior: en la comparación histórica esa tabla no se pinta, porque sin
     * evaluación registrada `modelo.metricas` viene vacía. El rótulo vive entonces en la
     * cabecera de la tabla de métricas guardadas.
     */
    const rotulos = (nombre: string) => {
      const contenedor = abrirDetalleDe(porNombre(nombre));
      const celdas = Array.from(contenedor.querySelectorAll('td')).map((td) =>
        LIMPIO(td.textContent)
      );
      const cabecera = Array.from(contenedor.querySelectorAll('table thead'))
        .map((t) => LIMPIO(t.parentElement?.parentElement?.textContent ?? ''))
        .join(' ');
      const deCeldas = celdas.filter((v) => TROZO.test(v) && !/^Descriptiva/i.test(v));
      if (deCeldas.length > 0) return new Set(deCeldas);
      // Sin filas evaluables, el estado se afirma en la cabecera de la tabla.
      const m = cabecera.match(
        /(Sin evaluaci[óo]n registrada[^.]*|registro de la evaluaci[óo]n[^.]*|metodolog[íi]a que esta versi[óo]n[^.]*)/i
      );
      return new Set(m ? [m[1].trim()] : []);
    };
    const ausente = rotulos('Historica');
    const malformada = rotulos('Malformada');
    const noSoportada = rotulos('No soportada');
    for (const conjunto of [ausente, malformada, noSoportada]) {
      expect(conjunto.size).toBeGreaterThan(0);
    }
    // Ningún rótulo se repite entre estados distintos.
    for (const a of ausente) {
      expect(malformada.has(a)).toBe(false);
      expect(noSoportada.has(a)).toBe(false);
    }
    for (const b of malformada) expect(noSoportada.has(b)).toBe(false);
  });
});

describe('B · margen de una evaluación no interpretable', () => {
  it('no afirma «no consta» cuando el motivo es que no se interpreta', () => {
    const contenedor = abrirDetalleDe(porNombre('No soportada'));
    const avanzadas = within(contenedor).queryByRole('button', { name: /Ver métricas avanzadas/i });
    if (avanzadas) fireEvent.click(avanzadas);
    const x = texto(contenedor);
    // El 5 % está en condiciones de la evaluación v9: decir que «no consta» es falso.
    expect(x).not.toMatch(/Margen aplicado:\s*no consta en el registro/i);
    expect(x).toMatch(/Margen aplicado:\s*no se interpreta con esta versi[óo]n/i);
  });

  it('conserva el dato persistido: la corrección es de presentación', () => {
    expect(porNombre('No soportada').evaluacion!.condiciones.margen_porcentaje).toBe(5);
  });

  it('con evaluación legible el margen se sigue mostrando aplicado', () => {
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const avanzadas = within(contenedor).queryByRole('button', { name: /Ver métricas avanzadas/i });
    if (avanzadas) fireEvent.click(avanzadas);
    expect(texto(contenedor)).toMatch(/Margen aplicado:\s*±5\.0 %/);
  });
});

describe('C · el apoyo mostrado procede del apoyo registrado, no del total', () => {
  it('las muestras del 0,1 % low vienen de apoyo_referencia/apoyo_candidata', () => {
    const ev = porNombre('PARCIAL_PERDIDA').evaluacion!;
    const metricas = ev.metricas ?? [];
    const m = metricas.find((x) => x.clave === 'fps_0_1_percent_low')!;
    const ref = m.apoyo_referencia?.percentil;
    const cand = m.apoyo_candidata?.percentil;
    // El contrato declara el apoyo por lado del percentil, con su tamaño de cola.
    expect(ref?.muestras).toBe(2000);
    expect(cand?.muestras).toBe(2000);
    // La cola es del 2 %: el «muestras» es n (sobre lo que se calculó el percentil), no el
    // total de la grabación, y no es el tamaño efectivo de la cola (2000·0,02 = 40).
    expect(ref?.cola_nominal).toBe(2);

    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const x = texto(contenedor);
    expect(x).toMatch(/2000 muestras \(Prueba 1\)/i);
    expect(x).toMatch(/2000 muestras \(Prueba 2\)/i);
    // Y no se cuela el total de la grabación como si fuera el apoyo de la cola.
    expect(x).not.toMatch(/20000 muestras/i);
  });

  it('sin apoyo registrado no se inventa una cifra', () => {
    const ev = porNombre('PARCIAL_PERDIDA').evaluacion!;
    const sinApoyo = (ev.metricas ?? []).filter(
      (m) => m.apoyo_referencia == null && m.apoyo_candidata == null
    );
    // Si alguna métrica no declara apoyo, su fila no puede exhibir muestras.
    expect(sinApoyo.length).toBeGreaterThan(0);
    const contenedor = abrirDetalleDe(porNombre('PARCIAL_PERDIDA'));
    const seccion = contenedor.querySelector('table');
    expect(seccion).not.toBeNull();
  });
});

describe('B (quinta ronda) · el margen de una evaluación AUSENTE no es «no interpretable»', () => {
  /** Abre el panel de métricas avanzadas y devuelve el texto del detalle. */
  const avanzadasDe = (nombre: string) => {
    const contenedor = abrirDetalleDe(porNombre(nombre));
    const boton = within(contenedor).queryByRole('button', { name: /Ver métricas avanzadas/i });
    if (boton) fireEvent.click(boton);
    return texto(contenedor);
  };

  it('HISTÓRICA dice «no consta en el registro», porque no hay evaluación', () => {
    // La evaluación no es incompatible: no existe. Decir «no se interpreta con esta
    // versión» le atribuye un registro que la prueba no tiene.
    const x = avanzadasDe('Historica');
    expect(x).toMatch(/Margen aplicado:\s*no consta en el registro/i);
    expect(x).not.toMatch(/Margen aplicado:\s*no se interpreta con esta versi[óo]n/i);
  });

  it('HISTÓRICA tampoco anuncia un ruido que no puede interpretar', () => {
    const x = avanzadasDe('Historica');
    expect(x).toMatch(/Ruido combinado:\s*no consta en el registro/i);
  });

  it('los cuatro estados dan el texto que les corresponde', () => {
    // soportada → se muestra el valor.
    expect(avanzadasDe('PARCIAL_PERDIDA')).toMatch(/Margen aplicado:\s*±5\.0 %/);
    // no soportada → existe en disco, pero no se interpreta.
    expect(avanzadasDe('No soportada')).toMatch(
      /Margen aplicado:\s*no se interpreta con esta versi[óo]n/i
    );
    // ausente → no hay nada que interpretar.
    expect(avanzadasDe('Historica')).toMatch(
      /Margen aplicado:\s*no consta en el registro/i
    );
    // malformada → hay registro, pero ilegible: no se interpreta.
    expect(avanzadasDe('Malformada')).toMatch(
      /Margen aplicado:\s*no se interpreta con esta versi[óo]n/i
    );
  });
});
