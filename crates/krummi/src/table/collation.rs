//! The collation of a `string` column (IC1 and IC2 of the collation PRD, 7.7): when two texts are the same value.
//!
//! A column's collation decides its **folded form**: the text its keys, unique checks, ranges and statistics compare
//! byte by byte. `binary` keeps the text as it is; `ci` folds case; `ci_ai` also drops the accents of Latin, Greek and
//! Cyrillic letters, keeping `ñ`. The folding is pinned to the Unicode 17.0 data in `crates/krummi/unicode/17.0.0`;
//! changing it changes what keys hold, so it is a format change (IC2, IC10).

use std::borrow::Cow;
use std::fmt;

use super::collation_tables as tables;

/// The collation of a `string` column (IC1). `Binary` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Collation {
    /// Texts are equal only when their bytes are: no folding.
    #[default]
    Binary,
    /// Case-insensitive: full Unicode case folding with canonical normalization (IC2).
    Ci,
    /// Case- and accent-insensitive: the `ci` form without the marks of Latin, Greek and Cyrillic letters, keeping
    /// the tilde of `ñ` (IC2).
    CiAi,
}

impl fmt::Display for Collation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Collation::Binary => "binary",
            Collation::Ci => "ci",
            Collation::CiAi => "ci_ai",
        })
    }
}

/// The folded form of `text` under `collation` (IC2): borrowed when it is the text itself.
///
/// `ci` is NFC(NFD(full case folding(NFD(text)))), with the case folding entries of status C and F. `ci_ai` takes
/// the `ci` form, decomposes it, removes every General_Category Mn scalar whose base (the nearest preceding scalar
/// that is not Mn) has Script Latin, Greek or Cyrillic, except a U+0303 right after an `n`, and composes the rest.
/// A text that cannot change (only ASCII without capitals, or scalars the quick check marks stable) is returned
/// without allocating.
pub fn fold(collation: Collation, text: &str) -> Cow<'_, str> {
    let (bit, accents) = match collation {
        Collation::Binary => return Cow::Borrowed(text),
        Collation::Ci => (tables::QUICK_CI, false),
        Collation::CiAi => (tables::QUICK_CI_AI, true),
    };
    if text.is_ascii() {
        return if text.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(text.to_ascii_lowercase())
        } else {
            Cow::Borrowed(text)
        };
    }
    if text.chars().all(|c| quick_check(c) & bit == 0) {
        return Cow::Borrowed(text);
    }
    let folded = fold_by_segments(text, bit, accents);
    if folded == text {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(folded)
    }
}

/// `fold_slowly` of `text`, run only where it can change something: the stable scalars (those the quick check `bit`
/// clears: starters that are no second of a composition and fold to themselves) are copied, and an ASCII capital is
/// lowered, while each run of other scalars is folded whole from the simple scalar before it, which may compose with
/// its marks or be their base, up to the next simple scalar. A stable scalar blocks canonical reordering and
/// composition across it (it is a starter that composes with nothing before it), so folding the segments apart gives
/// the folded form of the whole text.
fn fold_by_segments(text: &str, bit: u8, accents: bool) -> String {
    let simple = |c: char| c.is_ascii_uppercase() || quick_check(c) & bit == 0;
    let mut out = String::with_capacity(text.len());
    // Where the text not yet written starts, and where the last simple scalar before it starts.
    let mut written = 0;
    let mut last_simple: Option<usize> = None;
    // The start of the segment being gathered, while one is.
    let mut segment: Option<usize> = None;
    for (at, c) in text.char_indices() {
        if simple(c) {
            if let Some(start) = segment.take() {
                out.push_str(&fold_slowly(&text[start..at], accents));
                written = at;
            }
            last_simple = Some(at);
        } else if segment.is_none() {
            let start = last_simple.filter(|&s| s >= written).unwrap_or(written);
            push_simple(&mut out, &text[written..start]);
            segment = Some(start);
        }
    }
    match segment {
        Some(start) => out.push_str(&fold_slowly(&text[start..], accents)),
        None => push_simple(&mut out, &text[written..]),
    }
    out
}

/// Appends `run`, simple scalars only, folded: an ASCII capital lowered, every other scalar as it is.
fn push_simple(out: &mut String, run: &str) {
    out.extend(run.chars().map(|c| c.to_ascii_lowercase()));
}

/// The general path of `fold`: decompose and order, case fold, decompose and order the folded scalars, remove the
/// marks (`ci_ai`), and compose. The `ci` form's NFD is the folded sequence itself, so `ci_ai` does not compose
/// and decompose it again.
fn fold_slowly(text: &str, accents: bool) -> String {
    let mut decomposed = Vec::with_capacity(text.len());
    for c in text.chars() {
        decompose(c, &mut decomposed);
    }
    let mut folded = Vec::with_capacity(decomposed.len());
    for &(c, class) in &decomposed {
        match search(tables::CASE_FOLDING, c) {
            Some(f) => f.iter().for_each(|&x| decompose(x, &mut folded)),
            None => push_ordered(&mut folded, c, class),
        }
    }
    if accents {
        remove_marks(&mut folded);
    }
    compose(&folded)
}

