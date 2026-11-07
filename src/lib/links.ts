import { api } from './api';

/**
 * Abre un enlace externo SIEMPRE a través de Rust, que valida contra una
 * allowlist de dominios. Un fallo solo se registra: no se abre nada.
 *
 * (Antes existía un fallback con window.open() que saltaba esa validación.)
 */
export const openExternal = async (url: string): Promise<void> => {
  try {
    await api.openUrl(url);
  } catch (e) {
    console.error('No se pudo abrir el enlace externo:', url, e);
  }
};
