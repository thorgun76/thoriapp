import { useEffect, useState } from 'react';
import { Gamepad2 } from 'lucide-react';
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
}: {
  game: Game;
  onOpen: (game: Game) => void;
}) {
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
      className="group relative aspect-[3/4] rounded-xl bg-[var(--card)] border border-[var(--border)] hover:border-[var(--action)]/50 hover:scale-[1.02] transition-all overflow-hidden text-left"
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

      <div className="absolute top-2 right-2 z-10">
        <span className={`text-[10px] px-2 py-1 rounded font-semibold ${badge.className}`}>
          {badge.label}
        </span>
      </div>

      {/* Título: sin carátula ocupa más y se puede leer completo */}
      <div className="absolute inset-x-0 bottom-0 p-2.5">
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
