//! The Markdown report and the summary line (PRD, *Salida*; K7, K13).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, FixedOffset};

use crate::measure::{meets, ratio_text, thousands, whole};
use crate::workload::{Engine, Load, Scale, runs};

/// What is known of the machine. `None` is shown as `desconocido`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Machine {
    /// Name and version of the operating system; on Linux, the distribution with its version and the kernel's.
    pub system: Option<String>,
    pub cpu_model: Option<String>,
    pub logical_cores: Option<usize>,
    pub memory_bytes: Option<u64>,
    /// The disk of the work folder.
    pub disk_model: Option<String>,
    pub file_system: Option<String>,
}

/// Where the engines were measured: the *Núcleos* line (B5 of the Krummi and Voron only PRD).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Cores {
    /// Every engine process ran on the `performance` logical cores, out of the `total` the system gives.
    Performance { performance: usize, total: usize },
    /// The system gives all its cores one class, or none.
    Unpinned,
    /// The classes could not be asked for, or a process could not be limited.
    #[default]
    Unknown,
}

/// Versions found at run time. `None` is shown as `desconocido`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Versions {
    /// The first 12 characters of the last commit of the working copy.
    pub krummi_commit: Option<String>,
    /// Whether tracked files have changes; untracked files do not count.
    pub krummi_dirty: bool,
    /// The .NET runtime of the Voron program, such as `10.0.5`.
    pub dotnet: Option<String>,
    /// The LMDB C library heed compiles, such as `0.9.70`.
    pub lmdb: Option<String>,
    /// The Rust compiler, such as `1.89.0`.
    pub rust: Option<String>,
}

/// A finished run: everything the report shows.
#[derive(Debug, Clone)]
pub struct Run {
    pub started: DateTime<FixedOffset>,
    pub scale: Scale,
    /// `--solo-krummi-voron`: the report has only the Krummi and Voron columns (B3).
    pub only_krummi_voron: bool,
    pub machine: Machine,
    /// The *Núcleos* line (B5).
    pub cores: Cores,
    pub versions: Versions,
    /// The metric of every load and engine that ran it (K8).
    pub metrics: BTreeMap<(Load, Engine), f64>,
    /// The size of the store after the loads of K9.
    pub sizes: BTreeMap<(Load, Engine), u64>,
    /// The options the benchmark changed, per engine with any, in the order of K3 (*Opciones cambiadas*).
    pub changed_options: Vec<(Engine, Vec<String>)>,
}

/// The options the `engines` of a run run with (`Engine::changed_options`), for `Run::changed_options`.
pub fn changed_options(engines: &[Engine]) -> Vec<(Engine, Vec<String>)> {
    engines
        .iter()
        .copied()
        .filter(|e| !e.changed_options().is_empty())
        .map(|e| {
            (
                e,
                e.changed_options().iter().map(|o| o.to_string()).collect(),
            )
        })
        .collect()
}

const UNKNOWN: &str = "desconocido";
const NOT_APPLICABLE: &str = "no aplica";

fn or_unknown(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or(UNKNOWN)
}

impl Run {
    /// The loads whose threshold Krummi misses, in table order. Empty in a reduced run (K11).
    pub fn missed(&self) -> Vec<Load> {
        if self.scale == Scale::Reduced {
            return Vec::new();
        }
        Load::ALL
            .into_iter()
            .filter(|&load| {
                !meets(
                    self.metric(load, Engine::Krummi),
                    self.metric(load, Engine::Voron),
                )
            })
            .collect()
    }

    fn metric(&self, load: Load, engine: Engine) -> f64 {
        *self
            .metrics
            .get(&(load, engine))
            .unwrap_or_else(|| panic!("no metric for {engine:?} in {load:?}"))
    }

    /// The exit code of the command once the report is written (K13): `1` if a threshold is missed.
    pub fn exit_code(&self) -> i32 {
        if self.missed().is_empty() { 0 } else { 1 }
    }

