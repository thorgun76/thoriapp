# Sesiones de ejecución (subfase 5.3)

Cómo Thoriapp sabe que un juego estuvo corriendo, qué puede afirmar de esa ejecución y qué
no. Es el documento de decisiones de la subfase; el código está en `src-tauri/src/`.

## El problema

Antes de 5.3, la captura era tardía por construcción: Thoriapp esperaba a que apareciera el
`_summary.csv`, que MangoHud escribe **al terminar** la grabación. Todo lo que se leía
después (perfil cargado, opciones de Steam) describía el momento del análisis, no el de la
partida. Si el usuario cambiaba de perfil entre medias, la instantánea lo registraba como si
hubiera estado así durante el juego.

## La regla que lo gobierna todo

> La ausencia de evidencia nunca se convierte en evidencia de ausencia.

Tiene dos consecuencias prácticas:

1. **No ver el proceso no es que no corra.** En un contenedor, con otro usuario o con
   `hidepid`, `/proc` no muestra lo que hay. Eso se registra como `SinVisibilidad`, y la
   interfaz dice «no consta», nunca «el juego no está corriendo».
2. **Ver el proceso no es que la optimización funcione.** Un proceso bajo Proton demuestra
   que había un proceso bajo Proton. No demuestra que FSR, NTSYNC o MangoHud entraran en
   acción. Por eso `ejecucion_verificada` sigue en `null` y `VerificadoEnEjecucion` sin
   usar.

## Qué se observa

Thoriapp **no lanza juegos** y no recibe avisos del sistema. Mira `/proc` cada pocos
segundos, que es lo máximo que se puede hacer sin privilegios.

| Dato | ¿Observable? | Cómo |
|---|---|---|
| Proceso del juego | Sí, mismo usuario | `comm`, `exe`, `stat` |
| Instante de arranque | Sí | `starttime` (campo 22) + `btime` |
| AppID del entorno | Sí, si lo heredó | `SteamAppId` / `SteamGameId` |
| Bajo Proton | Inferido del árbol | `comm` propio y de los padres |
| Configuración de Thoriapp | Sí | La fija el frontend al cambiar |
| Opciones de Steam | Sí (lectura) | `localconfig.vdf` |
| Inicio de la grabación | Aproximado | Nombre del CSV + `elapsed` |
| Fin de la grabación | Aproximado | Aparición del `_summary.csv` |

## Identidad del juego

Se cruza en este orden, y el primero que resuelve sin ambigüedad gana:

1. **Entorno del proceso**, si `SteamAppId` y `SteamGameId` coinciden. Si discrepan —y pasa,
   porque `SteamAppId` a veces lleva el ID del cliente— queda **ambiguo con los dos
   candidatos**: no se elige.
2. **Carpeta de instalación**: si el ejecutable vive en
   `.../steamapps/common/<dir>` y ese `<dir>` es de un solo juego instalado.
3. **Nombre**, exigiendo coincidencia única. Se marca **inferida**, no observada.

El juego abierto en la interfaz no entra en ninguna de las tres. Si no se resuelve, se
ofrece asignación manual, que se guarda como `ConfirmadoPorUsuario`.

El `.acf` no lista ejecutables; solo el `installdir`. No se deduce lo que no está.

## Asociación grabación ↔ sesión

Se exigen **las dos cosas**:

- **Temporal**: la ventana de la grabación (nombre del CSV + `elapsed`) cae dentro de la vida
  del proceso, con ±60 s de holgura por desfases de reloj.
- **Identidad**: si los dos lados dicen un AppID, tiene que coincidir.

Resultado en cinco grados: `Verificada`, `Probable`, `UnicaSesionDelJuego`, `Ambigua`,
`NoCoincide`, `NoComprobable`. Solo los tres primeros permiten guardar el enlace.

Una grabación de hace un mes no se cuelga de un proceso de hoy aunque el juego se llame
igual: la ventana temporal lo impide.

## Persistencia

`sessions.json` (junto a `benchmarks.json`), con el mismo `leer_datos` / `escribir_atomico` /
cerrojo de proceso de 5.1:

