//! Motor puro de interpretación de comparaciones (bloque 5.4.1.C.1).
//!
//! Traduce una [`BenchmarkComparison`] —un contrato **numérico**— en una
//! [`EvaluacionDeComparacion`] —un contrato **semántico**: qué se puede afirmar de esa
//! comparación y con qué apoyo.
//!
//! ## Qué es este módulo y qué no es
//!
//! Es una función **pura**: sin reloj, sin disco, sin IPC y sin interfaz. Las condiciones
//! del experimento entran como parámetro; la fecha de evaluación la añade quien persiste
//! (subfase C.2), no el motor. Por eso no hay `Result`: todo caso de entrada admitido
//! tiene una representación válida, y lo que falta se representa como «no consta», nunca
//! como cero.
//!
//! Tampoco decide sobre grupos de grabaciones: la evaluación individual es independiente
//! de cuántos grupos haya disponibles en cada momento. Un grupo no concluyente no es
//! evidencia negativa de una diferencia observada entre dos grabaciones.
//!
//! ## Las cinco dimensiones que no se pueden mezclar
//!
//! - **magnitud**: los números observados (`antes`, `despues`, `diferencia`);
//! - **dirección**: si el número sube o baja, sin decir si eso es bueno;
//! - **apoyo**: con qué muestras se calculó (heredado de `ApoyoDePercentil`);
//! - **elegibilidad**: si el criterio operativo permite que esa métrica **decida**;
//! - **preferencia**: qué lado favorece el criterio aplicado, y solo si es elegible.
//!
//! Una métrica puede tener magnitud y dirección y **no** ser elegible: entonces se
//! registra, se muestra, y no decide. Confundir esas dos cosas es lo que convertía una
//! pérdida observada en un «sin cambio» silencioso.
//!
//! ## Límites conocidos (no son defectos introducidos aquí)
//!
//! - El margen operativo mide el **error de estimación dentro de cada grabación**
//!   (`noise_percent`, variación entre bloques contiguos). Superarlo **no** demuestra una
//!   mejora reproducible entre partidas ni significación estadística validada: solo que
//!   las dos grabaciones midieron distinto más allá del ruido de muestreo.
//! - Un ruido observado de cero **no** demuestra ausencia de variabilidad real: significa
//!   que los bloques no detectaron variación en esa grabación.
//! - La elegibilidad del 0,1 % low es el criterio heredado de `low01Reliability`
//!   (≥10 muestras extremas). Es un **criterio operativo**, no una demostración de
//!   fiabilidad estadística validada.
//! - **No existe** un criterio validado de elegibilidad para el 1 % low: hoy decide
//!   cuando sus valores son válidos y su apoyo consta. Su revisión corresponde a 5.4.3.

use crate::benchmarks::{
    ApoyoDeMetrica, ApoyoDePercentil, Benchmark, BenchmarkComparison, CodigoDeEvaluacion,
    CompatibilidadMetodologica, CondicionesDeComparacion, DireccionDeCambio, EvaluacionDeMetrica,
    EvaluacionPersistida, IdDeBenchmark, MetodologiaCalculo, PreferenciaDeMetrica,
    VersionDeInterpretacion,
};

/// Margen mínimo de interpretación individual (%), por encima del suelo de estimación de
/// la aplicación (`benchmarks::MIN_NOISE`, 3 %).
///
/// Son **dos conceptos distintos**: `MIN_NOISE` es el suelo de la *estimación* de ruido;
/// esto es el margen a partir del cual se *afirma* un cambio en una comparación de dos
/// grabaciones. No se aplica a las comparaciones de grupos (`stats.rs`), que tienen sus
/// propios suelos.
///
/// Quien evalúa lo pasa en `CondicionesDeComparacion::margen_porcentaje`; este valor es el
/// que la aplicación usa como política. El motor **no** lo aplica por su cuenta: recibe el
/// margen ya decidido para que lo guardado refleje lo aplicado, no la regla de hoy.
pub const MIN_TIE_PERCENT: f64 = 5.0;

/// Metodología de interpretación que produce este módulo. Es la que se escribe en
/// `EvaluacionPersistida::metodologia_interpretacion`.
pub const METODOLOGIA_INTERPRETACION: &str = VersionDeInterpretacion::V1;

/// Muestras extremas mínimas para que el 0,1 % low se considere elegible.
///
/// Heredado literalmente de `low01Reliability` (`src/lib/estabilidad.ts`): no es un
/// mínimo estadístico nuevo ni validado, es el criterio operativo que ya decidía si el
/// 0,1 % low era presentable. Se reutiliza para decidir si puede **decidir**.
pub const MUESTRAS_EXTREMAS_MINIMAS: f64 = 10.0;

/// Claves de métrica. Son vocabulario cerrado y estable: viajan en disco.
pub const CLAVE_AVG_FPS: &str = "avg_fps";
pub const CLAVE_FPS_1_PERCENT_LOW: &str = "fps_1_percent_low";
pub const CLAVE_FPS_0_1_PERCENT_LOW: &str = "fps_0_1_percent_low";

/// Códigos de motivo y limitación que produce esta metodología.
///
/// Son códigos, no frases: la interfaz los traduce. Un código desconocido se conserva y se
/// muestra crudo, así que añadir uno nuevo no invalida las evaluaciones ya guardadas.
pub mod codigos {
    /// Participó en la decisión del estado.
    ///
    /// Es distinto de tener preferencia: una métrica **dentro del margen** participa (es
    /// la razón de que el estado sea `sin_cambio`) sin favorecer a ningún lado. Por eso la
    /// participación se registra aparte y no se deduce de `preferencia`.
    pub const PARTICIPO: &str = "participo";
    /// Dentro del margen operativo.
    pub const DENTRO_DEL_MARGEN: &str = "dentro_del_margen";
    /// Fuera del margen, a favor de la candidata.
    pub const FAVORECE_CANDIDATA: &str = "favorece_candidata";
    /// Fuera del margen, a favor de la referencia.
    pub const FAVORECE_REFERENCIA: &str = "favorece_referencia";
    /// Sin valor en alguna de las dos grabaciones.
    pub const SIN_DATOS: &str = "sin_datos";
    /// El valor existe pero no es un número utilizable (no finito o no positivo donde debe
    /// serlo). No se convierte en cero.
    pub const VALOR_NO_VALIDO: &str = "valor_no_valido";

