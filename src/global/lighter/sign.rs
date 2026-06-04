//! Lighter 주문 서명 — **순수 Rust 포팅. 암호 코어 검증 / 와이어 봉투 미검증**.
//!
//! # 경고 (HARD RULE 3)
//!
//! 이 모듈은 `github.com/elliottech/lighter-go`(+`poseidon_crypto`)의 서명 스킴을
//! 순수 Rust로 **수기 포팅**한 것이다. Lighter 주문 서명은 다음으로 구성된다:
//!
//! - **필드**: Goldilocks (p = 2^64 − 2^32 + 1).
//! - **확장체**: GFp5 = Goldilocks의 5차 확장 (기약다항식 X^5 − 3).
//! - **해시**: Poseidon2 (plonky2 파라미터, width=12, rate=8, D=7, RF=8, RP=22).
//! - **곡선**: ECgFp5 — GFp5 위의 소수위(prime-order) 타원곡선 (pornin/ecgfp5).
//! - **서명**: Schnorr over ECgFp5. e = H(r ‖ H(m)), s = k − e·sk.
//!
//! ## 검증 상태
//!
//! **암호 프리미티브: 검증됨.** 공식 저장소의 결정적 테스트 벡터를 `#[cfg(test)]`에
//! 박아 `cargo test`로 통과 확인했고(이 세션에서 실행), 그 벡터가 **upstream Go 소스와
//! 바이트 일치**함을 독립 대조했다:
//!
//! - [`tests::schnorr_comparative_vector`] — `poseidon_crypto`
//!   `signature/schnorr/schnorr_test.go::TestComparativeSchnorrSignAndVerify`의 고정
//!   (sk,msg,k)→(S,E) 3케이스. 상위 소스 limb과 일치 확인. Schnorr sign+verify가
//!   필드·확장체(GFp5)·선형층·상수·스칼라·점곱·Poseidon2를 **독립 검증 경로**로 전이
//!   검증하므로, 이 벡터 통과 = 암호 코어가 바이트 단위로 정확함.
//! - [`tests::poseidon2_permute_vector`]·[`tests::poseidon2_hash_n_to_m_vector`] —
//!   Poseidon2 결정적 벡터(보조 앵커; Schnorr 경로가 동일 해시를 전이 검증).
//!
//! **미검증: tx_info JSON 와이어 봉투 + 십진 스케일링 + chain_id.** 이 계층은 불투명한
//! native `.so`가 만들며 공식 픽스처가 없어, 메시지 필드 구성·바이트 레이아웃·메인넷
//! chain_id를 종단 확정할 수 없다. `scripts/lighter_capture_vector.md`의 절차로 공식 SDK
//! 출력과 대조해야 한다. 즉 **암호는 옳으나 end-to-end 제출은 미검증**이다.
//!
//! **이 모듈의 모든 서명 함수는 실자금에 쓰기 전 반드시 위 절차로 검증할 것.**
//! 런타임 경로는 [`crate::global::lighter::config::LighterConfig::allow_unverified_signing`]
//! 가드 뒤에 있으며 기본값은 false다.

#![allow(clippy::needless_range_loop)]
// 충실 포팅(faithful transcription): Go 원본의 완전성을 위해 일부 헬퍼(from_u64/
// double/add 등)는 현재 미사용이나 보존한다. 삭제하면 원본 대조가 어려워진다.
#![allow(dead_code)]

// ───────────────────────────── Goldilocks field ─────────────────────────────
//
// p = 2^64 − 2^32 + 1. GoldilocksField는 비정규(non-canonical) u64 표현을 허용하고
// 비교/직렬화 시에만 정규화한다 — Go 원본(`field/goldilocks/goldilocks_plonky2.go`)과 동일.

pub(crate) mod goldilocks {
    pub const ORDER: u64 = 0xffff_ffff_0000_0001;
    pub const EPSILON: u64 = (1 << 32) - 1;

    /// Goldilocks 원소 (비정규 u64 래퍼).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct F(pub u64);

    #[inline]
    pub fn from_u64(x: u64) -> F {
        F(x)
    }

    impl F {
        /// 정규(canonical) u64. [0, ORDER).
        #[inline]
        pub fn canonical(self) -> u64 {
            let mut x = self.0;
            if x >= ORDER {
                x -= ORDER;
            }
            x
        }

        #[inline]
        pub fn is_zero(self) -> bool {
            self.canonical() == 0
        }

        #[inline]
        pub fn to_le_bytes(self) -> [u8; 8] {
            self.canonical().to_le_bytes()
        }

        #[inline]
        pub fn from_le_bytes(b: &[u8]) -> F {
            let mut a = [0u8; 8];
            a.copy_from_slice(&b[..8]);
            F(u64::from_le_bytes(a))
        }
    }

    /// 비정규 덧셈 (Go AddF).
    #[inline]
    pub fn add(lhs: F, rhs: F) -> F {
        let (mut sum, over1) = lhs.0.overflowing_add(rhs.0);
        let (s2, over2) = sum.overflowing_add(EPSILON * over1 as u64);
        sum = s2;
        sum = sum.wrapping_add(EPSILON * over2 as u64);
        F(sum)
    }

    /// lhs가 정규일 때 rhs(임의 u64)를 더함 (Go AddCanonicalUint64).
    #[inline]
    pub fn add_canonical_u64(lhs: F, rhs: u64) -> F {
        let (sum, over) = lhs.0.overflowing_add(rhs);
        F(sum.wrapping_add(EPSILON * over as u64))
    }

    /// 비정규 뺄셈 (Go SubF).
    #[inline]
    pub fn sub(lhs: F, rhs: F) -> F {
        let (mut diff, borrow1) = lhs.0.overflowing_sub(rhs.0);
        let (d2, borrow2) = diff.overflowing_sub(EPSILON * borrow1 as u64);
        diff = d2;
        diff = diff.wrapping_sub(EPSILON * borrow2 as u64);
        F(diff)
    }

    /// 비정규 곱셈 (Go MulF). 128비트 곱 후 Goldilocks 환원.
    #[inline]
    pub fn mul(lhs: F, rhs: F) -> F {
        let prod = (lhs.0 as u128) * (rhs.0 as u128);
        let x_lo = prod as u64;
        let x_hi = (prod >> 64) as u64;

        let x_hi_hi = x_hi >> 32;
        let x_hi_lo = x_hi & EPSILON;

        let (mut t0, borrow) = x_lo.overflowing_sub(x_hi_hi);
        t0 = t0.wrapping_sub(EPSILON * borrow as u64);

        let t1 = x_hi_lo * EPSILON;

        let (sum, over) = t0.overflowing_add(t1);
        F(sum.wrapping_add(EPSILON * over as u64))
    }

    #[inline]
    pub fn square(x: F) -> F {
        mul(x, x)
    }

    /// self + x*y (Go MulAccF).
    #[inline]
    pub fn mul_acc(self_: F, x: F, y: F) -> F {
        let prod = (x.0 as u128) * (y.0 as u128);
        let (lo, c0) = (prod as u64).overflowing_add(self_.0);
        let hi = ((prod >> 64) as u64).wrapping_add(c0 as u64);

        let (mut t0, borrow) = lo.overflowing_sub(hi >> 32);
        t0 = t0.wrapping_sub(EPSILON * borrow as u64);
        let (res_wrapped, c) = t0.overflowing_add((hi & EPSILON) * EPSILON);
        F(res_wrapped.wrapping_add(EPSILON * c as u64))
    }

    #[inline]
    pub fn neg(x: F) -> F {
        if x.is_zero() {
            F(0)
        } else {
            F(ORDER - x.canonical())
        }
    }

    #[inline]
    pub fn double(x: F) -> F {
        add(x, x)
    }

    /// x^(2^n).
    pub fn exp_power_of_2(mut x: F, n: u32) -> F {
        for _ in 0..n {
            x = square(x);
        }
        x
    }

    /// x^e.
    pub fn exp(x: F, mut e: u64) -> F {
        let mut current = x;
        let mut product = F(1);
        while e > 0 {
            if e & 1 == 1 {
                product = mul(product, current);
            }
            current = square(current);
            e >>= 1;
        }
        product
    }

    /// 곱셈 역원 (Go InverseOrZero). 0이면 0.
    pub fn inverse_or_zero(self_: F) -> F {
        if self_.is_zero() {
            return F(0);
        }
        let t2 = mul(square(self_), self_);
        let t3 = mul(square(t2), self_);
        let t6 = mul(exp_power_of_2(t3, 3), t3);
        let t12 = mul(exp_power_of_2(t6, 6), t6);
        let t24 = mul(exp_power_of_2(t12, 12), t12);
        let t30 = mul(exp_power_of_2(t24, 6), t6);
        let t31 = mul(square(t30), self_);
        let t63 = mul(exp_power_of_2(t31, 32), t31);
        mul(square(t63), self_)
    }

    #[inline]
    pub fn inverse(self_: F) -> F {
        debug_assert!(!self_.is_zero(), "inverse of zero");
        inverse_or_zero(self_)
    }

    /// 1부터 시작하는 거듭제곱 [1, e, e^2, ...].
    pub fn powers(e: F, count: usize) -> Vec<F> {
        let mut ret = Vec::with_capacity(count);
        if count == 0 {
            return ret;
        }
        ret.push(F(1));
        for i in 1..count {
            ret.push(mul(ret[i - 1], e));
        }
        ret
    }

    pub const TWO_ADICITY: u32 = 32;
    pub const POWER_OF_TWO_GENERATOR: u64 = 7277203076849721926;

