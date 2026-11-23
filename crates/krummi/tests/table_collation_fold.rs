//! AC-IC2 and AC-IC12 of the collation PRD (7.7): the folded forms of `ci` and `ci_ai` against a reference built in
//! this test from the vendored Unicode 17.0 data files, and the library's dependencies.
//!
//! The reference does not use the library's generated tables: it parses `UnicodeData.txt`, `CaseFolding.txt`,
//! `Scripts.txt` and `CompositionExclusions.txt` into plain maps and applies the definitions of IC2 literally (naive
//! recursive decomposition, bubble-sort canonical ordering, and the canonical composition algorithm of Unicode D117).

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use krummi::table::collation::{Collation, fold};

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("unicode/17.0.0")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(data_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn hex(s: &str) -> u32 {
    u32::from_str_radix(s.trim(), 16).unwrap_or_else(|e| panic!("{s:?}: {e}"))
}

fn ch(s: &str) -> char {
    char::from_u32(hex(s)).unwrap()
}

/// A cheap hasher for the reference's maps, so the debug build checks every scalar in seconds.
#[derive(Default)]
struct ScalarHasher(u64);

impl Hasher for ScalarHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        bytes.iter().for_each(|&b| self.write_u32(u32::from(b)));
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(n)).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
}

type Map<K, V> = HashMap<K, V, BuildHasherDefault<ScalarHasher>>;

const ASSIGNED: u8 = 1;
const MN: u8 = 2;
/// Script Latin, Greek or Cyrillic.
const LGC: u8 = 4;
/// The second scalar of a primary composite's pair.
const SECOND: u8 = 8;

/// The Unicode 17.0 data the reference needs, parsed from the vendored files.
struct Ucd {
    /// Canonical combining class, by scalar value.
    ccc: Vec<u8>,
    /// `ASSIGNED`, `MN`, `LGC` and `SECOND` bits, by scalar value.
    flags: Vec<u8>,
    /// Canonical decompositions, one level, as `UnicodeData.txt` gives them.
    decomposition: Map<char, Vec<char>>,
    /// Primary composites by their pair.
    composition: Map<(char, char), char>,
    /// Case folding, statuses C and F.
    folding: Map<char, Vec<char>>,
}

impl Ucd {
    fn load() -> Ucd {
        let mut ccc = vec![0u8; 0x110000];
        let mut flags = vec![0u8; 0x110000];
        let mut decomposition = Map::default();
        let mut first = None;
        for line in read("UnicodeData.txt").lines() {
            let f: Vec<&str> = line.split(';').collect();
            let code = hex(f[0]);
            if f[1].ends_with(", First>") {
                first = Some(code);
                continue;
            }
            if f[1].ends_with(", Last>") {
                assert_ne!(f[2], "Mn");
                for c in first.take().unwrap()..=code {
                    flags[c as usize] |= ASSIGNED;
                }
                continue;
            }
            flags[code as usize] |= ASSIGNED;
            ccc[code as usize] = f[3].parse().unwrap();
            if f[2] == "Mn" {
                flags[code as usize] |= MN;
            }
            if !f[5].is_empty() && !f[5].starts_with('<') {
                let c = char::from_u32(code).unwrap();
                decomposition.insert(c, f[5].split(' ').map(ch).collect::<Vec<_>>());
            }
        }
        let excluded: HashSet<char> = read("CompositionExclusions.txt")
            .lines()
            .map(|l| l.split('#').next().unwrap().trim())
            .filter(|l| !l.is_empty())
            .map(ch)
            .collect();
        let class = |c: char| ccc[c as usize];
        let mut composition = Map::default();
        for (&c, d) in &decomposition {
            // Full composition exclusion: the listed exclusions, singletons, and non-starter decompositions.
            if d.len() == 2 && !excluded.contains(&c) && class(c) == 0 && class(d[0]) == 0 {
                composition.insert((d[0], d[1]), c);
                flags[d[1] as usize] |= SECOND;
            }
        }
        for jamo in (V_BASE..V_BASE + V_COUNT).chain(T_BASE + 1..T_BASE + T_COUNT) {
            flags[jamo as usize] |= SECOND;
        }
        let mut folding = Map::default();
        for line in read("CaseFolding.txt").lines() {
            let line = line.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split(';').map(str::trim).collect();
            if f[1] == "C" || f[1] == "F" {
                folding.insert(ch(f[0]), f[2].split(' ').map(ch).collect::<Vec<_>>());
            }
        }
        for line in read("Scripts.txt").lines() {
            let line = line.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let (range, script) = line.split_once(';').unwrap();
            if !matches!(script.trim(), "Latin" | "Greek" | "Cyrillic") {
                continue;
            }
            let (lo, hi) = match range.trim().split_once("..") {
                Some((lo, hi)) => (hex(lo), hex(hi)),
                None => (hex(range), hex(range)),
            };
            for c in lo..=hi {
                flags[c as usize] |= LGC;
            }
        }
        Ucd {
            ccc,
            flags,
            decomposition,
            composition,
            folding,
        }
    }