    /// La métrica existe, tiene magnitud, pero su apoyo no permite que decida.
    pub const NO_ELEGIBLE: &str = "no_elegible";
    /// No se puede saber si el apoyo alcanza: el recuento de muestras no consta.
    pub const APOYO_NO_CONSTA: &str = "apoyo_no_consta";
    /// El apoyo consta y no alcanza el criterio operativo.
    pub const APOYO_INSUFICIENTE: &str = "apoyo_insuficiente";

    /// La evaluación se hizo solo con una parte de las métricas.
    pub const EVALUACION_PARCIAL: &str = "evaluacion_parcial";
    /// Hay una pérdida observada en una métrica excluida de la decisión.
    pub const PERDIDA_OBSERVADA_SIN_EVALUAR: &str = "perdida_observada_sin_evaluar";
    /// Una sola métrica decidió: no permite una conclusión sobre toda la experiencia.
    pub const COBERTURA_INSUFICIENTE_PARA_CONCLUSION_GLOBAL: &str =
        "cobertura_insuficiente_para_conclusion_global";
    /// Ninguna métrica tiene valor utilizable.
    pub const SIN_METRICAS_MEDIBLES: &str = "sin_metricas_medibles";
    /// No se aplicó ningún margen: no se juzgó la diferencia.
    pub const SIN_MARGEN: &str = "sin_margen";
    /// La compatibilidad metodológica no está establecida: solo cabe describir.
    pub const COMPATIBILIDAD_NO_ESTABLECIDA: &str = "compatibilidad_no_establecida";

    // --- Niveles de **confianza** ---------------------------------------------------
    //
    // Vocabulario propio, cerrado, distinto del de estado. Un estado dice *qué* cambió;
    // una confianza dice *qué sostiene* esa afirmación. Escribir un estado en el campo
    // `confianza` es un error: `mejora` no es una confianza.

    /// La diferencia cabe dentro del margen: no se distingue de cero.
    pub const CONFIANZA_NO_CONCLUYENTE: &str = "no_concluyente";
    /// La diferencia supera el margen, pero una sola grabación por lado no la confirma.
    pub const CONFIANZA_OBSERVADA: &str = "observada";
    /// Faltan datos para afirmar nada.
    pub const CONFIANZA_INSUFICIENTE: &str = "insuficiente";
}

/// Resultado de evaluar una comparación: el contrato semántico.
///
/// Es el mismo tipo que se persiste, **sin** la fecha ni los identificadores: esos
/// describen el registro, no el significado, y los añade `a_persistir` cuando se guarda.
/// Se elige así (y no con `serde(flatten)`) para no tocar la representación JSON aprobada
/// en el bloque B: el registro se compone campo a campo, con los mismos nombres y el mismo
/// anidamiento.
pub type EvaluacionDeComparacion = EvaluacionPersistida;

/// Cómo entra una métrica en la decisión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Elegibilidad {
    /// Tiene valor y apoyo suficiente: decide.
    Elegible,
    /// Tiene valor, pero el apoyo consta como insuficiente: no decide.
    NoElegible,
    /// Tiene valor, pero no consta con cuántas muestras se calculó: no decide (no se
    /// inventa el apoyo).
    ApoyoNoConsta,
}

/// Lo que se sabe de una métrica antes de decidir el estado global.
struct Metricas {
    avg_fps: EvaluacionDeMetrica,
    fps_1_percent_low: EvaluacionDeMetrica,
    fps_0_1_percent_low: EvaluacionDeMetrica,
    elegibilidad_avg_fps: Elegibilidad,
    elegibilidad_1_percent_low: Elegibilidad,
    elegibilidad_0_1_percent_low: Elegibilidad,
}