/// The quick check bits of `c` (`tables::QUICK_*`).
fn quick_check(c: char) -> u8 {
    if c.is_ascii() {
        return if c.is_ascii_uppercase() {
            tables::QUICK_CI | tables::QUICK_CI_AI
        } else {
            0
        };
    }
    in_ranges(tables::QUICK_CHECK, c).unwrap_or(0)
}

/// The canonical combining class of `c`.
fn combining_class(c: char) -> u8 {
    in_ranges(tables::COMBINING_CLASS, c).unwrap_or(0)
}

fn is_nonspacing_mark(c: char) -> bool {
    in_ranges(tables::NONSPACING_MARK, c).is_some()
}

fn is_latin_greek_cyrillic(c: char) -> bool {
    in_ranges(tables::LATIN_GREEK_CYRILLIC, c).is_some()
}

/// The value of the inclusive range holding `c`, in a sorted table of ranges `(first, last, value)`.
fn in_ranges<R: RangeEntry>(table: &[R], c: char) -> Option<R::Value> {
    let i = table.partition_point(|r| r.last() < c);
    table.get(i).filter(|r| r.first() <= c).map(R::value)
}

trait RangeEntry {
    type Value;
    fn first(&self) -> char;
    fn last(&self) -> char;
    fn value(&self) -> Self::Value;
}

impl RangeEntry for (char, char) {
    type Value = ();
    fn first(&self) -> char {
        self.0
    }
    fn last(&self) -> char {
        self.1
    }
    fn value(&self) {}
}

impl RangeEntry for (char, char, u8) {
    type Value = u8;
    fn first(&self) -> char {
        self.0
    }
    fn last(&self) -> char {
        self.1
    }
    fn value(&self) -> u8 {
        self.2
    }
}

/// The mapping of `c` in a table of `(scalar, mapping)` sorted by scalar.
fn search(table: &'static [(char, &'static [char])], c: char) -> Option<&'static [char]> {
    table
        .binary_search_by_key(&c, |&(k, _)| k)
        .ok()
        .map(|i| table[i].1)
}

// Hangul syllables (Unicode 3.12).
const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

fn jamo(code: u32) -> char {
    char::from_u32(code).expect("Hangul jamo and syllables are scalar values")
}

/// Appends the full canonical decomposition of `c` to `out`, keeping `out` in canonical order.
fn decompose(c: char, out: &mut Vec<(char, u8)>) {
    let s = c as u32;
    if (S_BASE..S_BASE + S_COUNT).contains(&s) {
        let i = s - S_BASE;
        out.push((jamo(L_BASE + i / N_COUNT), 0));
        out.push((jamo(V_BASE + (i % N_COUNT) / T_COUNT), 0));
        if !i.is_multiple_of(T_COUNT) {
            out.push((jamo(T_BASE + i % T_COUNT), 0));
        }
    } else if let Some(d) = search(tables::DECOMPOSITION, c) {
        for &x in d {
            push_ordered(out, x, combining_class(x));
        }
    } else {
        push_ordered(out, c, combining_class(c));
    }
}

/// Appends `c`, of combining class `class`, moving it before the non-starters of a higher class (canonical ordering,
/// stable).
fn push_ordered(out: &mut Vec<(char, u8)>, c: char, class: u8) {
    let mut i = out.len();
    out.push((c, class));
    if class != 0 {
        while i > 0 && out[i - 1].1 > class {
            out.swap(i - 1, i);
            i -= 1;
        }
    }
}

/// Removes the Mn scalars whose base is Latin, Greek or Cyrillic, keeping a U+0303 whose preceding scalar, before
/// any removal, is `n` (IC2). A mark with no base is kept.
fn remove_marks(decomposed: &mut Vec<(char, u8)>) {
    let mut base_removes_marks: Option<bool> = None;
    let mut previous: Option<char> = None;
    decomposed.retain(|&(c, _)| {
        let before = previous.replace(c);
        if is_nonspacing_mark(c) {
            base_removes_marks != Some(true) || (c == '\u{303}' && before == Some('n'))
        } else {
            base_removes_marks = Some(is_latin_greek_cyrillic(c));
            true
        }
    });
}

/// The primary composite of the pair, if any.
fn compose_pair(a: char, b: char) -> Option<char> {
    let (a32, b32) = (a as u32, b as u32);
    if (L_BASE..L_BASE + L_COUNT).contains(&a32) && (V_BASE..V_BASE + V_COUNT).contains(&b32) {
        return Some(jamo(
            S_BASE + ((a32 - L_BASE) * V_COUNT + (b32 - V_BASE)) * T_COUNT,
        ));
    }
    if (S_BASE..S_BASE + S_COUNT).contains(&a32)
        && (a32 - S_BASE).is_multiple_of(T_COUNT)
        && (T_BASE + 1..T_BASE + T_COUNT).contains(&b32)
    {
        return Some(jamo(a32 + (b32 - T_BASE)));
    }
    tables::COMPOSITION
        .binary_search_by(|&(x, y, _)| (x, y).cmp(&(a, b)))
        .ok()
        .map(|i| tables::COMPOSITION[i].2)
}