- Sin `version` = primera versión.
- Versión futura ⇒ se rechaza **sin** copia (el archivo está bien; la app es la antigua).
- Corrupto o vacío ⇒ se avisa **y** se guarda copia `.bak`; nunca se sobrescribe en silencio.
- Máximo 100 sesiones; se conservan primero las abiertas.

El `sesion_id` del benchmark es **solo un enlace**. Todo lo interpretable está en la
instantánea (subfase 5.2), así que borrar sesiones no deja pruebas ilegibles.

## Cómo se decide que un proceso es un juego

La primera versión buscaba **un proceso que se llamara como el juego**. Funciona en nativo y
falla en todos los de Proton, donde el proceso se llama `GameThread` y el nombre del juego
solo existe en la línea de comandos. Ahora se recorre `/proc` entero y se agrupa:

| Evidencia | Qué aporta |
|---|---|
| `SteamAppId` = `SteamGameId` | el AppID |
| `SteamLaunch AppId=` en argumentos | el AppID, cuando el entorno no lo trae |
| `.exe` en la línea de comandos | el nombre del juego |
| ruta `compatibilitytools.d` / `proton_*` | ejecución bajo Proton |
| `steamapps/common/<dir>` | nombre a partir de la ruta |
| `ppid` compartido | una sesión por juego, no por proceso |

Ninguna aislada basta. Dos AppID distintos ⇒ ambiguo, **no se elige**. Un auxiliar conocido
(`wineserver`, `winedevice`, `steamwebhelper`, `reaper`, `python3`) nunca abre sesión, aunque
tenga entorno de Steam. **El orden del filtro importa**: primero la evidencia y después el
descarte; descartar antes de leer la línea de comandos fue exactamente el defecto que dejó a
Palworld sin detectar.

## Privacidad

- Solo procesos **del mismo usuario**.
- Lista blanca de cuatro variables: `SteamAppId`, `SteamGameId`, `SteamClientLaunch`,
  `PROTON_VERSION`. Cada valor se valida (numérico, longitud, caracteres).
- De `cmdline` se extraen **solo** dos cosas: el nombre del ejecutable del juego y un
  `AppId=` numérico. El texto completo de la línea de comandos no se guarda, no se registra
  y no viaja en ninguna respuesta IPC; tampoco sus opciones (`-dx12`, rutas del usuario).
  Ignorar `cmdline` del todo no era una opción: en un juego de Proton el nombre del juego
  **solo** aparece ahí.
- `environ` se lee **solo** de los procesos que ya pasaron un filtro barato, no de todos.
- El observador es de solo lectura.

Hay un test que mete un entorno lleno de claves y comprueba que no escapa ninguna.

## Rendimiento

- Pasada completa sobre ~70 entradas de `/proc`: **0,55–0,66 ms**.
- En la app real: **1,6–2,3 ms por pasada**.
- Sondeo adaptativo: 5 s buscando, 10 s siguiendo, 30 s sin visibilidad, 0 s apagado. Si una
  pasada se lleva más de un cuarto del intervalo, la espera se estira para no comerse la CPU
  que se está midiendo.

## Diagnóstico (opcional)

La detección falló dos veces en un equipo real mientras las pruebas con datos simulados
pasaban. Para no volver a arreglar a ciegas hay un volcado opcional:

```bash
THORIAPP_DIAGNOSTICO_PROCESOS=/ruta/diagnostico.jsonl thoriapp
```

Sin la variable no se ejecuta nada: ni una lectura de `/proc` de más ni una escritura. Con
ella, cada pasada añade una línea JSON con lo que hizo y por qué:

| Campo | Responde |
|---|---|
| `procesos_vistos`, `inaccesibles` | cuánto se pudo mirar |
| `candidatos[]` | `comm`, ejecutable, ruta del juego, AppID del entorno y de argumentos |
| `descartes[]` | el motivo exacto por el que se rechazó cada proceso |
| `sesiones[]` | sesión creada o rechazada, y con qué AppID |
| `nota` | de qué rama sale la línea |

No registra el entorno completo ni la línea de comandos entera: solo las cuatro variables de
la lista blanca y el nombre del ejecutable con su AppID numérico. No pide permisos y no tiene
excepciones por juego.

## Qué añade y qué no