/// Evalúa una comparación con las condiciones dadas.
///
/// `condiciones` entra **explícita**: la compatibilidad metodológica y la variable
/// evaluada las determina quien llama, no se infieren aquí de una diferencia de comandos.
pub fn evaluar(
    comparacion: &BenchmarkComparison,
    condiciones: &CondicionesDeComparacion,
) -> EvaluacionDeComparacion {
    let margen = condiciones.margen_porcentaje;

    // La compatibilidad se toma de los **benchmarks**, no de la condición: `condiciones`
    // llega de fuera y no puede fabricar una comparabilidad que las grabaciones no
    // establecen. Si la condición dice «establecida» pero los benchmarks no la sostienen,
    // gana el más conservador de los dos.
    let compatibilidad = match (
        condiciones.compatibilidad_metodologica,
        compatibilidad_metodologica(comparacion),
    ) {
        (CompatibilidadMetodologica::Establecida, CompatibilidadMetodologica::Establecida) => {
            CompatibilidadMetodologica::Establecida
        }
        _ => CompatibilidadMetodologica::NoEstablecida,
    };
    let compatibilidad_establecida = compatibilidad == CompatibilidadMetodologica::Establecida;
    let condiciones = &CondicionesDeComparacion {
        compatibilidad_metodologica: compatibilidad,
        ..condiciones.clone()
    };
    let mut metricas = evaluar_metricas(comparacion, margen);

    // Sin compatibilidad metodológica no se proclama ninguna preferencia, tampoco **dentro**
    // de cada métrica. Dejar una `FavorDeCandidata` suelta permitiría que cualquier consumidor
    // posterior (o una pantalla) coronase un ganador por un veredicto que no se ha dado.
    // La magnitud y la dirección observadas se conservan: describir no es decidir.
    if !compatibilidad_establecida {
        for metrica in [
            &mut metricas.avg_fps,
            &mut metricas.fps_1_percent_low,
            &mut metricas.fps_0_1_percent_low,
        ] {
            metrica.preferencia = PreferenciaDeMetrica::NoEstablecida;
            // Se retiran tanto la participación como los motivos de preferencia: dejarlos
            // afirmaría por la puerta de atrás lo que el campo acaba de negar.
            metrica.motivos.retain(|c| {
                c.0 != codigos::PARTICIPO
                    && c.0 != codigos::FAVORECE_CANDIDATA
                    && c.0 != codigos::FAVORECE_REFERENCIA
            });
        }
    }

    let mut motivos = Vec::new();
    let mut limitaciones = Vec::new();

    let decisorias = [
        (
            CLAVE_AVG_FPS,
            &metricas.avg_fps,
            metricas.elegibilidad_avg_fps,
        ),
        (
            CLAVE_FPS_1_PERCENT_LOW,
            &metricas.fps_1_percent_low,
            metricas.elegibilidad_1_percent_low,
        ),
        (
            CLAVE_FPS_0_1_PERCENT_LOW,
            &metricas.fps_0_1_percent_low,
            metricas.elegibilidad_0_1_percent_low,
        ),
    ];

    // Cobertura: cuántas de las tres métricas pueden decidir y cuántas tienen valor.
    let elegibles_medibles = decisorias
        .iter()
        .filter(|(_, m, e)| {
            *e == Elegibilidad::Elegible && m.direccion != DireccionDeCambio::NoConsta
        })
        .count();
    let excluidas_con_cambio = decisorias
        .iter()
        .filter(|(_, m, e)| {
            *e != Elegibilidad::Elegible && m.direccion != DireccionDeCambio::NoConsta
        })
        .count();

    for (_clave, metrica, elegibilidad) in decisorias {
        if elegibilidad != Elegibilidad::Elegible {
            push_unico(
                &mut limitaciones,
                CodigoDeEvaluacion(codigos::NO_ELEGIBLE.to_string()),
            );
            match elegibilidad {
                Elegibilidad::NoElegible => push_unico(
                    &mut limitaciones,
                    CodigoDeEvaluacion(codigos::APOYO_INSUFICIENTE.to_string()),
                ),
                Elegibilidad::ApoyoNoConsta => push_unico(
                    &mut limitaciones,
                    CodigoDeEvaluacion(codigos::APOYO_NO_CONSTA.to_string()),
                ),
                Elegibilidad::Elegible => {}
            }
        }
        // Una pérdida observada en una métrica que no decide **no desaparece**: se registra
        // como limitación para que la presentación no pueda omitirla.
        //
        // Se mira `direccion` y no `preferencia` a propósito: en una métrica no elegible la
        // preferencia queda sin establecer (no decide), pero la magnitud y el sentido del
        // cambio sí se observaron, y una bajada en un `*_low` es una pérdida observada.
        if elegibilidad != Elegibilidad::Elegible
            && metrica.preferencia == PreferenciaDeMetrica::NoEstablecida
            && metrica.direccion == DireccionDeCambio::Baja
        {
            push_unico(
                &mut limitaciones,
                CodigoDeEvaluacion(codigos::PERDIDA_OBSERVADA_SIN_EVALUAR.to_string()),
            );
        }
    }

    if excluidas_con_cambio > 0 || elegibles_medibles < decisorias.len() {
        push_unico(
            &mut limitaciones,
            CodigoDeEvaluacion(codigos::EVALUACION_PARCIAL.to_string()),
        );
    }

    // Estados, por orden estricto de prioridad. Las condiciones son excluyentes: cada
    // rama corta la anterior.
    let (estado, confianza) = if !compatibilidad_establecida {
        push_unico(
            &mut motivos,
            CodigoDeEvaluacion(codigos::COMPATIBILIDAD_NO_ESTABLECIDA.to_string()),
        );
        // Sin compatibilidad establecida no se afirma dirección: la evaluación es
        // descriptiva y **no** declara confianza interpretativa. No es «provisional»
        // porque no hay ninguna conclusión provisional que hacer.
        (
            CodigoDeEvaluacion(CodigoDeEvaluacion::SOLO_DESCRIPTIVA.to_string()),
            CodigoDeEvaluacion(codigos::CONFIANZA_NO_CONCLUYENTE.to_string()),
        )
    } else if margen.is_none() {
        // Sin margen no se ha aplicado ninguna regla de decisión: no es que el cambio sea
        // indistinguible, es que no se juzgó. Llamarlo `sin_cambio` afirmaría que todo
        // quedó dentro de un margen que nunca existió.
        push_unico(
            &mut motivos,
            CodigoDeEvaluacion(codigos::SIN_MARGEN.to_string()),
        );
        (
            CodigoDeEvaluacion(CodigoDeEvaluacion::DATOS_INSUFICIENTES.to_string()),
            CodigoDeEvaluacion(codigos::CONFIANZA_INSUFICIENTE.to_string()),
        )
    } else if elegibles_medibles == 0 {
        push_unico(
            &mut motivos,
            CodigoDeEvaluacion(codigos::SIN_METRICAS_MEDIBLES.to_string()),
        );
        // Faltan datos: no hay estimación de la que hablar.
        (
            CodigoDeEvaluacion(CodigoDeEvaluacion::DATOS_INSUFICIENTES.to_string()),
            CodigoDeEvaluacion(codigos::CONFIANZA_INSUFICIENTE.to_string()),
        )
    } else {
        let cambios: Vec<&EvaluacionDeMetrica> = decisorias
            .iter()
            .filter(|(_, _m, e)| *e == Elegibilidad::Elegible)
            .map(|(_, m, _)| *m)
            .filter(|m| {
                matches!(
                    m.preferencia,
                    PreferenciaDeMetrica::FavorDeCandidata
                        | PreferenciaDeMetrica::FavorDeReferencia
                )
            })
            .collect();

        let a_favor = cambios
            .iter()
            .filter(|m| m.preferencia == PreferenciaDeMetrica::FavorDeCandidata)
            .count();
        let en_contra = cambios
            .iter()
            .filter(|m| m.preferencia == PreferenciaDeMetrica::FavorDeReferencia)
            .count();

        if elegibles_medibles == 1 {
            push_unico(
                &mut limitaciones,
                CodigoDeEvaluacion(
                    codigos::COBERTURA_INSUFICIENTE_PARA_CONCLUSION_GLOBAL.to_string(),
                ),
            );
        }

        let estado = if cambios.is_empty() {
            CodigoDeEvaluacion::SIN_CAMBIO
        } else if a_favor > 0 && en_contra > 0 {
            CodigoDeEvaluacion::MIXTO
        } else if a_favor > 0 {
            CodigoDeEvaluacion::MEJORA
        } else {
            CodigoDeEvaluacion::EMPEORA
        };

        // La confianza describe **qué sostiene** el estado, no cuánta certeza da. Solo se
        // usan códigos de confianza (`provisional`, `observada`, `no_concluyente`,
        // `insuficiente`); un estado (`mejora`, `sin_cambio`…) **no** es un nivel de
        // confianza y no se escribe en este campo.
        //
        // Superar el margen y ser elegible no dan confianza alta: como mucho, la
        // diferencia está `observada` — se midió por encima del ruido de muestreo —, que
        // no es reproducibilidad demostrada.
        let confianza = if estado == CodigoDeEvaluacion::SIN_CAMBIO {
            // El cambio cabe dentro del margen: no se distingue de cero.
            CodigoDeEvaluacion(codigos::CONFIANZA_NO_CONCLUYENTE.to_string())
        } else {
            // Una grabación por lado nunca confirma: la diferencia está observada, su
            // reproducibilidad no.
            CodigoDeEvaluacion(codigos::CONFIANZA_OBSERVADA.to_string())
        };

        (CodigoDeEvaluacion(estado.to_string()), confianza)
    };

    EvaluacionPersistida {
        metodologia_interpretacion: VersionDeInterpretacion(METODOLOGIA_INTERPRETACION.to_string()),
        // Lo rellena la capa de persistencia: el motor no consulta el reloj.
        evaluada_en: String::new(),
        referencia: IdDeBenchmark {
            id: comparacion.baseline.id.clone(),
        },
        candidata: IdDeBenchmark {
            id: comparacion.test.id.clone(),
        },
        estado,
        confianza,
        metricas: vec![
            metricas.avg_fps,
            metricas.fps_1_percent_low,
            metricas.fps_0_1_percent_low,
        ],
        motivos,
        limitaciones,
        condiciones: condiciones.clone(),
    }
}

