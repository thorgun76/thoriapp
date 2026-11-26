import {
  AlertCircle,
  AlertTriangle,
  Check,
  ChevronDown,
  CloudOff,
  Copy,
  History,
  Info,
  Loader2,
  Pencil,
  RotateCcw,
  Star,
  Terminal,
} from 'lucide-react';
import { useState } from 'react';
import type { Procedencia } from '../hooks/useComando';
import type { FalloMotor } from '../hooks/useComando';
import { MenuRestablecer, type AccionRestablecer } from './MenuRestablecer';
import type { GameInfo, OpcionesSteam, RuntimeEnv } from '../lib/types';
import { preflight, type PreflightCheck } from '../lib/preflight';
import { AppidPicker } from './AppidPicker';
import { AvisoSteamDesincronizado } from './AvisoSteamDesincronizado';
import { GameBadges } from './GameBadges';

interface CommandBoxProps {
  /**
   * El usuario está preparando el comando para Proton, aunque el juego tenga build nativo.
   *
   * Es distinto de «el juego es de Proton» (dato de Steam) y de «Steam va a lanzarlo con
   * Proton» (no lo sabe Thoriapp: eso lo decide el usuario en Steam). Mezclar los tres hacía
   * que la caja dijera «Linux nativo» con Proton activado y, a la vez, avisara de que las
   * variables de Proton no hacen nada.
   */
  preparandoProton?: boolean;
  gameName: string;
  launchCommand: string;
  activeCount: number;
  /**
   * Qué lleva el comando ahora mismo, para resumirlo en chips debajo del código.
   * `{ value, kind }` con `kind` en `env` (variable), `wrapper` (envoltorio como
   * gamemoderun/mangohud) y `arg` (bandera del juego).
   */
  activeParts?: { value: string; kind: 'env' | 'wrapper' | 'arg' }[];
  copied: boolean;
  manualEditing: boolean;
  /** Comando confirmado por el motor. Es el punto de partida del editor. */
  manualCommand: string;
  /**
   * Problema detectado en el comando manual (vacío si está bien).
   *
   * Un comando manual puede llegar de un perfil importado, así que no se da por
   * bueno: si lleva caracteres que encadenan órdenes o un ejecutable que no es un
   * lanzador conocido, se muestra el motivo y se bloquea copiarlo.
   */
  manualCommandError?: string;
  info: GameInfo;
  /**
   * AppID de Steam, solo para enlazar la etiqueta de ProtonDB con el análisis del
   * juego. Sin él la etiqueta se pinta igual, pero sin acción.
   */
  appid?: number;
  /**
   * Vincula el juego con un AppID de Steam elegido a mano.
   *
   * Sin esta función la etiqueta «sin datos» no se puede pulsar; con ella, la
   * app ofrece buscar el juego cuando no lo reconoce por su nombre.
   */
  onLinkAppid?: (appid: number) => void;
  /** Estado real de la máquina; `null` mientras carga (no se muestran avisos). */
  env: RuntimeEnv | null;
  /** Launcher del juego (`steam`, `lutris`…) para avisos de sandbox. */
  launcher?: string;
  /**
   * Anti-cheat denegado: **no hay comando**. La caja deja de ofrecer copiar,
   * guardar o editar, y explica por qué. El texto llega ya montado en
   * `launchCommand`/`blockedReason` para no duplicar el criterio de `isDenied`.
   */
  blocked?: boolean;
  /** Motivo del bloqueo (mismo texto que usa el portapapeles). */
  blockedReason?: string;
  /** De dónde sale el comando que se está viendo. */
  procedencia?: Procedencia;
  /**
   * Nombre del perfil cargado, si el comando viene de uno.
   *
   * Se dice «cargado», nunca «activo»: Steam no ha visto esto todavía. Sin el nombre, el
   * usuario solo sabría que hay «un perfil» y no cuál.
   */
  perfilCargado?: string | null;
  /** Hay cambios sin guardar respecto a ese perfil. */
  cambiosSinGuardar?: boolean;
  /** Ajustes reconocidos que están puestos, para los chips. */
  ajustesActivos?: { valor: string; estado: string }[];
  /** Comando importado original, para el desplegable. `null` si no se importó nada. */
  comandoImportado?: string | null;
  /** Relación con Steam. `null` = no se sabe (no se afirma nada). */
  sincronizacion?:
    | { tipo: 'sincronizado' }
    | { tipo: 'local' }
    | { tipo: 'steam_cambiado' }
    | { tipo: 'desconocido' }
    | { tipo: 'pendiente' };
  /** Hay una operación del motor en curso. */
  actualizando?: boolean;
  /** Último fallo del motor, ya explicado. */
  fallo?: FalloMotor | null;
  /** Acciones del menú de restablecimiento. */
  accionesRestablecer?: AccionRestablecer[];
  /** Se eligió una acción del menú. */
  onRestablecer?: (id: string) => void;
  onCopy: () => void;
  onSaveProfile: () => void;
  /** Acción única de restablecimiento. Se usa solo si no hay menú (`accionesRestablecer`). */
  onReset?: () => void;
  /**
   * Confirma el texto escrito: lo manda al motor, que valida y actualiza la revisión.
   * Solo se llama al pulsar «Confirmar»; escribir no cambia nada.
   */
  onConfirmarEdicion: (value: string) => void;
  /**
   * Opciones de lanzamiento que Steam tiene para este juego, si se pudieron leer.
   * `null`/`undefined` = no se sabe (no se avisa de nada).
   */
  steamOpciones?: OpcionesSteam | null;
  /** Trae el texto de Steam al editor. Solo cambia el editor, nunca Steam. */
  onImportarDeSteam?: () => void;
}