    /// The sentence of the *Resultado* section, without the list of missed loads.
    fn result_sentence(&self) -> String {
        match self.scale {
            Scale::Reduced => "Ejecución reducida, sin umbrales.".to_string(),
            Scale::Full => {
                let total = Load::ALL.len();
                format!(
                    "Krummi cumple {} de {total} umbrales.",
                    total - self.missed().len()
                )
            }
        }
    }

    /// The line the command writes on standard output, with the report's file as shown.
    pub fn summary_line(&self, report: &str) -> String {
        format!("{} Informe: {report}", self.result_sentence())
    }

    /// The Markdown report.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let m = &self.machine;
        let v = &self.versions;
        let cpu = match (&m.cpu_model, m.logical_cores) {
            (None, None) => UNKNOWN.to_string(),
            (model, cores) => format!(
                "{}, {} núcleos lógicos",
                or_unknown(model),
                cores.map_or(UNKNOWN.to_string(), |c| c.to_string())
            ),
        };
        let memory = m.memory_bytes.map_or(UNKNOWN.to_string(), |b| {
            format!("{} GiB", (b as f64 / (1u64 << 30) as f64).round() as u64)
        });
        let disk = match (&m.disk_model, &m.file_system) {
            (None, None) => UNKNOWN.to_string(),
            (model, fs) => format!("{}, {}", or_unknown(model), or_unknown(fs)),
        };
        let krummi = match &v.krummi_commit {
            None => UNKNOWN.to_string(),
            Some(c) if v.krummi_dirty => format!("{c} (con cambios sin confirmar)"),
            Some(c) => c.clone(),
        };
        let (scale, repetitions) = match self.scale {
            Scale::Full => ("completa", "1 previa y 3 medidas, mediana"),
            Scale::Reduced => ("reducida", "1 medida"),
        };
        let execution = if self.only_krummi_voron {
            format!("{scale}, solo Krummi y Voron")
        } else {
            scale.to_string()
        };
        let voron = format!("Voron 7.2.7 (.NET {})", or_unknown(&v.dotnet));
        let engines_line = if self.only_krummi_voron {
            voron
        } else {
            format!(
                "{voron}, LMDB {} (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13",
                or_unknown(&v.lmdb)
            )
        };
        let _ = writeln!(out, "# Banco comparativo de Krummi\n");
        let _ = writeln!(
            out,
            "- Fecha: {}",
            self.started.format("%Y-%m-%d %H:%M (UTC%:z)")
        );
        let _ = writeln!(out, "- Ejecución: {execution}");
        let _ = writeln!(out, "- Sistema: {}", or_unknown(&m.system));
        let _ = writeln!(out, "- Procesador: {cpu}");
        let cores = match self.cores {
            Cores::Performance { performance, total } => format!(
                "{performance} lógicos de rendimiento de {total} (los motores se miden en ellos)"
            ),
            Cores::Unpinned => {
                "sin fijar (el sistema no distingue núcleos de rendimiento)".to_string()
            }
            Cores::Unknown => UNKNOWN.to_string(),
        };
        let _ = writeln!(out, "- Núcleos: {cores}");
        let _ = writeln!(out, "- Memoria: {memory}");
        let _ = writeln!(out, "- Disco: {disk}");
        let _ = writeln!(out, "- Krummi: {krummi}");
        let _ = writeln!(out, "- Motores: {engines_line}");
        let _ = writeln!(out, "- Rust: {}", or_unknown(&v.rust));
        let _ = writeln!(out, "- Repeticiones: {repetitions}");

        let _ = writeln!(out, "\n## Resultado\n");
        let missed = self.missed();
        if missed.is_empty() {
            let _ = writeln!(out, "{}", self.result_sentence());
        } else {
            let list: Vec<String> = missed.iter().map(|l| format!("`{}`", l.name())).collect();
            let _ = writeln!(
                out,
                "{} Incumple: {}.",
                self.result_sentence(),
                list.join(", ")
            );
        }