/// Añade un código solo si no estaba: dos motivos repetidos no dicen más que uno.
fn push_unico(lista: &mut Vec<CodigoDeEvaluacion>, codigo: CodigoDeEvaluacion) {
    if !lista.contains(&codigo) {
        lista.push(codigo);
    }
}

/// Evalúa las tres métricas decisorias.
fn evaluar_metricas(comparacion: &BenchmarkComparison, margen: Option<f64>) -> Metricas {
    let antes = &comparacion.baseline;
    let despues = &comparacion.test;

    // `sample_count` es el nombre correcto de lo que se analizó. Si no consta, el apoyo
    // del 0,1 % low **no se inventa** con `total_frames`: ese campo conserva el mismo
    // significado solo por compatibilidad con los JSON antiguos, y su ausencia en una
    // grabación nueva no es un cero.

    let avg = metrica_de(EntradaDeMetrica {
        clave: CLAVE_AVG_FPS,
        antes: valor_positivo(antes.avg_fps),
        despues: valor_positivo(despues.avg_fps),
        mas_es_mejor: true,
        margen,
        apoyo: [None, None],
        // Los FPS medios no son un percentil: su apoyo es el recuento de muestras, que no
        // se declara aquí como `ApoyoDePercentil` porque no hay cola que declarar.
        elegibilidad: Elegibilidad::Elegible,
    });

    let low1 = metrica_de(EntradaDeMetrica {
        clave: CLAVE_FPS_1_PERCENT_LOW,
        antes: valor_positivo(antes.fps_1_percent_low),
        despues: valor_positivo(despues.fps_1_percent_low),
        mas_es_mejor: true,
        margen,
        apoyo: [
            antes
                .apoyo_metricas
                .as_ref()
                .and_then(|a| a.fps_1_percent_low),
            despues
                .apoyo_metricas
                .as_ref()
                .and_then(|a| a.fps_1_percent_low),
        ],
        // No hay criterio validado de elegibilidad para el 1 % low (5.4.3): decide cuando
        // sus valores son válidos.
        elegibilidad: Elegibilidad::Elegible,
    });

    let elegibilidad_low01 = elegibilidad_del_01(antes, despues);

    let low01 = metrica_de(EntradaDeMetrica {
        clave: CLAVE_FPS_0_1_PERCENT_LOW,
        antes: valor_positivo(antes.fps_0_1_percent_low),
        despues: valor_positivo(despues.fps_0_1_percent_low),
        mas_es_mejor: true,
        margen,
        apoyo: [
            antes
                .apoyo_metricas
                .as_ref()
                .and_then(|a| a.fps_0_1_percent_low),
            despues
                .apoyo_metricas
                .as_ref()
                .and_then(|a| a.fps_0_1_percent_low),
        ],
        elegibilidad: elegibilidad_low01,
    });

    Metricas {
        elegibilidad_avg_fps: avg.1,
        elegibilidad_1_percent_low: low1.1,
        elegibilidad_0_1_percent_low: low01.1,
        avg_fps: avg.0,
        fps_1_percent_low: low1.0,
        fps_0_1_percent_low: low01.0,
    }
}