/** El comando de lanzamiento: elemento protagonista de la vista de juego. */
export function CommandBox({
  gameName,
  launchCommand,
  activeCount,
  activeParts = [],
  copied,
  manualEditing,
  manualCommand,
  manualCommandError = '',
  info,
  appid,
  onLinkAppid,
  env,
  launcher,
  preparandoProton = false,
  blocked = false,
  blockedReason = '',
  onCopy,
  onSaveProfile,
  onConfirmarEdicion,
  steamOpciones = null,
  onImportarDeSteam,
  procedencia = 'inicial',
  perfilCargado = null,
  cambiosSinGuardar = false,
  ajustesActivos = [],
  comandoImportado = null,
  sincronizacion,
  actualizando = false,
  fallo = null,
  accionesRestablecer = [],
  onRestablecer,
  onReset,
}: CommandBoxProps) {
  // Diálogo para vincular el juego a mano con su ficha de Steam. Solo aplica a
  // juegos sin AppID cuya detección por nombre ha fallado.
  const [picking, setPicking] = useState(false);

  /**
   * Editor manual: desplegado o no, y el texto que se está escribiendo.
   *
   * El borrador es **local** a propósito. Mientras se escribe no se toca el motor: el
   * comando confirmado solo cambia al pulsar «Confirmar». Así «Cancelar» es de verdad no
   * hacer nada, y escribir no provoca reanálisis ni saltos en los ajustes de abajo.
   */
  const [editando, setEditando] = useState(false);
  const [borrador, setBorrador] = useState('');

  const abrirEditor = () => {
    // Se parte del comando confirmado: se edita sobre lo que hay, no en blanco.
    setBorrador(manualCommand);
    setEditando(true);
  };

  const cancelarEditor = () => {
    // Se descarta el borrador y no se llama al motor: el comando queda igual.
    setBorrador('');
    setEditando(false);
  };

  const confirmarEditor = () => {
    onConfirmarEdicion(borrador);
    setEditando(false);
  };

  // Revisión previa: los problemas que se pagan con un "el juego no arranca",
  // detectados antes de copiar el comando al lanzador. Con el juego bloqueado no
  // se revisa nada: no hay comando que revisar y los avisos serían ruido.
  const checks = blocked
    ? []
    : preflight(launchCommand, env, {
        // Con «Probar con Proton» activo el comando se prepara para Proton, así que sus
        // variables sí aplican y no hay que avisar de que «no hacen nada». El modo
        // **detectado** del juego no cambia; eso se dice aparte, en las etiquetas.
        native: info.native === true && !preparandoProton,
        launcher,
        manual: manualEditing,
      });

  /**
   * ¿Hay que avisar de que Steam tiene otro comando?
   *
   * Solo si Steam contestó, el juego tiene AppID y el texto no coincide. Sin dato
   * (`steamOpciones` nulo) no se dice nada: avisar «no coincide» sin saber el valor del
   * otro lado sería inventarse la discrepancia.
   */
  const mostrarAvisoSteam =
    !blocked && !!appid && steamOpciones != null && steamOpciones.opciones.trim() !== launchCommand.trim();

  if (blocked) {
    return (
      <section className="mb-8 p-6 rounded-2xl bg-[var(--error-bg)] border border-error/50 shadow-[0_0_44px_-24px_var(--error)]">
        <div className="flex items-start gap-3">
          <AlertCircle className="w-6 h-6 mt-0.5 shrink-0 text-[var(--error)]" />
          <div className="min-w-0">
            <p className="text-xs text-[var(--error)] font-bold uppercase tracking-wider mb-2">
              Sin comando de lanzamiento
            </p>
            <h2 className="text-2xl font-bold mb-1">{gameName}</h2>
            <p className="text-sm text-[var(--text)] leading-relaxed">{blockedReason}</p>
            <p className="text-xs text-[var(--text-muted)] mt-3 leading-relaxed">
              Ningún parámetro de esta app cambia una decisión del desarrollador. Si el dato
              cambia, la lista de anti-cheat se refresca sola una vez al día.
            </p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="mb-8 p-6 rounded-2xl bg-[var(--surface)] border border-action/40 shadow-[0_0_56px_-26px_var(--action)]">
      {/* Encabezado: qué juego y en qué estado. El título manda (era el mismo
          tamaño que el de una tarjeta de la biblioteca) y los avisos de
          compatibilidad se agrupan a la derecha, fuera del flujo del texto. */}
      <div className="flex items-start justify-between gap-4 mb-5">
        <div className="min-w-0">
          <p className="flex items-center gap-2 text-[11px] text-[var(--action)] font-bold uppercase tracking-[0.14em] mb-2">
            <Terminal className="w-4 h-4" />
            Comando de lanzamiento
          </p>
          <h2 className="text-3xl font-bold tracking-tight mb-2">{gameName}</h2>
          <p className="text-sm text-[var(--text-muted)] leading-relaxed max-w-2xl">
            {/* Vacío = aún no se sabe el comando (reglas sin cargar). No se dice «sin
                modificaciones»: eso afirmaría algo que no está comprobado. */}
            {launchCommand === ''
              ? 'Todavía no se ha cargado el comando de este juego.'
              : launchCommand === '%command%'
                ? 'Punto de partida sin modificaciones. Activa los ajustes opcionales para realizar pruebas.'
                : `${activeCount} ajuste(s) activo(s). Prueba cada cambio por separado.`}
          </p>
          <div className="flex flex-wrap items-center gap-1.5 mt-2.5">
            <ProcedenciaEtiqueta procedencia={procedencia} />
            {perfilCargado && (
              <span
                className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--border)] text-[var(--text-muted)] max-w-[16rem] truncate"
                title={`Perfil cargado: ${perfilCargado}`}
              >
                {perfilCargado}
              </span>
            )}
            {perfilCargado && cambiosSinGuardar && (
              <span className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--caution)] text-[var(--caution)]">
                Sin guardar
              </span>
            )}
            <ModoEjecucionEtiqueta
              nativoDetectado={info.native === true}
              preparandoProton={preparandoProton}
            />
            {sincronizacion && <SincronizacionEtiqueta tipo={sincronizacion.tipo} />}
            {actualizando && (
              <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider text-[var(--text-muted)]">
                <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />
                Actualizando
              </span>
            )}
          </div>
        </div>
        <GameBadges
          native={info.native}
          tier={info.tier}
          appid={appid}
          name={info.name}
          onLink={onLinkAppid ? () => setPicking(true) : undefined}
        />
      </div>

      {/* Vincular el juego con su ficha de Steam cuando no se reconoce por nombre */}
      {picking && onLinkAppid && (
        <AppidPicker
          initialQuery={gameName}
          onClose={() => setPicking(false)}
          onPick={(id) => {
            setPicking(false);
            onLinkAppid(id);
          }}
        />
      )}

      {/* Codebox grande y prominente */}
      <div className="p-4 rounded-xl bg-[var(--bg)] border border-[var(--border-hover)] mb-4">
        <code className="text-[15px] text-[var(--compatible)] font-mono break-all leading-relaxed">
          {launchCommand || (
            <span className="text-[var(--text-dim)] italic">Sin comando cargado todavía</span>
          )}
        </code>

        {/* Steam ya tiene configurado otro comando para este juego. El aviso vive aquí,
            pegado al código, en vez de ser un banner o un botón permanente: solo aparece
            cuando hay una diferencia real. */}
        {mostrarAvisoSteam && steamOpciones && (
          <AvisoSteamDesincronizado
            steam={steamOpciones}
            comandoActual={launchCommand}
            onImportar={() => onImportarDeSteam?.()}
            onRevisar={onCopy}
          />
        )}
      </div>

      {/* Ajustes reconocidos que están puestos. Los chips vienen del motor
          (`EstadoAjuste`), no de un cálculo de React: si un ajuste está «parcial» se ve
          aquí como parcial y no como activo. */}
      {/* La fila de chips está SIEMPRE montada, con una altura mínima. Antes se montaba
          solo si había ajustes: activar el primero añadía una fila entera y empujaba hacia
          abajo todo lo de debajo (~92 px medidos en la app real). Con la fila reservada, el
          primer chip entra en un hueco que ya existía y los toggles siguientes reutilizan el
          mismo renglón. No se oculta nada: si no hay ajustes, la fila está vacía pero
          presente. */}
      <div
        className="flex flex-wrap items-center gap-1.5 mb-4 min-h-[30px]"
        aria-label="Ajustes reconocidos activos"
      >
        {ajustesActivos.length > 0 && (
          <>
            <span className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mr-1">
              Ajustes
            </span>
          {ajustesActivos.map((a) => (
            <span
              key={a.valor}
              className={`text-[11px] px-2 py-1 rounded-md font-mono border ${
                a.estado === 'parcial'
                  ? 'bg-[var(--caution-bg)] text-[var(--caution)] border-caution/25'
                  : 'bg-action/10 text-[var(--action)] border-action/25'
              }`}
              title={
                a.estado === 'parcial'
                  ? 'Puesto en parte del comando: queda una aparición fuera del control de Thoriapp'
                  : 'Ajuste reconocido y activo'
              }
            >
              {a.valor}
            </span>
            ))}
          </>
        )}
      </div>

      {/* Resumen del resto de piezas (wrappers y argumentos propios). Igual que la fila de
          arriba: siempre presente para que añadir una pieza no cambie el alto de la caja. */}
      <div className="flex flex-wrap items-center gap-1.5 mb-4 min-h-[30px]">
        {activeParts.length > 0 && (
          <>
            <span className="text-[10px] uppercase tracking-wider text-[var(--text-dim)] mr-1">
              Incluye
            </span>
            {activeParts.map((part) => (
              <span
                key={`${part.kind}-${part.value}`}
                className={`text-[11px] px-2 py-1 rounded-md font-mono border ${PART_STYLES[part.kind]}`}
                title={PART_TITLES[part.kind]}
              >
                {part.value}
              </span>
            ))}
          </>
        )}
      </div>

      {/* El error del motor, pegado al comando: es lo que acaba de fallar. Se explica qué
          se ha conservado, porque el usuario tiene que saber que no ha perdido nada. */}
      {fallo && (
        <div
          role="alert"
          className="mb-4 p-3 rounded-xl bg-[var(--error-bg)] border border-error/40 flex items-start gap-2.5"
        >
          <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-[var(--error)]" aria-hidden="true" />
          <div className="min-w-0">
            <p className="text-sm font-semibold text-[var(--error)]">{fallo.mensaje}</p>
            {fallo.detalle && (
              <p className="text-xs text-[var(--text-muted)] mt-0.5 leading-relaxed">
                {fallo.detalle}
              </p>
            )}
            <p className="text-[11px] text-[var(--text-dim)] mt-1.5">
              Se sigue mostrando el último comando que el motor confirmó.
            </p>
          </div>
        </div>
      )}

      {/* Comando original importado, desplegable. Vive recogido porque es para consultar,
          no para trabajar: si estuviera siempre visible competiría con el comando actual. */}
      {comandoImportado !== null && comandoImportado !== launchCommand && (
        <details className="group mb-4 rounded-xl bg-[var(--bg)] border border-[var(--border)]">
          <summary className="flex items-center gap-2 px-3 py-2.5 cursor-pointer list-none text-[11px] uppercase tracking-wider text-[var(--text-muted)] hover:text-[var(--text)] transition-colors duration-150">
            <History className="w-3.5 h-3.5" aria-hidden="true" />
            Comando original importado
            <ChevronDown
              className="w-3.5 h-3.5 ml-auto transition-transform duration-150 group-open:rotate-180"
              aria-hidden="true"
            />
          </summary>
          <div className="px-3 pb-3">
            <code className="block text-xs font-mono text-[var(--text-muted)] break-all leading-relaxed">
              {comandoImportado}
            </code>
          </div>
        </details>
      )}

      {/* Comprobaciones sobre el sistema real (wrappers, capa Vulkan, sandbox…) */}
      {checks.length > 0 && (
        <ul className="mb-4 space-y-2">
          {checks.map((check) => (
            <PreflightRow key={check.id} check={check} />
          ))}
        </ul>
      )}
      {env && checks.length === 0 && (
        <p className="mb-4 text-xs text-[var(--text-muted)] flex items-center gap-1.5">
          <Check className="w-3.5 h-3.5 text-[var(--compatible)]" />
          Todo lo que usa este comando está instalado en tu sistema.
        </p>
      )}

      <div className="flex flex-col sm:flex-row gap-3">
        <button
          onClick={onCopy}
          disabled={!!manualCommandError}
          title={manualCommandError || undefined}
          className={`flex-1 flex items-center justify-center gap-2.5 px-6 py-4 rounded-xl font-bold text-base transition-all ${
            manualCommandError
              ? 'bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] cursor-not-allowed'
              : copied
                ? 'bg-[var(--compatible)] text-[var(--bg)]'
                : 'bg-gradient-to-b from-[var(--action-hover)] to-[var(--action)] text-[var(--bg)] shadow-lg shadow-action/30 hover:to-[var(--action-hover)]'
          }`}
        >
          {manualCommandError ? (
            <>
              <AlertCircle className="w-5 h-5" /> Revisa el comando
            </>
          ) : copied ? (
            <>
              <Check className="w-5 h-5" /> ¡Copiado!
            </>
          ) : (
            <>
              <Copy className="w-5 h-5" /> Copiar comando
            </>
          )}
        </button>
        <button
          onClick={onSaveProfile}
          className="px-5 py-4 rounded-xl text-sm font-semibold bg-action/10 text-[var(--action)] border border-action/30 hover:bg-action/20 transition-colors flex items-center justify-center gap-2"
        >
          <Star className="w-4 h-4" />
          Guardar perfil
        </button>
        {onRestablecer && accionesRestablecer.length > 0 ? (
          <MenuRestablecer
            acciones={accionesRestablecer}
            onElegir={onRestablecer}
            deshabilitado={actualizando}
          />
        ) : (
          onReset && (
            <button
              onClick={onReset}
              className="px-5 py-4 rounded-xl text-sm font-semibold bg-[var(--bg)] text-[var(--text-muted)] border border-[var(--border)] hover:bg-[var(--card-hover)] hover:text-[var(--text)] transition-colors flex items-center justify-center gap-2"
            >
              <RotateCcw className="w-4 h-4" aria-hidden="true" />
              Restablecer
            </button>
          )
        )}
      </div>

      {/* Editor manual del comando. El botón es discreto y está siempre disponible, pero
          el editor **no** está desplegado: solo aparece al pulsarlo. Cancelar no toca el
          comando; confirmar es lo que lo manda al motor. */}
      <div className="mt-4">
        {!editando ? (
          <button
            onClick={abrirEditor}
            className="inline-flex items-center gap-2 text-xs font-semibold text-[var(--text-muted)] hover:text-[var(--action)] transition-colors px-2 py-1.5 rounded-lg hover:bg-action/10"
            aria-expanded={editando}
          >
            <Pencil className="w-3.5 h-3.5" aria-hidden="true" />
            Editar comando
          </button>
        ) : (
          <div className="p-4 rounded-xl bg-[var(--card)] border border-[var(--border)]">
            <div className="flex items-start justify-between gap-4 mb-3">
              <div className="min-w-0">
                <p className="flex items-center gap-2 text-sm font-semibold text-[var(--text)]">
                  <Pencil className="w-4 h-4 text-[var(--text-muted)]" />
                  Editar comando manualmente
                </p>
                <p className="text-xs text-[var(--text-muted)] mt-1">
                  Al confirmar, el comando pasa por el motor y los ajustes de abajo se
                  actualizan para describirlo. Si cancelas, no cambia nada.
                </p>
              </div>
            </div>
            <textarea
              value={borrador}
              onChange={(e) => setBorrador(e.target.value)}
              aria-invalid={!!manualCommandError}
              aria-describedby={manualCommandError ? 'manual-command-error' : undefined}
              aria-label="Comando de lanzamiento"
              className={`w-full p-3 rounded-lg bg-[var(--bg)] border text-sm text-[var(--compatible)] font-mono focus:outline-none resize-none ${
                manualCommandError
                  ? 'border-[var(--error)]'
                  : 'border-[var(--border)] focus:border-[var(--action)]'
              }`}
              rows={3}
              placeholder="%command%"
            />
            {manualCommandError && (
              <p
                id="manual-command-error"
                role="alert"
                className="mt-2 text-xs text-[var(--error)] flex items-start gap-1.5"
              >
                <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
                <span>{manualCommandError}</span>
              </p>
            )}
            <div className="mt-3 flex items-center justify-end gap-2">
              <button
                onClick={cancelarEditor}
                className="px-3 py-2 rounded-lg text-xs font-semibold text-[var(--text-muted)] hover:text-[var(--text)] border border-[var(--border)] hover:bg-[var(--card-hover)] transition-colors"
              >
                Cancelar
              </button>
              <button
                onClick={confirmarEditor}
                disabled={!!manualCommandError || !borrador.trim()}
                className={`px-4 py-2 rounded-lg text-xs font-bold transition-colors ${
                  manualCommandError || !borrador.trim()
                    ? 'bg-[var(--bg)] text-[var(--text-dim)] border border-[var(--border)] cursor-not-allowed'
                    : 'bg-action text-[var(--bg)] hover:bg-[var(--action-hover)]'
                }`}
              >
                Confirmar
              </button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

/**
 * De dónde sale el comando. Se dice explícitamente porque «esto es tu comando» no
 * significa lo mismo si viene de Steam, si lo ha generado Thoriapp o si se ha vuelto al
 * original tras haberlo tocado.
 */
/**
 * En qué modo se está preparando el comando.
 *
 * Tres cosas distintas que antes se mezclaban en una sola:
 *
 *  - **detectado**: el juego trae build nativo de Linux (lo dice Steam);
 *  - **preparando**: el usuario activó «Probar con Proton» en Thoriapp;
 *  - **Steam**: con qué lanza Steam de verdad. Thoriapp **no lo sabe ni lo cambia**.
 *
 * Decir «Linux nativo» con Proton activado era contradecirse en la misma tarjeta. Ahora se
 * enseña lo detectado y, si el usuario está preparando Proton, se dice que el comando va
 * para Proton y que Steam no se ha tocado.
 */
function ModoEjecucionEtiqueta({
  nativoDetectado,
  preparandoProton,
}: {
  nativoDetectado: boolean;
  preparandoProton: boolean;
}) {
  const detectado = nativoDetectado ? 'Linux nativo' : 'Proton / Wine';
  if (!preparandoProton) {
    return (
      <span className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--border)] text-[var(--text-muted)]">
        {detectado}
      </span>
    );
  }
  return (
    <span
      className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--accent)]/40 text-[var(--accent)]"
      title="Steam no se modifica: si quieres que Steam use Proton, actívalo también en Steam."
    >
      Preparando para Proton
      {nativoDetectado ? ' · el juego es nativo' : ''}
    </span>
  );
}

