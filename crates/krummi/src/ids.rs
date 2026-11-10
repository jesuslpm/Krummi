//! `ulid` and `uuid` values (N4 of the tables PRD): their text forms (E9 of the tables with schema PRD) and their
//! generators (E15).

use std::fmt;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use krummi_pager::{Error, Result, ValueRule};

/// A `ulid`: 16 bytes, 48 bits of Unix milliseconds and then 80 random bits, big-endian (document 1 of the table
/// design). Its byte order is its creation order and the order of its text form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ulid(pub [u8; 16]);

/// A `uuid`, always in the byte order of RFC 9562 (E7), also for a column with the order of SQL Server: the order
/// is a property of the column's key, not of the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uuid(pub [u8; 16]);

// --- Text forms (E9) ---

/// Crockford's Base32 alphabet, in digit order.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const HEX: &[u8; 16] = b"0123456789abcdef";
/// Where the hyphens of a `uuid` text go (8-4-4-4-12).
const HYPHENS: [usize; 4] = [8, 13, 18, 23];

/// A text that is not the one form of E9.
fn malformed() -> Error {
    Error::InvalidValue {
        folder: None,
        table: None,
        column: None,
        sequence: None,
        rule: ValueRule::MalformedText,
    }
}

/// The one text form of E9: 26 characters of Crockford's Base32 in uppercase, the first at most `7` (it holds the
/// top 3 of the 128 bits).
impl fmt::Display for Ulid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = u128::from_be_bytes(self.0);
        let mut text = [0u8; 26];
        for (i, c) in text.iter_mut().enumerate() {
            *c = CROCKFORD[((n >> (5 * (25 - i))) & 31) as usize];
        }
        f.pad(std::str::from_utf8(&text).expect("ASCII"))
    }
}

/// Reads the one text form of E9; anything else (lowercase, another length, a character outside the alphabet, a
/// first character above `7`) fails with [`Error::InvalidValue`], rule [`ValueRule::MalformedText`].
impl FromStr for Ulid {
    type Err = Error;

    fn from_str(text: &str) -> Result<Ulid> {
        let text = text.as_bytes();
        if text.len() != 26 || text[0] > b'7' {
            return Err(malformed());
        }
        let mut n = 0u128;
        for &c in text {
            let digit = CROCKFORD
                .iter()
                .position(|&d| d == c)
                .ok_or_else(malformed)?;
            n = (n << 5) | digit as u128;
        }
        Ok(Ulid(n.to_be_bytes()))
    }
}

/// The one text form of E9: RFC 9562 text in lowercase with hyphens (8-4-4-4-12), the bytes in their order.
impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = [b'-'; 36];
        let mut at = 0;
        for byte in self.0 {
            if HYPHENS.contains(&at) {
                at += 1;
            }
            text[at] = HEX[usize::from(byte >> 4)];
            text[at + 1] = HEX[usize::from(byte & 15)];
            at += 2;
        }
        f.pad(std::str::from_utf8(&text).expect("ASCII"))
    }
}

/// Reads the one text form of E9; anything else (uppercase, braces, no hyphens, another length) fails with
/// [`Error::InvalidValue`], rule [`ValueRule::MalformedText`].
impl FromStr for Uuid {
    type Err = Error;

    fn from_str(text: &str) -> Result<Uuid> {
        let text = text.as_bytes();
        if text.len() != 36 || HYPHENS.iter().any(|&i| text[i] != b'-') {
            return Err(malformed());
        }
        let mut digits = text
            .iter()
            .enumerate()
            .filter(|(i, _)| !HYPHENS.contains(i))
            .map(|(_, c)| HEX.iter().position(|d| d == c).ok_or_else(malformed));
        let mut bytes = [0u8; 16];
        for byte in &mut bytes {
            let high = digits.next().expect("32 digits")?;
            let low = digits.next().expect("32 digits")?;
            *byte = ((high << 4) | low) as u8;
        }
        Ok(Uuid(bytes))
    }
}

// --- Generators (E15) ---

/// The last Unix millisecond that fits in 48 bits (E15).
const MAX_MILLISECOND: u64 = (1 << 48) - 1;
/// Bytes drawn from the operating system at each new millisecond: 80 bits, of which the `uuid` version 7 keeps the
/// low 74.
const DRAW: usize = 10;

/// The kind of value a generator makes (a test hook, not part of the interface: see [`testing`]).
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Ulid,
    UuidV7,
    SequentialUuid,
}

impl Layout {
    /// The random part as one number: 80 bits, or the 74 of the `uuid` version 7, whose version and variant bits
    /// are outside it, so the increment's carry skips them (E15).
    const fn random_mask(self) -> u128 {
        match self {
            Layout::UuidV7 => (1 << 74) - 1,
            Layout::Ulid | Layout::SequentialUuid => (1 << 80) - 1,
        }
    }

    /// The 16 bytes of a value, in RFC 9562 order for the `uuid`s.
    fn bytes(self, millisecond: u64, random: u128) -> [u8; 16] {
        let millisecond = u128::from(millisecond);
        match self {
            Layout::Ulid => ((millisecond << 80) | random).to_be_bytes(),
            // RFC 9562 §5.7: unix_ts_ms (48), ver = 7 (4), rand_a (12), var = 0b10 (2), rand_b (62); the random part
            // is rand_a then rand_b.
            Layout::UuidV7 => {
                let rand_a = random >> 62;
                let rand_b = random & ((1 << 62) - 1);
                ((millisecond << 80) | (0x7 << 76) | (rand_a << 64) | (0b10 << 62) | rand_b)
                    .to_be_bytes()
            }
            // The user's .NET layout, in the bytes of `Guid.ToByteArray`: 10-15 the millisecond, 8-9 the high 16
            // random bits, then 6-7, 4-5 and 0-3 the low 64 from most to least significant, each group big-endian.
            // RFC 9562 order reverses the groups 0-3, 4-5 and 6-7 of that one.
            Layout::SequentialUuid => {
                let low = random as u64;
                let mut bytes = [0u8; 16];
                bytes[0..4].copy_from_slice(&(low as u32).to_le_bytes());
                bytes[4..6].copy_from_slice(&((low >> 32) as u16).to_le_bytes());
                bytes[6..8].copy_from_slice(&((low >> 48) as u16).to_le_bytes());
                bytes[8..10].copy_from_slice(&((random >> 64) as u16).to_be_bytes());
                bytes[10..16].copy_from_slice(&millisecond.to_be_bytes()[10..]);
                bytes
            }
        }
    }
}

