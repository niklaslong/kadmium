//! Protocol identifier types.

#[cfg(feature = "codec")]
use bincode::{Decode, Encode};

/// A 256-bit identifier that implements a non-euclidian XOR-based distance metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct Id {
    bytes: [u8; Self::BYTES],
}

impl Id {
    /// The size of the identifier in bytes.
    pub const BYTES: usize = 32;

    /// The size of the identifier in bits.
    pub const BITS: usize = 32 * 8;

    /// Creates a new identifier from the supplied bytes.
    pub fn new(bytes: [u8; Self::BYTES]) -> Self {
        Id { bytes }
    }

    /// Returns the bytes backing the identifier.
    pub fn bytes(&self) -> [u8; Self::BYTES] {
        self.bytes
    }

    #[cfg(test)]
    /// Convenience function for working with small identifiers during testing.
    pub fn from_u16(raw: u16) -> Self {
        let mut bytes = [0u8; Self::BYTES];
        bytes[..2].copy_from_slice(&raw.to_le_bytes());

        Self { bytes }
    }

    #[doc(hidden)]
    /// Convenience function for generating random identifiers during testing.
    pub fn rand() -> Self {
        use rand::{thread_rng, Fill};

        let mut rng = thread_rng();
        let mut bytes = [0u8; Self::BYTES];
        assert!(bytes.try_fill(&mut rng).is_ok());

        Self { bytes }
    }

    /// Computes the log2 of the XOR-based distance between two identifiers.
    pub fn log2_distance(&self, other: &Id) -> Option<u32> {
        // Search process:
        //
        // [2, 1, 0, 0] <- array bytes in LE
        //  0  1  2  3  <- i
        //
        // We're looking for the most-significant bit, in this case it is at index 1, this becomes
        // clear when we reverse the array.
        //
        // [0, 0, 1, 2] <- array bytes in BE
        //  3  2  1  0  <- keeping the original i (accounts for reading the indexes from right to
        //                 left). Our most-significant byte is therefore at index 1. We then need
        //                 to calculate the most-significant bit in that byte (0-indexed) and
        //                 adding the index in bits.

        self.bytes
            .iter()
            .zip(other.bytes.iter())
            .map(|(&a, &b)| a ^ b)
            // See above.
            .enumerate()
            .rev()
            .find(|(_, byte)| byte != &0b0)
            // The left shift multiplies the index by 8 to get its value in bits.
            .map(|(i, byte)| Self::msb(byte) + ((i as u32) << 3))
    }

    // Returns the position of the most-significant bit set in a byte (0-indexed).
    fn msb(n: u8) -> u32 {
        debug_assert_ne!(n, 0);
        // Safety: can't be 0 - 1.
        u8::BITS - n.leading_zeros() - 1
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;

    #[test]
    fn id() {
        const N: usize = 1000;

        let mut rng = thread_rng();

        for _ in 0..N {
            let a = rng.gen();
            let b = rng.gen();

            // Skip as log2 will overflow (-INF).
            if a == b {
                continue;
            }

            let id_a = Id::from_u16(a);
            let id_b = Id::from_u16(b);

            let xor = a ^ b;
            let xor_log2 = u16::BITS - xor.leading_zeros() - 1;

            assert_eq!(id_a.log2_distance(&id_b), Some(xor_log2))
        }
    }
}