    /// 제곱근 (Tonelli–Shanks, Go SqrtF). 비-QR이면 None.
    pub fn sqrt(self_: F) -> Option<F> {
        if self_.is_zero() {
            return Some(F(0));
        }
        if !is_quadratic_residue(self_) {
            return None;
        }
        let self_ = F(self_.canonical());
        let t = (ORDER - 1) / (1u64 << TWO_ADICITY);
        let mut z = F(POWER_OF_TWO_GENERATOR);
        let mut w = exp(self_, (t - 1) / 2);
        let mut x = mul(self_, w);
        let mut b = mul(x, w);
        let mut v = TWO_ADICITY as i64;

        while b.canonical() != 1 {
            let mut k = 0i64;
            let mut b2k = b;
            while b2k.canonical() != 1 {
                b2k = square(b2k);
                k += 1;
            }
            let j = v - k - 1;
            w = z;
            for _ in 0..j {
                w = square(w);
            }
            z = square(w);
            b = mul(b, z);
            x = mul(x, w);
            v = k;
        }
        Some(x)
    }

    pub fn is_quadratic_residue(x: F) -> bool {
        if x.is_zero() {
            return true;
        }
        let power = (neg(F(1)).canonical()) >> 1;
        let e = exp(x, power);
        match e.canonical() {
            1 => true,
            v if v == ORDER - 1 => false,
            _ => unreachable!("legendre symbol not ±1"),
        }
    }
}

// ───────────────────────── Goldilocks quintic extension (GFp5) ─────────────────────────
//
// Element = [F; 5], 기약다항식 X^5 − 3 (FP5_W = 3). Go
// `field/goldilocks_quintic_extension/goldilocks_quintic_extension.go`와 동일.

pub(crate) mod gfp5 {
    use super::goldilocks::{self as g, F};

    pub type Element = [F; 5];

    pub const BYTES: usize = 8 * 5;
    pub const W: u64 = 3;
    pub const DTH_ROOT: u64 = 1041288259238279555;

    pub const ZERO: Element = [F(0), F(0), F(0), F(0), F(0)];
    pub const ONE: Element = [F(1), F(0), F(0), F(0), F(0)];
    pub const TWO: Element = [F(2), F(0), F(0), F(0), F(0)];

    #[inline]
    pub fn from_f(e: F) -> Element {
        [e, F(0), F(0), F(0), F(0)]
    }

    #[inline]
    pub fn from_u64(a: u64) -> Element {
        [F(a), F(0), F(0), F(0), F(0)]
    }

    pub fn to_le_bytes(e: &Element) -> [u8; BYTES] {
        let mut out = [0u8; BYTES];
        for i in 0..5 {
            out[i * 8..i * 8 + 8].copy_from_slice(&e[i].to_le_bytes());
        }
        out
    }

    pub fn from_canonical_le_bytes(b: &[u8]) -> Result<Element, &'static str> {
        if b.len() != BYTES {
            return Err("invalid input length, expected 40 bytes");
        }
        let mut e = ZERO;
        for i in 0..5 {
            e[i] = F::from_le_bytes(&b[i * 8..i * 8 + 8]);
        }
        Ok(e)
    }

    #[inline]
    pub fn equals(a: &Element, b: &Element) -> bool {
        (0..5).all(|i| a[i].canonical() == b[i].canonical())
    }

    #[inline]
    pub fn is_zero(e: &Element) -> bool {
        e.iter().all(|x| x.is_zero())
    }

    #[inline]
    pub fn neg(e: &Element) -> Element {
        [g::neg(e[0]), g::neg(e[1]), g::neg(e[2]), g::neg(e[3]), g::neg(e[4])]
    }

    #[inline]
    pub fn add(a: &Element, b: &Element) -> Element {
        [
            g::add(a[0], b[0]),
            g::add(a[1], b[1]),
            g::add(a[2], b[2]),
            g::add(a[3], b[3]),
            g::add(a[4], b[4]),
        ]
    }

    #[inline]
    pub fn sub(a: &Element, b: &Element) -> Element {
        [
            g::sub(a[0], b[0]),
            g::sub(a[1], b[1]),
            g::sub(a[2], b[2]),
            g::sub(a[3], b[3]),
            g::sub(a[4], b[4]),
        ]
    }

    #[inline]
    pub fn double(a: &Element) -> Element {
        add(a, a)
    }

    #[inline]
    pub fn triple(a: &Element) -> Element {
        let three = F(3);
        [
            g::mul(a[0], three),
            g::mul(a[1], three),
            g::mul(a[2], three),
            g::mul(a[3], three),
            g::mul(a[4], three),
        ]
    }

    #[inline]
    pub fn scalar_mul(a: &Element, s: F) -> Element {
        [
            g::mul(a[0], s),
            g::mul(a[1], s),
            g::mul(a[2], s),
            g::mul(a[3], s),
            g::mul(a[4], s),
        ]
    }

    /// GFp5 곱 (X^5 = 3 환원). Go Mul과 항별 동일.
    pub fn mul(a: &Element, b: &Element) -> Element {
        let w = F(W);

        let a0b0 = g::mul(a[0], b[0]);
        let a1b4 = g::mul(a[1], b[4]);
        let a2b3 = g::mul(a[2], b[3]);
        let a3b2 = g::mul(a[3], b[2]);
        let a4b1 = g::mul(a[4], b[1]);
        let added = g::add(g::add(a1b4, a2b3), g::add(a3b2, a4b1));
        let muld = g::mul(w, added);
        let c0 = g::add(a0b0, muld);

        let a0b1 = g::mul(a[0], b[1]);
        let a1b0 = g::mul(a[1], b[0]);
        let a2b4 = g::mul(a[2], b[4]);
        let a3b3 = g::mul(a[3], b[3]);
        let a4b2 = g::mul(a[4], b[2]);
        let added = g::add(g::add(a2b4, a3b3), a4b2);
        let muld = g::mul(w, added);
        let c1 = g::add(g::add(a0b1, a1b0), muld);

        let a0b2 = g::mul(a[0], b[2]);
        let a1b1 = g::mul(a[1], b[1]);
        let a2b0 = g::mul(a[2], b[0]);
        let a3b4 = g::mul(a[3], b[4]);
        let a4b3 = g::mul(a[4], b[3]);
        let added = g::add(a3b4, a4b3);
        let muld = g::mul(w, added);
        let c2 = g::add(g::add(a0b2, a1b1), g::add(a2b0, muld));

        let a0b3 = g::mul(a[0], b[3]);
        let a1b2 = g::mul(a[1], b[2]);
        let a2b1 = g::mul(a[2], b[1]);
        let a3b0 = g::mul(a[3], b[0]);
        let a4b4 = g::mul(a[4], b[4]);
        let muld = g::mul(w, a4b4);
        let c3 = g::add(g::add(g::add(a0b3, a1b2), g::add(a2b1, a3b0)), muld);

        let a0b4 = g::mul(a[0], b[4]);
        let a1b3 = g::mul(a[1], b[3]);
        let a2b2 = g::mul(a[2], b[2]);
        let a3b1 = g::mul(a[3], b[1]);
        let a4b0 = g::mul(a[4], b[0]);
        let c4 = g::add(g::add(g::add(a0b4, a1b3), g::add(a2b2, a3b1)), a4b0);

        [c0, c1, c2, c3, c4]
    }

    /// GFp5 제곱 (Go Square — 항별 동일).
    pub fn square(a: &Element) -> Element {
        let w = F(W);
        let double_w = g::add(w, w);

        let a0s = g::mul(a[0], a[0]);
        let a1a4 = g::mul(a[1], a[4]);
        let a2a3 = g::mul(a[2], a[3]);
        let added = g::add(a1a4, a2a3);
        let muld = g::mul(double_w, added);
        let c0 = g::add(a0s, muld);

        let a0_double = g::add(a[0], a[0]);
        let a0_double_a1 = g::mul(a0_double, a[1]);
        let a2a4_double_w = g::mul(g::mul(a[2], a[4]), double_w);
        let a3a3w = g::mul(g::mul(a[3], a[3]), w);
        let c1 = g::add(g::add(a0_double_a1, a2a4_double_w), a3a3w);

        let a0_double_a2 = g::mul(a0_double, a[2]);
        let a1_square = g::mul(a[1], a[1]);
        let a4a3_double_w = g::mul(g::mul(a[4], a[3]), double_w);
        let c2 = g::add(g::add(a0_double_a2, a1_square), a4a3_double_w);

        let a1_double = g::add(a[1], a[1]);
        let a0_double_a3 = g::mul(a0_double, a[3]);
        let a1_double_a2 = g::mul(a1_double, a[2]);
        let a4_square_w = g::mul(g::mul(a[4], a[4]), w);
        let c3 = g::add(g::add(a0_double_a3, a1_double_a2), a4_square_w);

        let a0_double_a4 = g::mul(a0_double, a[4]);
        let a1_double_a3 = g::mul(a1_double, a[3]);
        let a2_square = g::mul(a[2], a[2]);
        let c4 = g::add(g::add(a0_double_a4, a1_double_a3), a2_square);

        [c0, c1, c2, c3, c4]
    }

    pub fn exp_power_of_2(x: &Element, power: u32) -> Element {
        let mut res = *x;
        for _ in 0..power {
            res = square(&res);
        }
        res
    }

    pub fn frobenius(x: &Element) -> Element {
        repeated_frobenius(x, 1)
    }

    pub fn repeated_frobenius(x: &Element, count: usize) -> Element {
        if count == 0 {
            return *x;
        } else if count >= 5 {
            return repeated_frobenius(x, count % 5);
        }
        let mut z0 = F(DTH_ROOT);
        for _ in 1..count {
            z0 = g::mul(F(DTH_ROOT), z0);
        }
        let mut res = ZERO;
        let powers = g::powers(z0, 5);
        for i in 0..5 {
            res[i] = g::mul(x[i], powers[i]);
        }
        res
    }

    pub fn inverse_or_zero(a: &Element) -> Element {
        if is_zero(a) {
            return ZERO;
        }
        let d = frobenius(a);
        let e = mul(&d, &frobenius(&d));
        let f = mul(&e, &repeated_frobenius(&e, 2));

        let a0b0 = g::mul(a[0], f[0]);
        let a1b4 = g::mul(a[1], f[4]);
        let a2b3 = g::mul(a[2], f[3]);
        let a3b2 = g::mul(a[3], f[2]);
        let a4b1 = g::mul(a[4], f[1]);
        let added = g::add(g::add(a1b4, a2b3), g::add(a3b2, a4b1));
        let muld = g::mul(F(W), added);
        let gg = g::add(a0b0, muld);

        scalar_mul(&f, g::inverse(gg))
    }

    pub fn div(a: &Element, b: &Element) -> Element {
        let b_inv = inverse_or_zero(b);
        debug_assert!(!is_zero(&b_inv), "division by zero");
        mul(a, &b_inv)
    }

    /// Legendre 기호 (Go Legendre). QR이면 1.
    pub fn legendre(x: &Element) -> F {
        let frob1 = frobenius(x);
        let frob2 = frobenius(&frob1);
        let frob1_times_frob2 = mul(&frob1, &frob2);
        let frob2_frob1 = repeated_frobenius(&frob1_times_frob2, 2);
        let xr_ext = mul(&mul(x, &frob1_times_frob2), &frob2_frob1);
        let xr = xr_ext[0];

        let xr31 = g::exp_power_of_2(xr, 31);
        let xr31_inv = g::inverse_or_zero(xr31);
        let xr63 = g::exp_power_of_2(xr31, 32);
        g::mul(xr63, xr31_inv)
    }

    pub fn sqrt(x: &Element) -> Option<Element> {
        let three = F(3);
        let v = exp_power_of_2(x, 31);
        let d = mul(&mul(x, &exp_power_of_2(&v, 32)), &inverse_or_zero(&v));
        let e = frobenius(&mul(&d, &repeated_frobenius(&d, 2)));
        let _f = square(&e);

        let x1f4 = g::mul(x[1], _f[4]);
        let x2f3 = g::mul(x[2], _f[3]);
        let x3f2 = g::mul(x[3], _f[2]);
        let x4f1 = g::mul(x[4], _f[1]);
        let added = g::add(g::add(x1f4, x2f3), g::add(x3f2, x4f1));
        let muld = g::mul(three, added);
        let x0f0 = g::mul(x[0], _f[0]);
        let _g = g::add(x0f0, muld);

        let s = g::sqrt(_g)?;
        let e_inv = inverse_or_zero(&e);
        Some(mul(&from_f(s), &e_inv))
    }

    pub fn sgn0(x: &Element) -> bool {
        let mut sign = false;
        let mut zero = true;
        for limb in x {
            let sign_i = (limb.canonical() & 1) == 0;
            let zero_i = limb.is_zero();
            sign = sign || (zero && sign_i);
            zero = zero && zero_i;
        }
        sign
    }

    pub fn canonical_sqrt(x: &Element) -> Option<Element> {
        let sqrt_x = sqrt(x)?;
        if sgn0(&sqrt_x) {
            Some(neg(&sqrt_x))
        } else {
            Some(sqrt_x)
        }
    }
}

