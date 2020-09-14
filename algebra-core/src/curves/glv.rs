use crate::{biginteger::BigInteger, ModelParameters, PrimeField};
use core::ops::Neg;

/// TODO: deal with the case where b1 and b2 have the same sign
pub trait GLVParameters: Send + Sync + 'static + ModelParameters {
    type WideBigInt: BigInteger;

    const LAMBDA: Self::ScalarField; // lambda in ZZ s.t. phi(P) = lambda*P for all P
    const OMEGA: Self::BaseField; // phi((x, y)) = (\omega x, y)
    const Q1: <Self::ScalarField as PrimeField>::BigInt; // round(R*|b2|/n)
    const Q2: <Self::ScalarField as PrimeField>::BigInt; // round(R*|b1|/n)
    const B1: <Self::ScalarField as PrimeField>::BigInt; // |b1|
    const B2: <Self::ScalarField as PrimeField>::BigInt; // |b2|
    const B1_IS_NEG: bool;
    const B2_IS_NEG: bool;
    const R_BITS: u32;

    #[inline]
    fn glv_scalar_decomposition_inner(
        k: <Self::ScalarField as PrimeField>::BigInt,
    ) -> (
        (bool, <Self::ScalarField as PrimeField>::BigInt),
        (bool, <Self::ScalarField as PrimeField>::BigInt),
    ) {
        let limbs = <Self::ScalarField as PrimeField>::BigInt::NUM_LIMBS;
        let modulus = Self::ScalarField::modulus();

        // If we are doing a subgroup check, we should multiply by the original scalar
        // since the GLV decomposition does not guarantee that we would not be
        // adding and subtracting back to zero
        if k == modulus {
            return (
                (false, k),
                (false, <Self::ScalarField as PrimeField>::BigInt::from(0)),
            );
        }

        let mut half = Self::WideBigInt::from(1);
        half.muln(Self::R_BITS - 1);

        let mut c1_wide = Self::WideBigInt::mul_no_reduce(k.as_ref(), Self::Q1.as_ref());
        // add half to achieve rounding rather than flooring
        c1_wide.add_nocarry(&half);
        // Approximation to round(|b2|*k/n)
        c1_wide.divn(Self::R_BITS);
        let c1 = &c1_wide.as_ref()[..limbs];

        let mut c2_wide = Self::WideBigInt::mul_no_reduce(k.as_ref(), Self::Q2.as_ref());
        c2_wide.add_nocarry(&half);
        c2_wide.divn(Self::R_BITS);
        let c2 = &c2_wide.as_ref()[..limbs];

        let d1 =
            <Self::ScalarField as PrimeField>::BigInt::mul_no_reduce_lo(&c1, Self::B1.as_ref());
        let d2 =
            <Self::ScalarField as PrimeField>::BigInt::mul_no_reduce_lo(&c2, Self::B2.as_ref());

        // We check if they have the same sign. If they do, we must do a subtraction. Else, we must do an
        // addition. Then, we will conditionally add or subtract the product of this with lambda from k.
        let mut k2 = if Self::B1_IS_NEG {
            d2.clone()
        } else {
            d1.clone()
        };
        let borrow = if Self::B1_IS_NEG {
            k2.sub_noborrow(&d1)
        } else {
            k2.sub_noborrow(&d2)
        };
        if borrow {
            while k2 >= modulus {
                k2.add_nocarry(&modulus);
            }
        } else {
            while k2 >= modulus {
                k2.sub_noborrow(&modulus);
            }
        }
        let k2_field = Self::ScalarField::from(k2);
        let k1 = (Self::ScalarField::from(k) - &(k2_field * &Self::LAMBDA)).into_repr();
        let (neg2, k2) = if k2.num_bits() > Self::R_BITS / 2 + 1 {
            (true, k2_field.neg().into_repr())
        } else {
            (false, k2)
        };

        let (neg1, k1) = if k1.num_bits() > Self::R_BITS / 2 + 1 {
            (true, Self::ScalarField::from(k1).neg().into_repr())
        } else {
            (false, k1)
        };

        ((neg1, k1), (neg2, k2))
    }
}