function ProcedenciaEtiqueta({ procedencia }: { procedencia: Procedencia }) {
  const texto: Record<Procedencia, string> = {
    inicial: 'Sin cargar',
    propuesta: 'Generado por Thoriapp',
    importado: 'Importado de Steam',
    original_restaurado: 'Original restaurado',
    // De un perfil guardado: no es «importado de Steam», así que no puede decir eso.
    perfil: 'Perfil cargado',
  };
  if (procedencia === 'inicial') return null;
  return (
    <span className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--border)] text-[var(--text-muted)]">
      {texto[procedencia]}
    </span>
  );
}

/**
 * Relación con Steam.
 *
 * Cinco casos y no dos, porque «no coincide» no siempre es un problema: si el usuario acaba
 * de tocar un ajuste, es normal que Thoriapp tenga algo distinto de Steam y **no** hay que
 * pintarlo de naranja. El naranja se reserva para cuando Steam cambió por fuera.
 */
function SincronizacionEtiqueta({ tipo }: { tipo: string }) {
  if (tipo === 'sincronizado') {
    return (
      <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-compatible/30 text-[var(--compatible)]">
        <Check className="w-3 h-3" aria-hidden="true" />
        Igual que Steam
      </span>
    );
  }
  if (tipo === 'local') {
    // Cambio hecho por el usuario: información, no alarma.
    return (
      <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-action/30 text-[var(--action)]">
        Cambios sin aplicar a Steam
      </span>
    );
  }
  if (tipo === 'steam_cambiado') {
    return (
      <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-caution/40 text-[var(--caution)]">
        <AlertTriangle className="w-3 h-3" aria-hidden="true" />
        Steam tiene otro comando
      </span>
    );
  }
  if (tipo === 'pendiente') {
    return (
      <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--border)] text-[var(--text-muted)]">
        <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />
        Leyendo Steam
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1 text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded border border-[var(--border)] text-[var(--text-dim)]">
      <CloudOff className="w-3 h-3" aria-hidden="true" />
      Steam sin datos
    </span>
  );
}

