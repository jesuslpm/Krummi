//! The engines and the 22 loads of the benchmark (decisions K1, K5, K6 and K11 and the workload table of the PRD).

/// The six engines, in the order of K3: the order of the runs, of the report's columns and of the options list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Engine {
    Krummi,
    Voron,
    Lmdb,
    Redb,
    Canopydb,
    Fjall,
}

impl Engine {
    pub const ALL: [Engine; 6] = [
        Engine::Krummi,
        Engine::Voron,
        Engine::Lmdb,
        Engine::Redb,
        Engine::Canopydb,
        Engine::Fjall,
    ];

    /// The name the report and the messages use.
    pub fn name(self) -> &'static str {
        match self {
            Engine::Krummi => "Krummi",
            Engine::Voron => "Voron",
            Engine::Lmdb => "LMDB",
            Engine::Redb => "redb",
            Engine::Canopydb => "canopydb",
            Engine::Fjall => "fjall",
        }
    }

    /// The identifier of the worker protocol.
    pub fn id(self) -> &'static str {
        match self {
            Engine::Krummi => "krummi",
            Engine::Voron => "voron",
            Engine::Lmdb => "lmdb",
            Engine::Redb => "redb",
            Engine::Canopydb => "canopydb",
            Engine::Fjall => "fjall",
        }
    }

    pub fn from_id(id: &str) -> Option<Engine> {
        Engine::ALL.into_iter().find(|e| e.id() == id)
    }

    /// The options the benchmark changes in this engine (K3, *Opciones cambiadas*), each without a final period.
    /// The workers apply exactly these and nothing else.
    pub fn changed_options(self) -> &'static [&'static str] {
        match self {
            Engine::Krummi | Engine::Voron | Engine::Redb => &[],
            Engine::Lmdb => &[
                "tamaño del mapa de 4 GiB",
                "claves duplicadas ordenadas en W19 y W20",
                "claves enteras en W21 y W22",
            ],
            Engine::Canopydb => &[
                "caché de páginas de 1 GiB",
                "confirmación que espera al disco del diario",
            ],
            Engine::Fjall => &[
                "caché de bloques de 1 GiB",
                "llevar el diario al disco (SyncData) en cada confirmación",
            ],
        }
    }
}

/// The 22 loads, in the order of the workload table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Load {
    W1,
    W2,
    W3,
    W4,
    W5,
    W6,
    W7,
    W8,
    W9,
    W10,
    W11,
    W12,
    W13,
    W14,
    W15,
    W16,
    W17,
    W18,
    W19,
    W20,
    W21,
    W22,
}

/// Where the store of a load starts (column *Empieza en*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// A new, empty store.
    Empty,
    /// A fresh copy of the base store (K4).
    Base,
    /// A copy of the store left by the last measured repetition of another load.
    After(Load),
}

impl Load {
    pub const ALL: [Load; 22] = [
        Load::W1,
        Load::W2,
        Load::W3,
        Load::W4,
        Load::W5,
        Load::W6,
        Load::W7,
        Load::W8,
        Load::W9,
        Load::W10,
        Load::W11,
        Load::W12,
        Load::W13,
        Load::W14,
        Load::W15,
        Load::W16,
        Load::W17,
        Load::W18,
        Load::W19,
        Load::W20,
        Load::W21,
        Load::W22,
    ];

    /// The loads of K9, whose store size the report gives.
    pub const SIZED: [Load; 8] = [
        Load::W1,
        Load::W2,
        Load::W3,
        Load::W14,
        Load::W15,
        Load::W17,
        Load::W19,
        Load::W21,
    ];

    /// The number of the load: `1` for W1.
    pub fn number(self) -> u64 {
        self as u64 + 1
    }

    /// The identifier of the worker protocol: `W1`.
    pub fn id(self) -> String {
        format!("W{}", self.number())
    }

    pub fn from_id(id: &str) -> Option<Load> {
        Load::ALL.into_iter().find(|l| l.id() == id)
    }

    /// The name the report uses.
    pub fn name(self) -> &'static str {
        match self {
            Load::W1 => "carga-en-orden",
            Load::W2 => "carga-al-azar",
            Load::W3 => "carga-masiva",
            Load::W4 => "escrituras-sueltas",
            Load::W5 => "escrituras-concurrentes-4",
            Load::W6 => "escrituras-concurrentes-16",
            Load::W7 => "lecturas-al-azar",
            Load::W8 => "lecturas-al-azar-4",
            Load::W9 => "lecturas-al-azar-16",
            Load::W10 => "recorrido",
            Load::W11 => "intervalos",
            Load::W12 => "lecturas-con-escritor",
            Load::W13 => "escritor-con-lectores",
            Load::W14 => "borrado",
            Load::W15 => "reutilizacion",
            Load::W16 => "apertura",
            Load::W17 => "valores-grandes-carga",
            Load::W18 => "valores-grandes-lectura",
            Load::W19 => "multivalor-carga",
            Load::W20 => "multivalor-lectura",
            Load::W21 => "tamano-fijo-carga",
            Load::W22 => "tamano-fijo-lectura",
        }
    }

    pub fn start(self) -> Start {
        match self {
            Load::W1 | Load::W2 | Load::W3 | Load::W17 | Load::W19 | Load::W21 => Start::Empty,
            Load::W18 => Start::After(Load::W17),
            Load::W20 => Start::After(Load::W19),
            Load::W22 => Start::After(Load::W21),
            _ => Start::Base,
        }
    }

    /// The loads that only read (K4): their process opens the store once and measures with warm caches.
    pub fn read_only(self) -> bool {
        matches!(
            self,
            Load::W7
                | Load::W8
                | Load::W9
                | Load::W10
                | Load::W11
                | Load::W18
                | Load::W20
                | Load::W22
        )
    }

    /// The loads of structures that only some engines offer natively (K6).
    pub fn native_only(self) -> bool {
        matches!(self, Load::W19 | Load::W20 | Load::W21 | Load::W22)
    }

    /// Whether a worker process runs this load. W13 is measured in the same process as W12 (K3).
    pub fn has_process(self) -> bool {
        self != Load::W13
    }
}

