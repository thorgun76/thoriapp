# Planificación de fases 5.4 y 6 — requisitos registrados

Este documento existe para que una sesión nueva recupere los requisitos sin depender del
historial de conversación. **Nada de lo de aquí está implementado salvo donde se diga
explícitamente.** Registrar una propuesta no es autorización para hacerla.

Estado a fecha de este documento: subfases 1–5.1, 5.2 y 5.3 cerradas. **5.4 y la fase 6 no
están empezadas.**

> **5.3 se cierra con una limitación conocida.** El observador es experimental y opcional,
> apagado por defecto. Validado individualmente en Steam, Heroic, Heronite y Lutris, y en
> simultáneo en Steam + Steam y Lutris + Steam. La simultaneidad **Heroic + Heronite** no está
> garantizada; la causa exacta no está demostrada y la incidencia queda aplazada, **sin
> bloquear 5.4**. Reabrirla exige una captura nueva del diagnóstico en ese escenario: los
> registros existentes no bastan.

## Frontera entre 5.4 y la fase 6

La fase 6 se decidió que sea **un rediseño integral de la experiencia de usuario**, no una
limpieza estética. Eso obliga a separar con nitidez qué es fiabilidad de datos y qué es
presentación, porque lo primero no puede plegarse a lo segundo.

### Reservado para 5.4 — fiabilidad de los datos

Todo esto toca **cómo se mide y qué se puede afirmar**, no cómo se pinta:

1. **Fiabilidad estadística**: tamaños de muestra, dispersión, cuándo una serie de pruebas
   justifica una conclusión.
2. **Identificación de diferencias reales de hardware**: hoy dos nombres de la misma GPU
   (`AMD Radeon RX 6600` y `RADV NAVI23`) producen una diferencia de hardware falsa.
3. **Distinción entre muestras periódicas y frames**: MangoHud muestrea a intervalos; una
   muestra no es un fotograma. Hoy se usa «frames» para las dos cosas.
4. **Diagnóstico de shaders basado en evidencias**: hoy se afirma la causa sin evidencia
   suficiente. Debe presentarse como hipótesis cuando no la haya.
5. **Coherencia de las conclusiones**: la misma regla en todas las pantallas.
6. **Márgenes de ruido e incertidumbre**: hoy existe `noise_percent`, pero falta revisar si
   los márgenes son coherentes con las muestras reales.

**Restricción vigente**: las fórmulas estadísticas actuales **no se tocan** hasta que 5.4 se
autorice expresamente, y entonces se versiona la metodología para no romper el histórico.

### Reservado para la fase 6 — presentación, navegación, usabilidad y automatización

Recogido en la sección siguiente.

---

## Requisitos registrados para la fase 6 (NO implementados)

### Historial de benchmarks

**Dos vistas del mismo historial (aclarado al cerrar 5.3).** Benchmarks no es solo la ficha
 de un juego. Se quieren dos puertas de entrada al **mismo** almacenamiento, la **misma**
 lógica de dominio y los **mismos** servicios; nunca dos estados paralelos ni copias de datos.

- **Centro general de benchmarks**: historial de **todos** los juegos.
- **Historial dentro de la ficha de cada juego**: historial **exclusivamente** de ese juego.
- **Selector de juegos** en el historial general.
- **Componentes reutilizables** entre ambas vistas (lista, tarjeta de prueba, cabecera de
  alcance, diálogo de borrado).
- **Acceso a comparaciones e informes desde ambas vistas sin duplicar datos.**
- **Sin filtro por juego forzado en la pestaña general.**

> Estado al cerrar 5.3: la pestaña general **ya** enseña todos los juegos cuando no hay
> ninguno abierto y se limita a uno cuando lo hay. Lo que faltaba era que el texto dijera la
> verdad; eso ya está corregido. Lo de arriba es el rediseño, y es de fase 6.

- Historial compacto en **formato tabla**.
- **Eliminación del scroll interno** de la lista.
- **Selección manual** de benchmarks mediante checkboxes.
- **Comparación personalizada A vs. B**, con selección explícita de **referencia y candidata**
  (son papeles, no identificadores: la referencia no tiene por qué ser la más antigua).
