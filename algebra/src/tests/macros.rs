macro_rules! std_curve_tests {
    ($CURVE_IDENT: ident, $GTField: ident) => {
        use algebra_core::{
            test_rng, AffineCurve, Field, One, PairingEngine, PrimeField, ProjectiveCurve,
            UniformRand,
        };
        use rand::Rng;

        use crate::tests::{curves::*, groups::*, msm::*};

        #[test]
        fn test_g1_projective_curve() {
            curve_tests::<G1Projective>();

            sw_tests::<g1::Parameters>();
        }

        #[test]
        fn test_g1_projective_group() {
            let mut rng = test_rng();
            let a: G1Projective = rng.gen();
            let b: G1Projective = rng.gen();
            group_test(a, b);
        }

        #[test]
        fn test_g1_generator() {
            let generator = G1Affine::prime_subgroup_generator();
            assert!(generator.is_on_curve());
            assert!(generator.is_in_correct_subgroup_assuming_on_curve());
        }

        #[test]
        fn test_g2_projective_curve() {
            curve_tests::<G2Projective>();

            sw_tests::<g2::Parameters>();
        }

        #[test]
        fn test_g2_projective_group() {
            let mut rng = test_rng();
            let a: G2Projective = rng.gen();
            let b: G2Projective = rng.gen();
            group_test(a, b);
        }

        #[test]
        fn test_g2_generator() {
            let generator = G2Affine::prime_subgroup_generator();
            assert!(generator.is_on_curve());
            assert!(generator.is_in_correct_subgroup_assuming_on_curve());
        }

        #[test]
        fn test_g1_msm() {
            test_msm::<G1Affine>();
        }

        #[test]
        fn test_g2_msm() {
            test_msm::<G2Affine>();
        }

        #[test]
        fn test_bilinearity() {
            let mut rng = test_rng();
            let a: G1Projective = rng.gen();
            let b: G2Projective = rng.gen();
            let s: Fr = rng.gen();

            let sa = a.mul(s);
            let sb = b.mul(s);

            let ans1 = $CURVE_IDENT::pairing(sa, b);
            let ans2 = $CURVE_IDENT::pairing(a, sb);
            let ans3 = $CURVE_IDENT::pairing(a, b).pow(s.into_repr());

            assert_eq!(ans1, ans2);
            assert_eq!(ans2, ans3);

            assert_ne!(ans1, $GTField::one());
            assert_ne!(ans2, $GTField::one());
            assert_ne!(ans3, $GTField::one());

            assert_eq!(ans1.pow(Fr::characteristic()), $GTField::one());
            assert_eq!(ans2.pow(Fr::characteristic()), $GTField::one());
            assert_eq!(ans3.pow(Fr::characteristic()), $GTField::one());
        }

        #[test]
        fn test_product_of_pairings() {
            let rng = &mut test_rng();

            let a = G1Projective::rand(rng).into_affine();
            let b = G2Projective::rand(rng).into_affine();
            let c = G1Projective::rand(rng).into_affine();
            let d = G2Projective::rand(rng).into_affine();
            let ans1 = $CURVE_IDENT::pairing(a, b) * &$CURVE_IDENT::pairing(c, d);
            let ans2 =
                $CURVE_IDENT::product_of_pairings(&[(a.into(), b.into()), (c.into(), d.into())]);
            assert_eq!(ans1, ans2);
        }
    };
}

macro_rules! edwards_curve_tests {
    () => {
        use algebra_core::{
            curves::{AffineCurve, ProjectiveCurve},
            test_rng,
        };
        use rand::Rng;

        use crate::tests::{curves::*, groups::*, msm::*};

        #[test]
        fn test_projective_curve() {
            curve_tests::<EdwardsProjective>();

            edwards_tests::<EdwardsParameters>();
        }

        #[test]
        fn test_projective_group() {
            let mut rng = test_rng();
            let a = rng.gen();
            let b = rng.gen();

            for _i in 0..100 {
                group_test::<EdwardsProjective>(a, b);
            }
        }

        #[test]
        fn test_affine_group() {
            let mut rng = test_rng();
            let a: EdwardsAffine = rng.gen();
            let b: EdwardsAffine = rng.gen();
            for _i in 0..100 {
                group_test::<EdwardsAffine>(a, b);
            }
        }

        #[test]
        fn test_affine_msm() {
            test_msm::<EdwardsAffine>();
        }

        #[test]
        fn test_generator() {
            let generator = EdwardsAffine::prime_subgroup_generator();
            assert!(generator.is_on_curve());
            assert!(generator.is_in_correct_subgroup_assuming_on_curve());
        }

        #[test]
        fn test_conversion() {
            let mut rng = test_rng();
            let a: EdwardsAffine = rng.gen();
            let b: EdwardsAffine = rng.gen();
            let a_b = {
                use crate::groups::Group;
                (a + &b).double().double()
            };
            let a_b2 = (a.into_projective() + &b.into_projective())
                .double()
                .double();
            assert_eq!(a_b, a_b2.into_affine());
            assert_eq!(a_b.into_projective(), a_b2);
        }

        #[test]
        fn test_montgomery_conversion() {
            montgomery_conversion_test::<EdwardsParameters>();
        }
    };
}
