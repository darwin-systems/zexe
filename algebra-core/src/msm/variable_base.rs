use crate::{
    batch_bucketed_add,
    prelude::{AffineCurve, BigInteger, FpParameters, One, PrimeField, ProjectiveCurve, Zero},
    BucketPosition, Vec,
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub struct VariableBaseMSM;

impl VariableBaseMSM {
    fn msm_inner<G: AffineCurve>(
        bases: &[G],
        scalars: &[<G::ScalarField as PrimeField>::BigInt],
    ) -> G::Projective
    where
        G::Projective: ProjectiveCurve<Affine = G>,
    {
        let c = if scalars.len() < 32 {
            3
        } else {
            super::ln_without_floats(scalars.len()) + 2
        };

        let num_bits = <G::ScalarField as PrimeField>::Params::MODULUS_BITS as usize;
        let fr_one = G::ScalarField::one().into_repr();

        let zero = G::Projective::zero();
        let window_starts: Vec<_> = (0..num_bits).step_by(c).collect();

        #[cfg(feature = "parallel")]
        let window_starts_iter = window_starts.into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let window_starts_iter = window_starts.into_iter();

        // Each window is of size `c`.
        // We divide up the bits 0..num_bits into windows of size `c`, and
        // in parallel process each such window.
        let window_sums: Vec<_> = window_starts_iter
            .map(|w_start| {
                let mut res = zero;
                // We don't need the "zero" bucket, so we only have 2^c - 1 buckets
                let log2_n_bucket = if (w_start % c) != 0 { w_start % c } else { c };
                let mut buckets = vec![zero; (1 << log2_n_bucket) - 1];

                scalars
                    .iter()
                    .zip(bases)
                    .filter(|(s, _)| !s.is_zero())
                    .for_each(|(&scalar, base)| {
                        if scalar == fr_one {
                            // We only process unit scalars once in the first window.
                            if w_start == 0 {
                                res.add_assign_mixed(base);
                            }
                        } else {
                            let mut scalar = scalar;

                            // We right-shift by w_start, thus getting rid of the
                            // lower bits.
                            scalar.divn(w_start as u32);

                            // We mod the remaining bits by the window size.
                            let scalar = scalar.as_ref()[0] % (1 << c);

                            // If the scalar is non-zero, we update the corresponding
                            // bucket.
                            // (Recall that `buckets` doesn't have a zero bucket.)
                            if scalar != 0 {
                                buckets[(scalar - 1) as usize].add_assign_mixed(base);
                            }
                        }
                    });
                let buckets = G::Projective::batch_normalization_into_affine(&buckets);

                let mut running_sum = G::Projective::zero();
                for b in buckets.into_iter().rev() {
                    running_sum.add_assign_mixed(&b);
                    res += &running_sum;
                }

                (res, log2_n_bucket)
            })
            .collect();

        // We store the sum for the lowest window.
        let lowest = window_sums.first().unwrap().0;

        // We're traversing windows from high to low.
        lowest
            + &window_sums[1..].iter().rev().fold(
                zero,
                |total: G::Projective, (sum_i, window_size): &(G::Projective, usize)| {
                    let mut total = total + sum_i;
                    for _ in 0..*window_size {
                        total.double_in_place();
                    }
                    total
                },
            )
    }

    pub fn multi_scalar_mul<G: AffineCurve>(
        bases: &[G],
        scalars: &[<G::ScalarField as PrimeField>::BigInt],
    ) -> G::Projective {
        Self::msm_inner(bases, scalars)
    }

    pub fn multi_scalar_mul_batched<G: AffineCurve, BigInt: BigInteger>(
        bases: &[G],
        scalars: &[BigInt],
        num_bits: usize,
    ) -> G::Projective {
        let c = if scalars.len() < 32 {
            1
        } else {
            super::ln_without_floats(scalars.len()) + 2
        };

        let zero = G::Projective::zero();
        let window_starts: Vec<_> = (0..num_bits).step_by(c).collect();

        #[cfg(feature = "parallel")]
        let window_starts_iter = window_starts.into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let window_starts_iter = window_starts.into_iter();

        // Each window is of size `c`.
        // We divide up the bits 0..num_bits into windows of size `c`, and
        // in parallel process each such window.
        let window_sums: Vec<_> = window_starts_iter
            .map(|w_start| {
                // We don't need the "zero" bucket, so we only have 2^c - 1 buckets
                let log2_n_bucket = if (w_start % c) != 0 { w_start % c } else { c };
                let n_buckets = (1 << log2_n_bucket) - 1;

                let _now = timer!();
                let mut bucket_positions: Vec<_> = scalars
                    .iter()
                    .enumerate()
                    .map(|(pos, &scalar)| {
                        let mut scalar = scalar;

                        // We right-shift by w_start, thus getting rid of the
                        // lower bits.
                        scalar.divn(w_start as u32);

                        // We mod the remaining bits by the window size.
                        let res = (scalar.as_ref()[0] % (1 << c)) as i32;
                        BucketPosition {
                            bucket: (res - 1) as u32,
                            position: pos as u32,
                        }
                    })
                    .collect();
                timer_println!(_now, "scalars->buckets");

                let _now = timer!();
                let buckets =
                    batch_bucketed_add::<G>(n_buckets, &bases[..], &mut bucket_positions[..]);
                timer_println!(_now, "bucket add");

                let _now = timer!();
                let mut res = zero;
                let mut running_sum = G::Projective::zero();
                for b in buckets.into_iter().rev() {
                    running_sum.add_assign_mixed(&b);
                    res += &running_sum;
                }
                timer_println!(_now, "accumulating sums");
                (res, log2_n_bucket)
            })
            .collect();

        // We store the sum for the lowest window.
        let lowest = window_sums.first().unwrap().0;

        // We're traversing windows from high to low.
        lowest
            + &window_sums[1..].iter().rev().fold(
                zero,
                |total: G::Projective, (sum_i, window_size): &(G::Projective, usize)| {
                    let mut total = total + sum_i;
                    for _ in 0..*window_size {
                        total.double_in_place();
                    }
                    total
                },
            )
    }
}