// ───────────────────────────── Poseidon2 (plonky2) ─────────────────────────────
//
// width=12, rate=8, D=7, RF=8 (half 4), RP=22. 외부/내부 라운드 상수와 대각 MDS는
// `poseidon_crypto/hash/poseidon2_goldilocks_plonky2/config.go`에서 그대로 가져왔다.
// 외부 선형층은 Go처럼 96비트 누산(u128) 후 Reduce96Bit로 환원한다.

pub(crate) mod poseidon2 {
    use super::gfp5;
    use super::goldilocks::{self as g, F, EPSILON};

    pub const WIDTH: usize = 12;
    pub const RATE: usize = 8;
    pub const ROUNDS_F: usize = 8;
    pub const ROUNDS_F_HALF: usize = 4;
    pub const ROUNDS_P: usize = 22;

    // 외부 라운드 상수 [ROUNDS_F][WIDTH].
    pub const EXTERNAL_CONSTANTS: [[u64; WIDTH]; ROUNDS_F] = [
        [
            15492826721047263190, 11728330187201910315, 8836021247773420868, 16777404051263952451,
            5510875212538051896, 6173089941271892285, 2927757366422211339, 10340958981325008808,
            8541987352684552425, 9739599543776434497, 15073950188101532019, 12084856431752384512,
        ],
        [
            4584713381960671270, 8807052963476652830, 54136601502601741, 4872702333905478703,
            5551030319979516287, 12889366755535460989, 16329242193178844328, 412018088475211848,
            10505784623379650541, 9758812378619434837, 7421979329386275117, 375240370024755551,
        ],
        [
            3331431125640721931, 15684937309956309981, 578521833432107983, 14379242000670861838,
            17922409828154900976, 8153494278429192257, 15904673920630731971, 11217863998460634216,
            3301540195510742136, 9937973023749922003, 3059102938155026419, 1895288289490976132,
        ],
        [
            5580912693628927540, 10064804080494788323, 9582481583369602410, 10186259561546797986,
            247426333829703916, 13193193905461376067, 6386232593701758044, 17954717245501896472,
            1531720443376282699, 2455761864255501970, 11234429217864304495, 4746959618548874102,
        ],
        [
            13571697342473846203, 17477857865056504753, 15963032953523553760, 16033593225279635898,
            14252634232868282405, 8219748254835277737, 7459165569491914711, 15855939513193752003,
            16788866461340278896, 7102224659693946577, 3024718005636976471, 13695468978618890430,
        ],
        [
            8214202050877825436, 2670727992739346204, 16259532062589659211, 11869922396257088411,
            3179482916972760137, 13525476046633427808, 3217337278042947412, 14494689598654046340,
            15837379330312175383, 8029037639801151344, 2153456285263517937, 8301106462311849241,
        ],
        [
            13294194396455217955, 17394768489610594315, 12847609130464867455, 14015739446356528640,
            5879251655839607853, 9747000124977436185, 8950393546890284269, 10765765936405694368,
            14695323910334139959, 16366254691123000864, 15292774414889043182, 10910394433429313384,
        ],
        [
            17253424460214596184, 3442854447664030446, 3005570425335613727, 10859158614900201063,
            9763230642109343539, 6647722546511515039, 909012944955815706, 18101204076790399111,
            11588128829349125809, 15863878496612806566, 5201119062417750399, 176665553780565743,
        ],
    ];

    // 내부 라운드 상수 [ROUNDS_P].
    pub const INTERNAL_CONSTANTS: [u64; ROUNDS_P] = [
        11921381764981422944, 10318423381711320787, 8291411502347000766, 229948027109387563,
        9152521390190983261, 7129306032690285515, 15395989607365232011, 8641397269074305925,
        17256848792241043600, 6046475228902245682, 12041608676381094092, 12785542378683951657,
        14546032085337914034, 3304199118235116851, 16499627707072547655, 10386478025625759321,
        13475579315436919170, 16042710511297532028, 1411266850385657080, 9024840976168649958,
        14047056970978379368, 838728605080212101,
    ];

    // 대각 MDS (Plonky3). config.go MATRIX_DIAG_12_U64.
    pub const MATRIX_DIAG_12_U64: [u64; WIDTH] = [
        0xc3b6c08e23ba9300, 0xd84b5de94a324fb6, 0x0d0c371c5b35b84f, 0x7964f570e7188037,
        0x5daf18bbd996604b, 0x6743bc47b9595257, 0x5528b9362c59bb70, 0xac45e25b7127b68b,
        0xa2077d7dfbb606b5, 0xf3faac6faee378ae, 0x0c6388b51545e883, 0xd27dbb6944917b60,
    ];

    /// 96비트 값을 Goldilocks로 환원 (Go Reduce96Bit). x_hi는 32비트 이내 가정.
    ///
    /// Go 원본은 `GoldilocksField(resWrapped) + GoldilocksField(carry*EPSILON)`이며,
    /// 여기서 `+`는 Go의 native u64 덧셈(타입 `GoldilocksField=uint64`, 연산자 오버로딩
    /// 없음)이다. 즉 비정규 표현을 반환하는 **wrapping** 합이다 — `AddF`가 아니다.
    #[inline]
    fn reduce_96bit(hi: u64, lo: u64) -> F {
        let t1 = hi.wrapping_mul(EPSILON);
        let (res_wrapped, carry) = lo.overflowing_add(t1);
        F(res_wrapped.wrapping_add((carry as u64) * EPSILON))
    }

    /// 외부 MDS 선형층 (Go externalLinearLayer + 128). u128 누산.
    fn external_linear_layer(s: &mut [F; WIDTH]) {
        let mut s128: [u128; WIDTH] = [0u128; WIDTH];
        for i in 0..WIDTH {
            s128[i] = s[i].0 as u128;
        }
        external_linear_layer_128(&mut s128);
        for i in 0..WIDTH {
            let hi = (s128[i] >> 64) as u64;
            let lo = s128[i] as u64;
            s[i] = reduce_96bit(hi, lo);
        }
    }

    fn external_linear_layer_128(s: &mut [u128; WIDTH]) {
        for chunk in 0..3 {
            let b = chunk * 4;
            let (x0, x1, x2, x3) = (s[b], s[b + 1], s[b + 2], s[b + 3]);
            let t01 = x0 + x1;
            let t23 = x2 + x3;
            let t0123 = t01 + t23;
            s[b] = t0123 + t01 + x1;
            s[b + 1] = t0123 + x1 + (x2 + x2);
            s[b + 2] = t0123 + t23 + x3;
            s[b + 3] = t0123 + x3 + (x0 + x0);
        }
        let mut sums = [0u128; 4];
        for j in 0..4 {
            sums[j] = s[j] + s[j + 4] + s[j + 8];
        }
        for j in 0..4 {
            s[j] += sums[j];
            s[j + 4] += sums[j];
            s[j + 8] += sums[j];
        }
    }

