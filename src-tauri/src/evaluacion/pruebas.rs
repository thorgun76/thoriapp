//! Pruebas del motor de interpretación (subfase 5.4.1.C.1).
//!
//! Se separan **dos planos**, y no se mezclan:
//!
//! - **Pruebas abstractas de la regla**: construyen `Benchmark` mínimos y comprueban la
//!   función de decisión sobre sus casos de interés (fronteras del margen, estados,
//!   cobertura). Dicen si la regla está bien escrita; no pretenden representar grabaciones
//!   reales.
//! - **Pruebas de conservación del significado**: comprueban que lo evaluado se relee
//!   idéntico, que los campos desconocidos se conservan y que no se recalcula nada.
//!
//! No se construyen combinaciones físicamente imposibles (un 1 % low mayor que la media,
//! un frametime negativo) como si fueran grabaciones: para lo que no puede pasar en una
//! medición real se prueba la **representación** («no consta»), no la aritmética.

use super::*;
use crate::benchmarks::{
    ApoyoDeMetricas, ApoyoDePercentil, Benchmark, MetodologiaCalculo, VariableEvaluada,
};
use std::collections::BTreeSet;

// ─────────────────────────── Fixtures ───────────────────────────

/// Grabación mínima con solo lo que el motor lee.
fn bench(id: &str, avg: f64, low1: f64, low01: f64, muestras: Option<u64>) -> Benchmark {
    Benchmark {
        id: id.to_string(),
        profile_id: String::new(),
        game_name: "Juego".to_string(),
        appid: 1,
        timestamp: "2026-01-01 10:00:00".to_string(),
        duration_seconds: 60.0,
        avg_fps: avg,
        fps_1_percent_low: low1,
        fps_0_1_percent_low: low01,
        max_fps: avg,
        min_fps: low1,
        frame_time_avg: 1000.0 / avg,
        frame_time_1_percent_high: 1000.0 / low1,
        total_frames: muestras.unwrap_or(0),
        notes: None,
        launch_command: String::new(),
        origen_comando: Default::default(),
        steam_command: None,
        number: None,
        snapshot: None,
        sesion_id: None,
        spikes_per_min: None,
        pct_over_33ms: None,
        jitter_ms: None,
        worst_frame_ms: None,
        warmup_seconds: Some(10.0),
        noise_percent: None,
        shader_stutter: None,
        log_path: String::new(),
        sample_count: muestras,
        muestras_temporales: None,
        duracion_medida: Some(60.0),
        metodologia_calculo: Some(MetodologiaCalculo::V2),
        contabilidad_muestras: None,
        apoyo_metricas: None,
    }
}

/// Comparación entre dos grabaciones. Los cambios porcentuales del contrato numérico no
/// los lee el motor: los recalcula a partir de las magnitudes.
fn comparar(base: Benchmark, test: Benchmark) -> BenchmarkComparison {
    BenchmarkComparison {
        baseline: base,
        test,
        fps_change_percent: 0.0,
        fps_1_percent_change_percent: 0.0,
        fps_0_1_percent_change_percent: 0.0,
        frame_time_change_percent: 0.0,
        conclusion: String::new(),
        noise_percent: None,
        within_noise: false,
        comandos_iguales: None,
    }
}

/// Comparación a partir de los tres porcentajes de cambio y un margen, con las muestras
/// indicadas. Los valores base son redondos (100 FPS) para que el porcentaje sea exacto.
fn con_cambios(
    avg_pct: f64,
    low1_pct: f64,
    low01_pct: f64,
    muestras: Option<u64>,
    margen: Option<f64>,
) -> (BenchmarkComparison, CondicionesDeComparacion) {
    let base = bench("a", 100.0, 100.0, 100.0, muestras);
    let test = bench(
        "b",
        100.0 * (1.0 + avg_pct / 100.0),
        100.0 * (1.0 + low1_pct / 100.0),
        100.0 * (1.0 + low01_pct / 100.0),
        muestras,
    );
    let c = comparar(base, test);
    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: margen,
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    (c, cond)
}

fn tiene(lista: &[CodigoDeEvaluacion], codigo: &str) -> bool {
    lista.iter().any(|c| c.0 == codigo)
}

fn metrica<'a>(ev: &'a EvaluacionDeComparacion, clave: &str) -> &'a EvaluacionDeMetrica {
    ev.metricas
        .iter()
        .find(|m| m.clave == clave)
        .expect("la métrica debe existir")
}

/// 9 500 muestras es la frontera real: `max(1, round(9500 * 0.001)) == 10`.
const MUESTRAS_FIABLES: u64 = 9500;
/// Una menos: 9 extremas, no elegible.
const MUESTRAS_NO_FIABLES: u64 = 9499;

// ───────────────────────── Fronteras de apoyo ─────────────────────────

/// Frontera exacta heredada de `low01Reliability`: 9499 no, 9500 sí.
///
/// Se prueba aquí sobre el criterio del motor porque es el que decide elegibilidad. El
/// módulo TS sigue siendo la definición de presentación: no se toca.
#[test]
fn c1_frontera_de_elegibilidad_del_01_por_ciento_low() {
    let casos = [
        (0u64, false),
        (1, false),
        (300, false),
        (493, false),
        (5_000, false),
        (MUESTRAS_NO_FIABLES, false),
        (9_500, true),
        (9_999, true),
        (10_000, true),
    ];
    for (n, esperado) in casos {
        let (c, cond) = con_cambios(0.0, 0.0, 0.0, Some(n), Some(5.0));
        let ev = evaluar(&c, &cond);
        let m = metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);
        let elegible =
            m.preferencia != PreferenciaDeMetrica::NoEstablecida || m.limitaciones.is_empty();
        assert_eq!(
            elegible, esperado,
            "con {n} muestras la elegibilidad no coincide"
        );
        assert_eq!(
            tiene(&m.limitaciones, codigos::NO_ELEGIBLE),
            !esperado,
            "con {n} muestras la limitación no coincide"
        );
    }
}

/// Sin `sample_count` no se inventa el apoyo: la métrica no decide.
///
/// `total_frames` es el nombre heredado del mismo dato, pero **no** se usa como sustituto
/// para decidir: se declara lo que se sabe, que es que el recuento no consta.
#[test]
fn c1_sin_muestras_declaradas_el_01_por_ciento_low_no_es_elegible() {
    let (c, cond) = con_cambios(0.0, 0.0, 20.0, None, Some(5.0));
    let ev = evaluar(&c, &cond);
    let m = metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);
    assert_eq!(m.preferencia, PreferenciaDeMetrica::NoEstablecida);
    assert!(tiene(&m.limitaciones, codigos::APOYO_NO_CONSTA));
    // El cambio se ve, aunque no decida.
    assert_eq!(m.direccion, DireccionDeCambio::Sube);
    assert!((m.diferencia_pct.unwrap() - 20.0).abs() < 1e-9);
}

// ───────────────────────── Frontera del margen ─────────────────────────

