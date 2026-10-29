//! Validation of compressed ed25519 public-key encodings.
//!
//! The accepted set mirrors `libsodium`'s `crypto_core_ed25519_is_valid_point`:
//!
//! * the encoding must be canonical (`y < p`, and no non-canonical sign bit for
//!   a point with `x == 0`);
//! * it must decompress to a point on the twisted Edwards curve
//!   `-x^2 + y^2 = 1 + d * x^2 * y^2`;
//! * the point must be torsion-free (in the prime-order subgroup); and
//! * the point must not be in the small-order (order 1/2/4/8) subgroup.
//!
//! The final two checks are what separate a genuine ed25519 public key from a
//! mixed-order point such as the repeated `0x01` encoding, which is on the
//! curve and non-small-order but has a non-zero torsion component. Such a value
//! cannot have been produced by ed25519 key generation and must be rejected.
//!
//! Point decompression and subgroup membership are delegated to
//! `curve25519-dalek`, a vetted, `no_std` curve implementation, instead of
//! re-implementing the curve arithmetic in this contract.

use curve25519_dalek::edwards::CompressedEdwardsY;

/// Returns `true` when `bytes` is a canonical, on-curve, torsion-free,
/// non-small-order encoding of an ed25519 public key.
pub fn is_valid_ed25519_public_key(bytes: &[u8; 32]) -> bool {
    let compressed = CompressedEdwardsY(*bytes);

    // Reject encodings that do not decompress to a curve point.
    let Some(point) = compressed.decompress() else {
        return false;
    };

    // Re-encoding a canonical encoding is the identity. This rejects non-canonical
    // encodings (`y >= p`, or a sign bit set for a zero `x`), matching
    // `ge25519_is_canonical`.
    if point.compress() != compressed {
        return false;
    }

    // Reject the small-order subgroup (order 1/2/4/8) and anything with a
    // non-zero torsion component, matching `ge25519_has_small_order` and
    // `ge25519_is_on_main_subgroup` respectively. A torsion-free point is in
    // the prime-order subgroup; `is_torsion_free` alone would still accept the
    // identity, so the small-order check is kept.
    !point.is_small_order() && point.is_torsion_free()
}

#[cfg(test)]
mod test {
    use super::*;
    use curve25519_dalek::constants::{ED25519_BASEPOINT_POINT, EIGHT_TORSION};
    use curve25519_dalek::scalar::Scalar;

    fn bytes_from_hex(hex: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let raw = hex.as_bytes();
        let mut i = 0;
        while i < 32 {
            let hi = (raw[i * 2] as char).to_digit(16).unwrap() as u8;
            let lo = (raw[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
            out[i] = (hi << 4) | lo;
            i += 1;
        }
        out
    }

    #[test]
    fn accepts_rfc8032_public_keys() {
        // RFC 8032 section 7.1 public keys.
        let keys = [
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        ];
        for key in keys {
            assert!(
                is_valid_ed25519_public_key(&bytes_from_hex(key)),
                "RFC 8032 key {key} must be accepted"
            );
        }
    }

    #[test]
    fn accepts_prime_order_points() {
        // The basepoint and its small scalar multiples are all torsion-free.
        let base = ED25519_BASEPOINT_POINT;
        for k in [1u64, 2, 3, 8, 9, 42, 255, 256, u32::MAX as u64] {
            let point = base * Scalar::from(k);
            assert!(
                is_valid_ed25519_public_key(point.compress().as_bytes()),
                "scalar multiple {k} of the basepoint must be accepted"
            );
        }
    }

    #[test]
    fn rejects_all_zero() {
        assert!(!is_valid_ed25519_public_key(&[0u8; 32]));
    }

    #[test]
    fn rejects_small_order_points() {
        for point in EIGHT_TORSION.iter() {
            assert!(
                !is_valid_ed25519_public_key(point.compress().as_bytes()),
                "small-order point must be rejected"
            );
        }
    }

    /// Regression for the reviewer's reproducer: `[0x01; 32]` decompresses to an
    /// on-curve, non-small-order point that is *not* in the prime-order subgroup.
    #[test]
    fn rejects_mixed_order_points() {
        // Reproduce the reviewer's differential: `[0x01; 32]` decompresses to
        // an on-curve, non-small-order point that is *not* torsion-free.
        let repro = CompressedEdwardsY([0x01; 32]);
        let point = repro.decompress().expect("[0x01; 32] must decompress");
        assert!(!point.is_small_order());
        assert!(!point.is_torsion_free());
        assert!(
            !is_valid_ed25519_public_key(&[0x01; 32]),
            "[0x01; 32] is mixed-order and must be rejected"
        );

        // Basepoint + non-identity torsion is on the curve but not torsion-free.
        for torsion in EIGHT_TORSION.iter().skip(1) {
            let mixed = ED25519_BASEPOINT_POINT + torsion;
            assert!(
                !is_valid_ed25519_public_key(mixed.compress().as_bytes()),
                "basepoint + torsion must be rejected"
            );
        }
    }

    #[test]
    fn rejects_non_canonical_encodings() {
        // 0xff * 32 clears the sign bit to 2^255 - 1, which is >= p.
        assert!(!is_valid_ed25519_public_key(&[0xffu8; 32]));
        // Exactly p, little-endian.
        assert!(!is_valid_ed25519_public_key(&bytes_from_hex(
            "edffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f"
        )));
    }

    #[test]
    fn rejects_off_curve_encodings() {
        let keys = [
            "4f611d9cb8886f044449d848bbf6d4ddd34536cb117175af43bf53a76b5f5532",
            "7edd466be23f782d7eb7d8510b3bcb623a7f029c0ac83683140a04e71d54ad25",
            "44fa0c5f362a18260424583095cd9903e29f6fbabd97a7a877b4850c4be3baa4",
        ];
        for key in keys {
            assert!(
                !is_valid_ed25519_public_key(&bytes_from_hex(key)),
                "off-curve key {key} must be rejected"
            );
        }
        // `[2u8; 32]` and `[4u8; 32]` are the placeholders the old fixtures used.
        assert!(!is_valid_ed25519_public_key(&[2u8; 32]));
        assert!(!is_valid_ed25519_public_key(&[4u8; 32]));
    }
}
