/**
 * Umbrales de comparación que comparten el frontend y el backend.
 *
 * ## Por qué son dos conceptos y no uno
 *
 * Hay **dos** márgenes mínimos en la aplicación, y es importante no confundirlos:
 *
 * 1. **`MIN_NOISE_PERCENT` (`crate::benchmarks::MIN_NOISE_RS`, 3 %).** Suelo del margen
 *    que se *calcula* a partir de los datos: si la estimación de ruido de las dos
 *    grabaciones sale por debajo (o no se puede calcular), nunca se asume menos de 3 %.
 *    Es una decisión sobre **cuánto ruido creemos que existe**.
 *
 * 2. **`MIN_TIE_PERCENT` (5 %).** Diferencia mínima para **afirmar** un cambio en la
 *    interfaz. Es una decisión sobre **cuánto rigor exigimos para hablar de mejora**,
 *    más estricta porque un falso positivo aquí le hace cambiar la configuración al
 *    usuario.
 *
 * 3 % < 5 %: el margen calculado es el suelo de la estimación, y el 5 % es el listón
 * para decidir. Tenerlos separados es deliberado; tenerlos con nombres parecidos era el
 * problema. `MIN_TIE` debe ser siempre ≥ `MIN_NOISE`, y hay un test que lo comprueba.
 *
 * El backend usa `MIN_NOISE_RS` para su propio texto de conclusión. Los dos valores
 * están replicados (Rust y TS no comparten constantes), así que el test de consistencia
 * los compara en lugar de confiar en que alguien los mantenga sincronizados.
 */

/**
 * Suelo del margen de ruido estimado (%), igual que `MIN_NOISE` en `benchmarks.rs`.
 *
 * El nombre dice "suelo" y no "mínimo" a propósito: no es el mínimo que se exige para
 * afirmar nada, es el mínimo que se usa como estimación.
 */
export const MIN_NOISE_PERCENT = 3;

/**
 * Diferencia mínima (%) para considerar que una métrica cambió.
 *
 * MangoHud y el propio juego varían más que esto entre partidas, así que por debajo de
 * este listón la interfaz dice «sin cambio concluyente» aunque el margen calculado sea
 * menor.
 */
export const MIN_TIE_PERCENT = 5;

/**
 * Marcador de «no hay datos temporales»: usado en tests y en la UI.
 *
 * C3-26: **no se atribuye una causa**. Que falte la serie no prueba que la prueba sea
 * anterior al almacenamiento de tiempos: también ocurre con un registro importado, con un
 * log sin eje temporal o con una grabación que no produjo muestras. Afirmar lo primero
 * inventaba una procedencia que el dato no sostiene, así que el texto solo dice lo que
 * consta: que no hay series guardadas.
 */
export const SIN_DATOS_TEMPORALES = 'Esta prueba no tiene guardados los tiempos por muestra, así que no hay gráfico temporal.';