    fn class(&self, c: char) -> u8 {
        self.ccc[c as usize]
    }

    fn has(&self, c: char, flag: u8) -> bool {
        self.flags[c as usize] & flag != 0
    }

    fn assigned(&self, c: char) -> bool {
        self.has(c, ASSIGNED)
    }

    /// Whether the definition provably leaves `c` alone in any text: it has no decomposition and no case folding, is
    /// a starter, is not Mn, and is not the second scalar of a composition, so a text of such scalars is its own NFD,
    /// case folding and NFC, and has no mark to remove.
    fn inert(&self, c: char) -> bool {
        !(S_BASE..S_BASE + S_COUNT).contains(&(c as u32))
            && self.class(c) == 0
            && !self.has(c, MN | SECOND)
            && !self.decomposition.contains_key(&c)
            && !self.folding.contains_key(&c)
    }

    fn decompose(&self, c: char, out: &mut Vec<char>) {
        let s = c as u32;
        if (S_BASE..S_BASE + S_COUNT).contains(&s) {
            let i = s - S_BASE;
            out.push(char::from_u32(L_BASE + i / N_COUNT).unwrap());
            out.push(char::from_u32(V_BASE + (i % N_COUNT) / T_COUNT).unwrap());
            if !i.is_multiple_of(T_COUNT) {
                out.push(char::from_u32(T_BASE + i % T_COUNT).unwrap());
            }
        } else if let Some(d) = self.decomposition.get(&c) {
            for &x in d {
                self.decompose(x, out);
            }
        } else {
            out.push(c);
        }
    }

    fn nfd(&self, s: &[char]) -> Vec<char> {
        let mut out = Vec::new();
        for &c in s {
            self.decompose(c, &mut out);
        }
        // Canonical ordering: bubble adjacent non-starters until no pair is out of order.
        loop {
            let mut swapped = false;
            for i in 1..out.len() {
                let (a, b) = (self.class(out[i - 1]), self.class(out[i]));
                if b != 0 && a > b {
                    out.swap(i - 1, i);
                    swapped = true;
                }
            }
            if !swapped {
                return out;
            }
        }
    }

    fn primary(&self, l: char, c: char) -> Option<char> {
        let (l32, c32) = (l as u32, c as u32);
        if (L_BASE..L_BASE + L_COUNT).contains(&l32) && (V_BASE..V_BASE + V_COUNT).contains(&c32) {
            return char::from_u32(S_BASE + ((l32 - L_BASE) * V_COUNT + (c32 - V_BASE)) * T_COUNT);
        }
        if (S_BASE..S_BASE + S_COUNT).contains(&l32)
            && (l32 - S_BASE).is_multiple_of(T_COUNT)
            && (T_BASE + 1..T_BASE + T_COUNT).contains(&c32)
        {
            return char::from_u32(l32 + (c32 - T_BASE));
        }
        self.composition.get(&(l, c)).copied()
    }