Añade: `sessions.rs`, `proceso_linux.rs`, `sessions_service.rs`, siete comandos IPC, el
interruptor del pie de la barra lateral, el bloque «Sesión de ejecución» dentro de
`InstantaneaCard` y el arreglo de `norm_nombre`.

No cambia: fórmulas, métricas, puntuaciones, veredictos, perfiles, formato persistido de
perfiles, benchmarks históricos ni el sistema de captura. Eso es 5.4 y fase 6.

## Quinta corrección: envoltorios, evidencia repetida, configuración y mezcla de juegos

Viene de una validación real con **Deskrawl** (AppID 4623570, Prueba 2). Los cuatro puntos
eran defectos de verdad, y ninguno se arregla con más listas de nombres.

### 1. El proceso representativo era un envoltorio (`srt-bwrap`)

`elegir_principal` repartía puntos con números sueltos (AppID +100, envoltorio −150) y
listas (`LANZADORES`, `AUXILIARES`). Cada envoltorio nuevo exigía una excepción: ayer
`systemd-inhibit`, hoy `srt-bwrap`. Eso no termina nunca, porque el árbol de Steam cambia con
cada runtime.

Se sustituye por **papeles** (`PapelEnElLanzamiento`), deducidos del árbol y no del nombre:

| Papel | Cómo se reconoce |
|---|---|
| `EjecutableDelJuego` | Su `.exe` da nombre a la carpeta del juego (`Deskrawl.exe` en `steamapps/common/Deskrawl`) y ningún descendiente ejecuta el juego |
| `Envoltorio` | Tiene hijos en el grupo y no ejecuta el juego |
| `Ancla` | Es la raíz de la cadena (su padre no está en el grupo) |
| `Auxiliar` | Lo demás |

Solo un `EjecutableDelJuego` puede representar al grupo. Si no hay ninguno, `elegir_principal`
devuelve `None` y se dice «identidad desconocida»: es preferible a presentar un envoltorio.

Dos trampas que costaron encontrar, y que conviene no volver a pisar:

- Un envoltorio **también** menciona el `.exe` en su línea de comandos (lo pasa como
  argumento). Mirar solo eso lo ascendía a juego.
- `UnityCrashHandler64.exe` vive **dentro** de la carpeta del juego y no es el juego. Por eso
  no basta la carpeta: se exige que el nombre del `.exe` coincida con ella, admitiendo sufijos
  de plataforma (`Palworld-Win64-Shipping` ↔ `Palworld`).

`LANZADORES` y `es_lanzador` se han eliminado: ya no hay lista de envoltorios.

### 2. Evidencia repetida en la representación

La evidencia se acumulaba por miembro, así que «corre bajo Proton» salía cuatro veces y
«ejecuta Deskrawl» dos. `nombres.dedup()` solo quitaba duplicados **adyacentes**.

Se deduplica `evidencia` y `nombres` conservando el orden de aparición. Los datos originales
(`miembros`) no se tocan: solo se limpia lo que se lee.

### 3. La configuración no era la del arranque

La sesión de Deskrawl empezó a las 14:16:01 y su configuración figuraba capturada a las
14:20:30. La marca `posterior_al_arranque` **ya venía bien del backend, pero ningún componente
la mostraba**: se leía como si fuera lo puesto al empezar a jugar.

Se arregla en la representación: `InstantaneaCard` avisa de que la configuración se capturó
después del arranque y dice cuándo empezó la sesión. No se crea un segundo estado ni se
duplica la configuración.

### 4. Historial y comparaciones podían mezclar juegos

`compare_benchmarks` calculaba todo y solo añadía un aviso al final diciendo que la
comparación «no es válida», pero **devolvía el resultado igual**. Y `juego_coincide` caía al
nombre cuando faltaba un AppID, así que dos juegos homónimos podían mezclarse.

Ahora hay una sola regla, `coinciden_por_identidad`, con dos usos explícitos:

- **Historial** (no estricto): si la grabación antigua no tiene AppID, el nombre es la única
  pista. Descartarla escondería pruebas reales.
- **Comparación** (estricta): con AppID en un lado y ninguno en el otro no hay evidencia de
  que sea el mismo juego. `compare_benchmarks` devuelve error y no calcula nada.

