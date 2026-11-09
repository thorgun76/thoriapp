import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { buildProfileCommand, checkManualCommand } from '../lib/command';
import { deniedCommand, deniedReason, isDenied, isDeniedCommand } from '../lib/anticheat';
import type { GameEngine, ImportPreview, Profile } from '../lib/types';

interface UseProfilesOptions {
  /** Tags de hardware para filtrar recomendaciones. Getter para no recrear callbacks. */
  getHwTags: () => string[];
  /** GPU del equipo, para los parámetros del motor que dependen del modelo (RT). */
  getGpuName?: () => string | null;
  /** Avisos (toast). */
  onNotify?: (message: string) => void;
}

/**
 * Perfiles guardados: carga, reconstrucción de comandos, guardado, borrado,
 * reordenado e import/export.
 *
 * La reconstrucción de comandos se hace EN PARALELO. Antes era un bucle
 * secuencial de 4 llamadas IPC por perfil (~80 round-trips en serie con 20
 * perfiles) y bloqueaba el arranque.
 */
export function useProfiles({ getHwTags, getGpuName, onNotify }: UseProfilesOptions) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [profileCommands, setProfileCommands] = useState<Record<string, string>>({});
  /** Importación a la espera de que el usuario confirme en el cuadro de vista previa. */
  const [pendingImport, setPendingImport] = useState<{
    content: string;
    preview: ImportPreview;
  } | null>(null);

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  /**
   * Reconstruye un comando. `ctx` cachea lo que es igual para todos los
   * perfiles (recomendaciones de comunidad por hardware, y motor por appid),
   * así N perfiles no repiten N veces la misma consulta.
   *
   * Si el juego está `Denied` por el anti-cheat, el comando **no se reconstruye**:
   * se devuelve el motivo. Un perfil guardado antes de que la app bloqueara estos
   * juegos seguía generando un comando válido, y esa era la puerta de atrás del
   * bloqueo de la vista del juego.
   */
  const rebuildOne = useCallback(
    async (
      profile: Profile,
      communityRecs: Awaited<ReturnType<typeof api.getCommunityRecommendations>>,
      engineCache: Map<number, GameEngine | null>,
      /** Un solo aviso por appid aunque haya varios perfiles del mismo juego. */
      deniedCache?: Map<number, string | null>
    ): Promise<string> => {
      if (profile.use_manual_command && profile.manual_command) {
        // Mismo criterio que `buildProfileCommand`: un comando manual que no pasa la
        // comprobación se ignora y el perfil se reconstruye normal. Si no se hiciera
        // aquí, un perfil importado antes de este cambio seguiría devolviendo su
        // comando tal cual por esta puerta.
        const manual = profile.manual_command.trim();
        if (checkManualCommand(manual).safe) {
          return manual;
        }
      }
      try {
        // appid 0 = juego de otro launcher: no hay ficha de anti-cheat que consultar.
        if (profile.appid !== 0) {
          let denial = deniedCache?.get(profile.appid);
          if (denial === undefined) {
            denial = await api
              .getAnticheatInfo(profile.appid)
              .then((r) => (isDenied(r.info) ? deniedReason(r.info) : null))
              // Si el catálogo no se puede leer, no se bloquea: negar por defecto
              // dejaría sin comando a juegos que funcionan sin problema.
              .catch(() => null);
            deniedCache?.set(profile.appid, denial);
          }
          // `null` = aún no se consultó; `''` = se consultó y no está denegado.
          if (denial) return deniedCommand(profile.appid);
        }

        const rules = await api.getRules(profile.appid);

        let engine = engineCache.get(profile.appid);
        if (engine === undefined) {
          engine = await api.detectEngine(profile.appid, profile.game_name);
          engineCache.set(profile.appid, engine);
        }
        const engineRecs = engine
          ? await api.getEngineRecommendations(engine, getGpuName?.() ?? null)
          : [];

        return buildProfileCommand(profile, rules, communityRecs, engine, engineRecs);
      } catch (e) {
        console.error(`Error al reconstruir comando para perfil ${profile.id}:`, e);
        return '%command%';
      }
    },
    [getGpuName]
  );

  /** Carga todos los perfiles y reconstruye sus comandos en paralelo. */
  const loadAll = useCallback(async () => {
    try {
      const [allProfiles, communityRecs] = await Promise.all([
        api.loadProfiles(),
        api.getCommunityRecommendations(getHwTags()).catch(() => []),
      ]);
      if (!mountedRef.current) return;
      setProfiles(allProfiles);

      const engineCache = new Map<number, GameEngine | null>();
      const deniedCache = new Map<number, string | null>();
      const entries = await Promise.all(
        allProfiles.map(
          async (p) =>
            [p.id, await rebuildOne(p, communityRecs, engineCache, deniedCache)] as const
        )
      );
      if (!mountedRef.current) return;
      setProfileCommands(Object.fromEntries(entries));
    } catch (e) {
      console.error('Error al cargar perfiles:', e);
    }
  }, [getHwTags, rebuildOne]);

  /** Copia al portapapeles el comando de un perfil (recalculado al momento). */
  const copyCommand = useCallback(
    async (profile: Profile) => {
      let command = '%command%';
      try {
        const communityRecs = await api
          .getCommunityRecommendations(getHwTags())
          .catch(() => []);
        command = await rebuildOne(profile, communityRecs, new Map());
      } catch (e) {
        console.error('Error al cargar perfil:', e);
      }
      // Perfil de un juego con anti-cheat denegado: `command` es el motivo, no
      // un comando. Se avisa y NO se toca el portapapeles, para que nadie pegue
      // ahí lo que había (potencialmente el comando de otro juego).
      if (isDeniedCommand(command)) {
        onNotify?.(`✗ ${command}`);
        return;
      }
      await navigator.clipboard.writeText(command);
      onNotify?.(`✓ Comando copiado: ${command}`);
    },
    [getHwTags, rebuildOne, onNotify]
  );

  /** Guarda el perfil del juego abierto y recarga la lista. */
  const save = useCallback(
    async (payload: Parameters<typeof api.saveProfile>[0], name: string) => {
      try {
        await api.saveProfile(payload);
        onNotify?.(`Perfil "${name}" guardado`);
        await loadAll();
        return true;
      } catch (e) {
        // El mensaje de Rust explica la causa (permisos, ruta, JSON corrupto…).
        // Mostrarlo evita el genérico «error al guardar» que no dice nada.
        const detalle = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
        console.error('Error al guardar perfil:', e);
        onNotify?.(`Error al guardar perfil: ${detalle}`);
        return false;
      }
    },
    [loadAll, onNotify]
  );

  const remove = useCallback(
    async (profileId: string) => {
      try {
        await api.deleteProfile(profileId);
        onNotify?.('Perfil eliminado');
        await loadAll();
      } catch (e) {
        console.error('Error al eliminar perfil:', e);
        onNotify?.('Error al eliminar perfil');
      }
    },
    [loadAll, onNotify]
  );

  /** Reordena y persiste el nuevo orden. */
  const reorder = useCallback(
    async (newProfiles: Profile[]) => {
      setProfiles(newProfiles);
      try {
        await api.reorderProfiles(newProfiles.map((p) => p.id));
        onNotify?.('Orden de perfiles actualizado');
      } catch (e) {
        console.error('Error al reordenar perfiles:', e);
        onNotify?.('Error al guardar el orden');
      }
    },
    [onNotify]
  );

  const importFromText = useCallback(
    async (content: string) => {
      try {
        // Primero se mira qué hay, sin escribir nada. Si el archivo está mal, esto
        // falla y no se toca disco; si está bien, el llamante decide si confirma.
        const preview = await api.previewImportProfile(content);
        if (preview.profiles.length === 0) {
          onNotify?.(
            preview.duplicates > 0
              ? 'Esos perfiles ya están guardados, no hay nada que importar.'
              : 'El archivo no contiene perfiles que importar.'
          );
          return;
        }
        setPendingImport({ content, preview });
      } catch (err) {
        onNotify?.(`Error al importar: ${String(err)}`);
      }
    },
    [onNotify]
  );

  /** El usuario confirmó en el cuadro: ahora sí se escribe. */
  const confirmImport = useCallback(async () => {
    const pending = pendingImport;
    if (!pending) return;
    setPendingImport(null);
    try {
      const imported = await api.importProfileFromText(pending.content);
      onNotify?.(`${imported.length} perfil(es) importado(s) correctamente`);
      await loadAll();
    } catch (err) {
      onNotify?.(`Error al importar: ${String(err)}`);
    }
  }, [pendingImport, loadAll, onNotify]);

  const cancelImport = useCallback(() => setPendingImport(null), []);

  const exportAll = useCallback(
    async (path: string) => {
      try {
        const savedTo = await api.exportAllProfiles(path);
        onNotify?.(`Perfiles exportados en ${savedTo}`);
      } catch (err) {
        onNotify?.(`Error al exportar: ${String(err)}`);
      }
    },
    [onNotify]
  );

  const exportOne = useCallback(
    async (profileId: string, path: string) => {
      try {
        await api.exportProfile(profileId, path);
        onNotify?.('Perfil exportado correctamente');
      } catch {
        onNotify?.('Error al exportar perfil');
      }
    },
    [onNotify]
  );

  return {
    profiles,
    profileCommands,
    loadAll,
    copyCommand,
    save,
    remove,
    reorder,
    importFromText,
    confirmImport,
    cancelImport,
    pendingImport,
    exportAll,
    exportOne,
  };
}