/// `|Δ| <= margen` no se afirma; por encima sí. Exactamente el margen queda del lado
/// conservador.
#[test]
fn c1_frontera_del_margen_operativo() {
    for (pct, esperado) in [
        (4.99, PreferenciaDeMetrica::Indistinto),
        (5.00, PreferenciaDeMetrica::Indistinto),
        (5.01, PreferenciaDeMetrica::FavorDeCandidata),
    ] {
        let (c, cond) = con_cambios(pct, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
        let ev = evaluar(&c, &cond);
        assert_eq!(
            metrica(&ev, CLAVE_AVG_FPS).preferencia,
            esperado,
            "un cambio de {pct} % no se clasificó como se esperaba"
        );
    }
}

/// Sin margen no hay decisión: se registra la magnitud y la dirección, y nada más.
#[test]
fn c1_sin_margen_no_se_afirma_ningun_cambio() {
    let (c, cond) = con_cambios(50.0, 50.0, 50.0, Some(MUESTRAS_FIABLES), None);
    let ev = evaluar(&c, &cond);
    for clave in [
        CLAVE_AVG_FPS,
        CLAVE_FPS_1_PERCENT_LOW,
        CLAVE_FPS_0_1_PERCENT_LOW,
    ] {
        let m = metrica(&ev, clave);
        assert_eq!(m.preferencia, PreferenciaDeMetrica::NoEstablecida);
        assert_eq!(m.direccion, DireccionDeCambio::Sube);
        assert!(m.diferencia_pct.is_some());
    }
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::DATOS_INSUFICIENTES);
}

// ─────────────────── La tabla de casos del diseño ───────────────────

