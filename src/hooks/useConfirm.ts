import { useEffect, useState } from 'react';

export interface ConfirmRequest {
  title: string;
  message: string;
  confirmLabel: string;
  /** Opcional si el cuadro solo recoge un valor (ver `input`). */
  onConfirm?: () => void;
  /** Si se indica, el cuadro muestra un campo de texto (sustituye a prompt()). */
  input?: {
    label: string;
    defaultValue: string;
    placeholder?: string;
    onSubmit: (value: string) => void;
  };
  /** Color del botón principal: rojo para acciones destructivas (por defecto). */
  tone?: 'danger' | 'primary';
}

/**
 * Sustituye a `window.confirm()`, que bloquea la ventana entera en Tauri.
 *
 * Devuelve el estado del cuadro, la función para pedir una confirmación y la de
 * cerrarlo. Escape cierra el cuadro sin ejecutar nada.
 */
export function useConfirm() {
  const [confirmState, setConfirmState] = useState<ConfirmRequest | null>(null);

  useEffect(() => {
    if (!confirmState) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setConfirmState(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [confirmState]);

  const ask = (request: ConfirmRequest) => setConfirmState(request);
  const close = () => setConfirmState(null);

  return { confirmState, ask, close };
}

/**
 * Toast que se borra solo a los `ms` milisegundos. Cualquier componente con
 * avisos breves repite este patrón, así que vive aquí.
 */
export function useToast(ms = 3000) {
  const [toast, setToast] = useState<string | null>(null);

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), ms);
    return () => clearTimeout(timer);
  }, [toast, ms]);

  return { toast, setToast };
}
