import { useEffect, useState } from 'react';

export interface SectionLink {
  /** `id` del elemento al que salta. */
  id: string;
  label: string;
}

/**
 * Índice de subsecciones de la pestaña activa.
 *
 * Las pestañas de un juego son listas largas (anti-cheat, requisitos, ProtonDB,
 * parámetros…) y había que bajar a ciegas. Aquí cada entrada lleva a su bloque,
 * y la activa se marca según el scroll.
 */
export function SectionNav({ sections }: { sections: SectionLink[] }) {
  const [active, setActive] = useState(sections[0]?.id ?? '');

  // Marca la sección visible. Se escucha el contenedor con scroll de la vista de
  // juego, no `window`: el <main> es quien desplaza.
  useEffect(() => {
    if (sections.length === 0) return;
    // Sin IntersectionObserver (WebViews antiguos, jsdom en los tests) el índice
    // sigue funcionando para saltar; solo se pierde el resaltado automático.
    if (typeof IntersectionObserver === 'undefined') return;

    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
        if (visible?.target.id) setActive(visible.target.id);
      },
      // La franja superior es la que manda: es donde está mirando el usuario.
      { rootMargin: '-80px 0px -60% 0px', threshold: 0 }
    );

    for (const section of sections) {
      const el = document.getElementById(section.id);
      if (el) observer.observe(el);
    }

    return () => observer.disconnect();
  }, [sections.map((s) => s.id).join(',')]);

  if (sections.length < 2) return null;

  const jump = (id: string) => {
    const el = document.getElementById(id);
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setActive(id);
  };

  return (
    <nav
      aria-label="Secciones de esta pestaña"
      className="flex flex-wrap items-center gap-1.5 -mt-2 mb-4"
    >
      {sections.map((section) => (
        <button
          key={section.id}
          onClick={() => jump(section.id)}
          aria-current={active === section.id ? 'true' : undefined}
          className={`text-xs px-2.5 py-1 rounded-full border transition-colors ${
            active === section.id
              ? 'bg-action/15 text-[var(--action)] border-action/30 font-semibold'
              : 'bg-[var(--surface)] text-[var(--text-muted)] border-[var(--border)] hover:text-[var(--text)]'
          }`}
        >
          {section.label}
        </button>
      ))}
    </nav>
  );
}