/// Los casos acordados, uno a uno, con su margen y su elegibilidad.
///
/// Los porcentajes son los de la tabla del diseño; `None` en un cambio significa «esa
/// métrica no tiene valor» (`NaN`), no cero.
#[test]
fn c1_tabla_de_casos_acordada() {
    struct Caso {
        n: u32,
        margen: f64,
        avg: Option<f64>,
        low1: Option<f64>,
        low01: Option<f64>,
        muestras: Option<u64>,
        estado: &'static str,
    }

    let casos = [
        // 1: todo mejora, con apoyo de sobra.
        Caso {
            n: 1,
            margen: 5.0,
            avg: Some(8.0),
            low1: Some(8.0),
            low01: Some(8.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::MEJORA,
        },
        // 2: mejora en avg; el 0,1 % low se ve pero no decide (300 muestras).
        Caso {
            n: 2,
            margen: 5.0,
            avg: Some(8.0),
            low1: Some(1.0),
            low01: Some(14.0),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::MEJORA,
        },
        // 3: avg y low1 dentro; el 0,1 % low no decide. Sin cambio, con limitación.
        Caso {
            n: 3,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(0.4),
            low01: Some(14.0),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 4: empeora por avg.
        Caso {
            n: 4,
            margen: 5.0,
            avg: Some(-8.0),
            low1: Some(0.0),
            low01: Some(0.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::EMPEORA,
        },
        // 5: pérdida observada en métrica excluida → sin cambio CON limitación.
        Caso {
            n: 5,
            margen: 5.0,
            avg: Some(-0.4),
            low1: Some(-0.4),
            low01: Some(-14.0),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 6: solo el 1 % low se mueve, y baja → empeora.
        Caso {
            n: 6,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(-8.0),
            low01: Some(0.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::EMPEORA,
        },
        // 7: avg sube, low1 baja → mixto.
        Caso {
            n: 7,
            margen: 5.0,
            avg: Some(8.0),
            low1: Some(-8.0),
            low01: Some(0.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::MIXTO,
        },
        // 8: todo dentro del margen.
        Caso {
            n: 8,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(0.4),
            low01: Some(0.4),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 9: ninguna métrica medible.
        Caso {
            n: 9,
            margen: 5.0,
            avg: None,
            low1: None,
            low01: None,
            muestras: None,
            estado: CodigoDeEvaluacion::DATOS_INSUFICIENTES,
        },
        // 10: solo el 0,1 % low tiene valor, y no es elegible → sin cambio, cobertura 1/3.
        Caso {
            n: 10,
            margen: 5.0,
            avg: Some(0.4),
            low1: None,
            low01: Some(0.4),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 11: el 0,1 % low es la única que se mueve, y es elegible → decide → mejora.
        Caso {
            n: 11,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(0.4),
            low01: Some(14.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::MEJORA,
        },
        // 12: se mueve el 1 % low, elegible → mejora.
        Caso {
            n: 12,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(14.0),
            low01: Some(0.4),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::MEJORA,
        },
        // 13: margen enorme → todo dentro.
        Caso {
            n: 13,
            margen: 40.0,
            avg: Some(8.0),
            low1: Some(8.0),
            low01: Some(8.0),
            muestras: Some(12_000),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 14: mejora clara en avg, pérdida del 0,1 % low que no decide.
        Caso {
            n: 14,
            margen: 5.0,
            avg: Some(12.5),
            low1: Some(0.0),
            low01: Some(-25.0),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::MEJORA,
        },
        // 15: todo dentro salvo el 0,1 % low, que baja y no decide.
        Caso {
            n: 15,
            margen: 5.0,
            avg: Some(0.4),
            low1: Some(0.4),
            low01: Some(-14.0),
            muestras: Some(300),
            estado: CodigoDeEvaluacion::SIN_CAMBIO,
        },
        // 16: solo el 1 % low tiene valor → mejora con cobertura insuficiente.
        Caso {
            n: 16,
            margen: 5.0,
            avg: None,
            low1: Some(8.0),
            low01: None,
            muestras: Some(300),
            estado: CodigoDeEvaluacion::MEJORA,
        },
    ];

    for caso in casos {
        let base = bench("a", 100.0, 100.0, 100.0, caso.muestras);
        let valor = |pct: Option<f64>, defecto_fuera: f64| match pct {
            Some(p) => 100.0 * (1.0 + p / 100.0),
            // `None` = la métrica no consta. Se usa un valor que el motor descarta.
            None => defecto_fuera,
        };
        let mut test = bench(
            "b",
            valor(caso.avg, f64::NAN),
            valor(caso.low1, f64::NAN),
            valor(caso.low01, f64::NAN),
            caso.muestras,
        );
        // `NAN` rompe la aritmética del fixture; se deja explícito.
        if caso.avg.is_none() {
            test.avg_fps = f64::NAN;
        }
        if caso.low1.is_none() {
            test.fps_1_percent_low = f64::NAN;
        }
        if caso.low01.is_none() {
            test.fps_0_1_percent_low = f64::NAN;
        }

        let c = comparar(base, test);
        let cond = CondicionesDeComparacion {
            compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
            margen_porcentaje: Some(caso.margen),
            ruido_combinado_porcentaje: None,
            comandos_iguales: None,
            variable_evaluada: None,
        };

        let ev = evaluar(&c, &cond);
        assert_eq!(
            ev.estado.0, caso.estado,
            "el caso {} no dio el estado esperado",
            caso.n
        );
    }
}

/// Caso 15: un `sin_cambio` que oculta una pérdida observada no es aceptable.
#[test]
fn c1_el_caso_15_no_es_un_sin_cambio_desnudo() {
    let (c, cond) = con_cambios(0.4, 0.4, -14.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    assert_eq!(ev.estado.0, CodigoDeEvaluacion::SIN_CAMBIO);
    assert!(tiene(&ev.limitaciones, codigos::EVALUACION_PARCIAL));
    assert!(
        tiene(&ev.limitaciones, codigos::PERDIDA_OBSERVADA_SIN_EVALUAR),
        "la pérdida del 0,1 % low debe declararse: {:?}",
        ev.limitaciones
    );
    // Y sigue viéndose como magnitud, con su dirección.
    let m = metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);
    assert_eq!(m.direccion, DireccionDeCambio::Baja);
    assert!((m.diferencia_pct.unwrap() + 14.0).abs() < 1e-9);
}

/// Caso 14: una mejora parcial no puede presentarse como mejora global limpia.
#[test]
fn c1_el_caso_14_es_una_mejora_parcial() {
    let (c, cond) = con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    assert_eq!(ev.estado.0, CodigoDeEvaluacion::MEJORA);
    assert!(tiene(&ev.limitaciones, codigos::EVALUACION_PARCIAL));
    assert!(tiene(
        &ev.limitaciones,
        codigos::PERDIDA_OBSERVADA_SIN_EVALUAR
    ));

    // Las que deciden están a favor de la candidata; la excluida no tiene preferencia.
    assert_eq!(
        metrica(&ev, CLAVE_AVG_FPS).preferencia,
        PreferenciaDeMetrica::FavorDeCandidata
    );
    assert_eq!(
        metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW).preferencia,
        PreferenciaDeMetrica::NoEstablecida
    );
}

/// Con una sola métrica evaluable no se presume una conclusión sobre todo.
#[test]
fn c1_con_una_sola_metrica_cobertura_insuficiente() {
    let (c, cond) = con_cambios(8.0, 100.0, 100.0, Some(300), Some(5.0));
    let mut c = c;
    // Solo el 1 % low consta.
    c.test.avg_fps = f64::NAN;
    c.test.fps_0_1_percent_low = f64::NAN;

    let ev = evaluar(&c, &cond);
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::MEJORA);
    assert!(tiene(
        &ev.limitaciones,
        codigos::COBERTURA_INSUFICIENTE_PARA_CONCLUSION_GLOBAL
    ));
    assert!(tiene(&ev.limitaciones, codigos::EVALUACION_PARCIAL));
}

// ─────────────── Preferencia vs participación, y coherencia ───────────────

/// `participo` **solo** aparece cuando la métrica tiene preferencia, es decir, cuando
/// decidió. Participación y preferencia no son lo mismo: una métrica excluida puede tener
/// magnitud y aun así no haber participado.
#[test]
fn c1_participacion_y_preferencia_no_se_confunden() {
    let (c, cond) = con_cambios(8.0, 8.0, -30.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    let low01 = metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);
    // Tiene magnitud y dirección observadas...
    assert!(low01.diferencia_pct.is_some());
    assert_eq!(low01.direccion, DireccionDeCambio::Baja);
    // ...pero no decidió, y por tanto no «participó» ni tiene preferencia.
    assert_eq!(low01.preferencia, PreferenciaDeMetrica::NoEstablecida);
    assert!(!tiene(&low01.motivos, codigos::PARTICIPO));

    // La elegible sí participó.
    assert!(tiene(
        &metrica(&ev, CLAVE_AVG_FPS).motivos,
        codigos::PARTICIPO
    ));
}

/// Coherencia estructural: el estado producido por `evaluar` coincide **siempre** con el
/// que calcula la función de decisión escrita aparte.
///
/// Recorre las 4³ = 64 combinaciones de dirección sobre las tres métricas decisorias, todas
/// elegibles: es la cobertura exhaustiva de la regla, sin pretender que cada combinación
/// sea una grabación plausible.
#[test]
fn c1_las_64_combinaciones_coinciden_con_la_regla() {
    let opciones = [-8.0_f64, 0.0, 4.0, 8.0];
    let mut vistas = BTreeSet::new();

    for &a in &opciones {
        for &b in &opciones {
            for &d in &opciones {
                let (c, cond) = con_cambios(a, b, d, Some(MUESTRAS_FIABLES), Some(5.0));
                let ev = evaluar(&c, &cond);

                // La regla, escrita aparte y sin mirar el implementado.
                let clasificar = |pct: f64| -> i8 {
                    if pct.abs() <= 5.0 {
                        0
                    } else if pct > 0.0 {
                        1
                    } else {
                        -1
                    }
                };
                let cambios = [clasificar(a), clasificar(b), clasificar(d)];
                let arriba = cambios.iter().filter(|&&x| x > 0).count();
                let abajo = cambios.iter().filter(|&&x| x < 0).count();
                let esperado = if arriba > 0 && abajo > 0 {
                    CodigoDeEvaluacion::MIXTO
                } else if arriba > 0 {
                    CodigoDeEvaluacion::MEJORA
                } else if abajo > 0 {
                    CodigoDeEvaluacion::EMPEORA
                } else {
                    CodigoDeEvaluacion::SIN_CAMBIO
                };

                assert_eq!(
                    ev.estado.0, esperado,
                    "avg={a} low1={b} low01={d}: la regla y el motor no coinciden"
                );
                vistas.insert(ev.estado.0.clone());
            }
        }
    }

    // La cobertura de estados es real: aparecen los cuatro.
    assert_eq!(vistas.len(), 4, "faltan estados por cubrir: {vistas:?}");
}

// ─────────────── Compatibilidad metodológica ───────────────

/// Sin compatibilidad establecida no hay veredicto, solo descripción.
#[test]
fn c1_sin_compatibilidad_es_solo_descriptiva() {
    let (c, _) = con_cambios(50.0, 50.0, 50.0, Some(MUESTRAS_FIABLES), Some(5.0));
    let var = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::NoEstablecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&c, &var);

    assert_eq!(ev.estado.0, CodigoDeEvaluacion::SOLO_DESCRIPTIVA);
    // No se declara confianza interpretativa que no existe: `no_concluyente` es un
    // **nivel de confianza**, no el estado `solo_descriptiva` reutilizado.
    assert_eq!(ev.confianza.0, codigos::CONFIANZA_NO_CONCLUYENTE);
    assert!(tiene(&ev.motivos, codigos::COMPATIBILIDAD_NO_ESTABLECIDA));

    // Y ninguna métrica proclama una ganadora.
    for m in &ev.metricas {
        assert_eq!(m.preferencia, PreferenciaDeMetrica::NoEstablecida);
        assert!(m.diferencia_pct.is_some(), "las magnitudes se conservan");
    }
}

/// Dos metodologías **desconocidas** no son equivalentes entre sí.
#[test]
fn c1_dos_metodologias_desconocidas_no_son_compatibles() {
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let mut test = bench("b", 110.0, 99.0, 88.0, Some(12_000));
    base.metodologia_calculo = None;
    test.metodologia_calculo = None;

    let cond = condiciones_de(&comparar(base, test), None);
    assert_eq!(
        cond.compatibilidad_metodologica,
        CompatibilidadMetodologica::NoEstablecida
    );
}

/// V2 contra desconocida tampoco establece compatibilidad; V2 contra V2 sí.
#[test]
fn c1_compatibilidad_solo_con_v2_en_ambos_lados() {
    let mut conocido = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    conocido.metodologia_calculo = Some(MetodologiaCalculo::V2);
    let mut historico = bench("b", 110.0, 99.0, 88.0, Some(12_000));
    historico.metodologia_calculo = None;

    let mixta = condiciones_de(&comparar(conocido.clone(), historico), None);
    assert_eq!(
        mixta.compatibilidad_metodologica,
        CompatibilidadMetodologica::NoEstablecida
    );

    let otra = conditions_v2(&conocido);
    assert_eq!(
        otra.compatibilidad_metodologica,
        CompatibilidadMetodologica::Establecida
    );
}

fn conditions_v2(b: &Benchmark) -> CondicionesDeComparacion {
    condiciones_de(&comparar(b.clone(), b.clone()), None)
}

/// La variable evaluada no se inventa: sin declaración explícita queda desconocida.
#[test]
fn c1_la_variable_evaluada_no_se_infiere() {
    let (c, _) = con_cambios(8.0, 8.0, 8.0, Some(12_000), Some(5.0));
    let sin_declarar = condiciones_de(&c, None);
    assert!(sin_declarar.variable_evaluada.is_none());

    let declarada = condiciones_de(
        &c,
        Some(VariableEvaluada {
            codigo: "vsync".to_string(),
        }),
    );
    assert_eq!(
        declarada
            .variable_evaluada
            .as_ref()
            .map(|v| v.codigo.as_str()),
        Some("vsync")
    );
}

// ─────────────── Ruido original, suelo y margen aplicado ───────────────

/// Construye una comparación con los `noise_percent` indicados en cada lado y evalúa.
fn con_ruido(
    ruido_base: Option<f64>,
    ruido_test: Option<f64>,
) -> (BenchmarkComparison, CondicionesDeComparacion) {
    let (mut c, _) = con_cambios(8.0, 8.0, 8.0, Some(MUESTRAS_FIABLES), Some(5.0));
    c.baseline.noise_percent = ruido_base;
    c.test.noise_percent = ruido_test;
    let cond = condiciones_de(&c, None);
    (c, cond)
}

/// Los cuatro casos de ruido acordados, y qué margen produce cada uno.
///
/// «0/0» y «0/2» son **no detectado**, no ausencia: se combinan con la misma regla que
/// `combined_noise` y quedan por debajo del suelo, así que el margen es el del 5 %.
#[test]
fn c1_ruido_original_y_margen_aplicado() {
    // (etiqueta, ruido base, ruido test, ruido combinado, margen aplicado)
    type Caso = (&'static str, Option<f64>, Option<f64>, Option<f64>, f64);
    // El valor «combinado» es el que devuelve `benchmarks::combined_noise`, que aplica el
    // suelo de estimación `MIN_NOISE = 3`. Por eso 0/0 y 0/2 dan 3, no 0 ni 2: es el
    // mismo número que B guarda en `BenchmarkComparison::noise_percent`. El margen, por
    // su parte, sube al 5 % por su propio suelo.
    let casos: [Caso; 4] = [
        ("0/0", Some(0.0), Some(0.0), Some(3.0), 5.0),
        ("0/2", Some(0.0), Some(2.0), Some(3.0), 5.0),
        ("ausente/ausente", None, None, None, 5.0),
        ("6/8", Some(6.0), Some(8.0), Some(10.0), 10.0),
    ];

    for (etiqueta, base, test, combinado, margen) in casos {
        let (_c, cond) = con_ruido(base, test);
        assert_eq!(
            cond.ruido_combinado_porcentaje, combinado,
            "{etiqueta}: ruido combinado"
        );
        assert_eq!(cond.margen_porcentaje, Some(margen), "{etiqueta}: margen");
    }
}

/// Un ruido ausente **no** convierte la comparación en `datos_insuficientes`: se evalúa
/// con el suelo del 5 % y la ausencia queda registrada como «no consta».
#[test]
fn c1_ruido_ausente_no_es_datos_insuficientes() {
    let (c, cond) = con_ruido(None, None);
    let ev = evaluar(&c, &cond);

    assert_eq!(cond.ruido_combinado_porcentaje, None, "no consta, no cero");
    assert_eq!(cond.margen_porcentaje, Some(5.0));
    assert_ne!(
        ev.estado.0,
        CodigoDeEvaluacion::DATOS_INSUFICIENTES,
        "la falta de ruido no impide afirmar lo que sí se midió"
    );
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::MEJORA);
    // Y el margen aplicado queda en el resultado semántico.
    assert_eq!(ev.condiciones.margen_porcentaje, Some(5.0));
    assert_eq!(ev.condiciones.ruido_combinado_porcentaje, None);
}

/// Un ruido de cero es un valor medido, no una ausencia: no se confunde con «no consta».
#[test]
fn c1_cero_de_ruido_no_es_ausencia_de_ruido() {
    let (_c, cero) = con_ruido(Some(0.0), Some(0.0));
    let (_c, nada) = con_ruido(None, None);

    // El margen coincide (suelo), pero el significado no: uno es 0 medido, el otro no consta.
    assert_eq!(cero.margen_porcentaje, nada.margen_porcentaje);
    assert_ne!(
        cero.ruido_combinado_porcentaje, nada.ruido_combinado_porcentaje,
        "0 medido y ausente no son lo mismo"
    );
}

/// Ruido inválido o no finito se descarta y el margen vuelve al suelo: nunca produce un
/// margen NaN ni gigante.
#[test]
fn c1_ruido_invalido_cae_al_suelo() {
    for (base, test) in [
        (Some(f64::NAN), Some(f64::NAN)),
        (Some(f64::INFINITY), Some(f64::INFINITY)),
        (Some(f64::NEG_INFINITY), Some(1.0)),
        (Some(-1.0), Some(-5.0)),
    ] {
        let (_c, cond) = con_ruido(base, test);
        let margen = cond.margen_porcentaje.expect("siempre hay margen");
        assert!(margen.is_finite(), "margen no finito con {base:?}/{test:?}");
        assert_eq!(
            margen, 5.0,
            "con {base:?}/{test:?} el margen debe ser el suelo"
        );
    }
}

/// Frontera exacta del margen, sobre el margen **aplicado**.
#[test]
fn c1_frontera_exacta_499_500_501() {
    for (pct, esperado) in [
        (4.99, PreferenciaDeMetrica::Indistinto),
        (5.00, PreferenciaDeMetrica::Indistinto),
        (5.01, PreferenciaDeMetrica::FavorDeCandidata),
    ] {
        let (c, cond) = con_cambios(pct, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
        let ev = evaluar(&c, &cond);
        assert_eq!(
            metrica(&ev, CLAVE_AVG_FPS).preferencia,
            esperado,
            "{pct} % clasificado mal"
        );
    }
}

/// Un ruido grande eleva el margen y puede convertir una mejora en «dentro del margen»:
/// el margen aplicado es el que manda.
#[test]
fn c1_un_ruido_alto_puede_absorber_un_cambio() {
    let (mut c, _) = con_cambios(8.0, 8.0, 8.0, Some(MUESTRAS_FIABLES), Some(5.0));
    c.baseline.noise_percent = Some(6.0);
    c.test.noise_percent = Some(8.0);
    let cond = condiciones_de(&c, None);
    assert_eq!(cond.margen_porcentaje, Some(10.0));

    let ev = evaluar(&c, &cond);
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::SIN_CAMBIO);
    assert_eq!(ev.condiciones.margen_porcentaje, Some(10.0));
}

// ─────────────── Valores ausentes, cero válido y no finitos ───────────────

/// `NaN` y `±∞` no llegan nunca a un campo serializable como número inválido.
#[test]
fn c1_los_no_finitos_no_se_serializan() {
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let mut test = bench("b", 100.0, 90.0, 80.0, Some(12_000));
    base.avg_fps = f64::NAN;
    test.fps_1_percent_low = f64::INFINITY;
    test.fps_0_1_percent_low = f64::NEG_INFINITY;

    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&comparar(base, test), &cond);

    // Solo los campos que se hicieron no finitos quedan como «no consta».
    assert!(
        metrica(&ev, CLAVE_AVG_FPS).antes.is_none(),
        "NaN no es un valor"
    );
    assert!(
        metrica(&ev, CLAVE_FPS_1_PERCENT_LOW).despues.is_none(),
        "+∞ no es un valor"
    );
    assert!(
        metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW).despues.is_none(),
        "-∞ no es un valor"
    );
    // Y ninguno de ellos se proclama como una dirección observada.
    for clave in [
        CLAVE_AVG_FPS,
        CLAVE_FPS_1_PERCENT_LOW,
        CLAVE_FPS_0_1_PERCENT_LOW,
    ] {
        let m = metrica(&ev, clave);
        assert_eq!(m.direccion, DireccionDeCambio::NoConsta, "{clave}");
        assert_eq!(
            m.preferencia,
            PreferenciaDeMetrica::NoEstablecida,
            "{clave}"
        );
        assert!(!m.antes.is_some_and(|v| !v.is_finite()), "{clave}");
        assert!(!m.despues.is_some_and(|v| !v.is_finite()), "{clave}");
    }

    // Y el resultado serializa sin errores: ningún `NaN` se cuela como número.
    let json = serde_json::to_string(&ev).expect("debe serializar");
    assert!(!json.contains("NaN"), "se coló un NaN: {json}");
    assert!(!json.contains("null,\"diferencia\""));

    assert_eq!(ev.estado.0, CodigoDeEvaluacion::DATOS_INSUFICIENTES);
    assert_eq!(ev.confianza.0, codigos::CONFIANZA_INSUFICIENTE);
}

/// Un cero en una métrica de rendimiento no es un valor medido: es «no consta».
#[test]
fn c1_cero_en_fps_no_es_un_valor_medido() {
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let test = bench("b", 0.0, 90.0, 80.0, Some(12_000));
    base.avg_fps = 100.0;

    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&comparar(base, test), &cond);
    let m = metrica(&ev, CLAVE_AVG_FPS);
    assert!(m.despues.is_none(), "un 0 no es un valor medido");
    assert_eq!(m.direccion, DireccionDeCambio::NoConsta);
}

/// `datos_insuficientes` no lleva confianza «provisional»: no hay nada que sostener.
#[test]
fn c1_datos_insuficientes_no_declara_confianza_provisional() {
    let (mut c, cond) = con_cambios(8.0, 8.0, 8.0, Some(12_000), Some(5.0));
    c.baseline.avg_fps = f64::NAN;
    c.baseline.fps_1_percent_low = f64::NAN;
    c.baseline.fps_0_1_percent_low = f64::NAN;
    c.test.avg_fps = f64::NAN;
    c.test.fps_1_percent_low = f64::NAN;
    c.test.fps_0_1_percent_low = f64::NAN;

    let ev = evaluar(&c, &cond);
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::DATOS_INSUFICIENTES);
    assert_ne!(ev.confianza.0, CodigoDeEvaluacion::MEJORA);
}

// ─────────────── Independencia del índice de estabilidad ───────────────

/// La estabilidad no es una métrica decisoria: no aparece en la evaluación y no puede
/// cambiar el estado. Se prueba con dos grabaciones de estabilidad muy distinta y FPS
/// idénticos.
#[test]
fn c1_la_estabilidad_no_es_decisoria() {
    let base = bench("a", 100.0, 50.0, 20.0, Some(12_000));
    let test = bench("b", 100.0, 100.0, 100.0, Some(12_000));

    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&comparar(base.clone(), test), &cond);

    // Ninguna métrica se llama `estabilidad`.
    assert!(
        !ev.metricas.iter().any(|m| m.clave == "estabilidad"),
        "la estabilidad no debe ser una métrica decisoria"
    );
    // El estado lo decide el 1 % low: sube 100 % → mejora.
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::MEJORA);

    // Y con la estabilidad igual (mismos cocientes) el estado no cambia de sentido.
    let igual = bench("b", 100.0, 50.0, 20.0, Some(12_000));
    let ev2 = evaluar(&comparar(base, igual), &cond);
    assert_eq!(ev2.estado.0, CodigoDeEvaluacion::SIN_CAMBIO);
}

// ─────────────── Conservación del significado ───────────────

/// Lo evaluado se relee idéntico: la evaluación guardada no se recalcula.
#[test]
fn c1_la_evaluacion_se_conserva_byte_a_byte() {
    let (c, cond) = con_cambios(8.0, 8.0, -30.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    let json = serde_json::to_string(&ev).expect("serializa");
    let releido: EvaluacionDeComparacion = serde_json::from_str(&json).expect("deserializa");
    assert_eq!(releido, ev, "la ida y vuelta debe ser idéntica");
}

/// Un código de motivo desconocido se conserva tal cual.
#[test]
fn c1_los_codigos_desconocidos_se_conservan() {
    let (c, cond) = con_cambios(8.0, 8.0, 8.0, Some(12_000), Some(5.0));
    let mut ev = evaluar(&c, &cond);
    ev.motivos
        .push(CodigoDeEvaluacion("codigo_del_futuro".to_string()));

    let json = serde_json::to_string(&ev).unwrap();
    assert!(json.contains("codigo_del_futuro"));
    let releido: EvaluacionDeComparacion = serde_json::from_str(&json).unwrap();
    assert_eq!(releido, ev);
}

/// El margen aplicado queda registrado en las condiciones **de la evaluación**, y no se
/// recalcula al leer.
#[test]
fn c1_el_margen_aplicado_queda_registrado() {
    let (c, _) = con_cambios(3.0, 3.0, 3.0, Some(12_000), Some(5.0));
    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: Some(4.2),
        comandos_iguales: Some(true),
        variable_evaluada: None,
    };
    let ev = evaluar(&c, &cond);

    assert_eq!(ev.condiciones.margen_porcentaje, Some(5.0));
    assert_eq!(ev.condiciones.ruido_combinado_porcentaje, Some(4.2));

    // Guardado y releído con reglas distintas: lo evaluado NO cambia.
    let json = serde_json::to_string(&ev).unwrap();
    let releido: EvaluacionDeComparacion = serde_json::from_str(&json).unwrap();
    assert_eq!(releido.condiciones.margen_porcentaje, Some(5.0));
    assert_eq!(releido.estado, ev.estado);
}

// ─────────────── Determinismo y contrato sin Result ───────────────

/// Misma entrada, mismo resultado. Sin reloj no hay nada que varíe.
#[test]
fn c1_es_determinista() {
    let (c, cond) = con_cambios(8.0, -2.0, 30.0, Some(300), Some(5.0));
    let uno = evaluar(&c, &cond);
    let otro = evaluar(&c, &cond);
    assert_eq!(uno, otro);

    // Y no ha consultado el reloj: `evaluada_en` llega vacía y la pone quien persiste.
    assert_eq!(uno.evaluada_en, "");
}

/// Todo caso admitido tiene representación válida: no hace falta `Result`.
#[test]
fn c1_no_hay_caso_de_datos_que_sea_error() {
    let extremos = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -1.0, 1e300];

    for &a in &extremos {
        for &b in &extremos {
            let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
            let mut test = bench("b", 100.0, 90.0, 80.0, Some(12_000));
            base.avg_fps = a;
            test.avg_fps = b;

            let cond = CondicionesDeComparacion {
                compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
                margen_porcentaje: Some(5.0),
                ruido_combinado_porcentaje: None,
                comandos_iguales: None,
                variable_evaluada: None,
            };

            // Lo importante: no entra en pánico y el estado es uno del vocabulario.
            let ev = evaluar(&comparar(base, test), &cond);
            assert!(
                ev.estado.es_conocido(),
                "estado fuera del vocabulario con {a} / {b}"
            );
            let json = serde_json::to_string(&ev).expect("serializa cualquier entrada");
            assert!(!json.contains("NaN"));
            assert!(!json.contains("inf"));
        }
    }
}