/// ¿Puede decidir el 0,1 % low?
///
/// Reutiliza el criterio operativo de `low01Reliability`: `max(1, round(n·0,001)) >= 10`,
/// es decir **≥ 9500 muestras**. Se aplica sobre el **menor** de los dos lados: la métrica
/// es tan débil como el más débil de sus dos valores.
///
/// - `sample_count` ausente en cualquiera de los dos lados → [`Elegibilidad::ApoyoNoConsta`].
///   No se usa `total_frames` como sustituto cuando `sample_count` falta en una grabación
///   nueva: ahí `total_frames` conserva el mismo significado solo por compatibilidad, y
///   inventar el apoyo es exactamente lo que no se hace.
/// - Apoyo insuficiente → [`Elegibilidad::NoElegible`].
fn elegibilidad_del_01(
    antes: &crate::benchmarks::Benchmark,
    despues: &crate::benchmarks::Benchmark,
) -> Elegibilidad {
    let (Some(n_antes), Some(n_despues)) = (antes.sample_count, despues.sample_count) else {
        return Elegibilidad::ApoyoNoConsta;
    };
    let n = n_antes.min(n_despues) as f64;
    let extremas = (n * 0.001).round().max(1.0);
    if extremas >= MUESTRAS_EXTREMAS_MINIMAS {
        Elegibilidad::Elegible
    } else {
        Elegibilidad::NoElegible
    }
}

/// Un número utilizable como FPS: finito y positivo. Cualquier otra cosa es «no consta».
///
/// `NaN`, `±∞` y cero **no** se convierten en cero: un cero silencioso se leería como un
/// valor medido y entraría en la comparación como si fuera real.
fn valor_positivo(v: f64) -> Option<f64> {
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

/// Envuelve el apoyo de percentil en el tipo del contrato, sin tocarlo.
fn convertir_apoyo(percentil: Option<ApoyoDePercentil>) -> Option<ApoyoDeMetrica> {
    percentil.map(|percentil| ApoyoDeMetrica {
        percentil: Some(percentil),
    })
}

/// Construye una métrica con sus cinco dimensiones.
struct EntradaDeMetrica<'a> {
    clave: &'a str,
    antes: Option<f64>,
    despues: Option<f64>,
    /// Ampliar el soporte de la métrica empeora el resultado.
    mas_es_mejor: bool,
    margen: Option<f64>,
    apoyo: [Option<ApoyoDePercentil>; 2],
    elegibilidad: Elegibilidad,
}

fn metrica_de(entrada: EntradaDeMetrica<'_>) -> (EvaluacionDeMetrica, Elegibilidad) {
    let EntradaDeMetrica {
        clave,
        antes,
        despues,
        mas_es_mejor,
        margen,
        apoyo,
        elegibilidad,
    } = entrada;
    let mut motivos = Vec::new();

    let (direccion, diferencia, diferencia_pct, preferencia) = match (antes, despues) {
        (Some(a), Some(d)) => {
            let dif = d - a;
            let pct = if a.abs() < f64::EPSILON {
                None
            } else {
                Some((d - a) / a * 100.0)
            };
            let direccion = if dif > 0.0 {
                DireccionDeCambio::Sube
            } else if dif < 0.0 {
                DireccionDeCambio::Baja
            } else {
                DireccionDeCambio::Igual
            };

            // Sin margen no se puede afirmar nada: se registra la magnitud y la dirección,
            // y la preferencia queda sin establecer.
            let preferencia = match (pct, margen) {
                (Some(p), Some(m)) => {
                    if p.abs() <= m {
                        push_unico(
                            &mut motivos,
                            CodigoDeEvaluacion(codigos::DENTRO_DEL_MARGEN.into()),
                        );
                        PreferenciaDeMetrica::Indistinto
                    } else {
                        let favorable = if mas_es_mejor { p > 0.0 } else { p < 0.0 };
                        if favorable {
                            push_unico(
                                &mut motivos,
                                CodigoDeEvaluacion(codigos::FAVORECE_CANDIDATA.into()),
                            );
                            PreferenciaDeMetrica::FavorDeCandidata
                        } else {
                            push_unico(
                                &mut motivos,
                                CodigoDeEvaluacion(codigos::FAVORECE_REFERENCIA.into()),
                            );
                            PreferenciaDeMetrica::FavorDeReferencia
                        }
                    }
                }
                _ => PreferenciaDeMetrica::NoEstablecida,
            };
            (direccion, Some(dif), pct, preferencia)
        }
        _ => {
            // Al menos un lado no tiene valor utilizable. Se dice cuál, sin inventar.
            push_unico(&mut motivos, CodigoDeEvaluacion(codigos::SIN_DATOS.into()));
            if antes.is_none() || despues.is_none() {
                push_unico(
                    &mut motivos,
                    CodigoDeEvaluacion(codigos::VALOR_NO_VALIDO.into()),
                );
            }
            (
                DireccionDeCambio::NoConsta,
                None,
                None,
                PreferenciaDeMetrica::NoEstablecida,
            )
        }
    };

    // La preferencia **solo** existe si la métrica puede decidir. Una métrica excluida
    // conserva la magnitud, la dirección numérica y el apoyo, pero **no** favorece a
    // ninguna configuración: registrarlo sería hacerla decidir por la puerta de atrás.
    //
    // No basta con neutralizar el campo `preferencia`: los motivos `favorece_*` también
    // afirman una preferencia y se leen como tal. Se retiran **de los dos sitios a la vez**
    // para que el motivo y el campo no se contradigan.
    let preferencia = if elegibilidad == Elegibilidad::Elegible {
        preferencia
    } else {
        motivos
            .retain(|m| m.0 != codigos::FAVORECE_CANDIDATA && m.0 != codigos::FAVORECE_REFERENCIA);
        PreferenciaDeMetrica::NoEstablecida
    };

    let mut limitaciones = Vec::new();
    if elegibilidad != Elegibilidad::Elegible {
        push_unico(
            &mut limitaciones,
            CodigoDeEvaluacion(codigos::NO_ELEGIBLE.into()),
        );
        match elegibilidad {
            Elegibilidad::NoElegible => push_unico(
                &mut limitaciones,
                CodigoDeEvaluacion(codigos::APOYO_INSUFICIENTE.into()),
            ),
            Elegibilidad::ApoyoNoConsta => push_unico(
                &mut limitaciones,
                CodigoDeEvaluacion(codigos::APOYO_NO_CONSTA.into()),
            ),
            Elegibilidad::Elegible => {}
        }
    }

    // Participar = haber entrado en la decisión del estado: incluye estar dentro del
    // margen (donde no hay preferencia pero sí un juicio) y excluye las no elegibles.
    let participo =
        elegibilidad == Elegibilidad::Elegible && !matches!(direccion, DireccionDeCambio::NoConsta);
    if participo {
        push_unico(&mut motivos, CodigoDeEvaluacion(codigos::PARTICIPO.into()));
    }

    (
        EvaluacionDeMetrica {
            clave: clave.to_string(),
            antes,
            despues,
            diferencia,
            diferencia_pct,
            direccion,
            preferencia,
            confianza: None,
            apoyo_referencia: convertir_apoyo(apoyo[0]),
            apoyo_candidata: convertir_apoyo(apoyo[1]),
            motivos,
            limitaciones,
        },
        elegibilidad,
    )
}