/// Whether `engine` runs `load` (K6): canopydb and fjall have no multi-value trees or integer keys.
pub fn runs(engine: Engine, load: Load) -> bool {
    !(load.native_only() && matches!(engine, Engine::Canopydb | Engine::Fjall))
}

/// A complete run or the reduced one (K11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Full,
    Reduced,
}

impl Scale {
    pub fn id(self) -> &'static str {
        match self {
            Scale::Full => "full",
            Scale::Reduced => "reduced",
        }
    }

    pub fn from_id(id: &str) -> Option<Scale> {
        match id {
            "full" => Some(Scale::Full),
            "reduced" => Some(Scale::Reduced),
            _ => None,
        }
    }

    /// Repetitions without measuring (the warm-up) and measured ones (K8, K11).
    pub fn repetitions(self) -> (u32, u32) {
        match self {
            Scale::Full => (1, 3),
            Scale::Reduced => (0, 1),
        }
    }

    /// Entries of the base store (K4).
    pub fn base_entries(self) -> u64 {
        match self {
            Scale::Full => 1_000_000,
            Scale::Reduced => 10_000,
        }
    }
}

/// Bytes of every key (K4).
pub const KEY_LEN: usize = 16;
/// Bytes of every value unless a load says otherwise (K4).
pub const VALUE_LEN: usize = 100;
/// Entries per write of the batched loads.
pub const BATCH: u64 = 1_000;

/// The quantities of a load at a scale (columns *Qué hace* and *Reducida*). Fields a load does not use are 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    /// Entries the load writes, removes or reads once each (per thread for W8 and W9).
    pub entries: u64,
    /// Entries per write.
    pub batch: u64,
    /// Threads that write (W5, W6) or read (W8, W9, W12).
    pub threads: u64,
    /// Bytes of every value the load writes.
    pub value_len: usize,
    /// Scans of W11, and their length.
    pub ranges: u64,
    pub range_len: u64,
    /// W12: writes of the writer and reads per read transaction of each reader.
    pub writes: u64,
    pub reads_per_transaction: u64,
    /// W16: opens.
    pub opens: u64,
    /// W19, W20: keys and values per key.
    pub keys: u64,
    pub values_per_key: u64,
}

const NONE: Params = Params {
    entries: 0,
    batch: 0,
    threads: 1,
    value_len: VALUE_LEN,
    ranges: 0,
    range_len: 0,
    writes: 0,
    reads_per_transaction: 0,
    opens: 0,
    keys: 0,
    values_per_key: 0,
};

/// The quantities of `load` at `scale`.
pub fn params(load: Load, scale: Scale) -> Params {
    let full = scale == Scale::Full;
    let pick = |f: u64, r: u64| if full { f } else { r };
    let n = scale.base_entries();
    match load {
        Load::W1 | Load::W2 | Load::W14 | Load::W15 => Params {
            entries: n,
            batch: BATCH,
            ..NONE
        },
        Load::W3 => Params {
            entries: n,
            batch: n,
            ..NONE
        },
        Load::W4 | Load::W5 | Load::W6 => Params {
            entries: pick(10_000, 160),
            batch: 1,
            threads: match load {
                Load::W4 => 1,
                Load::W5 => 4,
                _ => 16,
            },
            ..NONE
        },
        Load::W7 | Load::W8 | Load::W9 | Load::W10 => Params {
            entries: n,
            threads: match load {
                Load::W8 => 4,
                Load::W9 => 16,
                _ => 1,
            },
            ..NONE
        },
        Load::W11 => Params {
            ranges: pick(100_000, 1_000),
            range_len: 100,
            ..NONE
        },
        Load::W12 | Load::W13 => Params {
            threads: 4,
            writes: pick(1_000, 10),
            batch: 100,
            reads_per_transaction: 1_000,
            ..NONE
        },
        Load::W16 => Params {
            opens: pick(100, 10),
            ..NONE
        },
        Load::W17 | Load::W18 => Params {
            entries: pick(4_096, 64),
            batch: 256,
            value_len: 64 * 1024,
            ..NONE
        },
        Load::W19 | Load::W20 => Params {
            keys: pick(100_000, 1_000),
            values_per_key: 10,
            batch: BATCH,
            value_len: 16,
            ..NONE
        },
        Load::W21 | Load::W22 => Params {
            entries: n,
            batch: BATCH,
            value_len: 8,
            ..NONE
        },
    }
}