/// El apoyo se transmite desde el benchmark a la evaluación, sin recalcular.
#[test]
fn c1_el_apoyo_se_copia_del_benchmark() {
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let mut test = bench("b", 108.0, 99.0, 88.0, Some(12_000));
    base.apoyo_metricas = Some(ApoyoDeMetricas {
        fps_1_percent_low: Some(ApoyoDePercentil {
            muestras: 12_000,
            cola_nominal: 120.0,
            observaciones_interpoladas: 2,
            muestras_en_la_media: None,
        }),
        ..Default::default()
    });
    test.apoyo_metricas = base.apoyo_metricas.clone();

    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&comparar(base, test), &cond);
    let m = metrica(&ev, CLAVE_FPS_1_PERCENT_LOW);

    let apoyo = m
        .apoyo_referencia
        .as_ref()
        .and_then(|a| a.percentil.as_ref())
        .expect("el apoyo debe viajar");
    assert_eq!(apoyo.muestras, 12_000);
    assert_eq!(apoyo.cola_nominal, 120.0);
}

/// §2: una condición que afirma compatibilidad no puede **contradecir** los benchmarks.
///
/// Si los benchmarks no la establecen (aquí, ambos sin metodología declarada), el motor
/// no puede aceptar una condición que diga lo contrario: `evaluar` toma la decisión más
/// conservadora de las dos, para que un llamador no pueda fabricar un veredicto.
#[test]
fn c1_una_condicion_no_puede_fabricar_compatibilidad() {
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let mut test = bench("b", 120.0, 110.0, 95.0, Some(12_000));
    base.metodologia_calculo = None;
    test.metodologia_calculo = None;

    // La condición miente a propósito.
    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::Establecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let ev = evaluar(&comparar(base, test), &cond);

    assert_eq!(
        ev.estado.0,
        CodigoDeEvaluacion::SOLO_DESCRIPTIVA,
        "no se puede proclamar un veredicto sobre dos metodologías desconocidas"
    );
    assert_eq!(
        ev.condiciones.compatibilidad_metodologica,
        CompatibilidadMetodologica::NoEstablecida
    );
}