    fn internal_linear_layer(state: &mut [F; WIDTH]) {
        // Go: sum = Σ state[i] (각 AsUInt128 = {Hi:0, Lo:limb} 누산) → Reduce96Bit.
        // 12개의 비정규 u64(<2^64) 합은 최대 ~2^68 이므로 u128 누산 후 hi는 ≤16비트다.
        let mut sum: u128 = 0;
        for i in 0..WIDTH {
            sum += state[i].0 as u128;
        }
        let sum_f = reduce_96bit((sum >> 64) as u64, sum as u64);
        // state[i] = MulAccF(sumF, state[i], DIAG[i]) = sumF + state[i]*DIAG[i].
        for i in 0..WIDTH {
            state[i] = g::mul_acc(sum_f, state[i], F(MATRIX_DIAG_12_U64[i]));
        }
    }

    fn add_rc(state: &mut [F; WIDTH], round: usize) {
        for i in 0..WIDTH {
            state[i] = g::add_canonical_u64(state[i], EXTERNAL_CONSTANTS[round][i]);
        }
    }

    fn add_rci(state: &mut [F; WIDTH], round: usize) {
        state[0] = g::add_canonical_u64(state[0], INTERNAL_CONSTANTS[round]);
    }

    /// 전체 sbox: x^7 (12개).
    fn sbox(state: &mut [F; WIDTH]) {
        for i in 0..WIDTH {
            state[i] = sbox_p(state[i]);
        }
    }

    #[inline]
    fn sbox_p(x: F) -> F {
        let p2 = g::square(x); // x^2
        let p4 = g::square(p2); // x^4
        let p3 = g::mul(x, p2); // x^3
        g::mul(p3, p4) // x^7
    }

    fn full_rounds(state: &mut [F; WIDTH], start: usize) {
        for r in start..start + ROUNDS_F_HALF {
            add_rc(state, r);
            sbox(state);
            external_linear_layer(state);
        }
    }

    fn partial_rounds(state: &mut [F; WIDTH]) {
        for r in 0..ROUNDS_P {
            add_rci(state, r);
            state[0] = sbox_p(state[0]);
            internal_linear_layer(state);
        }
    }

    pub fn permute(state: &mut [F; WIDTH]) {
        external_linear_layer(state);
        full_rounds(state, 0);
        partial_rounds(state);
        full_rounds(state, ROUNDS_F_HALF);
    }

    /// 패딩 없는 스폰지 흡수→짜내기 (Go HashNToMNoPad).
    pub fn hash_n_to_m_no_pad(input: &[F], num_outputs: usize) -> Vec<F> {
        let mut perm = [F(0); WIDTH];
        let mut i = 0;
        while i < input.len() {
            let mut j = 0;
            while j < RATE && i + j < input.len() {
                perm[j] = input[i + j];
                j += 1;
            }
            permute(&mut perm);
            i += RATE;
        }
        let mut outputs = Vec::with_capacity(num_outputs);
        loop {
            for k in 0..RATE {
                outputs.push(perm[k]);
                if outputs.len() == num_outputs {
                    return outputs;
                }
            }
            permute(&mut perm);
        }
    }

    /// 메시지 필드 원소들 → GFp5 원소 (Go HashToQuinticExtension).
    pub fn hash_to_quintic_extension(m: &[F]) -> gfp5::Element {
        let h = hash_n_to_m_no_pad(m, 5);
        [h[0], h[1], h[2], h[3], h[4]]
    }
}

// ───────────────────────── ECgFp5 scalar field ─────────────────────────
//
// 군 위수 n ≈ 2^319 (소수). 스칼라는 5×u64 (정규 표현). 곱셈은 Montgomery.
// Go `curve/ecgfp5/scalar_field.go`와 동일.

pub(crate) mod scalar {
    /// ECgFp5 스칼라 (5×u64, little-endian limbs).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub struct Scalar(pub [u64; 5]);

    // 군 위수 n. Go N.
    pub const N: [u64; 5] = [
        0xE80FD996948BFFE1,
        0xE8885C39D724A09C,
        0x7FFFFFE6CFB80639,
        0x7FFFFFF100000016,
        0x7FFFFFFD80000007,
    ];
    // -1/N[0] mod 2^64.
    const N0I: u64 = 0xD78BEF72057B7BDF;
    // 2^640 mod n.
    const R2: [u64; 5] = [
        0xA01001DCE33DC739,
        0x6C3228D33F62ACCF,
        0xD1D796CC91CF8525,
        0xAADFFF5D1574C1D8,
        0x4ACA13B28CA251F5,
    ];

    pub const ZERO: Scalar = Scalar([0, 0, 0, 0, 0]);

    impl Scalar {
        /// 작은 값에서 생성 (limb0만).
        pub fn from_u64(v: u64) -> Scalar {
            Scalar([v, 0, 0, 0, 0])
        }

        pub fn to_le_bytes(self) -> [u8; 40] {
            let mut out = [0u8; 40];
            for i in 0..5 {
                out[i * 8..i * 8 + 8].copy_from_slice(&self.0[i].to_le_bytes());
            }
            out
        }

        /// little-endian 40바이트 → Scalar. n 이상이면 mod n으로 환원 (Go
        /// ScalarElementFromLittleEndianBytes).
        pub fn from_le_bytes(b: &[u8]) -> Scalar {
            let mut v = [0u64; 5];
            for i in 0..5 {
                let mut a = [0u8; 8];
                a.copy_from_slice(&b[i * 8..i * 8 + 8]);
                v[i] = u64::from_le_bytes(a);
            }
            let s = Scalar(v);
            if s.is_canonical() {
                s
            } else {
                s.reduce_mod_n()
            }
        }

        pub fn is_canonical(self) -> bool {
            // self < N ?  (320비트 비교)
            cmp_lt(&self.0, &N)
        }

        pub fn equals(self, rhs: Scalar) -> bool {
            self.0 == rhs.0
        }

        pub fn is_zero(self) -> bool {
            self.0 == [0u64; 5]
        }

        /// raw 덧셈 (환원 없음). 캐리는 버림 (Go AddInner는 320비트로 truncate).
        fn add_inner(self, a: Scalar) -> Scalar {
            let mut r = [0u64; 5];
            let mut c = 0u128;
            for i in 0..5 {
                let z = self.0[i] as u128 + a.0[i] as u128 + c;
                r[i] = z as u64;
                c = z >> 64;
            }
            Scalar(r)
        }

        /// raw 뺄셈. borrow를 함께 반환 (0 또는 0xFFFF...) (Go SubInner).
        fn sub_inner(self, a: Scalar) -> (Scalar, u64) {
            let mut r = [0u64; 5];
            let mut borrow = 0i128;
            for i in 0..5 {
                let z = self.0[i] as i128 - a.0[i] as i128 - borrow;
                r[i] = z as u64;
                borrow = if z < 0 { 1 } else { 0 };
            }
            if borrow != 0 {
                (Scalar(r), 0xFFFF_FFFF_FFFF_FFFF)
            } else {
                (Scalar(r), 0)
            }
        }

        pub fn add(self, rhs: Scalar) -> Scalar {
            debug_assert!(self.is_canonical() && rhs.is_canonical());
            let r0 = self.add_inner(rhs);
            let (r1, c) = r0.sub_inner(Scalar(N));
            select(c, r1, r0)
        }

        pub fn sub(self, rhs: Scalar) -> Scalar {
            debug_assert!(self.is_canonical() && rhs.is_canonical());
            let (r0, c) = self.sub_inner(rhs);
            let r1 = r0.add_inner(Scalar(N));
            select(c, r0, r1)
        }

        /// Montgomery 곱: (self*rhs)/2^320 mod n. self < n 가정 (Go MontyMul).
        fn monty_mul(self, rhs: Scalar) -> Scalar {
            debug_assert!(self.is_canonical());
            let mut r = [0u64; 5];
            for i in 0..5 {
                let m = rhs.0[i];
                let f = (self.0[0].wrapping_mul(m).wrapping_add(r[0])).wrapping_mul(N0I);

                let mut cc1: u64 = 0;
                let mut cc2: u64 = 0;
                for j in 0..5 {
                    // z = self[j]*m + r[j] + cc1
                    let z = (self.0[j] as u128) * (m as u128) + (r[j] as u128) + (cc1 as u128);
                    cc1 = (z >> 64) as u64;
                    let z_lo = z as u64;
                    // z = f*N[j] + z_lo + cc2
                    let z2 = (f as u128) * (N[j] as u128) + (z_lo as u128) + (cc2 as u128);
                    cc2 = (z2 >> 64) as u64;
                    if j > 0 {
                        r[j - 1] = z2 as u64;
                    }
                }
                r[4] = cc1.wrapping_add(cc2);
            }
            let r_s = Scalar(r);
            let (r2, c) = r_s.sub_inner(Scalar(N));
            select(c, r2, r_s)
        }

        /// 모듈러 곱 (Go Mul). self, rhs < n.
        pub fn mul(self, rhs: Scalar) -> Scalar {
            debug_assert!(self.is_canonical() && rhs.is_canonical());
            self.monty_mul(Scalar(R2)).monty_mul(rhs)
        }

        /// 비정규 320비트 값을 mod n으로 환원 (스칼라가 N 이상일 때).
        fn reduce_mod_n(self) -> Scalar {
            // 320비트 self를 mod n으로. self < 2*... 보장 없음 — 일반 환원 필요.
            // big-int 없이: self는 [0, 2^320). n ≈ 2^319 이므로 최대 한두 번의 빼기로 부족.
            // 안전하게 반복 빼기(최대 ~2회지만 일반화) — 입력이 무작위 키이므로 충분.
            let mut s = self;
            // self가 N의 2배 이상일 수 있으므로 충분히 반복.
            while !s.is_canonical() {
                let (r, c) = s.sub_inner(Scalar(N));
                if c != 0 {
                    // 언더플로우 — 더 뺄 수 없음 (이론상 도달 안 함).
                    break;
                }
                s = r;
            }
            s
        }

        /// 4비트 limb 80개로 분해 (Go SplitTo4BitLimbs). MulAdd2 verify에 쓴다.
        pub fn split_to_4bit_limbs(self) -> [u8; 80] {
            let mut result = [0u8; 80];
            for i in 0..5 {
                for j in 0..16 {
                    result[i * 16 + j] = ((self.0[i] >> (j * 4)) & 0xF) as u8;
                }
            }
            result
        }