/// The operations a repetition of `load` counts (column *Operaciones*), or `None` when they depend on the run (W11
/// counts the entries read, W12 the reads finished while the writer runs). The orchestrator checks every worker's
/// count against it (AC-K5).
pub fn expected_operations(load: Load, scale: Scale) -> Option<u64> {
    let p = params(load, scale);
    match load {
        Load::W11 | Load::W12 => None,
        Load::W8 | Load::W9 => Some(p.entries * p.threads),
        Load::W13 => Some(p.writes * p.batch),
        Load::W16 => Some(p.opens),
        Load::W19 | Load::W20 => Some(p.keys * p.values_per_key),
        _ => Some(p.entries),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids_follow_the_workload_table() {
        assert_eq!(Load::ALL.len(), 22);
        assert_eq!(Load::W1.id(), "W1");
        assert_eq!(Load::W22.id(), "W22");
        assert_eq!(Load::from_id("W13"), Some(Load::W13));
        assert_eq!(Load::W13.name(), "escritor-con-lectores");
        assert_eq!(Load::W21.name(), "tamano-fijo-carga");
        assert_eq!(Engine::from_id("canopydb"), Some(Engine::Canopydb));
        let names: Vec<_> = Engine::ALL.iter().map(|e| e.name()).collect();
        assert_eq!(
            names,
            ["Krummi", "Voron", "LMDB", "redb", "canopydb", "fjall"]
        );
    }

    #[test]
    fn only_canopydb_and_fjall_skip_the_native_loads() {
        for load in Load::ALL {
            for engine in Engine::ALL {
                let skipped = !runs(engine, load);
                let expected =
                    load.native_only() && matches!(engine, Engine::Canopydb | Engine::Fjall);
                assert_eq!(skipped, expected, "{engine:?} {load:?}");
            }
        }
        let natives: Vec<_> = Load::ALL.into_iter().filter(|l| l.native_only()).collect();
        assert_eq!(natives, [Load::W19, Load::W20, Load::W21, Load::W22]);
    }

    #[test]
    fn full_operations_are_those_of_the_table() {
        let ops: Vec<_> = Load::ALL
            .iter()
            .map(|&l| expected_operations(l, Scale::Full))
            .collect();
        let m = Some(1_000_000);
        assert_eq!(
            ops,
            [
                m,
                m,
                m,
                Some(10_000),
                Some(10_000),
                Some(10_000),
                m,
                Some(4_000_000),
                Some(16_000_000),
                m,
                None,
                None,
                Some(100_000),
                m,
                m,
                Some(100),
                Some(4_096),
                Some(4_096),
                m,
                m,
                m,
                m
            ]
        );
    }

    #[test]
    fn reduced_quantities_are_those_of_the_reducida_column() {
        let r = |l| params(l, Scale::Reduced);
        let ops = |l| expected_operations(l, Scale::Reduced);
        assert_eq!(Scale::Reduced.base_entries(), 10_000);
        for l in [
            Load::W1,
            Load::W2,
            Load::W3,
            Load::W7,
            Load::W10,
            Load::W14,
            Load::W21,
            Load::W22,
        ] {
            assert_eq!(ops(l), Some(10_000), "{l:?}");
        }
        assert_eq!((r(Load::W4).entries, r(Load::W4).threads), (160, 1));
        assert_eq!((r(Load::W5).entries / r(Load::W5).threads), 40);
        assert_eq!((r(Load::W6).entries / r(Load::W6).threads), 10);
        assert_eq!(ops(Load::W8), Some(40_000));
        assert_eq!(ops(Load::W9), Some(160_000));
        assert_eq!(r(Load::W11).ranges, 1_000);
        assert_eq!((r(Load::W12).writes, r(Load::W12).batch), (10, 100));
        assert_eq!(ops(Load::W13), Some(1_000));
        assert_eq!(ops(Load::W16), Some(10));
        assert_eq!(ops(Load::W17), Some(64));
        assert_eq!(ops(Load::W18), Some(64));
        assert_eq!((r(Load::W19).keys, ops(Load::W19)), (1_000, Some(10_000)));
        // Sizes of keys, values and writes and thread counts do not change (K11).
        for l in Load::ALL {
            let (f, red) = (params(l, Scale::Full), r(l));
            assert_eq!(
                (f.threads, f.value_len, f.range_len),
                (red.threads, red.value_len, red.range_len)
            );
            if !matches!(l, Load::W3) {
                assert_eq!(f.batch, red.batch, "{l:?}");
            }
        }
        assert_eq!(Scale::Reduced.repetitions(), (0, 1));
        assert_eq!(Scale::Full.repetitions(), (1, 3));
    }
}