## Sexta corrección: la sesión no se sostiene con un auxiliar (5.3)

La validación real en CachyOS dejó una sesión de Tattoo Tycoon `OBSERVADA` durante horas con
el juego cerrado. Su `vivos_de_la_ejecucion` tenía 13 claves; de los 13 procesos, 11 ya no
existían. Los dos que seguían en `/proc` eran `dolphin` (PID 16137, el gestor de archivos de
KDE desde el que se lanzó el juego) y `srt-logger` (PID 127349, infraestructura de Steam).
Ninguno ejecuta el juego.

### La causa: cuatro cosas distintas guardadas en un solo campo

La primera versión de `vivos_de_la_ejecucion` guardaba **todos** los miembros del grupo.
Con eso la continuidad funcionaba, pero se mezclaban cuatro conceptos que no son lo mismo:

| Concepto | Qué responde | Campo |
|---|---|---|
| Topología | ¿qué procesos cuelgan del mismo árbol? | `miembros` del grupo |
| Identidad del juego | ¿qué juego es? | `identidad` (AppID, nombre, carpeta) |
| Continuidad de instancia | ¿es la misma partida de antes? | `arbol_del_lanzamiento` |
| Ejecución activa | ¿el juego se está ejecutando **ahora**? | `vivos_de_la_ejecucion` |

Al guardarlos todos juntos, la supervivencia de un auxiliar acreditaba ejecución. La
comprobación del ciclo de vida era un `.any()` sobre las 13 claves: bastaba **una** viva, y
`pasadas_sin_verlo` volvía a 0 en cada pasada.

### Las reglas nuevas

1. **`vivos_de_la_ejecucion` solo contiene procesos que acreditan ejecución.** Se acredita con
   `ejecuta_de_verdad == Some(true)`, o siendo el representante con evidencia propia. Un
   auxiliar (`auxiliar`) o un ancestro genérico nunca acredita. Ni el AppID, ni el catálogo, ni
   el prefijo de Wine bastan por separado: los envoltorios los **heredan**.
2. **`arbol_del_lanzamiento` guarda los envoltorios**, y sirve solo para reconocer la misma
   partida cuando cambia el representante. No decide nunca si la sesión sigue viva.
3. **El ciclo de vida usa el escaneo de la pasada.** Un escaneo válido con 0 grupos es la
   prueba de que no hay juego corriendo; `/proc` no puede contradecirlo con un envoltorio
   superviviente. Sin visibilidad **no** se cierra nada.
4. **`ultima_observacion` solo avanza cuando se ve el juego**, no cuando sobrevive un auxiliar.
5. **Las sesiones persistidas se revalidan al cargar.** Las que escribieron versiones
   anteriores llevan listas contaminadas y no se usan como prueba: si el escaneo actual no ve
   el juego, la sesión pasa a `VisibilidadPerdida` y el contador normal decide. No se borra
   nada, no se inventa un cierre y no se tocan identificadores ni fechas.

### Zombies

`stat()` leía el campo de estado de `/proc/<pid>/stat` y lo descartaba con `let _ =`. Un
proceso terminado pero no recogido por su padre conserva el **mismo** `starttime`, así que
`sigue_vivo` lo daba por vivo para siempre. Ahora el estado se lee y `Z`, `X` y `x` no
acreditan vida. `EstadoDeLaClave` distingue cinco casos: vivo, terminado, PID reutilizado,
zombi e inaccesible; «no lo veo» no es «terminó».

### Lo que no se hace

- No hay listas negras de `dolphin` ni de `srt-logger`: el defecto era estructural.
- No se alarga ni se acorta ningún plazo para esconder sesiones residuales.
- No se reescriben sesiones históricas cerradas ni se migran datos guardados.

## Séptima corrección: el lanzamiento y sus raíces (5.3)

La segunda validación real en CachyOS dejó dos defectos: cuatro juegos abrían **dos sesiones**
(Tattoo Tycoon, Eslabong, Deskrawl, Palworld) y Starsector abría una **nueva cada pasada**.

### La raíz de lanzamiento tiene que ser la misma para toda la cadena

`agrupar_con_topologia` agrupa por `(raíz, AppID)`. La raíz se calculaba subiendo por los
padres y parando donde tocara, así que dos procesos del **mismo** lanzamiento obtenían raíces
distintas. Medido en el equipo (registro 1219):