/// The state of a generator: the millisecond and the random part of its last value (D7).
#[derive(Debug, Clone, Copy)]
struct State {
    last: Option<(u64, u128)>,
}

impl State {
    const NEW: State = State { last: None };

    /// The next value at clock reading `now` (Unix milliseconds), with the three rules of E15; `draw` fills a buffer
    /// with random bits. A failure changes nothing.
    fn next(
        &mut self,
        layout: Layout,
        now: u64,
        draw: &mut dyn FnMut(&mut [u8]) -> Result<()>,
    ) -> Result<[u8; 16]> {
        let mask = layout.random_mask();
        let (millisecond, random) = match self.last {
            // (2) The same millisecond, or the clock went back: the last millisecond, random part plus 1.
            Some((last, random)) if now <= last && random < mask => (last, random + 1),
            // (3) The random part overflows: the next millisecond after the last used, new bits.
            Some((last, _)) if now <= last => (checked(last + 1)?, draw_bits(draw, mask)?),
            // (1) A later millisecond: new bits.
            _ => (checked(now)?, draw_bits(draw, mask)?),
        };
        self.last = Some((millisecond, random));
        Ok(layout.bytes(millisecond, random))
    }
}

/// `millisecond`, if it fits in 48 bits (E15).
fn checked(millisecond: u64) -> Result<u64> {
    if millisecond > MAX_MILLISECOND {
        return Err(Error::InvalidValue {
            folder: None,
            table: None,
            column: None,
            sequence: None,
            rule: ValueRule::MillisecondOutOfRange,
        });
    }
    Ok(millisecond)
}

/// New random bits: [`DRAW`] bytes, big-endian, limited to `mask`.
fn draw_bits(draw: &mut dyn FnMut(&mut [u8]) -> Result<()>, mask: u128) -> Result<u128> {
    let mut bytes = [0u8; 16];
    draw(&mut bytes[16 - DRAW..])?;
    Ok(u128::from_be_bytes(bytes) & mask)
}

/// The process-wide generators (D7): one state each, behind a lock, so values are strictly increasing in the process,
/// also between threads.
static ULID: Mutex<State> = Mutex::new(State::NEW);
static UUID_V7: Mutex<State> = Mutex::new(State::NEW);
static SEQUENTIAL_UUID: Mutex<State> = Mutex::new(State::NEW);

fn generate(state: &Mutex<State>, layout: Layout) -> Result<[u8; 16]> {
    // `State::next` changes the state only after its last fallible step, so a poisoned lock holds a valid state.
    let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
    // A clock before 1970 reads as millisecond 0.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    state.next(layout, now, &mut krummi_pager::fill_random)
}

/// A new `ulid` (E15): 48 bits of Unix milliseconds and 80 random bits from the operating system, drawn at each new
/// millisecond and incremented within it, strictly increasing in the process, also between threads. Fails with
/// [`Error::InvalidValue`], rule [`ValueRule::MillisecondOutOfRange`], past the 48-bit millisecond, and with
/// [`Error::Io`] if the operating system gives no random bits.
pub fn ulid() -> Result<Ulid> {
    generate(&ULID, Layout::Ulid).map(Ulid)
}

/// A new `uuid` version 7 (E15), for columns with the order of RFC 9562: 48 bits of Unix milliseconds, the version
/// and variant bits of RFC 9562 and 74 random bits, with the rules and failures of [`ulid`].
pub fn uuid_v7() -> Result<Uuid> {
    generate(&UUID_V7, Layout::UuidV7).map(Uuid)
}

/// A new sequential `uuid` (E15), for columns with the order of SQL Server: 48 bits of Unix milliseconds in the
/// bytes SQL Server compares first and 80 random bits in the others, with the layout of the user's .NET generator,
/// given in RFC 9562 byte order (E7); with the rules and failures of [`ulid`].
pub fn sequential_uuid() -> Result<Uuid> {
    generate(&SEQUENTIAL_UUID, Layout::SequentialUuid).map(Uuid)
}

/// A test hook, not part of the interface (AC12): a generator with its own state, whose clock reading and random bits
/// the caller gives.
#[doc(hidden)]
pub mod testing {
    use super::{Result, State};

    pub use super::Layout as Kind;

    /// A generator of `kind` with its own state, as the process-wide one starts.
    pub struct Generator {
        layout: Kind,
        state: State,
    }

    impl Generator {
        pub fn new(kind: Kind) -> Generator {
            Generator {
                layout: kind,
                state: State::NEW,
            }
        }

        /// The next value, in RFC 9562 byte order for the `uuid`s, at clock reading `now` (Unix milliseconds);
        /// `draw` stands for the operating system's generator.
        pub fn next(
            &mut self,
            now: u64,
            draw: &mut dyn FnMut(&mut [u8]) -> Result<()>,
        ) -> Result<[u8; 16]> {
            self.state.next(self.layout, now, draw)
        }
    }
}
