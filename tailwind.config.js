/** @type {import('tailwindcss').Config} */

// Colores del tema (ver src/index.css) con soporte de opacidad.
//
// `rgb(var(--x-rgb) / <alpha-value>)` es lo que hace que `bg-action/10`,
// `border-error/30` o `shadow-action/20` compilen. Con `bg-[var(--action)]/10`
// Tailwind no puede aplicar el modificador a un `var()` y descarta la clase.
const alpha = (name) => `rgb(var(--${name}-rgb) / <alpha-value>)`;

export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        action: alpha('action'),
        compatible: alpha('compatible'),
        caution: alpha('caution'),
        error: alpha('error'),
        // Metales de los niveles de ProtonDB (platino casi blanco, plata gris).
        platinum: alpha('platinum'),
        silver: alpha('silver'),
        bronze: alpha('bronze'),
        border: alpha('border'),
        'text-muted': alpha('text-muted'),
      },
    },
  },
  plugins: [],
};