| Proceso | `ppid` | Raíz antigua |
|---|---|---|
| `426782` wine64-preloader | 426674 | 426577 |
| `426577` systemd-inhibit | 426576 | 426576 |

Con raíces distintas, el AppID común no servía de nada: dos grupos, dos sesiones. Por eso
Deskrawl y Palworld se duplicaban **aunque el AppID coincidiera**.

Ahora se sube siempre hasta el **eslabón propio más alto**: el primero por debajo de la
infraestructura. Toda la cadena obtiene el mismo nodo.

### Qué es infraestructura

Se decide por estructura, nunca por una lista de nombres:

- Genérico **con hijos no genéricos** → encabeza **una** ejecución y es su ancla. Es el caso
  del `systemd-inhibit` que Steam lanza para un juego.
- Genérico cuyos hijos son todos genéricos → encabeza **varias** y no une nada. Es el
  `systemd` de usuario.

Los `systemd` de usuario no entran en el grupo del juego, y PID 1 no se usa como raíz
unificadora. Dos juegos con ancestro común conservan grupos distintos: lo comprueba
`dos_lanzamientos_bajo_el_mismo_systemd_no_se_fusionan`.

### La evidencia por directorio de trabajo

`acredita_ejecucion` pedía `ejecuta_de_verdad`, AppID, argumentos o carpeta declarada. Un juego
nativo lanzado por intérprete (`lutris-wrapper → starsector.sh → java`) no cumple ninguna: su
única señal es el directorio en el que trabaja. Sin admitirla, `vivos_de_la_ejecucion` quedaba
vacío y `paso()` creaba una sesión nueva en cada pasada con el mismo PID y el mismo
`starttime`.

Se admite el `cwd` con la condición **estricta**: solo cuando hay una relación verificable con
una carpeta conocida —la que el proceso declara, la de `install_dirs` del grupo, o la que la
correlación del catálogo dejó escrita al abrir la sesión—. No basta el parecido de nombres, y
estar dentro de la carpeta no convierte en ejecutable del juego a cualquier proceso. No hay
excepción para Java ni para Starsector.

### Una regla que hubo que corregir en el intento

La primera versión de la regla de infraestructura usaba «más de una rama de lanzamiento
debajo». El test de dos juegos simultáneos la tumbó: fusionaba dos ejecuciones que compartían
un `systemd-inhibit` padre. Se corrigió la regla —cada ancla se queda con su ejecución—, no el
test. Es el motivo de mantener ese test con datos reales: la primera regla «razonable» estaba
mal.

## Cierre de 5.3: qué queda garantizado

La subfase **se cierra** con una limitación declarada. El observador es **experimental y
opcional, apagado por defecto**: mientras no se active no se lee `/proc`.

### Validado en el equipo

Detección individual correcta en **Steam, Heroic, Heronite y Lutris**. Simultaneidad correcta
en **Steam + Steam** (Palworld + Deskrawl) y **Lutris + Steam** (Starsector + Deskrawl). Los
seis juegos cierran bien, y tras reiniciar Thoriapp no aparecen sesiones fantasma.

### No garantizado

**Eslabong (Heronite) + Tattoo Tycoon (Heroic) simultáneos**: la detección es incompleta en
ambos órdenes de lanzamiento. La causa exacta **no está demostrada**.

Se descartó con datos la hipótesis de fusión por `systemd` compartido: los árboles reales
tienen raíces separadas (procesos huérfanos adoptados por init). Con topologías y catálogos
reales el pipeline forma dos grupos independientes.

**El diagnóstico existente no basta para reabrir el caso.** Los registros guardados no incluyen
una pasada con ambas ejecuciones completas y sus catálogos simultáneos, así que no permiten
aislar el punto de pérdida. Reabrirlo exige capturar **una pasada nueva** con el observador
activo en ese escenario.

### Reglas que se mantienen

No se añadió ninguna excepción por nombre de proceso, no se relajó `dentro_del_juego` y no se
admiten representantes sin evidencia suficiente: cuando no hay prueba, el resultado correcto
sigue siendo **identidad desconocida**.