/// Condiciones del experimento para una comparación, calculadas una sola vez al evaluar.
///
/// Distingue tres cosas que no son lo mismo:
///
/// - **ruido original ausente**: alguna grabación no declara `noise_percent`. No es un
///   cero: en `ruido_combinado_porcentaje` queda `None` (no consta).
/// - **estimación operativa con suelo**: el margen no baja nunca de `MIN_TIE_PERCENT`,
///   porque por debajo de ese suelo una diferencia no se puede atribuir a nada.
/// - **margen aplicado**: `max(MIN_TIE_PERCENT, ruido combinado)`. Es lo que se guarda en
///   `margen_porcentaje` y lo que se usa para decidir; no se recalcula al releer.
///
/// El ruido **ausente no produce `datos_insuficientes`**: se evalúa con el suelo y la
/// falta queda registrada en `ruido_combinado_porcentaje: None`. Solo la ausencia de
/// *margen* impide afirmar (ver `evaluar`).
///
/// Recibe los dos `noise_percent` originales —no un valor ya combinado— para poder
/// conservar la diferencia entre «no consta» y «estimación con suelo». La combinación en
/// cuadratura se replica aquí con la misma regla que `benchmarks::combined_noise`, que es
/// privada de ese módulo y no se exporta para no tocar B: la regla es corta y estable, y
/// duplicarla en un motor puro es preferible a publicar API nueva desde B.
pub fn condiciones_de(
    comparacion: &BenchmarkComparison,
    variable: Option<crate::benchmarks::VariableEvaluada>,
) -> CondicionesDeComparacion {
    let compatibilidad = compatibilidad_metodologica(comparacion);

    let base = comparacion.baseline.noise_percent;
    let test = comparacion.test.noise_percent;

    // La **misma** función que usa la comparación numérica: no hay una segunda fórmula de
    // ruido. `combined_noise` combina en cuadratura y aplica el suelo de estimación
    // `MIN_NOISE`; aquí se reutiliza tal cual, sin tocar su comportamiento.
    let combinado = crate::benchmarks::combined_noise(base, test);

    // El suelo de estimación **no** es el margen de interpretación, y «no se declaró
    // ruido» no es «ruido = 0». Un `0.0` declarado es un valor **medido** («no se detectó
    // variación») y se conserva; solo lo no finito o negativo se descarta, porque no es
    // una estimación utilizable. `None` significa «no se estimó».
    let declarado = |v: Option<f64>| v.filter(|x| x.is_finite() && *x >= 0.0);
    let ruido_declarado = match (declarado(base), declarado(test)) {
        // Ninguna lo declara: no consta. `combined_noise` devuelve el suelo, que es una
        // estimación operativa, no un dato observado.
        (None, None) => None,
        // Al menos una lo declara: el valor de `combined_noise` es el que corresponde.
        _ => Some(combinado),
    };

    // Margen operativo: el suelo de interpretación (5 %) y, por encima, el ruido combinado.
    let margen = Some(combinado.max(MIN_TIE_PERCENT));

    CondicionesDeComparacion {
        compatibilidad_metodologica: compatibilidad,
        margen_porcentaje: margen,
        ruido_combinado_porcentaje: ruido_declarado,
        comandos_iguales: comparacion.comandos_iguales,
        variable_evaluada: variable,
    }
}

/// Compatibilidad metodológica: **solo** `v2` contra `v2` la establece.
///
/// Dos metodologías desconocidas no son equivalentes: que ninguna de las dos declare cómo
/// calculó sus números no significa que los calcularan igual.
fn compatibilidad_metodologica(comparacion: &BenchmarkComparison) -> CompatibilidadMetodologica {
    let declara_v2 = |b: &crate::benchmarks::Benchmark| {
        matches!(b.metodologia_calculo, Some(MetodologiaCalculo::V2))
    };
    if declara_v2(&comparacion.baseline) && declara_v2(&comparacion.test) {
        CompatibilidadMetodologica::Establecida
    } else {
        CompatibilidadMetodologica::NoEstablecida
    }
}

// ---------------------------------------------------------------------------------------
// C.2 — Confirmación de la vista previa
// ---------------------------------------------------------------------------------------
//
// La evaluación confirmada en pantalla y la guardada deben ser la **misma** en significado.
// No basta comparar ids ni el estado global: dos evaluaciones `mejora` pueden diferir en
// métricas, margen, preferencias y limitaciones. Y no basta con la evaluación sola: hay
// cambios en las entradas (comandos, procedencia, identidad) que pueden **no** alterar el
// veredicto y sí alterar lo que se está guardando.
//
// Por eso la confirmación son **dos** objetos, ambos generados por Rust:
//
// 1. `BenchmarkComparison` — lo que se va a persistir (incluye las dos grabaciones
//    completas embebidas).
// 2. `EvaluacionDeComparacion` — el significado que se mostró al usuario.
//
// React los devuelve como **expectativa**; nunca son datos autoritativos. Rust relee,
// recalcula y compara: si algo difiere, no se escribe nada.