        /// 부호있는 w-bit 윈도우로 재부호화 (Go RecodeSigned → RecodeSignedFromLimbs).
        pub fn recode_signed(self, ss: &mut [i32], w: i32) {
            recode_signed_from_limbs(&self.0, ss, w);
        }
    }

    /// self의 GFp5 표현으로부터 스칼라 생성 (Go FromGfp5): 5개 정규 limb을
    /// little-endian 320비트 정수로 본 뒤 mod n.
    pub fn from_gfp5(fp5: &super::gfp5::Element) -> Scalar {
        let limbs = [
            fp5[0].canonical(),
            fp5[1].canonical(),
            fp5[2].canonical(),
            fp5[3].canonical(),
            fp5[4].canonical(),
        ];
        let s = Scalar(limbs);
        if s.is_canonical() {
            s
        } else {
            s.reduce_mod_n()
        }
    }

    /// c==0 → a0, c==0xFFFF... → a1 (상수시간 선택, Go Select).
    fn select(c: u64, a0: Scalar, a1: Scalar) -> Scalar {
        let mut r = [0u64; 5];
        for i in 0..5 {
            r[i] = a0.0[i] ^ (c & (a0.0[i] ^ a1.0[i]));
        }
        Scalar(r)
    }

    /// a < b ? (320비트 little-endian limbs).
    fn cmp_lt(a: &[u64; 5], b: &[u64; 5]) -> bool {
        for i in (0..5).rev() {
            if a[i] < b[i] {
                return true;
            }
            if a[i] > b[i] {
                return false;
            }
        }
        false
    }

    /// Go RecodeSignedFromLimbs. limbs는 little-endian u64들. ss는 출력 부호있는 자리.
    pub fn recode_signed_from_limbs(limbs: &[u64], ss: &mut [i32], w: i32) {
        let mut acc: u64 = 0;
        let mut acc_len: i32 = 0;
        let mut j: usize = 0;
        let mw: u32 = (1u32 << w) - 1;
        let hw: u32 = 1u32 << (w - 1);
        let mut cc: u32 = 0;
        for i in 0..ss.len() {
            let bb: u32;
            if acc_len < w {
                if j < limbs.len() {
                    let nl = limbs[j];
                    j += 1;
                    bb = ((acc | (nl << acc_len)) as u32) & mw;
                    acc = nl >> (w - acc_len);
                } else {
                    bb = (acc as u32) & mw;
                    acc = 0;
                }
                acc_len += 64 - w;
            } else {
                bb = (acc as u32) & mw;
                acc_len -= w;
                acc >>= w;
            }
            let bb = bb.wrapping_add(cc);
            cc = (hw.wrapping_sub(bb)) >> 31;
            ss[i] = (bb as i32) - ((cc << w) as i32);
        }
    }
}

// ───────────────────────── ECgFp5 curve ─────────────────────────
//
// GFp5 위의 소수위 타원곡선 (pornin/ecgfp5). 내부 점은 (x,z,u,t) 분수 좌표.
// 공개키 = G·sk 의 인코딩(GFp5 원소 1개). 검증은 Weierstrass 좌표 MulAdd2.
// Go `curve/ecgfp5/{point,weierstrass_point,affine_point}.go`와 동일.

pub(crate) mod curve {
    use super::gfp5::{self, Element};
    use super::goldilocks::F;
    use super::scalar::Scalar;

    const B1: u64 = 263;
    // a = 2.
    fn a_ecgfp5() -> Element {
        [F(2), F(0), F(0), F(0), F(0)]
    }
    fn b_ecgfp5() -> Element {
        [F(0), F(B1), F(0), F(0), F(0)]
    }
    fn b_mul2() -> Element {
        [F(0), F(2 * B1), F(0), F(0), F(0)]
    }
    fn b_mul4() -> Element {
        [F(0), F(4 * B1), F(0), F(0), F(0)]
    }
    fn b_mul16() -> Element {
        [F(0), F(16 * B1), F(0), F(0), F(0)]
    }

    /// (x,z,u,t) 분수 좌표 곡선 점.
    #[derive(Clone, Copy, Debug)]
    pub struct Point {
        pub x: Element,
        pub z: Element,
        pub u: Element,
        pub t: Element,
    }

    pub fn neutral() -> Point {
        Point { x: gfp5::ZERO, z: gfp5::ONE, u: gfp5::ZERO, t: gfp5::ONE }
    }

    /// 생성원 G (Go GENERATOR_ECgFp5Point).
    pub fn generator() -> Point {
        Point {
            x: [
                F(12883135586176881569),
                F(4356519642755055268),
                F(5248930565894896907),
                F(2165973894480315022),
                F(2448410071095648785),
            ],
            z: gfp5::ONE,
            u: gfp5::ONE,
            t: [F(4), F(0), F(0), F(0), F(0)],
        }
    }

    impl Point {
        /// 점을 GFp5 원소로 인코딩 (Go Encode): t/u.
        pub fn encode(&self) -> Element {
            gfp5::mul(&self.t, &gfp5::inverse_or_zero(&self.u))
        }

        /// 완전 덧셈 공식 (Go Add). 10M.
        pub fn add(&self, rhs: &Point) -> Point {
            let x1 = self.x;
            let z1 = self.z;
            let u1 = self.u;
            let _t1 = self.t;
            let x2 = rhs.x;
            let z2 = rhs.z;
            let u2 = rhs.u;
            let _t2 = rhs.t;

            let t1 = gfp5::mul(&x1, &x2);
            let t2 = gfp5::mul(&z1, &z2);
            let t3 = gfp5::mul(&u1, &u2);
            let t4 = gfp5::mul(&_t1, &_t2);
            let t5 = gfp5::sub(
                &gfp5::mul(&gfp5::add(&x1, &z1), &gfp5::add(&x2, &z2)),
                &gfp5::add(&t1, &t2),
            );
            let t6 = gfp5::sub(
                &gfp5::mul(&gfp5::add(&u1, &_t1), &gfp5::add(&u2, &_t2)),
                &gfp5::add(&t3, &t4),
            );
            let t7 = gfp5::add(&t1, &gfp5::mul(&t2, &b_ecgfp5()));
            let t8 = gfp5::mul(&t4, &t7);
            let t9 = gfp5::mul(
                &t3,
                &gfp5::add(&gfp5::mul(&t5, &b_mul2()), &gfp5::double(&t7)),
            );
            let t10 = gfp5::mul(
                &gfp5::add(&t4, &gfp5::double(&t3)),
                &gfp5::add(&t5, &t7),
            );

            let x_new = gfp5::mul(&gfp5::sub(&t10, &t8), &b_ecgfp5());
            let z_new = gfp5::sub(&t8, &t9);
            let u_new = gfp5::mul(&t6, &gfp5::sub(&gfp5::mul(&t2, &b_ecgfp5()), &t1));
            let t_new = gfp5::add(&t8, &t9);

            Point { x: x_new, z: z_new, u: u_new, t: t_new }
        }

        /// 2배 (Go SetDouble). 4M+5S.
        pub fn double(&self) -> Point {
            let mut p = *self;
            p.set_double();
            p
        }

        fn set_double(&mut self) {
            let x = self.x;
            let z = self.z;
            let u = self.u;
            let t = self.t;

            let t1 = gfp5::mul(&z, &t);
            let t2 = gfp5::mul(&t1, &t);
            let x1 = gfp5::square(&t2);
            let z1 = gfp5::mul(&t1, &u);
            let t3 = gfp5::square(&u);
            let w1 = gfp5::sub(&t2, &gfp5::mul(&t3, &gfp5::double(&gfp5::add(&x, &z))));
            let t4 = gfp5::square(&z1);

            let x_new = gfp5::mul(&t4, &b_mul4());
            let z_new = gfp5::square(&w1);
            let u_new = gfp5::sub(
                &gfp5::square(&gfp5::add(&w1, &z1)),
                &gfp5::add(&t4, &z_new),
            );
            let t_new = gfp5::sub(
                &gfp5::double(&x1),
                &gfp5::add(&gfp5::mul(&t4, &[F(4), F(0), F(0), F(0), F(0)]), &z_new),
            );

            self.x = x_new;
            self.z = z_new;
            self.u = u_new;
            self.t = t_new;
        }

        /// n번 연속 2배 (Go SetMDouble). 윈도우 곱셈의 핵심.
        fn set_mdouble(&mut self, n: u32) {
            if n == 0 {
                return;
            }
            if n == 1 {
                self.set_double();
                return;
            }
            let four = [F(4), F(0), F(0), F(0), F(0)];

            let x0 = self.x;
            let z0 = self.z;
            let u0 = self.u;
            let t0 = self.t;

            let t1 = gfp5::mul(&z0, &t0);
            let t2 = gfp5::mul(&t1, &t0);
            let x1 = gfp5::square(&t2);
            let z1 = gfp5::mul(&t1, &u0);
            let t3 = gfp5::square(&u0);
            let w1 = gfp5::sub(&t2, &gfp5::mul(&gfp5::double(&gfp5::add(&x0, &z0)), &t3));
            let t4 = gfp5::square(&w1);
            let t5 = gfp5::square(&z1);
            let mut x = gfp5::mul(&gfp5::square(&t5), &b_mul16());
            let mut w = gfp5::sub(
                &gfp5::double(&x1),
                &gfp5::add(&gfp5::mul(&t5, &four), &t4),
            );
            let mut z = gfp5::sub(
                &gfp5::square(&gfp5::add(&w1, &z1)),
                &gfp5::add(&t4, &t5),
            );

            // i = 2..n: 중간 2배 (Go for i:=2; i<n).
            let mut i = 2u32;
            while i < n {
                let tt1 = gfp5::square(&z);
                let tt2 = gfp5::square(&tt1);
                let tt3 = gfp5::square(&w);
                let tt4 = gfp5::square(&tt3);
                let tt5 = gfp5::sub(
                    &gfp5::square(&gfp5::add(&w, &z)),
                    &gfp5::add(&tt1, &tt3),
                );
                z = gfp5::mul(
                    &tt5,
                    &gfp5::sub(&gfp5::double(&gfp5::add(&x, &tt1)), &tt3),
                );
                x = gfp5::mul(&gfp5::mul(&tt2, &tt4), &b_mul16());
                w = gfp5::neg(&gfp5::add(
                    &tt4,
                    &gfp5::mul(&tt2, &gfp5::sub(&b_mul4(), &four)),
                ));
                i += 1;
            }

            let t1f = gfp5::square(&w);
            let t2f = gfp5::square(&z);
            let t3f = gfp5::sub(
                &gfp5::square(&gfp5::add(&w, &z)),
                &gfp5::add(&t1f, &t2f),
            );
            let w1f = gfp5::sub(&t1f, &gfp5::double(&gfp5::add(&x, &t2f)));

            self.x = gfp5::mul(&gfp5::square(&t3f), &b_ecgfp5());
            self.z = gfp5::square(&w1f);
            self.u = gfp5::mul(&t3f, &w1f);
            self.t = gfp5::sub(
                &gfp5::mul(&gfp5::double(&t1f), &gfp5::sub(&t1f, &gfp5::double(&t2f))),
                &self.z,
            );
        }

