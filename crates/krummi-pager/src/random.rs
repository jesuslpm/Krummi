//! Random bytes from the operating system's generator (P20 of the table design, document 11 §2.8): the source of
//! the `ulid` and `uuid` generators (E15 of the tables with schema PRD) and of the catalog identities. Krummi has no
//! generator of its own: every byte comes from the operating system.

use std::io;
use std::path::PathBuf;

use crate::error::{Error, Result};

/// Fills `buf` with random bytes from the operating system's generator: `getrandom` on Linux, `BCryptGenRandom`
/// with the system-preferred generator on Windows. A failure is [`Error::Io`] with an empty `folder`, because no
/// store is involved (E15: "entrada y salida"); nothing weaker is drawn in its place.
pub fn fill_random(buf: &mut [u8]) -> Result<()> {
    fill(buf).map_err(|source| Error::Io {
        folder: PathBuf::new(),
        source,
    })
}

fn fill(buf: &mut [u8]) -> io::Result<()> {
    crate::sys::before(crate::sys::Op::Random, std::path::Path::new(""))?;
    os_fill(buf)
}

#[cfg(target_os = "linux")]
fn os_fill(mut buf: &mut [u8]) -> io::Result<()> {
    use rustix::io::Errno;
    use rustix::rand::{GetRandomFlags, getrandom};
    // `getrandom` may return fewer bytes than asked (above 32 MiB, or when a signal interrupts it).
    while !buf.is_empty() {
        match getrandom(&mut *buf, GetRandomFlags::empty()) {
            Ok(0) => return Err(io::Error::other("getrandom returned no bytes")),
            Ok(n) => buf = &mut buf[n..],
            Err(Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn os_fill(buf: &mut [u8]) -> io::Result<()> {
    use windows_sys::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    // `BCryptGenRandom` takes a 32-bit length.
    for chunk in buf.chunks_mut(u32::MAX as usize) {
        // SAFETY: the pointer and the length describe `chunk`, a valid, writable and exclusively borrowed buffer that
        // outlives the call; a null algorithm handle is what `BCRYPT_USE_SYSTEM_PREFERRED_RNG` requires.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                chunk.as_mut_ptr(),
                chunk.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        // An NTSTATUS below zero is an error.
        if status < 0 {
            return Err(io::Error::other(format!(
                "BCryptGenRandom failed with status {status:#010x}"
            )));
        }
    }
    Ok(())
}

/// Other Unix systems (not targets of the engine, D3): the kernel's generator through its device file.
#[cfg(not(any(target_os = "linux", windows)))]
fn os_fill(buf: &mut [u8]) -> io::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;
    use crate::sys::{Op, faults};

    /// Two draws of 64 bytes differ and are not all zeros (the chance of either by luck is 2^-512).
    #[test]
    fn fills_the_buffer_with_bytes_of_the_operating_system() {
        let mut a = [0u8; 64];
        let mut b = [0u8; 64];
        fill_random(&mut a).unwrap();
        fill_random(&mut b).unwrap();
        assert_ne!(a, [0u8; 64]);
        assert_ne!(a, b);
    }

    /// A buffer larger than one call of the system returns (32 MiB on Linux) is filled to its end, and an empty one
    /// is fine.
    #[test]
    fn fills_large_and_empty_buffers() {
        let mut big = vec![0u8; 33 << 20];
        fill_random(&mut big).unwrap();
        assert_ne!(&big[big.len() - 64..], &[0u8; 64][..]);
        fill_random(&mut []).unwrap();
    }

    /// A failure of the system's generator is an input/output error, with no store folder (E15).
    #[test]
    fn a_failure_of_the_generator_is_an_io_error() {
        faults::reset();
        faults::inject(
            Op::Random,
            0,
            faults::Action::Fail(io::ErrorKind::Unsupported),
        );
        let mut buf = [0u8; 16];
        let err = fill_random(&mut buf).unwrap_err();
        faults::reset();
        assert_eq!(err.kind(), ErrorKind::Io);
        let Error::Io { folder, source } = err else {
            unreachable!()
        };
        assert_eq!(folder, PathBuf::new());
        assert_eq!(source.kind(), io::ErrorKind::Unsupported);
        assert_eq!(buf, [0u8; 16]);
    }
}