/// La confirmación de una vista previa: qué se mostró al usuario y sobre qué entradas.
///
/// No es un testigo criptográfico ni una prueba de consentimiento: es una **expectativa**
/// que Rust contrasta contra el estado real en el momento de guardar. No acredita que la
/// interfaz pintara nada; solo permite detectar que las entradas cambiaron.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConfirmacionDeVistaPrevia {
    /// La comparación que se mostró. Se persiste **la recalculada**, no esta.
    pub comparacion: BenchmarkComparison,
    /// La evaluación que se mostró. Se persiste **la recalculada**, no esta.
    pub evaluacion: EvaluacionDeComparacion,
}

/// Por qué una confirmación no corresponde al estado actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotivoDeRechazo {
    /// Las entradas cambiaron: la comparación recalculada difiere de la confirmada.
    EntradasCambiadas,
    /// Las entradas coinciden pero el significado no: la evaluación recalculada difiere.
    SignificadoCambiado,
    /// La evaluación confirmada no describe a la comparación confirmada.
    EvaluacionNoCorresponde,
    /// Alguna entrada tiene un valor no finito, que no se puede persistir sin corromper
    /// el archivo (se escribiría como `null` en un campo obligatorio).
    ValorNoPersistible,
}

impl MotivoDeRechazo {
    /// Código estable para mensajes y pruebas.
    pub fn codigo(self) -> &'static str {
        match self {
            MotivoDeRechazo::EntradasCambiadas => "entradas_cambiadas",
            MotivoDeRechazo::SignificadoCambiado => "significado_cambiado",
            MotivoDeRechazo::EvaluacionNoCorresponde => "evaluacion_no_corresponde",
            MotivoDeRechazo::ValorNoPersistible => "valor_no_persistible",
        }
    }
}

/// Compara una confirmación con lo que Rust acaba de recalcular.
///
/// Los cuatro pasos, en este orden, porque cada uno da un motivo distinto y más preciso:
///
/// 1. La evaluación confirmada **describe** la comparación confirmada (ids y coherencia).
/// 2. Las **entradas** recalculadas coinciden con las confirmadas.
/// 3. El **significado** recalculado coincide con el confirmado (sin `evaluada_en`, que es
///    el instante del guardado y no forma parte del significado).
///
/// Devuelve `Ok(())` solo si los tres pasan.
pub fn comprobar_confirmacion(
    confirmada: &ConfirmacionDeVistaPrevia,
    recalculada: &ConfirmacionDeVistaPrevia,
) -> Result<(), MotivoDeRechazo> {
    // 0. Nada no finito, en ninguna de las dos. Un valor así no se puede escribir: el
    //    campo es obligatorio y `serde_json` lo serializa como `null`, con lo que el
    //    archivo dejaría de poder leerse. Se rechaza antes de comparar nada.
    if comparacion_tiene_no_finitos(&confirmada.comparacion)
        || comparacion_tiene_no_finitos(&recalculada.comparacion)
    {
        return Err(MotivoDeRechazo::ValorNoPersistible);
    }

    // 1. Internamente coherente: una evaluación que no habla de estas dos grabaciones no
    //    es una confirmación válida aunque sus campos suelten.
    if !confirmada.evaluacion.corresponde_a(&confirmada.comparacion) {
        return Err(MotivoDeRechazo::EvaluacionNoCorresponde);
    }

    // 2. Las entradas. `BenchmarkComparison` embute las dos grabaciones completas, así que
    //    aquí entran comandos, procedencia, identidad, notas y cualquier característica
    //    registrada; no solo las medidas que acaban en el veredicto.
    if !misma_comparacion(&confirmada.comparacion, &recalculada.comparacion) {
        return Err(MotivoDeRechazo::EntradasCambiadas);
    }

    // 3. El significado.
    if !misma_evaluacion(&confirmada.evaluacion, &recalculada.evaluacion) {
        return Err(MotivoDeRechazo::SignificadoCambiado);
    }

    Ok(())
}

/// Igualdad de dos valores `f64` **sin colapsar los no finitos**.
///
/// `serde_json` serializa `NaN`, `+inf` y `-inf` **los tres como `null`** (verificado):
/// comparar por la forma serializada los confundiría entre sí y, además, con un valor
/// ausente. Aquí se comparan por su bit pattern, que sí los distingue y hace que `NaN`
/// coincida consigo mismo (`to_bits` es una función total, no una relación).
///
/// `0.0` y `-0.0` tienen bits distintos y por tanto **no** se consideran iguales: son
/// entradas distintas en los campos donde aparecen (por ejemplo, un `-0.0` escrito a mano
/// en un JSON histórico). Ninguna medición real produce `-0.0` en estos campos, pero la
/// igualdad no puede depender de esa suposición.
pub(crate) fn mismos_f64(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// Igualdad de dos opcionales numéricos, distinguiendo **ausente** de **presente**.
///
/// `None` es «no consta»; `Some(0.0)` es «se midió cero». `serde_json` escribiría ambos
/// como `null` y los fundiría, así que aquí se distinguen explícitamente.
fn mismos_opcionales(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => mismos_f64(a, b),
        _ => false,
    }
}