/// §4: lo que produce el motor entra en la clasificación de evaluaciones soportadas de B.
///
/// La versión que escribe es la soportada y el estado está en el vocabulario conocido, así
/// que una evaluación futura guardada se reconocerá en vez de descartarse.
#[test]
fn c1_es_compatible_con_la_clasificacion_de_b() {
    let (c, cond) = con_cambios(8.0, 8.0, 8.0, Some(MUESTRAS_FIABLES), Some(5.0));
    let ev = evaluar(&c, &cond);

    assert!(
        ev.metodologia_interpretacion.es_soportada(),
        "la versión escrita debe ser la que B reconoce"
    );
    assert!(
        ev.estado.es_conocido(),
        "el estado debe estar en el vocabulario de B: {}",
        ev.estado.0
    );
    assert!(
        es_codigo_de_confianza(&ev.confianza.0),
        "la confianza debe ser un nivel de confianza, no un estado: {}",
        ev.confianza.0
    );
}

/// §4: `solo_descriptiva` y `datos_insuficientes` no pueden llevar preferencia ni
/// confianza interpretativa injustificada.
#[test]
fn c1_los_estados_sin_veredicto_no_llevan_preferencia() {
    // solo_descriptiva: dos metodologías desconocidas.
    let mut base = bench("a", 100.0, 90.0, 80.0, Some(12_000));
    let mut test = bench("b", 200.0, 180.0, 160.0, Some(12_000));
    base.metodologia_calculo = None;
    test.metodologia_calculo = None;
    let cond = CondicionesDeComparacion {
        compatibilidad_metodologica: CompatibilidadMetodologica::NoEstablecida,
        margen_porcentaje: Some(5.0),
        ruido_combinado_porcentaje: None,
        comandos_iguales: None,
        variable_evaluada: None,
    };
    let descriptiva = evaluar(&comparar(base, test), &cond);
    assert_eq!(descriptiva.estado.0, CodigoDeEvaluacion::SOLO_DESCRIPTIVA);

    // datos_insuficientes: ninguna métrica medible.
    let (mut c, cond2) = con_cambios(8.0, 8.0, 8.0, Some(12_000), Some(5.0));
    c.baseline.avg_fps = f64::NAN;
    c.baseline.fps_1_percent_low = f64::NAN;
    c.baseline.fps_0_1_percent_low = f64::NAN;
    c.test.avg_fps = f64::NAN;
    c.test.fps_1_percent_low = f64::NAN;
    c.test.fps_0_1_percent_low = f64::NAN;
    let insuficiente = evaluar(&c, &cond2);
    assert_eq!(
        insuficiente.estado.0,
        CodigoDeEvaluacion::DATOS_INSUFICIENTES
    );

    for ev in [&descriptiva, &insuficiente] {
        for m in &ev.metricas {
            assert_eq!(
                m.preferencia,
                PreferenciaDeMetrica::NoEstablecida,
                "{}: sin veredicto no hay preferencia",
                m.clave
            );
            assert!(
                !tiene(&m.motivos, codigos::PARTICIPO),
                "{}: sin veredicto no hay participación decisoria",
                m.clave
            );
        }
        // Y la confianza no afirma una fama que no existe.
        assert_ne!(ev.confianza.0, CodigoDeEvaluacion::MEJORA);
    }
}