- **Comparación rápida de las dos últimas pruebas**.
- **Comparaciones avanzadas por grupos**.
- **Filtros, búsqueda y ordenación**.
- **Identificación del perfil utilizado** en cada prueba.
- **Acciones destructivas con confirmación y alcance explícito.**

### Navegación y experiencia de usuario

- **Separar resumen, historial, comparación y grabación** en zonas distintas.
- **Evitar pantallas verticales interminables.**
- **Eliminar botones importantes escondidos tras el scroll.**
- **Reducir ruido visual e información redundante.**
- **Mostrar la información técnica avanzada bajo demanda.**
- **Mejorar contraste, jerarquía visual y accesibilidad.**

Pendientes visuales ya detectados que caen aquí (registrados, sin tocar):

- Decimales excesivos en memoria: `31.258648` en vez de `31,3 GiB`.
- «Frames» para muestras periódicas de MangoHud (el arreglo de fondo es de 5.4; el término
  visible, de aquí).
- Diagnóstico de shaders demasiado categórico (fondo en 5.4).
- Etiqueta «Planificador de CPU».
- El formulario de benchmarks es demasiado alto y «Analizar y guardar» queda lejos.
- Aprovechamiento del espacio horizontal.

### Asistente de benchmarks

- **Guiar al usuario desde la preparación hasta los resultados.**
- Permitir medir: la **configuración actual de Steam**, una **configuración base**, una
  **recomendada** o un **perfil personalizado**.
- **Explicar que `%command%` sin argumentos adicionales puede constituir una prueba base
  válida** (no es un comando «vacío» ni un error).
- **No confundir una configuración desconocida con una configuración vacía.** Es la misma
  regla de siempre: ausencia de evidencia no es evidencia de ausencia.
- **Mostrar claramente qué configuración se pretende evaluar y qué se ha podido verificar
  realmente.** Lo configurado no es lo verificado.
- **Guiar durante la apertura del juego y la grabación con MangoHud.**
- **Detectar y analizar las grabaciones automáticamente** cuando sea posible.
- **Guardar automáticamente cuando exista autorización del usuario.**
- **Evitar grabaciones duplicadas.**
- **Mantener la importación manual.**

### Simplificación de comparaciones

- **Evitar el doble guardado obligatorio**: hoy hay que guardar la grabación y después
  guardar la comparación con otro botón.
- **Generar las comparaciones a partir de las pruebas seleccionadas.**
- **Guardar una comparación como informe debe ser opcional.**
- **Mostrar el estado del proceso de benchmark de forma persistente** cuando exista evidencia
  suficiente.

### Principios de UX

- La aplicación **guía** al usuario; no le obliga a aprender su funcionamiento interno.
- Un principiante **no necesita** conocimientos previos de Proton, MangoHud ni argumentos de
  lanzamiento.
- El usuario avanzado **conserva** el acceso a las opciones y a las evidencias técnicas.
- **Automatizar tareas repetitivas con autorización.**
- **No sacrificar la fiabilidad de los datos para simplificar la interfaz.**
- **Reutilizar la arquitectura y funcionalidades existentes** siempre que se pueda.

---

## Reglas permanentes que la fase 6 no puede romper

Estas ya están implementadas y son anteriores a cualquier rediseño:

- **Ausencia de evidencia nunca se convierte en evidencia de ausencia.**
- **Lo configurado no es lo verificado.** El histórico se marca con `posterior_al_arranque`.
- **Procedencia siempre visible**: no se presenta un dato inferido como observado.
- **Una sola regla de identidad de juego** en todo el módulo (AppID manda sobre nombre).
- **Nada de estados paralelos**: el motor Rust es la única fuente de verdad del comando.
- **Ningún interruptor que parezca hacer algo y no lo haga.**
- **Las acciones destructivas respetan el alcance anunciado**, también para las pruebas
  antiguas sin AppID.
- El formato persistido de perfiles y los benchmarks históricos **no se reescriben**.