/// Igualdad de dos opcionales genéricos, distinguiendo ausente de presente.
fn mismos_opcionales_de<T: PartialEq>(a: Option<&T>, b: Option<&T>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// ¿Contiene esta grabación algún valor no finito en un campo numérico?
///
/// Un `f64` no finito **no se puede persistir**: `serde_json` lo escribe como `null` y el
/// campo es obligatorio, así que el archivo resultante **no se puede volver a leer**
/// (`invalid type: null, expected f64`). No es un caso de vista previa: es un archivo
/// escrito que se pierde. Antes de confirmar se rechaza explícitamente.
fn tiene_no_finitos(b: &Benchmark) -> bool {
    let obligatorios = [
        b.duration_seconds,
        b.avg_fps,
        b.fps_1_percent_low,
        b.fps_0_1_percent_low,
        b.max_fps,
        b.min_fps,
        b.frame_time_avg,
        b.frame_time_1_percent_high,
    ];
    let opcionales = [
        b.spikes_per_min,
        b.pct_over_33ms,
        b.jitter_ms,
        b.worst_frame_ms,
        b.warmup_seconds,
        b.noise_percent,
        b.duracion_medida,
    ];

    obligatorios.iter().any(|v| !v.is_finite())
        || opcionales.iter().flatten().any(|v| !v.is_finite())
}

/// ¿La comparación contiene algún valor no finito, directo o en sus grabaciones?
fn comparacion_tiene_no_finitos(c: &BenchmarkComparison) -> bool {
    let deltas = [
        c.fps_change_percent,
        c.fps_1_percent_change_percent,
        c.fps_0_1_percent_change_percent,
        c.frame_time_change_percent,
    ];

    tiene_no_finitos(&c.baseline)
        || tiene_no_finitos(&c.test)
        || deltas.iter().any(|v| !v.is_finite())
        || c.noise_percent.is_some_and(|v| !v.is_finite())
}

/// Igualdad de dos comparaciones, campo por campo y **sin pérdida de información**.
///
/// No se usa `==` sobre las structs porque `f64` no implementa una relación de igualdad
/// total (`NaN != NaN`), y no se usa la forma serializada porque `serde_json` colapsa los
/// no finitos a `null` y los confunde con la ausencia. Se comparan los campos con la
/// semántica explícita de arriba: los números por bits, los opcionales distinguiendo
/// ausente de presente, y todo lo demás con su propia igualdad.
pub(crate) fn misma_comparacion(a: &BenchmarkComparison, b: &BenchmarkComparison) -> bool {
    mismo_benchmark(&a.baseline, &b.baseline)
        && mismo_benchmark(&a.test, &b.test)
        && mismos_f64(a.fps_change_percent, b.fps_change_percent)
        && mismos_f64(
            a.fps_1_percent_change_percent,
            b.fps_1_percent_change_percent,
        )
        && mismos_f64(
            a.fps_0_1_percent_change_percent,
            b.fps_0_1_percent_change_percent,
        )
        && mismos_f64(a.frame_time_change_percent, b.frame_time_change_percent)
        && a.conclusion == b.conclusion
        && mismos_opcionales(a.noise_percent, b.noise_percent)
        && a.within_noise == b.within_noise
        && a.comandos_iguales == b.comandos_iguales
}

/// Igualdad de dos grabaciones, campo por campo.
///
/// Es exhaustiva a propósito: si mañana se añade un campo a `Benchmark` y no se incluye
/// aquí, el compilador **no** avisará, pero la prueba
/// `c2_la_igualdad_cubre_todos_los_campos` construye dos grabaciones que difieren solo en
/// un campo cada vez y exige que la igualdad las distinga.
pub(crate) fn mismo_benchmark(a: &Benchmark, b: &Benchmark) -> bool {
    a.id == b.id
        && a.profile_id == b.profile_id
        && a.game_name == b.game_name
        && a.appid == b.appid
        && a.timestamp == b.timestamp
        && mismos_f64(a.duration_seconds, b.duration_seconds)
        && mismos_f64(a.avg_fps, b.avg_fps)
        && mismos_f64(a.fps_1_percent_low, b.fps_1_percent_low)
        && mismos_f64(a.fps_0_1_percent_low, b.fps_0_1_percent_low)
        && mismos_f64(a.max_fps, b.max_fps)
        && mismos_f64(a.min_fps, b.min_fps)
        && mismos_f64(a.frame_time_avg, b.frame_time_avg)
        && mismos_f64(a.frame_time_1_percent_high, b.frame_time_1_percent_high)
        && a.total_frames == b.total_frames
        && mismos_opcionales_de(a.notes.as_ref(), b.notes.as_ref())
        && a.launch_command == b.launch_command
        && a.origen_comando == b.origen_comando
        && mismos_opcionales_de(a.steam_command.as_ref(), b.steam_command.as_ref())
        && a.number == b.number
        && mismos_opcionales_de(a.snapshot.as_ref(), b.snapshot.as_ref())
        && mismos_opcionales_de(a.sesion_id.as_ref(), b.sesion_id.as_ref())
        && mismos_opcionales(a.spikes_per_min, b.spikes_per_min)
        && mismos_opcionales(a.pct_over_33ms, b.pct_over_33ms)
        && mismos_opcionales(a.jitter_ms, b.jitter_ms)
        && mismos_opcionales(a.worst_frame_ms, b.worst_frame_ms)
        && mismos_opcionales(a.warmup_seconds, b.warmup_seconds)
        && mismos_opcionales(a.noise_percent, b.noise_percent)
        && mismos_opcionales_de(a.shader_stutter.as_ref(), b.shader_stutter.as_ref())
        && a.log_path == b.log_path
        && a.sample_count == b.sample_count
        && mismos_opcionales_de(
            a.muestras_temporales.as_ref(),
            b.muestras_temporales.as_ref(),
        )
        && mismos_opcionales(a.duracion_medida, b.duracion_medida)
        && a.metodologia_calculo == b.metodologia_calculo
        && mismos_opcionales_de(
            a.contabilidad_muestras.as_ref(),
            b.contabilidad_muestras.as_ref(),
        )
        && mismos_opcionales_de(a.apoyo_metricas.as_ref(), b.apoyo_metricas.as_ref())
}

/// Igualdad de significado de dos evaluaciones.
///
/// `evaluada_en` **queda fuera**: es el instante en que se registra, no lo que se afirma.
/// Todo lo demás entra, incluida la metodología de interpretación y las condiciones.
fn misma_evaluacion(a: &EvaluacionDeComparacion, b: &EvaluacionDeComparacion) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.evaluada_en.clear();
    b.evaluada_en.clear();
    a == b
}

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_c2;
#[cfg(test)]
mod pruebas_c3_20;