        /// affine 점을 더함 (Go AddAffine). 8M.
        fn add_affine(&self, rhs: &AffinePoint) -> Point {
            let x1 = self.x;
            let z1 = self.z;
            let u1 = self.u;
            let _t1 = self.t;
            let x2 = rhs.x;
            let u2 = rhs.u;

            let t1 = gfp5::mul(&x1, &x2);
            let t2 = z1;
            let t3 = gfp5::mul(&u1, &u2);
            let t4 = _t1;
            let t5 = gfp5::add(&x1, &gfp5::mul(&x2, &z1));
            let t6 = gfp5::add(&u1, &gfp5::mul(&u2, &_t1));
            let t7 = gfp5::add(&t1, &gfp5::mul(&t2, &b_ecgfp5()));
            let t8 = gfp5::mul(&t4, &t7);
            let t9 = gfp5::mul(&t3, &gfp5::add(&gfp5::mul(&t5, &b_mul2()), &gfp5::double(&t7)));
            let t10 = gfp5::mul(&gfp5::add(&t4, &gfp5::double(&t3)), &gfp5::add(&t5, &t7));

            Point {
                x: gfp5::mul(&gfp5::sub(&t10, &t8), &b_ecgfp5()),
                u: gfp5::mul(&t6, &gfp5::sub(&gfp5::mul(&t2, &b_ecgfp5()), &t1)),
                z: gfp5::sub(&t8, &t9),
                t: gfp5::add(&t8, &t9),
            }
        }

        /// 스칼라 곱 G·s 등 (Go Mul). 5비트 윈도우.
        pub fn mul(&self, s: Scalar) -> Point {
            let win = self.make_window_affine();
            let n_digits = (319 + WINDOW) / WINDOW;
            let mut digits = vec![0i32; n_digits as usize];
            s.recode_signed(&mut digits, WINDOW as i32);

            let mut p = lookup_vartime(&win, digits[digits.len() - 1]).to_point();
            for i in (0..digits.len() - 1).rev() {
                p.set_mdouble(WINDOW);
                let lk = lookup(&win, digits[i]);
                p = p.add_affine(&lk);
            }
            p
        }

        fn make_window_affine(&self) -> Vec<AffinePoint> {
            let mut tmp = vec![neutral(); WIN_SIZE];
            tmp[0] = *self;
            for i in 1..WIN_SIZE {
                if i & 1 == 0 {
                    tmp[i] = tmp[i - 1].add(self);
                } else {
                    tmp[i] = tmp[i >> 1].double();
                }
            }
            batch_to_affine(&tmp)
        }
    }

    const WINDOW: u32 = 5;
    const WIN_SIZE: usize = 1 << (WINDOW - 1);

    /// affine (x,u) 좌표 점.
    #[derive(Clone, Copy, Debug)]
    pub struct AffinePoint {
        x: Element,
        u: Element,
    }

    fn affine_neutral() -> AffinePoint {
        AffinePoint { x: gfp5::ZERO, u: gfp5::ZERO }
    }

    impl AffinePoint {
        fn to_point(self) -> Point {
            Point { x: self.x, z: gfp5::ONE, u: self.u, t: gfp5::ONE }
        }
        fn set_neg(&mut self) {
            self.u = gfp5::neg(&self.u);
        }
    }

    /// Montgomery 트릭으로 일괄 affine 변환 (Go BatchToAffine).
    fn batch_to_affine(src: &[Point]) -> Vec<AffinePoint> {
        let n = src.len();
        if n == 0 {
            return Vec::new();
        }
        if n == 1 {
            let p = src[0];
            let m1 = gfp5::inverse_or_zero(&gfp5::mul(&p.z, &p.t));
            return vec![AffinePoint {
                x: gfp5::mul(&gfp5::mul(&p.x, &p.t), &m1),
                u: gfp5::mul(&gfp5::mul(&p.u, &p.z), &m1),
            }];
        }

        let mut res = vec![affine_neutral(); n];
        let mut m = gfp5::mul(&src[0].z, &src[0].t);
        for i in 1..n {
            let x = m;
            m = gfp5::mul(&m, &src[i].z);
            let u = m;
            m = gfp5::mul(&m, &src[i].t);
            res[i] = AffinePoint { x, u };
        }
        m = gfp5::inverse_or_zero(&m);
        let mut i = n - 1;
        while i > 0 {
            res[i].u = gfp5::mul(&gfp5::mul(&src[i].u, &res[i].u), &m);
            m = gfp5::mul(&m, &src[i].t);
            res[i].x = gfp5::mul(&gfp5::mul(&src[i].x, &res[i].x), &m);
            m = gfp5::mul(&m, &src[i].z);
            i -= 1;
        }
        res[0].u = gfp5::mul(&gfp5::mul(&src[0].u, &src[0].z), &m);
        m = gfp5::mul(&m, &src[0].t);
        res[0].x = gfp5::mul(&src[0].x, &m);
        res
    }

    /// 상수시간 윈도우 lookup (Go Lookup). k ∈ [-n, n], 반환 k*P.
    fn lookup(win: &[AffinePoint], k: i32) -> AffinePoint {
        // sign = 0xFFFFFFFF if k<0 else 0 (Go: uint32(k>>31)).
        let sign32 = (k >> 31) as u32;
        let ka = ((k as u32) ^ sign32).wrapping_sub(sign32);
        let km1 = ka.wrapping_sub(1);

        let mut x = gfp5::ZERO;
        let mut u = gfp5::ZERO;
        for i in 0..win.len() {
            let m = km1.wrapping_sub(i as u32);
            let c_1 = (m | (!m).wrapping_add(1)) >> 31;
            let c = (c_1 as u64).wrapping_sub(1);
            if c != 0 {
                x = win[i].x;
                u = win[i].u;
            }
        }
        let sign = sign32 as u64;
        let c = sign | (sign << 32);
        if c != 0 {
            u = gfp5::neg(&u);
        }
        AffinePoint { x, u }
    }

    /// 가변시간 lookup (Go LookupVarTime). MSB 자리는 비밀이 아니므로 사용 가능.
    fn lookup_vartime(win: &[AffinePoint], k: i32) -> AffinePoint {
        if k == 0 {
            affine_neutral()
        } else if k > 0 {
            win[(k - 1) as usize]
        } else {
            let mut res = win[(-k - 1) as usize];
            res.set_neg();
            res
        }
    }

    // ── Weierstrass 좌표 (검증 경로) ──

    /// Weierstrass (X,Y) 점.
    #[derive(Clone, Copy, Debug)]
    pub struct WeierstrassPoint {
        pub x: Element,
        pub y: Element,
        pub is_inf: bool,
    }

    pub fn generator_weierstrass() -> WeierstrassPoint {
        WeierstrassPoint {
            x: [
                F(11712523173042564207),
                F(14090224426659529053),
                F(13197813503519687414),
                F(16280770174934269299),
                F(15998333998318935536),
            ],
            y: [
                F(14639054205878357578),
                F(17426078571020221072),
                F(2548978194165003307),
                F(8663895577921260088),
                F(9793640284382595140),
            ],
            is_inf: false,
        }
    }

    fn a_weierstrass() -> Element {
        [F(6148914689804861439), F(263), F(0), F(0), F(0)]
    }
    fn neutral_weierstrass() -> WeierstrassPoint {
        WeierstrassPoint { x: gfp5::ZERO, y: gfp5::ZERO, is_inf: true }
    }

    impl WeierstrassPoint {
        pub fn encode(&self) -> Element {
            gfp5::div(
                &self.y,
                &gfp5::sub(&gfp5::div(&a_ecgfp5(), &gfp5::from_u64(3)), &self.x),
            )
        }

        pub fn add(&self, q: &WeierstrassPoint) -> WeierstrassPoint {
            if self.is_inf {
                return *q;
            }
            if q.is_inf {
                return *self;
            }
            let (x1, y1) = (self.x, self.y);
            let (x2, y2) = (q.x, q.y);
            let x_same = gfp5::equals(&x1, &x2);
            let y_diff = !gfp5::equals(&y1, &y2);

            let (lambda0, lambda1) = if x_same {
                (
                    gfp5::add(&gfp5::triple(&gfp5::square(&x1)), &a_weierstrass()),
                    gfp5::double(&y1),
                )
            } else {
                (gfp5::sub(&y2, &y1), gfp5::sub(&x2, &x1))
            };
            let lambda = gfp5::div(&lambda0, &lambda1);
            let x3 = gfp5::sub(&gfp5::sub(&gfp5::square(&lambda), &x1), &x2);
            let y3 = gfp5::sub(&gfp5::mul(&lambda, &gfp5::sub(&x1, &x3)), &y1);
            WeierstrassPoint { x: x3, y: y3, is_inf: x_same && y_diff }
        }

