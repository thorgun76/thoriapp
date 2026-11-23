import { useEffect, useState } from 'react';
import { Check, Copy, Gamepad2, Zap } from 'lucide-react';
import type { Game } from '../lib/types';
import { useLocalCover } from '../hooks/useLocalCover';

/**
 * Carátula vertical del CDN de Steam (600×900, relación 2:3).
 * Es el tercer origen de imagen: antes se prueba la que el launcher tiene
 * guardada en disco, que es la que el usuario ve (y puede haber cambiado).
 */
export const coverUrl = (appid: number | undefined) =>
  appid && appid > 0
    ? `https://cdn.cloudflare.steamstatic.com/steam/apps/${appid}/library_600x900.jpg`
    : null;

const LAUNCHER_BADGE: Record<string, { label: string; className: string }> = {
  steam: { label: 'Steam', className: 'bg-[#1b2838]/90 text-white border border-[#1b2838]' },
  lutris: { label: 'Lutris', className: 'bg-orange-500/90 text-white border border-orange-600' },
  heroic: { label: 'Heroic', className: 'bg-blue-500/90 text-white border border-blue-600' },
  heronite: { label: 'Heronite', className: 'bg-purple-500/90 text-white border border-purple-600' },
};

const badgeFor = (launcher: string | undefined) =>
  LAUNCHER_BADGE[launcher ?? 'steam'] ?? {
    label: launcher ? launcher.charAt(0).toUpperCase() + launcher.slice(1) : 'Steam',
    className: 'bg-gray-500/90 text-white border border-gray-600',
  };

/**
 * Tarjeta de un juego en la biblioteca.
 *
 * Muestra la carátula real si existe; si el CDN responde 404 (juegos retirados,
 * demos, apps de herramientas) o el juego no es de Steam, cae a un marcador con
 * el icono de mando en vez de dejar un hueco roto.
 */
export function GameCard({
  game,
  onOpen,
  quickCommand = null,
  onCopyCommand,
}: {
  game: Game;
  onOpen: (game: Game) => void;
  /**
   * Comando del perfil guardado de este juego. `null` = no hay nada que copiar, y
   * entonces no se pinta el botón (copiar el `%command%` de partida no aporta).
   */
  quickCommand?: string | null;
  onCopyCommand?: (command: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(t);
  }, [copied]);

  const copyCommand = (e: React.MouseEvent) => {
    // Sin esto el clic burbujearía a la tarjeta y abriría el juego.
    e.stopPropagation();
    if (!quickCommand) return;
    onCopyCommand?.(quickCommand);
    setCopied(true);
  };

  const badge = badgeFor(game.launcher);
  // Orden: carátula del launcher (la que el usuario ve, puede ser la suya)
  // -> CDN de Steam -> icono de mando.
  const localCover = useLocalCover(game);
  // Mientras se consulta el disco no se pinta la del CDN para evitar que la
  // tarjeta cambie de imagen de golpe cuando llega la local.
  const waitingLocal = localCover === undefined;
  const src = waitingLocal ? null : (localCover ?? coverUrl(game.appid));

  // Se recuerda qué URL falló (no solo un booleano) para que al cambiar de
  // juego la tarjeta no arrastre el estado de la anterior.
  const [failedSrc, setFailedSrc] = useState<string | null>(null);
  useEffect(() => {
    setFailedSrc(null);
  }, [src]);

  const showCover = !!src && failedSrc !== src;

  return (
    <button
      onClick={() => onOpen(game)}
      title={game.name}
      className="group relative aspect-[3/4] rounded-xl bg-[var(--card)] border border-[var(--border)] hover:border-action/50 hover:scale-[1.02] transition-all overflow-hidden text-left"
    >
      {/* Carátula: hueco neutro mientras llega la del launcher */}
      {waitingLocal ? (
        <div className="absolute inset-0 bg-[var(--card)]" />
      ) : showCover ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          onError={() => setFailedSrc(src)}
          className="absolute inset-0 w-full h-full object-cover"
        />
      ) : (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-[var(--card)] to-[var(--bg)]">
          <Gamepad2 className="w-10 h-10 text-[var(--text-dim)] group-hover:text-[var(--action)] transition-colors" />
        </div>
      )}

      {/* Con carátula hace falta oscurecer la parte de abajo para leer el
          título; sin carátula el fondo ya es oscuro y sobraría. */}
      {showCover && (
        <div className="absolute inset-x-0 bottom-0 h-2/5 bg-gradient-to-t from-black/95 via-black/70 to-transparent" />
      )}

      {/* Invitación a la acción al pasar el ratón (o al enfocar con el teclado).
          No es un botón aparte a propósito: la tarjeta entera ya lo es, y un
          segundo control por juego duplicaría las paradas de tabulación en una
          cuadrícula de cientos. Por eso el aviso es decorativo
          (`pointer-events-none` + `aria-hidden`): el clic cae en la tarjeta. */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 z-[5] flex items-center justify-center opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-visible:opacity-100 motion-reduce:transition-none"
      >
        <div className="absolute inset-0 bg-[var(--bg)]/45" />
        <span className="relative flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-[var(--action)] text-[var(--bg)] text-xs font-bold shadow-lg shadow-black/40">
          <Zap className="w-3.5 h-3.5" aria-hidden="true" />
          Optimizar
        </span>
      </div>

      <div className="absolute top-2 right-2 z-10">
        <span className={`text-[10px] px-2 py-1 rounded font-semibold ${badge.className}`}>
          {badge.label}
        </span>
      </div>

      {/* Copiar el comando guardado sin abrir el juego: ahorra un viaje de ida y
          vuelta para lo que más se hace desde la biblioteca. */}
      {quickCommand && (
        <span
          role="button"
          tabIndex={0}
          aria-label={`Copiar el comando de «${game.name}»`}
          title={quickCommand}
          onClick={copyCommand}
          onKeyDown={(e) => {
            // La tarjeta es un <button>: Enter/Espacio aquí también abriría el
            // juego, así que se corta la propagación igual que con el ratón.
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              e.stopPropagation();
              if (quickCommand) {
                onCopyCommand?.(quickCommand);
                setCopied(true);
              }
            }
          }}
          className={`absolute top-2 left-2 z-10 flex items-center gap-1 px-2 py-1 rounded text-[10px] font-semibold transition-all ${
            copied
              ? 'bg-[var(--compatible)] text-[var(--bg)] opacity-100'
              : 'bg-black/70 text-white opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-[var(--action)]'
          }`}
        >
          {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
          {copied ? 'Copiado' : 'Comando'}
        </span>
      )}

      {/* Título: sin carátula ocupa más y se puede leer completo */}
      <div className="absolute inset-x-0 bottom-0 p-2.5 z-10">
        <p
          className={
            showCover
              ? 'text-sm font-semibold text-white line-clamp-2 drop-shadow-md'
              : 'text-sm font-semibold text-[var(--text)] line-clamp-4 group-hover:text-white transition-colors'
          }
        >
          {game.name}
        </p>
      </div>
    </button>
  );
}
