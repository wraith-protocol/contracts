//! Self-contained validation of compressed ed25519 public-key encodings.
//!
//! Soroban's host exposes `ed25519_verify` but no point decompression, and the
//! `stealth-announcer` is a deliberately dependency-free, frozen event emitter,
//! so this module implements the encoding checks directly over the field
//! `GF(2^255 - 19)` instead of pulling in a curve library.
//!
//! The accepted set mirrors `libsodium`'s `crypto_core_ed25519_is_valid_point`:
//!
//! * the encoding must be a canonical `y` coordinate (`y < p`);
//! * it must decompress to a point on the twisted Edwards curve
//!   `-x^2 + y^2 = 1 + d * x^2 * y^2`; and
//! * the point must not live in the small-order (order 1/2/4/8) subgroup.
//!
//! The all-zero encoding is rejected explicitly as well. It is a well-formed
//! encoding of an order-4 point, so the on-curve check alone would accept it;
//! it is the concrete input called out by audit finding `WA-ANN-03`.
//!
//! Field elements are held as four little-endian `u64` limbs whose value is
//! always reduced modulo `p`.

/// Field modulus `p = 2^255 - 19`.
const P: [u64; 4] = [
    0xffff_ffff_ffff_ffed,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0x7fff_ffff_ffff_ffff,
];

/// ed25519 curve constant `d = -121665 / 121666 (mod p)`.
const D: [u64; 4] = [
    0x75eb_4dca_1359_78a3,
    0x0070_0a4d_4141_d8ab,
    0x8cc7_4079_7779_e898,
    0x5203_6cee_2b6f_fe73,
];

/// Exponent `(p - 5) / 8`, used for the square-root candidate in decompression.
const POW_EXP: [u64; 4] = [
    0xffff_ffff_ffff_fffd,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0x0fff_ffff_ffff_ffff,
];

const ZERO: [u64; 4] = [0, 0, 0, 0];
const ONE: [u64; 4] = [1, 0, 0, 0];

/// Canonical encodings of the eight small-order points (order 1, 2, 4 and 8).
const SMALL_ORDER: [[u8; 32]; 8] = [
    [
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ],
    [
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0, 0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98,
        0xf0, 0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39, 0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53,
        0xfc, 0x85,
    ],
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x80,
    ],
    [
        0xc7, 0x17, 0x6a, 0x70, 0x3d, 0x4d, 0xd8, 0x4f, 0xba, 0x3c, 0x0b, 0x76, 0x0d, 0x10, 0x67,
        0x0f, 0x2a, 0x20, 0x53, 0xfa, 0x2c, 0x39, 0xcc, 0xc6, 0x4e, 0xc7, 0xfd, 0x77, 0x92, 0xac,
        0x03, 0xfa,
    ],
    [
        0xec, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ],
    [
        0xc7, 0x17, 0x6a, 0x70, 0x3d, 0x4d, 0xd8, 0x4f, 0xba, 0x3c, 0x0b, 0x76, 0x0d, 0x10, 0x67,
        0x0f, 0x2a, 0x20, 0x53, 0xfa, 0x2c, 0x39, 0xcc, 0xc6, 0x4e, 0xc7, 0xfd, 0x77, 0x92, 0xac,
        0x03, 0x7a,
    ],
    [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ],
    [
        0x26, 0xe8, 0x95, 0x8f, 0xc2, 0xb2, 0x27, 0xb0, 0x45, 0xc3, 0xf4, 0x89, 0xf2, 0xef, 0x98,
        0xf0, 0xd5, 0xdf, 0xac, 0x05, 0xd3, 0xc6, 0x33, 0x39, 0xb1, 0x38, 0x02, 0x88, 0x6d, 0x53,
        0xfc, 0x05,
    ],
];

