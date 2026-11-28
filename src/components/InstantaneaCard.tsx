import { useEffect, useId, useState } from 'react';
import { ChevronDown, ClipboardCopy, Hourglass, Info, MonitorPlay } from 'lucide-react';
import type { Benchmark, ConProcedencia, Procedencia, SesionDeEjecucion } from '../lib/types';
import { etiquetaPrueba } from '../lib/pruebaLabel';
import { api } from '../lib/api';

/**
 * Configuración histórica de una prueba (subfase 5.2).
 *
 * ## Qué es
 *
 * La instantánea que se guardó **con** el benchmark: comando confirmado, perfil, opciones
 * de Steam, entorno y la procedencia de cada dato. No se recalcula nunca, así que
 * renombrar el perfil, cambiar Steam o actualizar el hardware no la tocan.
 *
 * ## Por qué desplegable
 *
 * Son diez líneas de datos técnicos que la mayoría de las veces no se miran. Plegadas por
 * defecto no ocupan nada; abiertas quedan a mano para cuando hay que auditar por qué una
 * prueba salió como salió.
 *
 * ## Procedencia
 *
 * Cada valor dice de dónde salió. `Leído de Steam` y `motor de Thoriapp` **no** significan
 * «verificado»: describen lo que esos sitios decían cuando se capturó, no lo que se ejecutó
 * durante la grabación. Esa distinción es la razón de que la tarjeta no diga «configuración
 * usada» en ningún momento.
 */

const ETIQUETA_PROCEDENCIA: Record<Procedencia, string> = {
  motor_rust: 'Motor de Thoriapp',
  configurado_en_thoriapp: 'Configurado en Thoriapp',
  leido_de_steam: 'Leído de Steam',
  registrado_por_mangohud: 'Registrado por MangoHud',
  detectado_en_sistema: 'Detectado en el sistema',
  declarado_por_usuario: 'Declarado por ti',
  verificado_en_ejecucion: 'Verificado en ejecución',
  observado_en_proceso: 'Observado en el proceso',
  confirmado_por_usuario: 'Confirmado por ti',
  inferido: 'Inferido por el nombre',
  desconocido: 'Desconocido',
};

/** Del resumen de MangoHud, lo que se muestra de un vistazo. El resto queda guardado. */
const RESUMEN_VISIBLE = ['Average FPS', '1% Min FPS', '0.1% Min FPS', '97% Percentile FPS'];

/** Color del distintivo según cuánto se puede confiar en la procedencia del dato. */
function tonoDe(p: Procedencia): string {
  if (p === 'verificado_en_ejecucion') return 'text-[#38C9A3] border-[#38C9A3]/40';
  if (p === 'desconocido') return 'text-[var(--text-dim)] border-[var(--border)]';
  return 'text-[var(--text-muted)] border-[var(--border)]';
}

function DistintivoProcedencia({ procedencia }: { procedencia: Procedencia }) {
  return (
    <span
      className={`shrink-0 px-1.5 py-0.5 rounded border text-[10px] font-medium ${tonoDe(procedencia)}`}
    >
      {ETIQUETA_PROCEDENCIA[procedencia]}
    </span>
  );
}

/** Una fila «etiqueta: valor» con la procedencia a la derecha. */
function Fila({
  etiqueta,
  valor,
  vacio = 'No consta',
  sufijo = '',
  nota,
}: {
  etiqueta: string;
  valor: ConProcedencia<string | number | boolean> | null | undefined;
  vacio?: string;
  /** Unidad que se añade al valor (no se mete en el dato guardado). */
  sufijo?: string;
  /** Aclaración bajo la etiqueta, para valores que se malinterpretarían solos. */
  nota?: string;
}) {
  return (
    <div className="flex items-start justify-between gap-3 py-1.5 min-h-[30px]">
      <span className="shrink-0 text-[var(--text-dim)]">
        {etiqueta}
        {nota && <span className="block text-[10px] opacity-70">{nota}</span>}
      </span>
      <span className="flex items-center gap-2 min-w-0 justify-end">
        {valor ? (
          <>
            <span className="text-[var(--text)] text-right break-all">
              {typeof valor.valor === 'boolean'
                ? valor.valor
                  ? 'Sí'
                  : 'No'
                : typeof valor.valor === 'number'
                  ? // Un decimal para GB y ms: el CSV no tiene más precisión real.
                    `${valor.valor}${sufijo}`
                  : String(valor.valor)}
            </span>
            <DistintivoProcedencia procedencia={valor.procedencia} />
          </>
        ) : (
          // Se dice «no consta», no se rellena con el valor actual: eso convertiría un
          // hueco histórico en una afirmación falsa.
          <span className="text-[var(--text-dim)] italic">{vacio}</span>
        )}
      </span>
    </div>
  );
}

