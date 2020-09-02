use crate::{biginteger::BigInteger, ModelParameters, PrimeField};
use std::ops::Neg;

// TODO: Make GLV override slower mul
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

    fn glv_scalar_decomposition_inner(
        k: <Self::ScalarField as PrimeField>::BigInt,
    ) -> (
        (bool, <Self::ScalarField as PrimeField>::BigInt),
        (bool, <Self::ScalarField as PrimeField>::BigInt),
    ) {
        let limbs = <Self::ScalarField as PrimeField>::BigInt::NUM_LIMBS;
        let modulus = Self::ScalarField::modulus();

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
            k2.add_nocarry(&modulus);
        } else if k2 > modulus {
            k2.sub_noborrow(&modulus);
        }

        let mut k1 = k;
        let borrow = k1.sub_noborrow(&(Self::ScalarField::from(k2) * &Self::LAMBDA).into_repr());
        if borrow {
            k1.add_nocarry(&modulus);
        }

        let (neg2, k2) = if k2.num_bits() > Self::R_BITS / 2 + 1 {
            (true, Self::ScalarField::from(k2).neg().into_repr())
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

// fn mul_glv(&self, ) {
//
// }

// fn batch_scalar_mul_in_place_glv(
//     w: usize,
//     points: &mut [Self],
//     scalars: &mut [<Self::Fr as PrimeField>::BigInt],
// ) {
//     assert_eq!(points.len(), scalars.len());
//     let batch_size = points.len();
//     let glv_scalars: Vec<(Self::SmallBigInt, Self::SmallBigInt)> = scalars
//         .iter()
//         .map(|&s| Self::glv_scalar_decomposition(s))
//         .collect();
//     let (mut k1, mut k2): (Vec<Self::SmallBigInt>, Vec<Self::SmallBigInt>) = (
//         glv_scalars.iter().map(|x| x.0).collect(),
//         glv_scalars.iter().map(|x| x.1).collect(),
//     );
//
//     let mut p2 = points.to_vec();
//     p2.iter_mut().for_each(|p| p.glv_endomorphism_in_place());
//
//     // THIS IS WRONG and does not achieve the savings hoped for
//     Self::batch_scalar_mul_in_place::<Self::SmallBigInt>(points, &mut k1[..], w);
//     Self::batch_scalar_mul_in_place::<Self::SmallBigInt>(&mut p2[..], &mut k2[..], w);
//     Self::batch_add_in_place(
//         points,
//         &mut p2,
//         &(0..batch_size)
//             .map(|x| (x, x))
//             .collect::<Vec<(usize, usize)>>()[..],
//     );
// }