/// §4: cada situación se distingue por **más de un** indicador, no solo por `preferencia`.
///
/// Se comprueba que las seis situaciones producen combinaciones distintas y reconocibles
/// de (dirección, preferencia, motivos, limitaciones), de modo que un consumidor pueda
/// distinguirlas sin adivinar.
#[test]
fn c1_las_seis_situaciones_se_distinguen() {
    // Participante favorable: fuera del margen, a favor.
    let (c, cond) = con_cambios(8.0, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
    let favorable = evaluar(&c, &cond);
    let m = metrica(&favorable, CLAVE_AVG_FPS);
    assert_eq!(m.preferencia, PreferenciaDeMetrica::FavorDeCandidata);
    assert!(tiene(&m.motivos, codigos::PARTICIPO));
    assert!(tiene(&m.motivos, codigos::FAVORECE_CANDIDATA));

    // Participante desfavorable.
    let (c, cond) = con_cambios(-8.0, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
    let desfavorable = evaluar(&c, &cond);
    let m = metrica(&desfavorable, CLAVE_AVG_FPS);
    assert_eq!(m.preferencia, PreferenciaDeMetrica::FavorDeReferencia);
    assert!(tiene(&m.motivos, codigos::PARTICIPO));
    assert!(tiene(&m.motivos, codigos::FAVORECE_REFERENCIA));

    // Participante dentro del margen: no es «sin datos», es «se miró y no destaca».
    let (c, cond) = con_cambios(2.0, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
    let dentro = evaluar(&c, &cond);
    let m = metrica(&dentro, CLAVE_AVG_FPS);
    assert_eq!(m.preferencia, PreferenciaDeMetrica::Indistinto);
    assert!(tiene(&m.motivos, codigos::DENTRO_DEL_MARGEN));
    assert!(m.direccion != DireccionDeCambio::NoConsta);
    // Participó (es la razón del `sin_cambio`) aunque no favorezca a nadie: la
    // participación no se deduce de la preferencia.
    assert!(tiene(&m.motivos, codigos::PARTICIPO));
    assert!(!tiene(&m.motivos, codigos::FAVORECE_CANDIDATA));
    assert!(!tiene(&m.motivos, codigos::FAVORECE_REFERENCIA));

    // Excluida por apoyo insuficiente: tiene magnitud, no decide.
    let (c, cond) = con_cambios(0.0, 0.0, 25.0, Some(300), Some(5.0));
    let excluida = evaluar(&c, &cond);
    let m = metrica(&excluida, CLAVE_FPS_0_1_PERCENT_LOW);
    assert!(tiene(&m.limitaciones, codigos::APOYO_INSUFICIENTE));
    assert!(tiene(&m.limitaciones, codigos::NO_ELEGIBLE));
    assert!(m.diferencia_pct.is_some(), "la magnitud se conserva");
    assert_eq!(m.preferencia, PreferenciaDeMetrica::NoEstablecida);

    // Apoyo desconocido: distinto de «insuficiente».
    let (c, cond) = con_cambios(0.0, 0.0, 25.0, None, Some(5.0));
    let desconocido = evaluar(&c, &cond);
    let m = metrica(&desconocido, CLAVE_FPS_0_1_PERCENT_LOW);
    assert!(tiene(&m.limitaciones, codigos::APOYO_NO_CONSTA));
    assert!(!tiene(&m.limitaciones, codigos::APOYO_INSUFICIENTE));

    // Inválida o ausente.
    let (mut c, cond) = con_cambios(0.0, 0.0, 0.0, Some(MUESTRAS_FIABLES), Some(5.0));
    c.test.avg_fps = f64::NAN;
    let invalida = evaluar(&c, &cond);
    let m = metrica(&invalida, CLAVE_AVG_FPS);
    assert_eq!(m.direccion, DireccionDeCambio::NoConsta);
    assert!(tiene(&m.motivos, codigos::SIN_DATOS));
    assert!(m.diferencia.is_none());

    // Estabilidad: no es una métrica del motor, solo puede describirse fuera.
    for ev in [
        &favorable,
        &desfavorable,
        &dentro,
        &excluida,
        &desconocido,
        &invalida,
    ] {
        assert!(
            !ev.metricas.iter().any(|m| m.clave == "estabilidad"),
            "la estabilidad no entra en la decisión"
        );
    }

    // Las cuatro situaciones decisorias no colapsan en el mismo resultado.
    let firma = |ev: &EvaluacionDeComparacion, clave: &str| {
        let m = metrica(ev, clave);
        (m.direccion, m.preferencia, m.motivos.clone())
    };
    assert_ne!(
        firma(&favorable, CLAVE_AVG_FPS),
        firma(&desfavorable, CLAVE_AVG_FPS)
    );
    assert_ne!(
        firma(&favorable, CLAVE_AVG_FPS),
        firma(&dentro, CLAVE_AVG_FPS)
    );
    assert_ne!(
        firma(&dentro, CLAVE_AVG_FPS),
        firma(&invalida, CLAVE_AVG_FPS)
    );
}

/// §1: el motor y la comparación numérica usan **la misma** implementación de ruido.
///
/// Se compara, caso por caso, el valor que `condiciones_de` deja en las condiciones con el
/// que `benchmarks::combined_noise` devuelve para las mismas entradas. Coincidir aquí no
/// demuestra que sea la misma función (podría ser una copia fiel), pero junto con el
/// hecho de que el motor la **llama** directamente, fija el comportamiento compartido.
#[test]
fn c1_el_motor_reutiliza_la_misma_funcion_de_ruido() {
    let entradas = [
        (None, None),
        (Some(0.0), Some(0.0)),
        (Some(0.0), Some(2.0)),
        (Some(6.0), Some(8.0)),
        (Some(3.0), Some(4.0)),
        (Some(8.0), None),
        (Some(0.1), Some(0.1)),
        (Some(f64::NAN), Some(5.0)),
        (Some(-1.0), Some(4.0)),
    ];

    for (base, test) in entradas {
        let esperado = crate::benchmarks::combined_noise(base, test);
        let (_c, cond) = con_ruido(base, test);

        // Con al menos un valor declarado, la condición registra exactamente lo que
        // devuelve la función compartida. Sin ninguno, `None` = no consta.
        let tiene_dato = [base, test]
            .iter()
            .any(|v| v.is_some_and(|x| x.is_finite() && x >= 0.0));
        if tiene_dato {
            assert_eq!(
                cond.ruido_combinado_porcentaje,
                Some(esperado),
                "{base:?}/{test:?}: el motor no usa el ruido compartido"
            );
        } else {
            assert_eq!(
                cond.ruido_combinado_porcentaje, None,
                "{base:?}/{test:?}: no consta"
            );
        }

        // Y el margen es siempre el mayor entre el ruido combinado y el suelo del 5 %.
        assert_eq!(
            cond.margen_porcentaje,
            Some(esperado.max(MIN_TIE_PERCENT)),
            "{base:?}/{test:?}: margen"
        );
    }
}

/// §2: una evaluación parcial real sigue siendo **soportada** por la clasificación de B.
///
/// `es_soportada` valida solo la versión y el estado; `motivos` y `limitaciones` son
/// códigos abiertos a propósito. Se fija aquí para que C.2 no descubra tarde que los
/// códigos nuevos invalidaban el registro.
#[test]
fn c1_la_evaluacion_parcial_es_soportada_por_b() {
    let (c, cond) = con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    // El caso 14: mejora con una pérdida observada en una métrica excluida.
    assert_eq!(ev.estado.0, CodigoDeEvaluacion::MEJORA);
    assert!(tiene(&ev.limitaciones, codigos::EVALUACION_PARCIAL));
    assert!(tiene(
        &ev.limitaciones,
        codigos::PERDIDA_OBSERVADA_SIN_EVALUAR
    ));

    assert!(
        ev.es_soportada(),
        "una evaluación con códigos nuevos debe seguir siendo soportada"
    );
    assert!(ev.corresponde_a(&c), "referencia y candidata deben cuadrar");

    // Y sobrevive la vuelta a disco conservando los códigos.
    let json = serde_json::to_string(&ev).unwrap();
    let releida: EvaluacionDeComparacion = serde_json::from_str(&json).unwrap();
    assert!(releida.es_soportada());
    assert_eq!(releida.limitaciones, ev.limitaciones);
    assert_eq!(releida.condiciones.margen_porcentaje, Some(5.0));
}

/// Evalúa el fixture ya construido por `con_cambios`.
fn ev_de(preparado: &(BenchmarkComparison, CondicionesDeComparacion)) -> EvaluacionDeComparacion {
    evaluar(&preparado.0, &preparado.1)
}

/// El campo `confianza` admite **solo** niveles de confianza, nunca códigos de estado.
///
/// Es la garantía que impide volver a escribir `mejora` como si fuera una confianza.
fn es_codigo_de_confianza(c: &str) -> bool {
    matches!(
        c,
        codigos::CONFIANZA_OBSERVADA
            | codigos::CONFIANZA_NO_CONCLUYENTE
            | codigos::CONFIANZA_INSUFICIENTE
    )
}

/// Defecto 1 (revisión): el campo `confianza` llevaba **códigos de estado** (`mejora`,
/// `solo_descriptiva`). Un estado dice qué cambió; la confianza dice qué lo sostiene.
///
/// Se recorren los seis estados del motor y se exige que la confianza sea siempre un
/// nivel de confianza del vocabulario cerrado.
#[test]
fn c1_la_confianza_usa_solo_codigos_de_confianza() {
    let casos: Vec<EvaluacionDeComparacion> = vec![
        // compatibilidad establecida → mejora
        ev_de(&con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0))),
        // → empeora
        ev_de(&con_cambios(-12.5, 0.0, -25.0, Some(300), Some(5.0))),
        // → mixto
        ev_de(&con_cambios(12.5, 12.5, -25.0, Some(300), Some(5.0))),
        // → sin_cambio (todo dentro del margen)
        ev_de(&con_cambios(1.0, 1.0, 1.0, Some(20000), Some(5.0))),
        // → datos_insuficientes (sin margen)
        ev_de(&con_cambios(10.0, 0.0, 0.0, Some(20000), None)),
        // → solo_descriptiva (compatibilidad no establecida)
        {
            let (c, mut cond) = con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0));
            cond.compatibilidad_metodologica =
                crate::benchmarks::CompatibilidadMetodologica::NoEstablecida;
            evaluar(&c, &cond)
        },
    ];

    for ev in &casos {
        assert!(
            es_codigo_de_confianza(&ev.confianza.0),
            "estado `{}` con confianza `{}`: no es un nivel de confianza",
            ev.estado.0,
            ev.confianza.0
        );
        // Y en particular, nunca se repite el estado como si fuera confianza.
        assert_ne!(
            ev.confianza.0, ev.estado.0,
            "la confianza no puede ser el propio estado"
        );
    }

    // Superar el margen **no** da confianza alta: como mucho, «observada».
    let mejora = ev_de(&con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0)));
    assert_eq!(mejora.estado.0, CodigoDeEvaluacion::MEJORA);
    assert_eq!(mejora.confianza.0, codigos::CONFIANZA_OBSERVADA);
    assert_ne!(mejora.confianza.0, "alta");
}