export interface InstantaneaCardProps {
  bench: Benchmark | null | undefined;
  /** Copia un texto al portapapeles. Si falta, no se ofrece el botón. */
  onCopy?: (texto: string) => void;
}

export function InstantaneaCard({ bench, onCopy }: InstantaneaCardProps) {
  const [abierta, setAbierta] = useState(false);
  const [sesion, setSesion] = useState<SesionDeEjecucion | null>(null);
  const [sesionBuscada, setSesionBuscada] = useState<string | null>(null);
  const id = useId();
  const snap = bench?.snapshot ?? null;
  const benchId = bench?.id ?? null;

  // La sesión se pide solo al desplegar: es un dato de apoyo y no debe costar nada al
  // pintar la lista. Si la sesión se borró, `null` es una respuesta válida y no un fallo:
  // la instantánea del benchmark sigue explicando la prueba por sí sola.
  useEffect(() => {
    if (!abierta || !benchId || sesionBuscada === benchId) return;
    let vigente = true;
    api
      .sesionDeBenchmark(benchId)
      .then((s) => {
        if (vigente) setSesion(s);
      })
      .catch(() => {
        if (vigente) setSesion(null);
      })
      .finally(() => {
        if (vigente) setSesionBuscada(benchId);
      });
    return () => {
      vigente = false;
    };
  }, [abierta, benchId, sesionBuscada]);

  if (!bench) return null;

  if (!snap) {
    // F: la ausencia de instantánea **no demuestra** que la prueba sea anterior a que
    // Thoriapp registrara la configuración. Un archivo borrado, una escritura a medias o un
    // registro ilegible producen el mismo vacío. Se dice lo único que consta: que no está.
    return (
      <article className="p-4 rounded-xl bg-[var(--surface)] border border-[var(--border)]">
        <p className="text-sm font-semibold text-[var(--text)] mb-1">
          Instantánea no disponible
        </p>
        <p className="text-xs text-[var(--text-muted)] leading-relaxed">
          De {etiquetaPrueba(bench)} no consta la configuración con la que se hizo la prueba.
          No se completa con los datos actuales del juego ni del perfil: eso diría cómo está
          todo hoy, no cómo se hizo la prueba.
        </p>
      </article>
    );
  }

  const { configuracion: cfg, entorno, identidad } = snap;

  // Equipo que registró MangoHud durante la grabación. `null`/ausente en los benchmarks
  // anteriores a este incremento: entonces las condiciones quedan desconocidas.
  const grabacion = snap.hardware_grabacion ?? null;

  // Aviso informativo si el equipo de la grabación y el de ahora no coinciden en lo que
  // ambos conocen. Nunca modifica ninguno de los dos bloques: solo lo señala.
  const difiereDelActual = (() => {
    if (!grabacion) return false;
    const pares: Array<[string | null | undefined, string | null | undefined]> = [
      [grabacion.cpu?.valor, entorno.cpu?.valor],
      [grabacion.gpu?.valor, entorno.gpu?.valor],
      [grabacion.kernel?.valor, entorno.kernel?.valor],
    ];
    return pares.some(
      ([deLaGrabacion, actual]) =>
        !!deLaGrabacion && !!actual && deLaGrabacion.trim() !== actual.trim()
    );
  })();

  return (
    <article className="rounded-xl bg-[var(--surface)] border border-[var(--border)] overflow-hidden">
      <button
        type="button"
        onClick={() => setAbierta((v) => !v)}
        aria-expanded={abierta}
        aria-controls={`inst-${id}`}
        className="w-full flex items-center justify-between gap-3 px-4 py-3 text-left hover:bg-[var(--bg)] transition-colors"
      >
        <span className="min-w-0">
          <span className="block text-sm font-bold text-[var(--text)]">
            Configuración registrada de {etiquetaPrueba(bench)}
          </span>
          <span className="block text-xs text-[var(--text-muted)] mt-0.5">
            {cfg.perfil_nombre
              ? `Perfil «${cfg.perfil_nombre.valor}»`
              : 'Sin perfil cargado'}
            {' · '}
            {identidad.appid_real != null
              ? `AppID ${identidad.appid_real}${
                  identidad.appid_procedencia === 'observado_en_proceso'
                    ? ' (observado en el proceso)'
                    : ''
                }`
              : 'AppID desconocido'}
            {identidad.appid_real != null &&
            identidad.appid_procedencia === 'inferido'
              ? ' (inferido por nombre)'
              : ''}
          </span>
        </span>
        <ChevronDown
          className={`shrink-0 w-4 h-4 text-[var(--text-muted)] transition-transform ${abierta ? 'rotate-180' : ''}`}
        />
      </button>

      {abierta && (
        <div
          id={`inst-${id}`}
          className="px-4 pb-4 border-t border-[var(--border)] text-xs divide-y divide-[var(--border)]"
        >
          {/* Comando: literal, tal como lo confirmó el motor. */}
          <section className="py-3">
            <div className="flex items-center justify-between gap-2 mb-1.5">
              <span className="font-semibold text-[var(--text)]">
                Comando confirmado por Thoriapp
              </span>
              {cfg.comando_confirmado && onCopy && (
                <button
                  type="button"
                  onClick={() => onCopy(cfg.comando_confirmado!.valor)}
                  className="flex items-center gap-1 text-[var(--text-muted)] hover:text-[var(--text)]"
                  aria-label="Copiar el comando registrado"
                >
                  <ClipboardCopy className="w-3.5 h-3.5" />
                  Copiar
                </button>
              )}
            </div>
            {cfg.comando_confirmado ? (
              <>
                <code className="block font-mono text-[var(--text)] break-all leading-relaxed bg-[var(--bg)] rounded p-2">
                  {cfg.comando_confirmado.valor || '(vacío)'}
                </code>
                <p className="mt-1.5 text-[var(--text-dim)]">
                  Es el texto que Thoriapp proponía. No prueba que Steam lo ejecutara: eso lo
                  dice la línea de Steam.
                </p>
              </>
            ) : (
              <p className="text-[var(--text-dim)] italic">
                No se registró el comando de este juego.
              </p>
            )}
          </section>

          <section className="py-3">
            <Fila etiqueta="Opciones de Steam" valor={cfg.opciones_steam} vacio="No se pudieron leer" />
            <Fila etiqueta="Modo Proton" valor={cfg.modo_proton} vacio="No consta" />
            <Fila
              etiqueta="Cambios sin guardar"
              valor={
                cfg.cambios_sin_guardar == null
                  ? null
                  : { valor: cfg.cambios_sin_guardar, procedencia: 'configurado_en_thoriapp' }
              }
              vacio="No consta"
            />
            {cfg.ejecucion_verificada ? (
              <Fila etiqueta="Ejecución real" valor={cfg.ejecucion_verificada} />
            ) : (
              // Distinción del punto 10: tener configurado Proton no demuestra que el juego
              // corriera con Proton.
              <p className="py-1.5 text-[var(--text-dim)] leading-relaxed">
                Modo de ejecución real: <span className="italic">no verificado</span>. Se
                registró la configuración, no lo que el proceso usó al arrancar.
              </p>
            )}
          </section>

          {/* Equipo registrado por MangoHud DURANTE la grabación. Es la fuente que
              describe las condiciones de la prueba; el bloque de abajo es el equipo de
              hoy. Van separados a propósito, porque pueden ser distintos. */}
          <section className="py-3 border-t border-[var(--border)]/50">
            <h4 className="text-[11px] font-semibold text-[var(--text-muted)] uppercase tracking-wide mb-1">
              Equipo durante la grabación
            </h4>
            {grabacion ? (
              <>
                <Fila etiqueta="GPU" valor={grabacion.gpu} />
                <Fila etiqueta="CPU" valor={grabacion.cpu} />
                <Fila etiqueta="RAM" valor={grabacion.ram_gib} sufijo=" GiB" />
                <Fila etiqueta="Kernel" valor={grabacion.kernel} />
                <Fila etiqueta="Driver" valor={grabacion.driver} vacio="No registrado" />
                <Fila
                  etiqueta="Entorno de ejecución"
                  valor={grabacion.os}
                  nota="Puede ser el runtime de Steam, no tu distribución"
                />
                <Fila etiqueta="Planificador de CPU" valor={grabacion.cpuscheduler} />
                <Fila
                  etiqueta="Muestreo"
                  valor={
                    grabacion.intervalo_muestreo_ms
                      ? {
                          valor: `${grabacion.intervalo_muestreo_ms.valor.toFixed(0)} ms`,
                          procedencia: grabacion.intervalo_muestreo_ms.procedencia,
                        }
                      : null
                  }
                  vacio="No registrado"
                />
                {difiereDelActual && (
                  <p className="mt-1.5 flex items-start gap-1.5 text-[var(--text-dim)] leading-relaxed">
                    <Info className="shrink-0 w-3.5 h-3.5 mt-0.5" />
                    <span>
                      El equipo de la grabación no coincide del todo con el de ahora. Se
                      muestran los dos por separado: la prueba se hizo con el de arriba.
                    </span>
                  </p>
                )}
              </>
            ) : (
              <p className="py-1.5 text-[var(--text-dim)] leading-relaxed">
                La grabación no traía datos del equipo. Queda{' '}
                <span className="italic">desconocido</span>, aunque hoy se detecte uno: no se
                rellena con el equipo actual.
              </p>
            )}
          </section>

          <section className="py-3 border-t border-[var(--border)]/50">
            <h4 className="text-[11px] font-semibold text-[var(--text-muted)] uppercase tracking-wide mb-1">
              Equipo al analizar
            </h4>
            <Fila etiqueta="GPU" valor={entorno.gpu} />
            <Fila etiqueta="CPU" valor={entorno.cpu} />
            <Fila etiqueta="RAM (GB)" valor={entorno.ram_gb} />
            <Fila etiqueta="Kernel" valor={entorno.kernel} />
            <Fila etiqueta="Mesa" valor={entorno.mesa} />
            <Fila etiqueta="Driver" valor={entorno.driver} vacio="No registrado" />
            <Fila
              etiqueta="Proton (instalado/seleccionado)"
              valor={entorno.proton_seleccionado}
              vacio="No registrado"
            />
            <Fila etiqueta="BuildID del juego" valor={entorno.build_id} vacio="No disponible" />
            <Fila
              etiqueta="Intervalo de MangoHud"
              valor={
                entorno.log_interval_ms
                  ? {
                      valor: `${entorno.log_interval_ms.valor} ms`,
                      procedencia: entorno.log_interval_ms.procedencia,
                    }
                  : null
              }
              vacio="No registrado"
            />
          </section>

          {/* Resumen que calculó MangoHud. Se muestra como fuente independiente, con su
              metodología, porque sus números no son comparables con los de Thoriapp: la
              media es armónica, el «0.1% low» es el peor fotograma y no se descarta el
              arranque. Ver docs/mangohud-summary.md. */}
          {snap.resumen_mangohud && snap.resumen_mangohud.valores.length > 0 && (
            <section className="py-3 border-t border-[var(--border)]/50">
              <h4 className="text-[11px] font-semibold text-[var(--text-muted)] uppercase tracking-wide mb-1">
                Resumen de MangoHud
              </h4>
              <p className="text-[var(--text-dim)] leading-relaxed mb-1.5">
                Lo calculó MangoHud con <strong>su</strong> método, sobre la grabación
                completa. No son las cifras de Thoriapp ni se usan para comparar.
              </p>
              <div className="space-y-0.5">
                {RESUMEN_VISIBLE.map((nombre) => {
                  const v = snap.resumen_mangohud!.valores.find(
                    ([n]) => n.trim().toLowerCase() === nombre.toLowerCase()
                  );
                  if (!v) return null;
                  return (
                    <div key={nombre} className="flex justify-between gap-3 py-0.5">
                      <span className="text-[var(--text-dim)]">{nombre}</span>
                      <span className="text-[var(--text)]">{v[1]}</span>
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          <section className="py-3 text-[var(--text-dim)] leading-relaxed">
            <p className="flex items-start gap-1.5">
              <Hourglass className="shrink-0 w-3.5 h-3.5 mt-0.5" />
              <span>
                {snap.posterior_a_la_grabacion ? (
                  <>
                    La configuración se capturó al guardar la prueba, <strong>después</strong>{' '}
                    de que terminara la grabación ({snap.capturado_en}). No atestigua qué
                    estuvo activo durante ella.
                  </>
                ) : (
                  <>Capturada durante la ejecución: {snap.capturado_en}.</>
                )}
              </span>
            </p>
          </section>

          {/* Sesión de ejecución observada (subfase 5.3). Compacto: una línea de resumen y
              el detalle solo si lo hubo. El objetivo es que se vea de un vistazo si la
              prueba tiene proceso detrás y, si no, qué impidió saberlo. */}
          <section className="py-3 border-t border-[var(--border)]/50">
            <h4 className="text-[11px] font-semibold text-[var(--text-muted)] uppercase tracking-wide mb-1 flex items-center gap-1.5">
              <MonitorPlay className="w-3.5 h-3.5" />
              Sesión de ejecución
            </h4>
            {sesion ? (
              <>
                <p className="py-1 text-[var(--text)] leading-relaxed">
                  {sesion.tipo === 'observada' ? (
                    <>
                      Se vio el juego en ejecución
                      {sesion.proceso?.ejecutable ? ` (${sesion.proceso.ejecutable})` : ''} desde{' '}
                      {sesion.abierta_en}
                      {sesion.cerrada_en ? ` hasta ${sesion.cerrada_en}` : ' (sigue abierto)'}.
                    </>
                  ) : (
                    <>
                      Grabaciones del mismo juego agrupadas{' '}
                      <span className="italic">sin proceso identificado</span>.
                    </>
                  )}
                </p>
                {sesion.identidad.evidencia.length > 0 && (
                  <p className="py-1 text-[var(--text-dim)] leading-relaxed">
                    {sesion.identidad.evidencia.join(' · ')}
                  </p>
                )}
                {sesion.proceso?.bajo_proton === true && (
                  <p className="py-1 text-[var(--text-dim)] leading-relaxed">
                    El proceso iba bajo Proton. Eso <span className="italic">no</span> confirma
                    que cada ajuste configurado estuviera activo.
                  </p>
                )}
                {/* La configuración de la sesión se toma cuando el observador ve el proceso,
                    y eso es siempre después de que arrancara. La marca ya venía del backend;
                    faltaba decirlo aquí, así que una captura de las 14:20:30 se leía como si
                    fuera la del arranque de las 14:16:01. */}
                {sesion.configuracion_inicial?.posterior_al_arranque === true && (
                  <p className="py-1 text-[var(--text-dim)] leading-relaxed">
                    La configuración de esta sesión se capturó{' '}
                    {sesion.configuracion_inicial.capturado_en},{' '}
                    <strong>después</strong> de que el juego empezara ({sesion.abierta_en}). Se
                    leyó con el sondeo del observador, no en el arranque: no atestigua qué
                    estaba puesto al empezar a jugar.
                  </p>
                )}
              </>
            ) : (
              <p className="py-1 text-[var(--text-dim)] leading-relaxed">
                No consta una ejecución observada de esta grabación. Se guardó con la
                configuración y lo que registró MangoHud, que es suficiente para leerla; lo
                que no hay es prueba del proceso.
              </p>
            )}
          </section>
        </div>
      )}
    </article>
  );
}