/// The canonical composition of a decomposed, canonically ordered sequence (NFC).
fn compose(decomposed: &[(char, u8)]) -> String {
    let mut out: Vec<char> = Vec::with_capacity(decomposed.len());
    let mut starter: Option<usize> = None;
    // The class of the last scalar in `out`: a scalar between the starter and the next one blocks it unless its
    // class is lower than the next one's.
    let mut last_class = 0;
    for &(c, class) in decomposed {
        if let Some(s) = starter
            && (s + 1 == out.len() || last_class < class)
            && let Some(p) = compose_pair(out[s], c)
        {
            out[s] = p;
            continue;
        }
        if class == 0 {
            starter = Some(out.len());
        }
        out.push(c);
        last_class = class;
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn borrowed(c: Cow<'_, str>) -> bool {
        matches!(c, Cow::Borrowed(_))
    }

    #[test]
    fn collation_binary_keeps_the_text() {
        let folded = fold(Collation::Binary, "JosÉ");
        assert_eq!(folded, "JosÉ");
        assert!(borrowed(folded));
    }

    #[test]
    fn collation_ascii_without_capitals_is_borrowed() {
        for collation in [Collation::Ci, Collation::CiAi] {
            let folded = fold(collation, "hello, world 42");
            assert_eq!(folded, "hello, world 42");
            assert!(borrowed(folded));
            let empty = fold(collation, "");
            assert_eq!(empty, "");
            assert!(borrowed(empty));
        }
    }

    #[test]
    fn collation_ascii_with_capitals_is_lowercased() {
        for collation in [Collation::Ci, Collation::CiAi] {
            let folded = fold(collation, "Hello, World");
            assert_eq!(folded, "hello, world");
            assert!(!borrowed(folded));
        }
    }

    #[test]
    fn collation_stable_non_ascii_text_is_borrowed() {
        for (collation, text) in [
            (Collation::Ci, "josé"),
            (Collation::Ci, "año"),
            (Collation::CiAi, "año"),
            (Collation::CiAi, "ærø"),
            (Collation::Ci, "가나다"),
            (Collation::CiAi, "東京"),
        ] {
            let folded = fold(collation, text);
            assert_eq!(folded, text, "{collation} of {text}");
            assert!(borrowed(folded), "{collation} of {text}");
        }
    }

    #[test]
    fn collation_unchanged_text_off_the_quick_path_is_borrowed() {
        // `n` + U+0303 is not composed, so the quick check sends it down the general path, which composes it.
        let folded = fold(Collation::Ci, "n\u{303}");
        assert_eq!(folded, "ñ");
        // U+0301 alone has no base, so `ci_ai` keeps it; it is a non-starter, so the quick check cannot tell.
        let mark = fold(Collation::CiAi, "\u{301}");
        assert_eq!(mark, "\u{301}");
        assert!(borrowed(mark));
    }

    /// The folding by segments gives what folding the whole text gives, over random mixes of stable letters, ASCII
    /// capitals, letters that fold, precomposed and combining accents, `ñ`, `n` + tilde, Hangul and lone marks.
    #[test]
    fn collation_segments_fold_as_the_whole_text() {
        const PIECES: [&str; 24] = [
            "a", "Z", "é", "É", "e\u{301}", "\u{301}", "\u{303}", "n", "N", "ñ", "Ñ", "ß", "ŉ",
            "Σ", "ς", "ᾳ", "İ", "가", "\u{1100}", "\u{1161}", " ", "0", "東", "\u{323}",
        ];
        let mut state = 0x5EED_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize
        };
        for _ in 0..20_000 {
            let pieces = next() % 9;
            let text: String = (0..pieces).map(|_| PIECES[next() % PIECES.len()]).collect();
            for (collation, bit, accents) in [
                (Collation::Ci, tables::QUICK_CI, false),
                (Collation::CiAi, tables::QUICK_CI_AI, true),
            ] {
                assert_eq!(
                    fold_by_segments(&text, bit, accents),
                    fold_slowly(&text, accents),
                    "{collation} of {text:?}"
                );
            }
        }
    }

    #[test]
    fn collation_folding_changes_case_and_accents() {
        assert_eq!(fold(Collation::Ci, "JOSÉ"), "josé");
        assert_eq!(fold(Collation::CiAi, "JOSÉ"), "jose");
        assert_eq!(fold(Collation::CiAi, "Ñandú"), "ñandu");
        assert_eq!(fold(Collation::Ci, "한글"), "한글");
    }
}