/** Color de los chips del resumen, por tipo de pieza del comando. */
const PART_STYLES: Record<'env' | 'wrapper' | 'arg', string> = {
  env: 'bg-action/10 text-[var(--action)] border-action/25',
  wrapper: 'bg-[var(--caution-bg)] text-[var(--caution)] border-caution/25',
  arg: 'bg-[var(--compatible-bg)] text-[var(--compatible)] border-compatible/25',
};

const PART_TITLES: Record<'env' | 'wrapper' | 'arg', string> = {
  env: 'Variable de entorno: se aplica a todos los procesos del juego',
  wrapper: 'Programa que envuelve el lanzamiento (overlay, gestor de energía…)',
  arg: 'Argumento propio del juego',
};

/** Colores e icono de cada nivel de aviso. */
const SEVERITY_STYLES = {  error: {
    Icon: AlertCircle,
    box: 'bg-[var(--error-bg)] border-error/40',
    text: 'text-[var(--error)]',
  },
  warn: {
    Icon: AlertTriangle,
    box: 'bg-[var(--caution-bg)] border-caution/40',
    text: 'text-[var(--caution)]',
  },
  info: {
    Icon: Info,
    box: 'bg-[var(--surface)] border-[var(--border)]',
    text: 'text-[var(--text-muted)]',
  },
} as const;

function PreflightRow({ check }: { check: PreflightCheck }) {
  const { Icon, box, text } = SEVERITY_STYLES[check.severity];

  return (
    <li className={`flex items-start gap-3 p-3 rounded-xl border ${box}`}>
      <Icon className={`w-4 h-4 mt-0.5 shrink-0 ${text}`} />
      <div className="min-w-0">
        <p className={`text-sm font-semibold ${text}`}>{check.title}</p>
        <p className="text-xs text-[var(--text-muted)] mt-0.5">{check.detail}</p>
        {check.hint && (
          <p className="text-xs text-[var(--text-muted)] mt-1 opacity-80">{check.hint}</p>
        )}
      </div>
    </li>
  );
}