        pub fn double(&self) -> WeierstrassPoint {
            if self.is_inf {
                return *self;
            }
            let x = self.x;
            let y = self.y;
            let mut lambda0 = gfp5::square(&x);
            lambda0 = gfp5::triple(&lambda0);
            lambda0 = gfp5::add(&lambda0, &a_weierstrass());
            let lambda1 = gfp5::double(&y);
            let lambda = gfp5::div(&lambda0, &lambda1);
            let mut x2 = gfp5::square(&lambda);
            let two_x = gfp5::double(&x);
            x2 = gfp5::sub(&x2, &two_x);
            let mut y2 = gfp5::sub(&x, &x2);
            y2 = gfp5::mul(&lambda, &y2);
            y2 = gfp5::sub(&y2, &y);
            WeierstrassPoint { x: x2, y: y2, is_inf: false }
        }

        fn precompute_window(&self, window_bits: u32) -> Vec<WeierstrassPoint> {
            let mut mult = vec![neutral_weierstrass(), *self, self.double()];
            for _ in 3..(1usize << window_bits) {
                let last = *mult.last().unwrap();
                mult.push(self.add(&last));
            }
            mult
        }
    }

    /// GFp5 원소를 Weierstrass 점으로 디코드 (Go DecodeFp5AsWeierstrass).
    pub fn decode_fp5_as_weierstrass(w: &Element) -> Option<WeierstrassPoint> {
        let e = gfp5::sub(&gfp5::square(w), &a_ecgfp5());
        let delta = gfp5::sub(&gfp5::square(&e), &b_mul4());
        let (r, success) = match gfp5::canonical_sqrt(&delta) {
            Some(r) => (r, true),
            None => (gfp5::ZERO, false),
        };

        let x1 = gfp5::div(&gfp5::add(&e, &r), &gfp5::TWO);
        let x2 = gfp5::div(&gfp5::sub(&e, &r), &gfp5::TWO);
        let mut x = x2;
        if gfp5::legendre(&x1).canonical() == 1 {
            x = x1;
        }
        let y = gfp5::neg(&gfp5::mul(w, &x));
        if success {
            x = gfp5::add(&x, &gfp5::div(&a_ecgfp5(), &gfp5::from_u64(3)));
        } else {
            x = gfp5::ZERO;
        }
        let is_inf = !success;
        if success || gfp5::is_zero(w) {
            Some(WeierstrassPoint { x, y, is_inf })
        } else {
            None
        }
    }

    /// scalarA*A + scalarB*B (Go MulAdd2). 4비트 윈도우, 검증식 s*G + e*pk.
    pub fn mul_add2(
        a: &WeierstrassPoint,
        b: &WeierstrassPoint,
        scalar_a: Scalar,
        scalar_b: Scalar,
    ) -> WeierstrassPoint {
        let a_window = a.precompute_window(4);
        let a_limbs = scalar_a.split_to_4bit_limbs();
        let b_window = b.precompute_window(4);
        let b_limbs = scalar_b.split_to_4bit_limbs();
        let num = a_limbs.len();

        let mut res = a_window[a_limbs[num - 1] as usize]
            .add(&b_window[b_limbs[num - 1] as usize]);
        for i in (0..num - 1).rev() {
            for _ in 0..4 {
                res = res.double();
            }
            let term = a_window[a_limbs[i] as usize].add(&b_window[b_limbs[i] as usize]);
            res = res.add(&term);
        }
        res
    }
}

// ───────────────────────── Schnorr over ECgFp5 ─────────────────────────
//
// e = H(r ‖ H(m)), s = k − e·sk, r = (k·G).encode(). 서명 = (s ‖ e) little-endian 80바이트.
// Go `signature/schnorr/schnorr.go`와 동일.

pub(crate) mod schnorr {
    use super::curve;
    use super::gfp5::Element;
    use super::goldilocks::F;
    use super::poseidon2;
    use super::scalar::Scalar;

    /// Schnorr 서명 (S, E 스칼라).
    #[derive(Clone, Copy, Debug)]
    pub struct Signature {
        pub s: Scalar,
        pub e: Scalar,
    }

    impl Signature {
        /// (s little-endian) ‖ (e little-endian) = 80바이트.
        #[allow(clippy::wrong_self_convention)]
        pub fn to_bytes(&self) -> [u8; 80] {
            let mut out = [0u8; 80];
            out[..40].copy_from_slice(&self.s.to_le_bytes());
            out[40..].copy_from_slice(&self.e.to_le_bytes());
            out
        }
    }

    /// 공개키 = (G·sk).encode() (Go SchnorrPkFromSk).
    pub fn pk_from_sk(sk: Scalar) -> Element {
        curve::generator().mul(sk).encode()
    }

    /// 고정 nonce k로 결정적 서명 (Go SchnorrSignHashedMessage2). 테스트·검증용.
    pub fn sign_hashed_message_with_k(hashed_msg: &Element, sk: Scalar, k: Scalar) -> Signature {
        let r = curve::generator().mul(k).encode();
        let e = compute_challenge(&r, hashed_msg);
        Signature { s: k.sub(e.mul(sk)), e }
    }

    /// 무작위 nonce k로 서명 (Go SchnorrSignHashedMessage). 런타임 경로.
    pub fn sign_hashed_message(hashed_msg: &Element, sk: Scalar, k: Scalar) -> Signature {
        // k 생성은 호출부가 CSPRNG로 담당(여기선 결정적 코어만).
        sign_hashed_message_with_k(hashed_msg, sk, k)
    }

    /// e = FromGfp5(H(r ‖ H(m))).
    fn compute_challenge(r: &Element, hashed_msg: &Element) -> Scalar {
        let mut preimage = [F(0); 10];
        preimage[..5].copy_from_slice(&r[..]);
        preimage[5..].copy_from_slice(&hashed_msg[..]);
        let h = poseidon2::hash_to_quintic_extension(&preimage);
        super::scalar::from_gfp5(&h)
    }

    /// 서명 검증 s·G + e·pk =?= r 후 e 재계산 (Go IsSchnorrSignatureValid).
    pub fn is_valid(pubkey: &Element, hashed_msg: &Element, sig: &Signature) -> bool {
        if !sig.e.is_canonical() || !sig.s.is_canonical() {
            return false;
        }
        let Some(pubkey_ws) = curve::decode_fp5_as_weierstrass(pubkey) else {
            return false;
        };
        let rv = curve::mul_add2(&curve::generator_weierstrass(), &pubkey_ws, sig.s, sig.e)
            .encode();
        let ev = compute_challenge(&rv, hashed_msg);
        ev.equals(sig.e)
    }
}

// ───────────────────────── tx 메시지 해시 + tx_info 빌더 ─────────────────────────
//
// create_order/cancel_order 메시지 해시는 Go `create_order.go`·`cancel_order.go`의 Hash와
// 동일한 필드·순서로 Poseidon2 HashToQuinticExtension을 돌린다. attributes(SkipNonce 등)는
// AggregateTxHash로 합성한다.
//
// **UNVERIFIED — tx_info JSON 와이어 봉투:** Go `getTxInfo`는 `json.Marshal(txInfo)`이며
// exported 필드를 Go 필드명(PascalCase)으로 직렬화한다. 임베디드 `*OrderInfo`는 평탄화되고,
// `Sig []byte`는 Go json에서 **base64 문자열**로, `SignedHash`는 `json:"-"`로 제외된다.
// 임베디드 `L2TxAttributes`(맵)의 직렬화 형태와 서버가 기대하는 정확한 키 집합은 라이브
// 응답 없이는 확정 불가다. 따라서 [`build_create_order_tx_info`]는 **검증 보류**이며,
// `scripts/lighter_capture_vector.md`로 공식 SDK 출력과 1:1 대조해야 한다.

/// 트랜잭션 타입 코드 (constants.go).
pub mod tx_type {
    pub const CREATE_ORDER: u8 = 14;
    pub const CANCEL_ORDER: u8 = 15;
}

/// create_order 메시지 (정수 스케일 적용 후). 필드·타입은 Go OrderInfo/L2CreateOrderTxInfo.
#[derive(Clone, Debug)]
pub struct CreateOrderMsg {
    pub account_index: i64,
    pub api_key_index: u8,
    pub market_index: i16,
    pub client_order_index: i64,
    pub base_amount: i64,
    pub price: u32,
    pub is_ask: u8,
    pub order_type: u8,
    pub time_in_force: u8,
    pub reduce_only: u8,
    pub trigger_price: u32,
    pub order_expiry: i64,
    pub nonce: i64,
    pub expired_at: i64,
}

/// cancel_order 메시지.
#[derive(Clone, Debug)]
pub struct CancelOrderMsg {
    pub account_index: i64,
    pub api_key_index: u8,
    pub market_index: i16,
    pub index: i64,
    pub nonce: i64,
    pub expired_at: i64,
}

/// i64/u32/u8 등을 Goldilocks 필드 원소로 (Go `g.GoldilocksField(x)` — int64→uint64
/// 2의 보수 캐스팅과 동일). 음수는 wrapping.
#[inline]
fn gf(x: i64) -> goldilocks::F {
    goldilocks::F(x as u64)
}

/// attributes 없는 경우의 메시지 해시 = txHash의 little-endian 40바이트.
/// (SkipNonce 등 attributes가 필요하면 별도 합성 — 본 어댑터는 attributes 미사용.)
fn finalize_hash_no_attributes(tx_hash: &gfp5::Element) -> [u8; gfp5::BYTES] {
    gfp5::to_le_bytes(tx_hash)
}