    fn nfc(&self, s: &[char]) -> Vec<char> {
        let mut d = self.nfd(s);
        let mut i = 1;
        while i < d.len() {
            let c = d[i];
            let cc = self.class(c);
            if let Some(l) = (0..i).rev().find(|&j| self.class(d[j]) == 0) {
                let blocked = (l + 1..i).any(|j| {
                    let b = self.class(d[j]);
                    b == 0 || b >= cc
                });
                if !blocked && let Some(p) = self.primary(d[l], c) {
                    d[l] = p;
                    d.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        d
    }

    /// `ci`: NFC(NFD(case fold(NFD(text)))).
    fn ci(&self, s: &[char]) -> Vec<char> {
        let folded: Vec<char> = self
            .nfd(s)
            .into_iter()
            .flat_map(|c| self.folding.get(&c).cloned().unwrap_or_else(|| vec![c]))
            .collect();
        self.nfc(&self.nfd(&folded))
    }

    /// `ci_ai`: the `ci` form, decomposed, without the Mn marks whose base is Latin, Greek or Cyrillic (keeping a
    /// U+0303 right after an `n`), composed.
    fn ci_ai(&self, s: &[char]) -> Vec<char> {
        let y = self.nfd(&self.ci(s));
        let mut out = Vec::new();
        for (i, &c) in y.iter().enumerate() {
            if self.has(c, MN) {
                let base = y[..i].iter().rev().find(|&&x| !self.has(x, MN));
                let tilde_of_n = c == '\u{303}' && i > 0 && y[i - 1] == 'n';
                if base.is_some_and(|&b| self.has(b, LGC)) && !tilde_of_n {
                    continue;
                }
            }
            out.push(c);
        }
        self.nfc(&out)
    }

    fn reference(&self, collation: Collation, text: &str) -> String {
        if text.chars().all(|c| self.inert(c)) {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        match collation {
            Collation::Binary => text.to_string(),
            Collation::Ci => self.ci(&chars).into_iter().collect(),
            Collation::CiAi => self.ci_ai(&chars).into_iter().collect(),
        }
    }
}

fn ucd() -> &'static Ucd {
    static UCD: OnceLock<Ucd> = OnceLock::new();
    UCD.get_or_init(Ucd::load)
}

const FOLDING: [Collation; 2] = [Collation::Ci, Collation::CiAi];

fn escaped(s: &str) -> String {
    s.chars().map(|c| format!("{:04X} ", c as u32)).collect()
}

/// Checks one text: the fold equals the reference, is idempotent, and is borrowed exactly when it is the text.
/// Returns a description of the first difference.
fn check(collation: Collation, text: &str) -> Result<String, String> {
    let got = fold(collation, text);
    let want = ucd().reference(collation, text);
    if got != want {
        return Err(format!(
            "{collation} of [{}]: got [{}], want [{}]",
            escaped(text),
            escaped(&got),
            escaped(&want)
        ));
    }
    if matches!(got, Cow::Owned(_)) && got == text {
        return Err(format!(
            "{collation} of [{}] is unchanged but owned",
            escaped(text)
        ));
    }
    let again = fold(collation, &got);
    if again != got {
        return Err(format!(
            "{collation} is not idempotent on [{}]: [{}] then [{}]",
            escaped(text),
            escaped(&got),
            escaped(&again)
        ));
    }
    if !matches!(again, Cow::Borrowed(_)) {
        return Err(format!(
            "{collation} of the folded [{}] is not borrowed",
            escaped(&got)
        ));
    }
    Ok(got.into_owned())
}

fn assert_no_failures(failures: Vec<String>) {
    assert!(
        failures.is_empty(),
        "{} failures, the first ones:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// AC-IC2 (1): every scalar value folds to the definition's form, folding again gives the same form, and an
/// unassigned scalar stays as it is.
#[test]
fn every_scalar_value_folds_to_the_definition() {
    let ucd = ucd();
    let mut failures = Vec::new();
    let mut unassigned = 0;
    for c in (0..=0x10FFFF).filter_map(char::from_u32) {
        let text = c.to_string();
        let assigned = ucd.assigned(c);
        unassigned += usize::from(!assigned);
        for collation in FOLDING {
            match check(collation, &text) {
                Ok(folded) if !assigned && folded != text => {
                    failures.push(format!(
                        "{collation} changes the unassigned U+{:04X}",
                        c as u32
                    ));
                }
                Ok(_) => {}
                Err(e) => failures.push(e),
            }
        }
    }
    assert!(unassigned > 800_000, "{unassigned}");
    assert_no_failures(failures);
}

/// AC-IC2 (2): the three canonically equivalent columns of every line of `NormalizationTest.txt` (c1, c2, c3) give
/// the same folded form, the definition's.
#[test]
fn canonically_equivalent_texts_fold_alike() {
    let mut failures = Vec::new();
    let mut lines = 0;
    for line in read("NormalizationTest.txt").lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        lines += 1;
        let columns: Vec<String> = line
            .split(';')
            .take(3)
            .map(|f| f.split(' ').map(ch).collect())
            .collect();
        for collation in FOLDING {
            let forms: Vec<Result<String, String>> =
                columns.iter().map(|t| check(collation, t)).collect();
            match &forms[..] {
                [Ok(a), Ok(b), Ok(c)] if a == b && b == c => {}
                _ => failures.push(format!("{collation} on line {line}: {forms:?}")),
            }
        }
    }
    assert!(lines > 19_000, "{lines}");
    assert_no_failures(failures);
}

/// A small linear congruential generator, so the property test is reproducible.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

/// The pieces of the property test: Latin, Greek and Cyrillic letters, letters of other scripts, marks of several
/// classes and scripts, joiners, and the PRD's cases.
const PIECES: &[&str] = &[
    // Latin.
    "a",
    "A",
    "e",
    "E",
    "i",
    "I",
    "n",
    "N",
    "o",
    "s",
    "S",
    "k",
    "\u{DF}",
    "\u{1E9E}",
    "\u{130}",
    "\u{131}",
    "\u{149}",
    "\u{1E9A}",
    "\u{C5}",
    "\u{E5}",
    "\u{212B}",
    "\u{212A}",
    "\u{C6}",
    "\u{F8}",
    "\u{142}",
    "\u{1C5}",
    "\u{FB01}",
    "\u{F1}",
    "\u{D1}",
    "\u{E9}",
    "\u{1EC7}",
    "\u{1E69}",
    // Greek.
    "\u{3B1}",
    "\u{391}",
    "\u{3C3}",
    "\u{3C2}",
    "\u{3A3}",
    "\u{390}",
    "\u{1FB3}",
    "\u{1FBC}",
    "\u{B5}",
    "\u{3D0}",
    "\u{3AC}",
    "\u{1F80}",
    "\u{3B9}",
    // Cyrillic.
    "\u{401}",
    "\u{451}",
    "\u{419}",
    "\u{439}",
    "\u{416}",
    "\u{436}",
    "\u{4D0}",
    // Other scripts: Devanagari, Hiragana, Hebrew, Hangul jamo and syllables.
    "\u{915}",
    "\u{941}",
    "\u{93F}",
    "\u{929}",
    "\u{304B}",
    "\u{304C}",
    "\u{3099}",
    "\u{5D0}",
    "\u{5BC}",
    "\u{5B8}",
    "\u{FB2F}",
    "\u{1100}",
    "\u{1161}",
    "\u{11A8}",
    "\u{AC00}",
    // Marks.
    "\u{300}",
    "\u{301}",
    "\u{302}",
    "\u{303}",
    "\u{307}",
    "\u{308}",
    "\u{30A}",
    "\u{323}",
    "\u{327}",
    "\u{331}",
    "\u{342}",
    "\u{345}",
    "\u{344}",
    "\u{20DD}",
    "\u{1AB0}",
    // Joiners and a space.
    "\u{200D}",
    "\u{200C}",
    " ",
    // The PRD's cases.
    "\u{149}\u{303}",
    "\u{1E9A}\u{301}",
    "\u{B5}\u{301}",
    "a\u{200D}\u{301}",
    "\u{F1}\u{323}",
    "n\u{301}\u{303}",
];

/// AC-IC2 (3): random sequences fold to the definition's form, folding again gives the same form, and texts
/// equivalent in `ci` are equivalent in `ci_ai`.
#[test]
fn random_sequences_fold_to_the_definition() {
    let ucd = ucd();
    let mut rng = Lcg(0x1C2_7A7E);
    let mut failures = Vec::new();
    let mut equivalent_pairs = 0;
    for _ in 0..6_000 {
        let len = 1 + rng.below(10);
        let text: String = (0..len).map(|_| PIECES[rng.below(PIECES.len())]).collect();
        let chars: Vec<char> = text.chars().collect();
        // Variants of the text: some are equivalent to it in `ci`, some are not.
        let variants = [
            text.to_uppercase(),
            text.to_lowercase(),
            ucd.nfd(&chars).into_iter().collect::<String>(),
            ucd.nfc(&chars).into_iter().collect::<String>(),
        ];
        for t in std::iter::once(&text).chain(&variants) {
            for collation in FOLDING {
                if let Err(e) = check(collation, t) {
                    failures.push(e);
                }
            }
        }
        for t in &variants {
            if fold(Collation::Ci, &text) == fold(Collation::Ci, t) {
                equivalent_pairs += 1;
                if fold(Collation::CiAi, &text) != fold(Collation::CiAi, t) {
                    failures.push(format!(
                        "[{}] and [{}] are equivalent in ci but not in ci_ai",
                        escaped(&text),
                        escaped(t)
                    ));
                }
            }
        }
    }
    assert!(equivalent_pairs >= 12_000, "{equivalent_pairs}");
    assert_no_failures(failures);
}

/// The examples of *Salida* in the PRD, row by row: the text, its `ci` form and its `ci_ai` form.
#[test]
fn the_examples_of_the_prd_fold_as_its_table_says() {
    let rows = [
        ("José", "josé", "jose"),
        ("JOSÉ", "josé", "jose"),
        ("Jose\u{301}", "josé", "jose"),
        ("Straße", "strasse", "strasse"),
        ("ΣΑΣ", "σασ", "σασ"),
        ("ᾳ", "αι", "αι"),
        ("µ\u{301}", "μ\u{301}", "μ"),
        ("ﬁn", "fin", "fin"),
        ("İstanbul", "i\u{307}stanbul", "istanbul"),
        ("Istanbul", "istanbul", "istanbul"),
        ("ılık", "ılık", "ılık"),
        ("Ærø", "ærø", "ærø"),
        ("Año", "año", "año"),
        ("Ångström", "ångström", "angstrom"),
        ("Ёлка", "ёлка", "елка"),
        ("Йод", "йод", "иод"),
        ("ña\u{323}", "ñạ", "ña"),
        ("n\u{301}\u{303}", "ń\u{303}", "n"),
        ("\u{301}", "\u{301}", "\u{301}"),
        ("が", "が", "が"),
        ("कु", "कु", "कु"),
    ];
    for (text, ci, ci_ai) in rows {
        assert_eq!(fold(Collation::Ci, text), ci, "ci of {text}");
        assert_eq!(fold(Collation::CiAi, text), ci_ai, "ci_ai of {text}");
        assert_eq!(fold(Collation::Binary, text), text, "binary of {text}");
    }
    // The PRD's equivalences: `año` and `ano` differ, `Año` and `AÑO` do not.
    assert_ne!(fold(Collation::CiAi, "año"), fold(Collation::CiAi, "ano"));
    assert_eq!(fold(Collation::CiAi, "Año"), fold(Collation::CiAi, "AÑO"));
}

/// AC-IC12 (IC11): the library's dependencies, without the tests', are the same as before the collation.
#[test]
fn the_library_gains_no_dependency() {
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let mut section = "";
    let mut dependencies = Vec::new();
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line;
            continue;
        }
        if section == "[dependencies]"
            && let Some((name, _)) = line.split_once('=')
            && !line.starts_with('#')
        {
            dependencies.push(name.trim().to_string());
        }
    }
    assert_eq!(dependencies, ["krummi-pager", "krummi-tree", "krummi-tx"]);
}
