macro_rules! batch_arith {
    () => {
        #[bench]
        fn bench_g1_batch_mul_affine(b: &mut ::test::Bencher) {
            const SAMPLES: usize = 10000;

            let mut rng = XorShiftRng::seed_from_u64(1231275789u64);

            let mut g: Vec<G1Affine> = (0..SAMPLES)
                .map(|_| G1::rand(&mut rng).into_affine())
                .collect();

            let s: Vec<FrRepr> = (0..SAMPLES)
                .map(|_| Fr::rand(&mut rng).into_repr())
                .collect();

            let now = std::time::Instant::now();
            println!("Start");
            b.iter(|| {
                g[..].batch_scalar_mul_in_place::<FrRepr>(&mut s.to_vec()[..], 4);
                println!("{:?}", now.elapsed().as_micros());
            });
        }

        #[bench]
        fn bench_g1_batch_mul_projective(b: &mut ::test::Bencher) {
            const SAMPLES: usize = 10000;

            let mut rng = XorShiftRng::seed_from_u64(1231275789u64);

            let mut g: Vec<G1> = (0..SAMPLES)
                .map(|_| G1::rand(&mut rng))
                .collect();

            let s: Vec<Fr> = (0..SAMPLES)
                .map(|_| Fr::rand(&mut rng))
                .collect();

            let now = std::time::Instant::now();
            b.iter(|| {
                g.iter_mut()
                    .zip(&s)
                    .map(|(p, sc)| p.mul_assign(*sc))
                    .collect::<()>();
                println!("{:?}", now.elapsed().as_micros());
            });
        }
    }
}