/// Returns `true` when `bytes` is a canonical, on-curve, non-small-order
/// encoding of an ed25519 public key.
pub fn is_valid_ed25519_public_key(bytes: &[u8; 32]) -> bool {
    // All-zero is the concrete `WA-ANN-03` reproducer. It decodes to an
    // order-4 point, so it also appears in `SMALL_ORDER`; the explicit check
    // keeps the rejection obvious and independent of the table.
    if *bytes == [0u8; 32] {
        return false;
    }
    for candidate in SMALL_ORDER.iter() {
        if bytes == candidate {
            return false;
        }
    }

    // The high bit of the last octet is the sign of x; the rest is y.
    let sign = bytes[31] >> 7;
    let mut y_bytes = *bytes;
    y_bytes[31] &= 0x7f;
    let y = from_le(&y_bytes);
    if ge(&y, &P) {
        // Non-canonical: y must be reduced below p.
        return false;
    }

    // Candidate root x = u * v^3 * (u * v^7)^((p-5)/8), with
    // u = y^2 - 1 and v = d * y^2 + 1. It is correct when v * x^2 == u, and
    // correct up to a factor of sqrt(-1) when v * x^2 == -u; otherwise the
    // encoding does not decompress and is rejected.
    let y2 = mul_mod(&y, &y);
    let u = sub_mod(&y2, &ONE);
    let v = add_mod(&mul_mod(&D, &y2), &ONE);
    let v3 = mul_mod(&mul_mod(&v, &v), &v);
    let v7 = mul_mod(&mul_mod(&v3, &v3), &v);
    let x = mul_mod(&mul_mod(&u, &v3), &pow_mod(&mul_mod(&u, &v7), &POW_EXP));
    let vx2 = mul_mod(&v, &mul_mod(&x, &x));

    if vx2 == u {
        // A zero root has no distinct negation, so an encoded sign of 1 would
        // not round-trip and the encoding is not canonical.
        !(is_zero(&x) && sign == 1)
    } else {
        vx2 == sub_mod(&ZERO, &u)
    }
}

/// Decodes 32 little-endian octets into field limbs.
fn from_le(bytes: &[u8; 32]) -> [u64; 4] {
    let mut out = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        let mut word = 0u64;
        let mut j = 0;
        while j < 8 {
            word |= (bytes[i * 8 + j] as u64) << (8 * j);
            j += 1;
        }
        out[i] = word;
        i += 1;
    }
    out
}

fn is_zero(a: &[u64; 4]) -> bool {
    a[0] | a[1] | a[2] | a[3] == 0
}