impl CreateOrderMsg {
    /// 메시지 해시 (Go L2CreateOrderTxInfo.Hash). chain_id 포함, 필드 순서 고정.
    pub fn hash(&self, chain_id: u32) -> [u8; gfp5::BYTES] {
        let elems = [
            gf(chain_id as i64),
            gf(tx_type::CREATE_ORDER as i64),
            gf(self.nonce),
            gf(self.expired_at),
            gf(self.account_index),
            gf(self.api_key_index as i64),
            gf(self.market_index as i64),
            gf(self.client_order_index),
            gf(self.base_amount),
            gf(self.price as i64),
            gf(self.is_ask as i64),
            gf(self.order_type as i64),
            gf(self.time_in_force as i64),
            gf(self.reduce_only as i64),
            gf(self.trigger_price as i64),
            gf(self.order_expiry),
        ];
        let tx_hash = poseidon2::hash_to_quintic_extension(&elems);
        finalize_hash_no_attributes(&tx_hash)
    }
}

impl CancelOrderMsg {
    /// 메시지 해시 (Go L2CancelOrderTxInfo.Hash).
    pub fn hash(&self, chain_id: u32) -> [u8; gfp5::BYTES] {
        let elems = [
            gf(chain_id as i64),
            gf(tx_type::CANCEL_ORDER as i64),
            gf(self.nonce),
            gf(self.expired_at),
            gf(self.account_index),
            gf(self.api_key_index as i64),
            gf(self.market_index as i64),
            gf(self.index),
        ];
        let tx_hash = poseidon2::hash_to_quintic_extension(&elems);
        finalize_hash_no_attributes(&tx_hash)
    }
}

/// hex 개인키 문자열(0x 선택)을 40바이트 검사 후 ECgFp5 스칼라로 (Go NewKeyManager +
/// ScalarElementFromLittleEndianBytes). 40바이트가 아니면 Err.
pub fn parse_private_key(hex_str: &str) -> Result<scalar::Scalar, String> {
    let h = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(h).map_err(|e| format!("invalid hex private key: {e}"))?;
    if bytes.len() != 40 {
        return Err(format!(
            "invalid private key length: expected 40 bytes, got {}",
            bytes.len()
        ));
    }
    Ok(scalar::Scalar::from_le_bytes(&bytes))
}

/// **UNVERIFIED.** 해시된 메시지에 서명하고 (sig 80바이트, signed_hash) 반환.
///
/// `k`는 호출부가 CSPRNG로 생성한 무작위 nonce(40바이트 little-endian, n으로 환원).
/// 반환 전 [`schnorr::is_valid`]로 자기 검증한다(잘못된 서명 제출 방지). 그래도 전체
/// 스킴이 공식 픽스처로 검증되기 전까진 실자금 사용 금지.
pub fn sign_message(
    sk: scalar::Scalar,
    msg_hash_le: &[u8; gfp5::BYTES],
    k: scalar::Scalar,
) -> Result<([u8; 80], [u8; gfp5::BYTES]), String> {
    let hashed = gfp5::from_canonical_le_bytes(msg_hash_le)
        .map_err(|e| format!("msg hash not canonical: {e}"))?;
    let sig = schnorr::sign_hashed_message(&hashed, sk, k);
    let pk = schnorr::pk_from_sk(sk);
    if !schnorr::is_valid(&pk, &hashed, &sig) {
        return Err("self-verification failed — refusing to emit signature".into());
    }
    Ok((sig.to_bytes(), *msg_hash_le))
}

#[cfg(test)]
mod tests {
    use super::goldilocks::F;
    use super::scalar::Scalar;
    use super::*;

    // ── 핵심 앵커 1: Poseidon2 permute 결정적 벡터 ──
    // 출처: poseidon_crypto `hash/poseidon2_goldilocks_plonky2/poseidon2_test.go` TestPermute.
    #[test]
    fn poseidon2_permute_vector() {
        let mut inp: [F; 12] = [
            F(5417613058500526590),
            F(2481548824842427254),
            F(6473243198879784792),
            F(1720313757066167274),
            F(2806320291675974571),
            F(7407976414706455446),
            F(1105257841424046885),
            F(7613435757403328049),
            F(3376066686066811538),
            F(5888575799323675710),
            F(6689309723188675948),
            F(2468250420241012720),
        ];
        poseidon2::permute(&mut inp);
        let expected: [u64; 12] = [
            5364184781011389007,
            15309475861242939136,
            5983386513087443499,
            886942118604446276,
            14903657885227062600,
            7742650891575941298,
            1962182278500985790,
            10213480816595178755,
            3510799061817443836,
            4610029967627506430,
            7566382334276534836,
            2288460879362380348,
        ];
        for i in 0..12 {
            assert_eq!(inp[i].canonical(), expected[i], "permute limb {i}");
        }
    }

    // ── 핵심 앵커 2: HashNToMNoPad 결정적 벡터 ──
    // 출처: 동 파일 TestHashNToMNoPad (입력 12원소 → 출력 12원소).
    #[test]
    fn poseidon2_hash_n_to_m_vector() {
        let inp: [F; 12] = [
            F(2963773914414780088),
            F(8389525300242074234),
            F(3700959901615818008),
            F(6116199383751757212),
            F(3418607418699599889),
            F(8793277256263635044),
            F(448623437464918480),
            F(1857310021116627925),
            F(6145634616307237342),
            F(1548353948794474539),
            F(2318110128254703527),
            F(8347759953730634762),
        ];
        let res = poseidon2::hash_n_to_m_no_pad(&inp, 12);
        let expected: [u64; 12] = [
            3627923032009111551,
            1460752551327577353,
            1084214837491058067,
            1841622875286057462,
            3996252440506437984,
            1276718204392552803,
            8564515621134952155,
            9252927025993202701,
            1147435538714642916,
            16407277821156164797,
            11997661877740155273,
            12485021000320141292,
        ];
        for i in 0..12 {
            assert_eq!(res[i].canonical(), expected[i], "hash limb {i}");
        }
    }

    // ── 핵심 앵커 3: Schnorr 결정적 (sk, msg, k) → (S, E) ──
    // 출처: poseidon_crypto `signature/schnorr/schnorr_test.go`
    //       TestComparativeSchnorrSignAndVerify. SchnorrSignHashedMessage2 사용.
    #[test]
    fn schnorr_comparative_vector() {
        let sks: [[u64; 5]; 3] = [
            [12235002942052073545, 1175977464658719998, 8536934969147463310, 6524687619313720391, 2922072024880609112],
            [14609471659974493146, 15558617123161593410, 853367204868339037, 17594253198278631904, 368396584122947478],
            [846395111423676945, 1354180063821346280, 5751371120309175011, 4898038106472090654, 1076345918732914302],
        ];
        let hashed: [[u64; 5]; 3] = [
            [8398652514106806347, 11069112711939986896, 9732488227085561369, 18076754337204438535, 17155407358725346236],
            [14569490467507212064, 2707063505563578676, 7506743487465742335, 12569771346154554175, 4305083698940175790],
            [17529153479246803593, 1743712677205511695, 4834285972617397460, 5486672566342530358, 7254989001695704129],
        ];
        let ks: [[u64; 5]; 3] = [
            [5245666847777449560, 15178169970799106939, 4403065012435293749, 15306540389399388999, 8935555081913173844],
            [1980123857560067020, 10696795398834097509, 3211831869376171671, 6194822139276031840, 3482023782412490864],
            [10299597990997564957, 8547298489021408803, 12250978550108858722, 5282281975236198197, 5328603554431393061],
        ];
        let exp_s: [[u64; 5]; 3] = [
            [6950590877883398434, 17178336263794770543, 11012823478139181320, 16445091359523510936, 5882925226143600273],
            [15189311883262425203, 16924634885527914505, 11098200095411565797, 11441434601417451505, 2245797172600273048],
            [1747989245728027396, 18083435619737379521, 18276259610811995786, 15101757397705334408, 5007814817019340642],
        ];
        let exp_e: [[u64; 5]; 3] = [
            [4544744459434870309, 4180764085957612004, 3024669018778978615, 15433417688859446606, 6775027260348937828],
            [4905460437060282008, 9275377852059362729, 10383772785796962929, 6858067464918579610, 7078247668913970626],
            [4911725746357568132, 12205663641120664338, 16433506899074513700, 14763562571101437023, 2547950465160283358],
        ];

        for i in 0..3 {
            let sk = Scalar(sks[i]);
            let hashed_msg: gfp5::Element = [
                F(hashed[i][0]), F(hashed[i][1]), F(hashed[i][2]), F(hashed[i][3]), F(hashed[i][4]),
            ];
            let k = Scalar(ks[i]);
            let sig = schnorr::sign_hashed_message_with_k(&hashed_msg, sk, k);
            assert_eq!(sig.s.0, exp_s[i], "case {i} sig.S");
            assert_eq!(sig.e.0, exp_e[i], "case {i} sig.E");

            // 라운드트립 검증.
            let pk = schnorr::pk_from_sk(sk);
            assert!(schnorr::is_valid(&pk, &hashed_msg, &sig), "case {i} verify");
        }
    }

    // GFp5 곱셈 항등성 sanity (a*1 = a, a*0 = 0).
    #[test]
    fn gfp5_mul_identity() {
        let a: gfp5::Element = [F(7), F(11), F(13), F(17), F(19)];
        assert!(gfp5::equals(&gfp5::mul(&a, &gfp5::ONE), &a));
        assert!(gfp5::is_zero(&gfp5::mul(&a, &gfp5::ZERO)));
        // 역원 라운드트립.
        let inv = gfp5::inverse_or_zero(&a);
        assert!(gfp5::equals(&gfp5::mul(&a, &inv), &gfp5::ONE));
    }

    // 스칼라 add/sub 라운드트립.
    #[test]
    fn scalar_add_sub_roundtrip() {
        let a = Scalar([123456789, 987654321, 555, 0, 1]);
        let b = Scalar([42, 99, 7, 3, 0]);
        assert!(a.is_canonical() && b.is_canonical());
        let c = a.add(b);
        assert!(c.sub(b).equals(a));
    }

    // 개인키 파싱: 40바이트 아닌 입력 거부.
    #[test]
    fn private_key_length_enforced() {
        assert!(parse_private_key("0xdeadbeef").is_err());
        let forty = "00".repeat(40);
        assert!(parse_private_key(&forty).is_ok());
    }
}





