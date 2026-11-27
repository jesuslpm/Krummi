//! The resident memory of the process (S6, "RSS"), read from `/proc/self/status` on Linux. Windows has no safe
//! standard API for it and the workspace forbids unsafe code, so there it is reported as not measured.

use std::fmt::Write;

/// The resident set and its peak, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resident {
    pub rss: u64,
    pub peak: u64,
}

/// The process's resident memory now, or `None` where it cannot be read.
pub fn resident() -> Option<Resident> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_status(&status)
}

/// `VmRSS` and `VmHWM` of a `/proc/<pid>/status` text.
fn parse_status(status: &str) -> Option<Resident> {
    let field = |name: &str| -> Option<u64> {
        let line = status.lines().find_map(|l| l.strip_prefix(name))?;
        let kib: u64 = line.trim().strip_suffix("kB")?.trim().parse().ok()?;
        Some(kib * 1024)
    };
    Some(Resident {
        rss: field("VmRSS:")?,
        peak: field("VmHWM:")?,
    })
}

/// How the resident memory grew from `before` while a structure was built, for the report.
pub fn growth(before: Option<Resident>) -> String {
    growth_between(before, resident())
}

fn growth_between(before: Option<Resident>, after: Option<Resident>) -> String {
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    match (before, after) {
        (Some(before), Some(after)) => {
            let mut text = String::new();
            let _ = write!(
                text,
                "el RSS creció {:.1} MiB al construirlo (de {:.1} a {:.1} MiB, pico del proceso {:.1} MiB; \
                 incluye los temporales que el asignador no devolvió)",
                mib(after.rss.saturating_sub(before.rss)),
                mib(before.rss),
                mib(after.rss),
                mib(after.peak)
            );
            text
        }
        _ => "RSS no medido en esta plataforma".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_fields_are_read_in_bytes() {
        let status = "Name:\tkrummi-ann-lab\nVmHWM:\t    2048 kB\nVmRSS:\t    1024 kB\n";
        assert_eq!(
            parse_status(status),
            Some(Resident {
                rss: 1024 * 1024,
                peak: 2048 * 1024
            })
        );
        assert_eq!(parse_status("Name:\tx\n"), None);
    }

    #[test]
    fn growth_states_the_growth_or_that_it_was_not_measured() {
        let mib = 1024 * 1024;
        let before = Resident {
            rss: 10 * mib,
            peak: 10 * mib,
        };
        let after = Resident {
            rss: 15 * mib,
            peak: 20 * mib,
        };
        assert_eq!(
            growth_between(Some(before), Some(after)),
            "el RSS creció 5.0 MiB al construirlo (de 10.0 a 15.0 MiB, pico del proceso 20.0 MiB; incluye los \
             temporales que el asignador no devolvió)"
        );
        assert_eq!(
            growth_between(None, Some(after)),
            "RSS no medido en esta plataforma"
        );
    }
}