/// Defecto 2 (revisión): una métrica **excluida** conservaba el motivo
/// `favorece_referencia` mientras su campo `preferencia` decía `no_establecida`. El motivo
/// afirmaba por la puerta de atrás la preferencia que el campo negaba.
///
/// Una métrica excluida conserva magnitud, dirección y motivo de exclusión, y **no**
/// favorece a nadie: ni por el campo ni por los motivos.
#[test]
fn c1_una_metrica_excluida_no_favorece_a_nadie() {
    // El 0,1 % low cae un 25 % (favorecería a la referencia) pero tiene 300 muestras.
    let (c, cond) = con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0));
    let ev = evaluar(&c, &cond);

    let low01 = metrica(&ev, CLAVE_FPS_0_1_PERCENT_LOW);

    // Conserva la información observada.
    assert_eq!(low01.direccion, DireccionDeCambio::Baja);
    assert_eq!(low01.diferencia_pct, Some(-25.0));
    assert!(tiene(&low01.limitaciones, codigos::NO_ELEGIBLE));
    assert!(tiene(&low01.limitaciones, codigos::APOYO_INSUFICIENTE));

    // Y no favorece a ninguna configuración, ni por campo ni por motivo.
    assert_eq!(low01.preferencia, PreferenciaDeMetrica::NoEstablecida);
    assert!(
        !tiene(&low01.motivos, codigos::FAVORECE_REFERENCIA),
        "el motivo no puede afirmar la preferencia que el campo niega"
    );
    assert!(
        !tiene(&low01.motivos, codigos::FAVORECE_CANDIDATA),
        "tampoco puede favorecer a la candidata"
    );
}