        let measured = Engine::measured(self.only_krummi_voron);
        let engines: Vec<&str> = measured.iter().map(|e| e.name()).collect();
        let _ = writeln!(out, "\n## Operaciones por segundo\n");
        let _ = writeln!(
            out,
            "| Carga | {} | Krummi / Voron | Umbral |",
            engines.join(" | ")
        );
        let _ = writeln!(out, "|---|{}---|---|", "---|".repeat(engines.len()));
        for load in Load::ALL {
            let cells: Vec<String> = measured
                .iter()
                .map(|&e| {
                    if runs(e, load) {
                        whole(self.metric(load, e))
                    } else {
                        NOT_APPLICABLE.to_string()
                    }
                })
                .collect();
            let (k, vo) = (
                self.metric(load, Engine::Krummi),
                self.metric(load, Engine::Voron),
            );
            let verdict = match self.scale {
                Scale::Reduced => "sin evaluar",
                Scale::Full if meets(k, vo) => "cumple",
                Scale::Full => "incumple",
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {verdict} |",
                load.name(),
                cells.join(" | "),
                ratio_text(k, vo)
            );
        }

        let _ = writeln!(out, "\n## Tamaño del almacén (bytes)\n");
        let _ = writeln!(out, "| Carga | {} |", engines.join(" | "));
        let _ = writeln!(out, "|---|{}", "---|".repeat(engines.len()));
        for load in Load::SIZED {
            let cells: Vec<String> = measured
                .iter()
                .map(|&e| {
                    if runs(e, load) {
                        let size = self
                            .sizes
                            .get(&(load, e))
                            .unwrap_or_else(|| panic!("no size for {e:?} in {load:?}"));
                        thousands(*size)
                    } else {
                        NOT_APPLICABLE.to_string()
                    }
                })
                .collect();
            let _ = writeln!(out, "| {} | {} |", load.name(), cells.join(" | "));
        }

        let _ = writeln!(out, "\n## Opciones cambiadas\n");
        if self.changed_options.is_empty() {
            let _ = writeln!(out, "Ninguna.");
        } else {
            for (e, options) in &self.changed_options {
                let _ = writeln!(out, "- {}: {}.", e.name(), options.join("; "));
            }
        }
        out
    }
}

