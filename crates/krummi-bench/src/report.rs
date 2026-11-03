//! The Markdown report and the summary line (PRD, *Salida*; K7, K13).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use chrono::{DateTime, FixedOffset};

use crate::measure::{meets, ratio_text};
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
    pub machine: Machine,
    pub versions: Versions,
    /// The metric of every load and engine that ran it (K8).
    pub metrics: BTreeMap<(Load, Engine), f64>,
    /// The size of the store after the loads of K9.
    pub sizes: BTreeMap<(Load, Engine), u64>,
    /// The options the benchmark changed, per engine with any, in the order of K3 (*Opciones cambiadas*).
    pub changed_options: Vec<(Engine, Vec<String>)>,
}

/// The options every engine runs with (`Engine::changed_options`), for `Run::changed_options`.
pub fn changed_options() -> Vec<(Engine, Vec<String>)> {
    Engine::ALL
        .into_iter()
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
        let (execution, repetitions) = match self.scale {
            Scale::Full => ("completa", "1 previa y 3 medidas, mediana"),
            Scale::Reduced => ("reducida", "1 medida"),
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
        let _ = writeln!(out, "- Memoria: {memory}");
        let _ = writeln!(out, "- Disco: {disk}");
        let _ = writeln!(out, "- Krummi: {krummi}");
        let _ = writeln!(
            out,
            "- Motores: Voron 7.2.7 (.NET {}), LMDB {} (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13",
            or_unknown(&v.dotnet),
            or_unknown(&v.lmdb)
        );
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

        let engines: Vec<&str> = Engine::ALL.iter().map(|e| e.name()).collect();
        let _ = writeln!(out, "\n## Operaciones por segundo\n");
        let _ = writeln!(
            out,
            "| Carga | {} | Krummi / Voron | Umbral |",
            engines.join(" | ")
        );
        let _ = writeln!(out, "|---|{}---|---|", "---|".repeat(engines.len()));
        for load in Load::ALL {
            let cells: Vec<String> = Engine::ALL
                .iter()
                .map(|&e| {
                    if runs(e, load) {
                        format!("{}", self.metric(load, e).round() as u64)
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
            let cells: Vec<String> = Engine::ALL
                .iter()
                .map(|&e| {
                    if runs(e, load) {
                        let size = self
                            .sizes
                            .get(&(load, e))
                            .unwrap_or_else(|| panic!("no size for {e:?} in {load:?}"));
                        size.to_string()
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
            changed_options: changed_options(),
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
        let expected_start = "Krummi cumple 22 de 22 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 1020 | 1000 | 1002 | 1003 | 1004 | 1005 | 1,02 | cumple |
";
        assert!(rest.starts_with(expected_start), "{rest}");
        assert_eq!(
            row(&text, Load::W21),
            "| tamano-fijo-carga | 21420 | 21000 | 21002 | 21003 | no aplica | no aplica | 1,02 | cumple |"
        );
        let tail = "
## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 1000 | 1001 | 1002 | 1003 | 1004 | 1005 |
| carga-al-azar | 2000 | 2001 | 2002 | 2003 | 2004 | 2005 |
| carga-masiva | 3000 | 3001 | 3002 | 3003 | 3004 | 3005 |
| borrado | 14000 | 14001 | 14002 | 14003 | 14004 | 14005 |
| reutilizacion | 15000 | 15001 | 15002 | 15003 | 15004 | 15005 |
| valores-grandes-carga | 17000 | 17001 | 17002 | 17003 | 17004 | 17005 |
| multivalor-carga | 19000 | 19001 | 19002 | 19003 | no aplica | no aplica |
| tamano-fijo-carga | 21000 | 21001 | 21002 | 21003 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 4 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: caché de páginas de 1 GiB; confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
";
        assert!(text.ends_with(tail), "{text}");
        assert_eq!(
            text.lines().filter(|l| l.starts_with("| ")).count(),
            1 + 22 + 1 + 8
        );
        assert_eq!(r.exit_code(), 0);
        assert_eq!(
            r.summary_line("banco-20261103-101500.md"),
            "Krummi cumple 22 de 22 umbrales. Informe: banco-20261103-101500.md"
        );
    }

    // AC-K2 and AC-K3: one missed threshold, then several; 0.7996 shows 0,79 and misses; exactly 0.80 meets.
    #[test]
    fn missed_thresholds_are_listed_in_table_order() {
        let one = run(Scale::Full, |l| if l == Load::W6 { 0.7996 } else { 0.8 });
        let text = one.render();
        assert!(text.contains(
            "\n## Resultado\n\nKrummi cumple 21 de 22 umbrales. Incumple: `escrituras-concurrentes-16`.\n"
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
            "Krummi cumple 21 de 22 umbrales. Informe: x.md"
        );

        let several = run(Scale::Full, |l| match l {
            Load::W22 | Load::W1 | Load::W13 => 0.5,
            _ => 1.0,
        });
        assert!(several.render().contains(
            "Krummi cumple 19 de 22 umbrales. Incumple: `carga-en-orden`, `escritor-con-lectores`, `tamano-fijo-lectura`.\n"
        ));
        assert_eq!(several.missed(), [Load::W1, Load::W13, Load::W22]);
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

    #[test]
    fn the_default_name_uses_the_local_start_time() {
        let started = FixedOffset::east_opt(3_600)
            .unwrap()
            .with_ymd_and_hms(2026, 11, 3, 10, 15, 0)
            .unwrap();
        assert_eq!(default_report_name(&started), "banco-20261103-101500.md");
    }
}