/// La misma comprobación que en `c1_una_metrica_excluida_no_favorece_a_nadie`, extendida a
/// los estados que no concluyen: `solo_descriptiva`, `datos_insuficientes` y las métricas
/// sin datos. En ninguno puede quedar una preferencia, ni por campo ni por motivo, y la
/// confianza no puede ser un estado.
#[test]
fn c1_los_estados_sin_veredicto_no_favorecen_ni_fingen_confianza() {
    let casos: Vec<(&str, EvaluacionDeComparacion)> = vec![
        ("solo_descriptiva", {
            let (c, mut cond) = con_cambios(12.5, 0.0, -25.0, Some(300), Some(5.0));
            cond.compatibilidad_metodologica =
                crate::benchmarks::CompatibilidadMetodologica::NoEstablecida;
            evaluar(&c, &cond)
        }),
        (
            "datos_insuficientes (sin margen)",
            ev_de(&con_cambios(10.0, 0.0, 0.0, Some(20000), None)),
        ),
        (
            "datos_insuficientes (sin métricas medibles)",
            ev_de(&con_cambios(0.0, 0.0, 0.0, Some(300), Some(5.0))),
        ),
    ];

    for (etiqueta, ev) in casos {
        assert!(
            es_codigo_de_confianza(&ev.confianza.0),
            "{etiqueta}: confianza `{}` no es un nivel de confianza",
            ev.confianza.0
        );

        // Ningún veredicto de preferencia global en los motivos.
        assert!(
            !tiene(&ev.motivos, codigos::FAVORECE_CANDIDATA)
                && !tiene(&ev.motivos, codigos::FAVORECE_REFERENCIA),
            "{etiqueta}: un estado sin veredicto no puede favorecer a nadie"
        );

        // Y cada métrica: si no es elegible, no favorece por ninguna vía.
        for m in &ev.metricas {
            if m.preferencia == PreferenciaDeMetrica::NoEstablecida {
                assert!(
                    !tiene(&m.motivos, codigos::FAVORECE_CANDIDATA)
                        && !tiene(&m.motivos, codigos::FAVORECE_REFERENCIA),
                    "{etiqueta}/{}: el motivo contradice la preferencia",
                    m.clave
                );
            }
        }
    }
}