/// The default name of the report: `banco-<AAAAMMDD>-<HHMMSS>.md`, with the local start time (K10).
pub fn default_report_name(started: &DateTime<FixedOffset>) -> String {
    started.format("banco-%Y%m%d-%H%M%S.md").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn run(scale: Scale, krummi_factor: impl Fn(Load) -> f64) -> Run {
        let mut metrics = BTreeMap::new();
        let mut sizes = BTreeMap::new();
        for load in Load::ALL {
            for (i, &e) in Engine::ALL.iter().enumerate() {
                if !runs(e, load) {
                    continue;
                }
                let voron = 1_000.0 * load.number() as f64;
                let value = match e {
                    Engine::Krummi => voron * krummi_factor(load),
                    Engine::Voron => voron,
                    _ => voron + i as f64 + 0.4,
                };
                metrics.insert((load, e), value);
                if Load::SIZED.contains(&load) {
                    sizes.insert((load, e), 1_000 * load.number() + i as u64);
                }
            }
        }
        Run {
            started: FixedOffset::east_opt(3_600)
                .unwrap()
                .with_ymd_and_hms(2026, 11, 3, 10, 15, 0)
                .unwrap(),
            scale,
            only_krummi_voron: false,
            cores: Cores::Performance {
                performance: 16,
                total: 32,
            },
            machine: Machine {
                system: Some("Windows 11 Pro 10.0.26200".into()),
                cpu_model: Some("CPU X".into()),
                logical_cores: Some(32),
                memory_bytes: Some(64 << 30),
                disk_model: Some("Disk Y".into()),
                file_system: Some("NTFS".into()),
            },
            versions: Versions {
                krummi_commit: Some("0123456789ab".into()),
                krummi_dirty: false,
                dotnet: Some("10.0.5".into()),
                lmdb: Some("0.9.70".into()),
                rust: Some("1.89.0".into()),
            },
            metrics,
            sizes,
            changed_options: changed_options(&Engine::ALL),
        }
    }

    fn row(report: &str, load: Load) -> &str {
        report
            .lines()
            .find(|l| l.starts_with(&format!("| {} |", load.name())))
            .unwrap()
    }

    const HEADER: &str = "# Banco comparativo de Krummi

- Fecha: 2026-11-03 10:15 (UTC+01:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: CPU X, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: Disk Y, NTFS
- Krummi: 0123456789ab
- Motores: Voron 7.2.7 (.NET 10.0.5), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.89.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

";

    // AC-K2: the whole report, byte for byte, with every threshold met.
    #[test]
    fn a_report_with_every_threshold_met_has_the_exact_shape() {
        let r = run(Scale::Full, |_| 1.02);
        let text = r.render();
        assert!(text.starts_with(HEADER), "{text}");
        let rest = &text[HEADER.len()..];
        let expected_start = "Krummi cumple 32 de 32 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 1.020 | 1.000 | 1.002 | 1.003 | 1.004 | 1.005 | 1,02 | cumple |
";
        assert!(rest.starts_with(expected_start), "{rest}");
        assert_eq!(
            row(&text, Load::W21),
            "| tamano-fijo-carga | 21.420 | 21.000 | 21.002 | 21.003 | no aplica | no aplica | 1,02 | cumple |"
        );
        // E21: the table loads, after W22, with a threshold and `no aplica` in the engines that are not Voron.
        assert_eq!(
            row(&text, Load::W23),
            "| tabla-insercion-en-orden | 23.460 | 23.000 | no aplica | no aplica | no aplica | no aplica | 1,02 | cumple |"
        );
        assert_eq!(
            row(&text, Load::W29),
            "| tabla-borrado | 29.580 | 29.000 | no aplica | no aplica | no aplica | no aplica | 1,02 | cumple |"
        );
        // IX15: the index loads, after W29, likewise.
        assert_eq!(
            row(&text, Load::W30),
            "| tabla-insercion-con-indices | 30.600 | 30.000 | no aplica | no aplica | no aplica | no aplica | 1,02 | cumple |"
        );
        assert_eq!(
            row(&text, Load::W32),
            "| tabla-intervalos-de-indice | 32.640 | 32.000 | no aplica | no aplica | no aplica | no aplica | 1,02 | cumple |"
        );
        let rates: Vec<&str> = text
            .lines()
            .skip_while(|l| !l.starts_with("| Carga | Krummi"))
            .skip(2)
            .take_while(|l| l.starts_with("| "))
            .map(|l| l.split(" | ").next().unwrap().trim_start_matches("| "))
            .collect();
        let names: Vec<&str> = Load::ALL.iter().map(|l| l.name()).collect();
        assert_eq!(rates, names);
        let tail = "
## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 1.000 | 1.001 | 1.002 | 1.003 | 1.004 | 1.005 |
| carga-al-azar | 2.000 | 2.001 | 2.002 | 2.003 | 2.004 | 2.005 |
| carga-masiva | 3.000 | 3.001 | 3.002 | 3.003 | 3.004 | 3.005 |
| borrado | 14.000 | 14.001 | 14.002 | 14.003 | 14.004 | 14.005 |
| reutilizacion | 15.000 | 15.001 | 15.002 | 15.003 | 15.004 | 15.005 |
| valores-grandes-carga | 17.000 | 17.001 | 17.002 | 17.003 | 17.004 | 17.005 |
| multivalor-carga | 19.000 | 19.001 | 19.002 | 19.003 | no aplica | no aplica |
| tamano-fijo-carga | 21.000 | 21.001 | 21.002 | 21.003 | no aplica | no aplica |
| tabla-insercion-en-orden | 23.000 | 23.001 | no aplica | no aplica | no aplica | no aplica |
| tabla-insercion-al-azar | 24.000 | 24.001 | no aplica | no aplica | no aplica | no aplica |
| tabla-carga-masiva | 25.000 | 25.001 | no aplica | no aplica | no aplica | no aplica |
| tabla-borrado | 29.000 | 29.001 | no aplica | no aplica | no aplica | no aplica |
| tabla-insercion-con-indices | 30.000 | 30.001 | no aplica | no aplica | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
";
        assert!(text.ends_with(tail), "{text}");
        assert_eq!(
            text.lines().filter(|l| l.starts_with("| ")).count(),
            1 + 32 + 1 + 13
        );
        assert_eq!(r.exit_code(), 0);
        assert_eq!(
            r.summary_line("banco-20261103-101500.md"),
            "Krummi cumple 32 de 32 umbrales. Informe: banco-20261103-101500.md"
        );
    }

    /// `run` with only the Krummi and Voron results, as a `--solo-krummi-voron` run leaves them (B2).
    fn only_krummi_voron(scale: Scale, krummi_factor: impl Fn(Load) -> f64) -> Run {
        let mut r = run(scale, krummi_factor);
        let kept = |e: &Engine| Engine::KRUMMI_VORON.contains(e);
        r.metrics.retain(|(_, e), _| kept(e));
        r.sizes.retain(|(_, e), _| kept(e));
        r.only_krummi_voron = true;
        r.changed_options = changed_options(&Engine::KRUMMI_VORON);
        r
    }

    // AC-B2 (B3): the whole `--solo-krummi-voron` report, byte for byte, with every threshold met.
    #[test]
    fn a_krummi_and_voron_only_report_has_the_exact_shape() {
        let r = only_krummi_voron(Scale::Full, |_| 1.02);
        let text = r.render();
        let header = HEADER
            .replace(
                "- Ejecución: completa
",
                "- Ejecución: completa, solo Krummi y Voron
",
            )
            .replace(
                ", LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13",
                "",
            );
        let mut expected = header
            + "Krummi cumple 32 de 32 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
";
        for load in Load::ALL {
            let voron = 1_000 * load.number();
            expected += &format!(
                "| {} | {} | {} | 1,02 | cumple |
",
                load.name(),
                thousands(voron * 102 / 100),
                thousands(voron)
            );
        }
        expected += "
## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
";
        for load in Load::SIZED {
            let size = 1_000 * load.number();
            expected += &format!(
                "| {} | {} | {} |
",
                load.name(),
                thousands(size),
                thousands(size + 1)
            );
        }
        expected += "
## Opciones cambiadas

Ninguna.
";
        assert_eq!(text, expected);
        assert!(!text.contains(NOT_APPLICABLE));
        assert_eq!(r.exit_code(), 0);
        assert_eq!(
            r.summary_line("x.md"),
            "Krummi cumple 32 de 32 umbrales. Informe: x.md"
        );
    }

    // AC-B2: a missed threshold and a reduced run keep the benchmark's rules with only two engine columns.
    #[test]
    fn a_krummi_and_voron_only_report_keeps_the_thresholds_and_the_reduced_rules() {
        let missed = only_krummi_voron(Scale::Full, |l| if l == Load::W28 { 0.79 } else { 1.0 });
        let text = missed.render();
        assert!(text.contains(
            "
## Resultado

Krummi cumple 31 de 32 umbrales. Incumple: `tabla-actualizacion`.
"
        ));
        assert_eq!(
            row(&text, Load::W28),
            "| tabla-actualizacion | 22.120 | 28.000 | 0,79 | incumple |"
        );
        assert_eq!(missed.exit_code(), 1);
        assert_eq!(
            missed.summary_line("x.md"),
            "Krummi cumple 31 de 32 umbrales. Informe: x.md"
        );

        let reduced = only_krummi_voron(Scale::Reduced, |_| 0.1);
        let text = reduced.render();
        assert!(text.contains(
            "- Ejecución: reducida, solo Krummi y Voron
"
        ));
        assert!(text.contains(
            "- Repeticiones: 1 medida
"
        ));
        assert!(text.contains(
            "
## Resultado

Ejecución reducida, sin umbrales.
"
        ));
        assert_eq!(
            row(&text, Load::W1),
            "| carga-en-orden | 100 | 1.000 | 0,10 | sin evaluar |"
        );
        assert_eq!(reduced.exit_code(), 0);
        assert_eq!(
            reduced.summary_line("r.md"),
            "Ejecución reducida, sin umbrales. Informe: r.md"
        );
    }

    // AC-K2 and AC-K3: one missed threshold, then several; 0.7996 shows 0,79 and misses; exactly 0.80 meets.
    #[test]
    fn missed_thresholds_are_listed_in_table_order() {
        let one = run(Scale::Full, |l| if l == Load::W6 { 0.7996 } else { 0.8 });
        let text = one.render();
        assert!(text.contains(
            "\n## Resultado\n\nKrummi cumple 31 de 32 umbrales. Incumple: `escrituras-concurrentes-16`.\n"
        ));
        assert!(
            row(&text, Load::W6).ends_with("| 0,79 | incumple |"),
            "{}",
            row(&text, Load::W6)
        );
        assert!(
            row(&text, Load::W5).ends_with("| 0,80 | cumple |"),
            "{}",
            row(&text, Load::W5)
        );
        assert_eq!(one.exit_code(), 1);
        assert_eq!(
            one.summary_line("x.md"),
            "Krummi cumple 31 de 32 umbrales. Informe: x.md"
        );

        let several = run(Scale::Full, |l| match l {
            Load::W22 | Load::W1 | Load::W13 => 0.5,
            _ => 1.0,
        });
        assert!(several.render().contains(
            "Krummi cumple 29 de 32 umbrales. Incumple: `carga-en-orden`, `escritor-con-lectores`, `tamano-fijo-lectura`.\n"
        ));
        assert_eq!(several.missed(), [Load::W1, Load::W13, Load::W22]);

        // A table load's threshold counts as any other (E21, K7).
        let table = run(Scale::Full, |l| if l == Load::W28 { 0.79 } else { 1.0 });
        let text = table.render();
        assert!(text.contains(
            "\n## Resultado\n\nKrummi cumple 31 de 32 umbrales. Incumple: `tabla-actualizacion`.\n"
        ));
        assert_eq!(
            row(&text, Load::W28),
            "| tabla-actualizacion | 22.120 | 28.000 | no aplica | no aplica | no aplica | no aplica | 0,79 | incumple |"
        );
        assert_eq!(table.exit_code(), 1);

        // So does an index load's (IX15).
        let index = run(Scale::Full, |l| if l == Load::W31 { 0.79 } else { 1.0 });
        assert!(index.render().contains(
            "
## Resultado

Krummi cumple 31 de 32 umbrales. Incumple: `tabla-busqueda-por-indice`.
"
        ));
        assert_eq!(index.missed(), [Load::W31]);
        assert_eq!(index.exit_code(), 1);
    }

    // AC-K2 and AC-K3: a Voron metric of 0 (W12 with no Voron read finished) shows `∞` when Krummi's is positive
    // and `0,00` when it is 0 too, and both meet the threshold (R1 of the Codex PRD review).
    #[test]
    fn a_zero_voron_metric_shows_infinity_or_zero_and_meets() {
        for (krummi, expected_row) in [
            (
                5.0,
                "| lecturas-con-escritor | 5 | 0 | 12.002 | 12.003 | 12.004 | 12.005 | ∞ | cumple |",
            ),
            (
                0.0,
                "| lecturas-con-escritor | 0 | 0 | 12.002 | 12.003 | 12.004 | 12.005 | 0,00 | cumple |",
            ),
        ] {
            let mut r = run(Scale::Full, |_| 1.0);
            r.metrics.insert((Load::W12, Engine::Voron), 0.0);
            r.metrics.insert((Load::W12, Engine::Krummi), krummi);
            let text = r.render();
            assert_eq!(row(&text, Load::W12), expected_row);
            assert!(text.contains("\n## Resultado\n\nKrummi cumple 32 de 32 umbrales.\n"));
            assert_eq!(r.exit_code(), 0);
            assert_eq!(
                r.summary_line("x.md"),
                "Krummi cumple 32 de 32 umbrales. Informe: x.md"
            );
        }
    }

    // AC-K2 (annotation of 2026-11-07): rates, sizes and the ratio's integer part carry a dot every three digits,
    // after rounding; numbers under 1,000 and the rest of the report stay as they were.
    #[test]
    fn table_numbers_carry_thousands_separators() {
        let mut r = run(Scale::Full, |_| 1.0);
        for (load, krummi, voron) in [
            (Load::W1, 2_500_009.0, 1_000.0),
            (Load::W2, 999.6, 86.0),
            (Load::W3, 0.0, 1_184.0),
        ] {
            r.metrics.insert((load, Engine::Krummi), krummi);
            r.metrics.insert((load, Engine::Voron), voron);
        }
        r.sizes.insert((Load::W1, Engine::Krummi), 134_217_728);
        r.sizes.insert((Load::W2, Engine::Voron), 86);
        let text = r.render();
        assert_eq!(
            row(&text, Load::W1),
            "| carga-en-orden | 2.500.009 | 1.000 | 1.002 | 1.003 | 1.004 | 1.005 | 2.500,00 | cumple |"
        );
        assert_eq!(
            row(&text, Load::W2),
            "| carga-al-azar | 1.000 | 86 | 2.002 | 2.003 | 2.004 | 2.005 | 11,62 | cumple |"
        );
        assert_eq!(
            row(&text, Load::W3),
            "| carga-masiva | 0 | 1.184 | 3.002 | 3.003 | 3.004 | 3.005 | 0,00 | incumple |"
        );
        let sizes = &text[text.find("## Tamaño del almacén").unwrap()..];
        assert!(
            sizes.contains(
                "\n| carga-en-orden | 134.217.728 | 1.001 | 1.002 | 1.003 | 1.004 | 1.005 |\n"
            ),
            "{sizes}"
        );
        assert!(
            sizes.contains("\n| carga-al-azar | 2.000 | 86 | 2.002 | 2.003 | 2.004 | 2.005 |\n"),
            "{sizes}"
        );
        assert!(text.starts_with(HEADER), "{text}");
        assert_eq!(
            r.summary_line("x.md"),
            "Krummi cumple 31 de 32 umbrales. Informe: x.md"
        );
    }

    // AC-K2: a reduced run shows its values and says `sin evaluar`.
    #[test]
    fn a_reduced_run_evaluates_no_threshold() {
        let r = run(Scale::Reduced, |_| 0.1);
        let text = r.render();
        assert!(text.contains("- Ejecución: reducida\n"));
        assert!(text.contains("- Repeticiones: 1 medida\n"));
        assert!(text.contains("\n## Resultado\n\nEjecución reducida, sin umbrales.\n"));
        assert!(
            row(&text, Load::W1).ends_with("| 0,10 | sin evaluar |"),
            "{}",
            row(&text, Load::W1)
        );
        assert!(text.contains("## Tamaño del almacén (bytes)"));
        assert_eq!(r.exit_code(), 0);
        assert_eq!(
            r.summary_line("r.md"),
            "Ejecución reducida, sin umbrales. Informe: r.md"
        );
    }

    // AC-K2: unknown machine data and versions, and a working copy with changes.
    #[test]
    fn unknown_values_are_shown_as_desconocido() {
        let mut r = run(Scale::Full, |_| 1.0);
        r.machine = Machine::default();
        r.versions = Versions::default();
        r.changed_options.clear();
        let text = r.render();
        assert!(
            text.ends_with(
                "
## Opciones cambiadas

Ninguna.
"
            ),
            "{text}"
        );
        for line in [
            "- Sistema: desconocido\n",
            "- Procesador: desconocido\n",
            "- Memoria: desconocido\n",
            "- Disco: desconocido\n",
            "- Krummi: desconocido\n",
            "- Motores: Voron 7.2.7 (.NET desconocido), LMDB desconocido (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13\n",
            "- Rust: desconocido\n",
        ] {
            assert!(text.contains(line), "{line}");
        }
        r.machine.file_system = Some("ext4".into());
        r.machine.logical_cores = Some(8);
        r.versions.krummi_commit = Some("abcdefabcdef".into());
        r.versions.krummi_dirty = true;
        let text = r.render();
        assert!(text.contains("- Disco: desconocido, ext4\n"));
        assert!(text.contains("- Procesador: desconocido, 8 núcleos lógicos\n"));
        assert!(text.contains("- Krummi: abcdefabcdef (con cambios sin confirmar)\n"));
    }

    // AC-B2 (B5): the three *Núcleos* lines, right after *Procesador*, also when it is `desconocido`, with and without
    // `--solo-krummi-voron`, complete and reduced.
    #[test]
    fn the_cores_line_says_where_the_engines_were_measured() {
        for only in [false, true] {
            for scale in [Scale::Full, Scale::Reduced] {
                let mut r = if only {
                    only_krummi_voron(scale, |_| 1.0)
                } else {
                    run(scale, |_| 1.0)
                };
                for (cores, line) in [
                    (
                        Cores::Performance {
                            performance: 16,
                            total: 32,
                        },
                        "- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)",
                    ),
                    (
                        Cores::Unpinned,
                        "- Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)",
                    ),
                    (Cores::Unknown, "- Núcleos: desconocido"),
                ] {
                    r.cores = cores;
                    r.machine.cpu_model = Some("CPU X".into());
                    r.machine.logical_cores = Some(32);
                    let text = r.render();
                    assert!(
                        text.contains(&format!(
                            "
- Procesador: CPU X, 32 núcleos lógicos
{line}
- Memoria: "
                        )),
                        "{text}"
                    );
                    r.machine.cpu_model = None;
                    r.machine.logical_cores = None;
                    assert!(
                        r.render().contains(&format!(
                            "
- Procesador: desconocido
{line}
- Memoria: "
                        )),
                        "{line}"
                    );
                }
            }
        }
    }

    // AC-B3: a failure to ask for the classes or to limit a process changes only the *Núcleos* line: the same results
    // give the same report otherwise, the same summary line and the same exit code.
    #[test]
    fn a_failure_with_the_cores_changes_only_the_cores_line() {
        for factor in [1.0, 0.5] {
            let pinned = run(Scale::Full, |l| if l == Load::W3 { factor } else { 1.0 });
            let mut failed = pinned.clone();
            failed.cores = Cores::Unknown;
            let (a, b) = (pinned.render(), failed.render());
            let differ: Vec<(&str, &str)> =
                a.lines().zip(b.lines()).filter(|(x, y)| x != y).collect();
            assert_eq!(
                differ,
                [(
                    "- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)",
                    "- Núcleos: desconocido"
                )]
            );
            assert_eq!(a.lines().count(), b.lines().count());
            assert_eq!(pinned.exit_code(), failed.exit_code());
            assert_eq!(pinned.summary_line("x.md"), failed.summary_line("x.md"));
        }
    }

    #[test]
    fn the_default_name_uses_the_local_start_time() {
        let started = FixedOffset::east_opt(3_600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 3, 10, 15, 0)
            .unwrap();
        assert_eq!(default_report_name(&started), "banco-20261103-101500.md");
    }
}
