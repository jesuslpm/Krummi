//! The `krummi-ann-lab` command: prepares the data sets, computes the exact ground truth and measures the
//! approximate searches of design document 12. Its messages are in Spanish (repository rule).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use krummi_ann_lab::kernels::Metric;
use krummi_ann_lab::run;

const USAGE: &str = "Uso: krummi-ann-lab <preparar|verdad|medir|grafo> --carpeta <carpeta> \
[--conjunto <sift|glove|cohere>] [--filas <n>] [--informe <archivo>] [--metrica <l2|dot|coseno>] [--sondeos <n,n,...>]";

fn parse(args: &[OsString]) -> Result<(String, run::Options), String> {
    let mut args = args.iter().map(|a| a.to_string_lossy().into_owned());
    let command = args.next().ok_or_else(|| USAGE.to_string())?;
    let mut options = run::Options::default();
    let mut folder = None;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| format!("Falta el valor de {arg}."))
        };
        match arg.as_str() {
            "--carpeta" => folder = Some(PathBuf::from(value()?)),
            "--conjunto" => options.set = Some(value()?.parse()?),
            "--filas" => {
                let text = value()?;
                options.rows = Some(
                    text.parse()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or_else(|| format!("Número de filas no válido: {text}."))?,
                );
            }
            "--informe" => options.report = Some(PathBuf::from(value()?)),
            "--metrica" => {
                let text = value()?;
                options.metric = Some(
                    [Metric::L2Squared, Metric::Dot, Metric::Cosine]
                        .into_iter()
                        .find(|m| m.name() == text)
                        .ok_or_else(|| {
                            format!("Métrica no válida: {text}. Son l2, dot y coseno.")
                        })?,
                );
            }
            "--sondeos" => {
                let text = value()?;
                let nprobes = text
                    .split(',')
                    .map(|n| n.parse::<usize>().ok().filter(|&n| n > 0))
                    .collect::<Option<Vec<usize>>>()
                    .ok_or_else(|| format!("Lista de sondeos no válida: {text}."))?;
                options.nprobes = Some(nprobes);
            }
            _ => return Err(format!("Opción no válida: {arg}.\n{USAGE}")),
        }
    }
    options.folder = folder.ok_or_else(|| format!("Falta --carpeta.\n{USAGE}"))?;
    // An option that the command would ignore is an error, so no run reports success without it.
    if options.nprobes.is_some() && command != "medir" {
        return Err("La opción --sondeos solo vale con medir.".to_string());
    }
    if options.metric.is_some() && command != "medir" && command != "grafo" {
        return Err("La opción --metrica solo vale con medir y grafo.".to_string());
    }
    if options.rows.is_some() && command == "preparar" {
        return Err("La opción --filas no vale con preparar.".to_string());
    }
    if options.report.is_some() && command != "medir" && command != "grafo" {
        return Err("La opción --informe solo vale con medir y grafo.".to_string());
    }
    Ok((command, options))
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let (command, options) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let result = match command.as_str() {
        "preparar" => run::prepare(&options),
        "verdad" => run::ground_truth(&options),
        "medir" => run::measure(&options),
        "grafo" => run::graph(&options),
        _ => {
            eprintln!("Orden no válida: {command}.\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<(String, run::Options), String> {
        parse(&args.iter().map(OsString::from).collect::<Vec<_>>())
    }

    #[test]
    fn sondeos_metrica_and_filas_take_valid_values_only() {
        let (_, options) = parsed(&[
            "medir",
            "--carpeta",
            "w",
            "--sondeos",
            "4,16",
            "--metrica",
            "dot",
        ])
        .unwrap();
        assert_eq!(options.nprobes, Some(vec![4, 16]));
        assert_eq!(options.metric, Some(Metric::Dot));
        for list in ["0", "4,,8", "a", "4,-1"] {
            assert_eq!(
                parsed(&["medir", "--carpeta", "w", "--sondeos", list]).unwrap_err(),
                format!("Lista de sondeos no válida: {list}.")
            );
        }
        assert_eq!(
            parsed(&["medir", "--carpeta", "w", "--metrica", "ip"]).unwrap_err(),
            "Métrica no válida: ip. Son l2, dot y coseno."
        );
        assert_eq!(
            parsed(&["medir", "--carpeta", "w", "--filas", "0"]).unwrap_err(),
            "Número de filas no válido: 0."
        );
    }

    #[test]
    fn options_a_command_would_ignore_are_rejected() {
        for command in ["preparar", "verdad", "grafo"] {
            assert_eq!(
                parsed(&[command, "--carpeta", "w", "--sondeos", "4"]).unwrap_err(),
                "La opción --sondeos solo vale con medir."
            );
        }
        for command in ["preparar", "verdad"] {
            assert_eq!(
                parsed(&[command, "--carpeta", "w", "--metrica", "l2"]).unwrap_err(),
                "La opción --metrica solo vale con medir y grafo."
            );
        }
        assert!(parsed(&["grafo", "--carpeta", "w", "--metrica", "l2"]).is_ok());
        assert_eq!(
            parsed(&["preparar", "--carpeta", "w", "--filas", "2"]).unwrap_err(),
            "La opción --filas no vale con preparar."
        );
        assert!(parsed(&["verdad", "--carpeta", "w", "--filas", "2"]).is_ok());
        for command in ["preparar", "verdad"] {
            assert_eq!(
                parsed(&[command, "--carpeta", "w", "--informe", "r.md"]).unwrap_err(),
                "La opción --informe solo vale con medir y grafo."
            );
        }
        assert!(parsed(&["grafo", "--carpeta", "w", "--informe", "r.md"]).is_ok());
    }
}