/// Returns `true` when `a >= b` for canonical (reduced) inputs.
fn ge(a: &[u64; 4], b: &[u64; 4]) -> bool {
    let mut i = 4;
    while i > 0 {
        i -= 1;
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

/// `a - b`, returning the reduced result and whether a borrow occurred.
fn sub(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], bool) {
    let mut out = ZERO;
    let mut borrow = 0u64;
    let mut i = 0;
    while i < 4 {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        out[i] = d2;
        borrow = (b1 as u64) | (b2 as u64);
        i += 1;
    }
    (out, borrow != 0)
}

/// `(a + b, carry)` over four limbs.
fn add(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = ZERO;
    let mut carry = 0u64;
    let mut i = 0;
    while i < 4 {
        let sum = a[i] as u128 + b[i] as u128 + carry as u128;
        out[i] = sum as u64;
        carry = (sum >> 64) as u64;
        i += 1;
    }
    (out, carry)
}

fn add_mod(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    // Inputs are < p < 2^255, so the sum never overflows four limbs.
    let (sum, _carry) = add(a, b);
    if ge(&sum, &P) {
        sub(&sum, &P).0
    } else {
        sum
    }
}

fn sub_mod(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let (diff, borrow) = sub(a, b);
    if borrow {
        add(&diff, &P).0
    } else {
        diff
    }
}

/// Schoolbook 256x256 -> 512 bit product.
fn mul_wide(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
    let mut t = [0u64; 8];
    let mut i = 0;
    while i < 4 {
        let mut carry = 0u64;
        let mut j = 0;
        while j < 4 {
            let cur = t[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry as u128;
            t[i + j] = cur as u64;
            carry = (cur >> 64) as u64;
            j += 1;
        }
        let mut k = i + 4;
        while carry != 0 && k < 8 {
            let cur = t[k] as u128 + carry as u128;
            t[k] = cur as u64;
            carry = (cur >> 64) as u64;
            k += 1;
        }
        i += 1;
    }
    t
}

/// `x * 38` as five limbs.
fn mul_38(x: &[u64; 4]) -> [u64; 5] {
    let mut out = [0u64; 5];
    let mut carry = 0u64;
    let mut i = 0;
    while i < 4 {
        let cur = (x[i] as u128) * 38 + carry as u128;
        out[i] = cur as u64;
        carry = (cur >> 64) as u64;
        i += 1;
    }
    out[4] = carry;
    out
}

/// Reduces a 512-bit value modulo `p`, folding on `2^256 = 38 (mod p)`.
fn reduce(t: &[u64; 8]) -> [u64; 4] {
    let hi = [t[4], t[5], t[6], t[7]];
    let mut wide = mul_38(&hi);
    let mut carry = 0u64;
    let mut i = 0;
    while i < 4 {
        let cur = wide[i] as u128 + t[i] as u128 + carry as u128;
        wide[i] = cur as u64;
        carry = (cur >> 64) as u64;
        i += 1;
    }
    // `wide[4]` is < 65 and `carry` is 0 or 1, so this cannot overflow.
    wide[4] += carry;

    // Fold the fifth limb back in: value = low + 2^256 * wide[4].
    let mut low = [wide[0], wide[1], wide[2], wide[3]];
    let mut carry = add_small(&mut low, 38 * wide[4]);
    while carry != 0 {
        carry = add_small(&mut low, 38);
    }

    let mut round = 0;
    while round < 3 && ge(&low, &P) {
        low = sub(&low, &P).0;
        round += 1;
    }
    low
}

/// Adds a small (<= 2^32) value to `limbs`, returning the carry out.
fn add_small(limbs: &mut [u64; 4], value: u64) -> u64 {
    let mut carry = value as u128;
    let mut i = 0;
    while i < 4 && carry != 0 {
        let cur = limbs[i] as u128 + carry;
        limbs[i] = cur as u64;
        carry = cur >> 64;
        i += 1;
    }
    carry as u64
}

fn mul_mod(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    reduce(&mul_wide(a, b))
}

/// Square-and-multiply exponentiation modulo `p`.
fn pow_mod(base: &[u64; 4], exp: &[u64; 4]) -> [u64; 4] {
    let mut result = ONE;
    let mut power = *base;
    let mut bit = 0;
    while bit < 256 {
        if (exp[bit / 64] >> (bit % 64)) & 1 == 1 {
            result = mul_mod(&result, &power);
        }
        power = mul_mod(&power, &power);
        bit += 1;
    }
    result
}

#[cfg(test)]
mod test {
    use super::*;

    fn fe(value: u64) -> [u64; 4] {
        [value, 0, 0, 0]
    }

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
    fn field_arithmetic_is_consistent() {
        // (-1)^2 == 1.
        let minus_one = sub_mod(&ZERO, &ONE);
        assert_eq!(mul_mod(&minus_one, &minus_one), ONE);

        // d * 121666 == -121665.
        let lhs = mul_mod(&D, &fe(121666));
        let rhs = sub_mod(&ZERO, &fe(121665));
        assert_eq!(lhs, rhs);

        // Fermat: 2^(p-1) == 1.
        let p_minus_one = sub(&P, &ONE).0;
        assert_eq!(pow_mod(&fe(2), &p_minus_one), ONE);
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
    fn accepts_canonical_on_curve_keys() {
        // Fill-byte keys that happen to be canonical, on-curve and not small
        // order; these keep the wide test fixtures readable.
        for fill in [1u8, 3, 6, 9, 0x2a, 0x32, 0x33, 0xaa] {
            let key = [fill; 32];
            assert!(
                is_valid_ed25519_public_key(&key),
                "fill 0x{fill:02x} must be accepted"
            );
        }
        // Arbitrary on-curve encodings generated from the reference implementation.
        let keys = [
            "307dd826d83e7052ff564654c67402c6a06023538136d505728adeb9f37e14a9",
            "d7148ddda885eef95f89e6b9053a17cf1b3d5725779d02851c4a94fb3f451396",
            "9b5b3f9bd89f5771aaa4c0a140bef0c5d907f67493a40fe8971e38076d52dca2",
        ];
        for key in keys {
            assert!(
                is_valid_ed25519_public_key(&bytes_from_hex(key)),
                "key {key} must be accepted"
            );
        }
    }

    #[test]
    fn rejects_all_zero() {
        assert!(!is_valid_ed25519_public_key(&[0u8; 32]));
    }

    #[test]
    fn rejects_small_order_points() {
        for candidate in SMALL_ORDER.iter() {
            assert!(
                !is_valid_ed25519_public_key(candidate),
                "small-order point must be rejected"
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
