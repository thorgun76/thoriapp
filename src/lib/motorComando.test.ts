import { describe, expect, it } from 'vitest';
import launchCommandRs from '../../src-tauri/src/launch_command.rs?raw';
import type { AnalisisComando, EstadoAjuste, SegmentoComando } from './motorComando';
import { conPedidos, esAccionable, estaEncendido, estadoDe } from './motorComando';

/**
 * El espejo de tipos (`motorComando.ts`) no puede desincronizarse del motor.
 *
 * TypeScript no mira el Rust: si allí cambia una etiqueta serde, aquí el tipo sigue
 * compilando y la UI lee `undefined` sin quejarse. Estos tests leen el archivo Rust y
 * comprueban que los nombres siguen siendo los mismos.
 */
describe('el espejo de tipos coincide con launch_command.rs', () => {
  it('Analisis tiene los campos que espera el frontend', () => {
    const m = launchCommandRs.match(/pub struct Analisis \{([\s\S]*?)\n\}/);
    expect(m, 'no se encontró `pub struct Analisis`').not.toBeNull();
    const campos = [...m![1].matchAll(/^\s*pub (\w+):/gm)].map((x) => x[1]);

    const esperados: (keyof AnalisisComando)[] = [
      'original',
      'segmentos',
      'avisos',
      'editable',
      'reconocidos',
      'conservados',
      'sintaxis_valida',
      'motivo_sintaxis',
    ];
    for (const campo of esperados) {
      expect(campos, `falta \`${campo}\` en el espejo de Analisis`).toContain(campo);
    }
  });

  it('el motivo de conservación vive en el campo `razon`, no en `motivo`', () => {
    // El fallo de la pantalla negra: el enum `Gestion` serializa `Conservado { razon }`, así
    // que el JSON es `{"tipo":"conservado","razon":{"motivo":…}}`. El espejo TS nombraba el
    // campo `motivo`, y leer la ruta equivocada reventaba el render.
    const m = launchCommandRs.match(/pub enum Gestion \{([\s\S]*?)\n\}/);
    expect(m, 'no se encontró `pub enum Gestion`').not.toBeNull();
    expect(m![1]).toMatch(/Conservado\s*\{\s*razon\s*:/);
    expect(m![1]).not.toMatch(/Conservado\s*\{\s*motivo\s*:/);
  });

  it('las etiquetas de los avisos son las que se leen en TS', () => {
    // Si alguien añade una variante al enum `Aviso`, la UI la ignoraría en silencio.
    const m = launchCommandRs.match(/pub enum Aviso \{([\s\S]*?)\n\}/);
    expect(m, 'no se encontró `pub enum Aviso`').not.toBeNull();
    const variantes = [...m![1].matchAll(/^\s{4}(\w+)[\s,{]/gm)].map((x) =>
      // snake_case, como `rename_all = "snake_case"`.
      x[1].replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase(),
    );

    const conocidas = [
      'sin_marcador',
      'marcador_repetido',
      'comilla_sin_cerrar',
      'duplicado',
      'contradiccion',
      'ambiguo',
      'mismo_nombre_distinto_valor',
    ];
    expect(variantes.sort()).toEqual([...conocidas].sort());
  });

  it('las cinco situaciones de un ajuste son las cinco del motor', () => {
    const m = launchCommandRs.match(/pub enum Situacion \{([\s\S]*?)\n\}/);
    expect(m, 'no se encontró `pub enum Situacion`').not.toBeNull();
    const variantes = [...m![1].matchAll(/^\s{4}(\w+)[\s,{]/gm)].map((x) =>
      x[1].replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase(),
    );

    expect(variantes.sort()).toEqual([
      'activo',
      'ambiguo',
      'conflicto',
      'inactivo',
      'parcial',
    ]);
    // Y el enum se serializa con la etiqueta `estado`, no `tipo`.
    expect(launchCommandRs).toContain('#[serde(tag = "estado", rename_all = "snake_case")]');
  });

  it('el estado lleva el índice dentro, no se deduce por la posición', () => {
    // Esto era un fallo real: los estados se alineaban por orden con el catálogo, así que
    // reordenar `rules.params` los hacía apuntar a otro ajuste sin que nada fallara.
    const m = launchCommandRs.match(/pub struct EstadoAjuste \{([\s\S]*?)\n\}/);
    expect(m, 'no se encontró `pub struct EstadoAjuste`').not.toBeNull();
    expect(m![1]).toContain('pub indice: usize');
  });

  it('el error del motor se serializa como `tipo` con variantes snake_case', () => {
    expect(launchCommandRs).toMatch(
      /#\[serde\(tag = "tipo", rename_all = "snake_case"\)\]\s*pub enum ErrorAnalisis/,
    );
    for (const variante of [
      'span_fuera_de_rango',
      'span_en_mitad_de_caracter',
      'spans_desordenados',
      'texto_no_coincide',
      'indice_desconocido',
    ]) {
      // Se comprueba en snake_case porque el serde lo renombra.
      const camel = variante.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
      const pascal = camel[0].toUpperCase() + camel.slice(1);
      expect(launchCommandRs, `falta la variante ${pascal}`).toContain(`${pascal} {`);
    }
  });
});

describe('estadoDe: el estado se recibe, no se deduce', () => {
  const estados: EstadoAjuste[] = [
    { indice: 0, valor: 'mangohud', kind: 'arg', estado: 'inactivo' },
    { indice: 1, valor: 'game-performance', kind: 'wrapper', estado: 'activo', span: { inicio: 0, fin: 8 } },
    {
      indice: 2,
      valor: 'RADV_PERFTEST=sam',
      kind: 'env',
      estado: 'parcial',
      gestionadas: [{ inicio: 0, fin: 12 }],
      conservadas: [{ inicio: 13, fin: 25 }],
    },
    { indice: 3, valor: 'FOO=1', kind: 'env', estado: 'ambiguo', spans: [], candidatos: [3, 4] },
    { indice: 4, valor: 'BAR=2', kind: 'env', estado: 'conflicto', spans: [], en_comando: ['X=2'] },
  ];

  it('encuentra el estado por índice y no por posición', () => {
    expect(estadoDe(estados, 3)?.estado).toBe('ambiguo');
    expect(estadoDe(estados, 99)).toBeNull();
  });

  it('solo `activo` se pinta encendido', () => {
    expect(estaEncendido(estadoDe(estados, 1))).toBe(true);
    // Este era el fallo que había que evitar: `parcial` NO es encendido pleno.
    expect(estaEncendido(estadoDe(estados, 2))).toBe(false);
    expect(estaEncendido(estadoDe(estados, 0))).toBe(false);
  });

  it('un ajuste ambiguo no es accionable', () => {
    // Thoriapp no sabe a qué ajuste corresponde lo que hay en el comando: activarlo sería
    // adivinar.
    expect(esAccionable(estadoDe(estados, 3))).toBe(false);
    expect(esAccionable(estadoDe(estados, 2))).toBe(true);
    expect(esAccionable(null)).toBe(true);
  });
});

describe('contrato JSON con Rust (regresión pantalla negra)', () => {
  /**
   * Este test fija la forma **exacta** en que Rust serializa un segmento.
   *
   * Faltaba: los 864 tests pasaban con un contrato roto, porque todos construían el objeto
   * a mano con la forma que el frontend esperaba. El motor usa `Gestion::Conservado {
   * razon }`, así que el motivo viaja en `gestion.razon.motivo`, no en `gestion.motivo`.
   * Leer la ruta equivocada lanzaba `undefined is not an object` al pintar la ficha y dejaba
   * la ventana en negro.
   */
  const SEGMENTO_DESCONOCIDO_REAL = {
    span: { inicio: 0, fin: 17 },
    texto: 'RADV_PERFTEST=sam',
    valor: 'RADV_PERFTEST=sam',
    rol: 'env',
    gestion: { tipo: 'conservado', razon: { motivo: 'desconocido' } },
    cerrado: true,
  } as const;

  it('el motivo de un conservado viaja en gestion.razon.motivo', () => {
    const gestion = SEGMENTO_DESCONOCIDO_REAL.gestion;
    expect(gestion.tipo).toBe('conservado');
    // Lo que el componente lee de verdad:
    expect(gestion.razon.motivo).toBe('desconocido');
    // Y lo que NO existe (la ruta que provocaba el fallo):
    expect((gestion as Record<string, unknown>).motivo).toBeUndefined();
  });

  it('el tipo TS acepta la forma real de Rust sin conversión', () => {
    const seg: SegmentoComando = SEGMENTO_DESCONOCIDO_REAL as SegmentoComando;
    expect(seg.gestion.tipo).toBe('conservado');
  });
});

describe('conPedidos: adelanto del toggle mientras el motor responde', () => {
  const activo = (indice: number, valor: string): EstadoAjuste => ({
    indice,
    valor,
    kind: 'env',
    estado: 'activo',
  });
  const inactivo = (indice: number, valor: string): EstadoAjuste => ({
    indice,
    valor,
    kind: 'env',
    estado: 'inactivo',
  });

  it('sin nada en vuelo devuelve exactamente lo confirmado', () => {
    const estados = [inactivo(0, 'A=1'), activo(1, 'B=1')];

    expect(conPedidos(estados, null)).toBe(estados);
  });

  it('un ajuste pedido se pinta encendido sin esperar al motor', () => {
    // Es el pestañeo: el toggle no se movía hasta volver el IPC.
    const r = conPedidos([inactivo(0, 'A=1')], new Set([0]));

    expect(r[0].estado).toBe('activo');
  });

  it('un ajuste pedido para apagar se pinta apagado', () => {
    const r = conPedidos([activo(0, 'A=1')], new Set());

    expect(r[0].estado).toBe('inactivo');
  });

  it('el conjunto pedido es el estado deseado completo, no un delta', () => {
    // `Set([1])` significa «quiero el 1 encendido y el 0 apagado», que es como lo manda
    // el motor. Por eso el 0 se pinta apagado aunque estuviera confirmado como activo.
    const r = conPedidos([activo(0, 'A=1'), inactivo(1, 'B=1')], new Set([1]));

    expect(r[0].estado).toBe('inactivo');
    expect(r[1].estado).toBe('activo');
  });

  it('conserva el valor y el tipo de cada ajuste', () => {
    const r = conPedidos([inactivo(3, 'PROTON_USE_NTSYNC=1')], new Set([3]));

    expect(r[0].valor).toBe('PROTON_USE_NTSYNC=1');
    expect(r[0].kind).toBe('env');
    expect(r[0].indice).toBe(3);
  });
});
