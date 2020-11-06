// TODO: make this more generic
#[macro_export]
macro_rules! impl_gpu_cpu_run_kernel {
    () =>  {
        fn clear_gpu_profiling_data() {
            #[cfg(feature = "cuda")]
            {
                let dir = dirs::cache_dir()
                    .unwrap()
                    .join("zexe-algebra")
                    .join("cuda-scalar-mul-profiler")
                    .join(P::namespace());
                std::fs::create_dir_all(&dir).expect("Could not create/get cache dir for profile data");
                std::fs::File::create(&dir.join("profile_data.txt")).expect("could not create profile_data.txt");
            }
        }

        /// We split up the job statically between the CPU and GPUs
        /// based on continuous profiling stored both in a static location in memory
        /// that is lost the moment the progam stops running.
        /// and also a txt file in the OS' cache dir.

        /// Only one such procedure should be running at any time.
        #[allow(unused_variables)]
        fn cpu_gpu_static_partition_run_kernel(
            bases_h: &mut [<Self as ProjectiveCurve>::Affine],
            exps_h: &[<<Self as ProjectiveCurve>::ScalarField as PrimeField>::BigInt],
            cuda_group_size: usize,
            // size of the batch for cpu scalar mul
            cpu_chunk_size: usize,
        ) {
            #[cfg(feature = "cuda")]
            {
                if !Device::init() {
                    panic!("Do not call this function unless the device has been checked to initialise successfully");
                }
                let n_devices = Device::get_count().unwrap();
                let n = bases_h.len();
                // Create references so we can split the slices
                let mut res_ref = &mut bases_h[..];
                let mut exps_h_ref = exps_h;

                let _now = timer!();
                // Get data for proportion of total throughput achieved by each device
                let dir = dirs::cache_dir()
                    .unwrap()
                    .join("zexe-algebra")
                    .join("cuda-scalar-mul-profiler")
                    .join(P::namespace());
                std::fs::create_dir_all(&dir).expect("Could not create/get cache dir for profile data");

                let arc_mutex = P::scalar_mul_static_profiler();
                let mut profile_data = arc_mutex.lock().unwrap();
                let mut proportions: Vec<f64> = profile_data.0.clone();

                // If the program has just been initialised, we must check for the existence of existing
                // cached profile data. If it does not exist, we create a new file
                if proportions.is_empty() {
                    let _ = std::fs::read_to_string(&dir.join("profile_data.txt"))
                        .and_then(|s| { let res = serde_json::from_str(&s)?; Ok(res) })
                        .and_then(|cached_data| {
                            *profile_data = cached_data;
                            proportions = profile_data.0.clone();
                            Ok(())
                        }
                    );
                }

                if proportions.is_empty() {
                    // By default we split the work evenly between devices and host
                    proportions = vec![1.0 / (n_devices as f64 + 1.0); n_devices];
                }
                timer_println!(_now, "prepare profiling");

                let _now = timer!();
                assert_eq!(proportions.len(), n_devices);
                // Allocate the number of elements in the job to each device/host
                let n_gpus = proportions.iter().map(|r| (r * n as f64).round() as usize).collect::<Vec<_>>();
                let n_cpu = n - n_gpus.iter().sum::<usize>();

                // Create storage for buffers and contexts for variable number of devices
                let mut bases_split = Vec::with_capacity(n_devices);
                let mut tables = Vec::with_capacity(n_devices);
                let mut exps = Vec::with_capacity(n_devices);
                let mut ctxs = Vec::with_capacity(n_devices);
                let (mut time_cpu, mut times_gpu) = (0, vec![0; n_devices]);

                // Split data and generate tables and u8 scalar encoding in device memory
                for (i, &num) in n_gpus.iter().enumerate() {
                    let device = Device::nth(i).unwrap();
                    let ctx = device.create_context();

                    let (lower, upper) = res_ref.split_at_mut(num);
                    res_ref = upper;
                    let lower_exps = &exps_h_ref[..num];
                    exps_h_ref = &exps_h_ref[num..];

                    let mut table = DeviceMemory::<Self>::zeros(&ctx, num * Self::table_size());
                    let mut exp = DeviceMemory::<u8>::zeros(&ctx, num * Self::num_u8());

                    Self::generate_tables_and_recoding(lower, &mut table[..], lower_exps, &mut exp[..]);

                    ctxs.push((device, ctx));
                    bases_split.push(lower);
                    tables.push(table);
                    exps.push(exp);
                };
                timer_println!(_now, "precomp and allocate on device");

                rayon::scope(|s| {
                    // Run jobs on GPUs
                    for (i, (bases_gpu, time_gpu)) in bases_split.iter_mut().zip(times_gpu.iter_mut()).enumerate() {
                        let n_gpu = n_gpus[i];
                        let ctx = &ctxs[i].1;
                        let table = &tables[i];
                        let exp = &exps[i];

                        s.spawn(move |_| {
                            let now = std::time::Instant::now();
                            let _now = timer!();

                            let mut out = DeviceMemory::<Self>::zeros(ctx, n_gpu);
                            P::scalar_mul_kernel(
                                ctx,
                                (n_gpu - 1) / cuda_group_size + 1, // grid
                                cuda_group_size,     // block
                                table.as_ptr(), exp.as_ptr(), out.as_mut_ptr(), n_gpu as isize
                            )
                            .expect("Kernel call failed");
                            Self::batch_normalization(&mut out[..]);
                            bases_gpu.clone_from_slice(&out.par_iter().map(|p| p.into_affine()).collect::<Vec<_>>()[..]);
                            *time_gpu = now.elapsed().as_micros();

                            timer_println!(_now, format!("gpu {} done", i));
                        });
                    }

                    // Run on CPU
                    s.spawn(|_| {
                        let now = std::time::Instant::now();
                        let _now = timer!();

                        let exps_mut = &mut exps_h_ref.to_vec()[..];
                        rayon::scope(|t| {
                            for (b, s) in res_ref.chunks_mut(cpu_chunk_size).zip(exps_mut.chunks_mut(cpu_chunk_size)) {
                                t.spawn(move |_| b[..].batch_scalar_mul_in_place(&mut s[..], 4));
                            }
                        });

                        time_cpu = now.elapsed().as_micros();
                        timer_println!(_now, "cpu done");
                    });
                });

                // Update global microbenchmarking state
                debug!("old profile_data: {:?}", profile_data);
                let cpu_throughput = n_cpu as f64 / time_cpu as f64;
                let gpu_throughputs = n_gpus
                    .iter()
                    .zip(times_gpu.iter())
                    .map(|(n_gpu, time_gpu)| {
                        *n_gpu as f64 / *time_gpu as f64
                })
                .collect::<Vec<_>>();
                let total_throughput = cpu_throughput + gpu_throughputs.iter().sum::<f64>();
                let n_data_points = profile_data.1 as f64;
                profile_data.1 += 1;
                let new_proportions = gpu_throughputs.iter().map(|t| t / total_throughput);

                if !profile_data.0.is_empty() {
                    profile_data.0 = new_proportions.zip(profile_data.0.clone()).map(|(new, old)| {
                        (new + n_data_points * old) / profile_data.1 as f64
                    }).collect();
                } else {
                    profile_data.0 = new_proportions.collect();
                }

                // Update cached profiling data on disk
                let _now = timer!();
                let mut file = std::fs::File::create(&dir.join("profile_data.txt")).expect("could not create profile_data.txt");
                let s: String = serde_json::to_string(&(*profile_data)).expect("could not convert profiling data to string");
                file.write_all(s.as_bytes()).expect("could not write profiling data to cache dir");
                file.sync_all().expect("could not sync profiling data to disc");
                timer_println!(_now, "write data");

                debug!("new profile_data: {:?}", profile_data);
            }
        }

        #[allow(unused_variables)]
        fn cpu_gpu_load_balance_run_kernel(
            ctx: &Context,
            bases_h: &[<Self as ProjectiveCurve>::Affine],
            exps_h: &[<<Self as ProjectiveCurve>::ScalarField as PrimeField>::BigInt],
            cuda_group_size: usize,
            // size of a single job in the queue e.g. 2 << 14
            job_size: usize,
            // size of the batch for cpu scalar mul
            cpu_chunk_size: usize,
        ) -> Vec<<Self as ProjectiveCurve>::Affine> {
            #[cfg(feature = "cuda")]
            {
                let mut bases_res = bases_h.to_vec();
                let queue = Mutex::new(bases_res.chunks_mut(job_size).zip(exps_h.chunks(job_size)).peekmore());

                rayon::scope(|s| {
                    // We launch two concurrent GPU threads that block on waiting for GPU to hide latency
                    for i in 0..2 {
                        s.spawn(closure!(move i, ref queue, |_| {
                            std::thread::sleep(std::time::Duration::from_millis(i * 500));
                            let mut iter = queue.lock().unwrap();
                            while let Some((bases, exps)) = iter.next() {
                                iter.peek();
                                if iter.peek().is_none() { break; }
                                let mut proj_res = Self::par_run_kernel_sync(ctx, bases, exps, cuda_group_size, iter);
                                Self::batch_normalization(&mut proj_res[..]);
                                bases.clone_from_slice(&proj_res.par_iter().map(|p| p.into_affine()).collect::<Vec<_>>()[..]);
                                iter = queue.lock().unwrap();
                            }
                        }));
                    }

                    s.spawn(|_| {
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        let mut iter = queue.lock().unwrap();
                        debug!("acquired cpu");
                        while let Some((bases, exps)) = iter.next() {
                            let exps_mut = &mut exps.to_vec()[..];
                            rayon::scope(|t| {
                                for (b, s) in bases.chunks_mut(cpu_chunk_size).zip(exps_mut.chunks_mut(cpu_chunk_size)) {
                                    t.spawn(move |_| b[..].batch_scalar_mul_in_place(&mut s[..], 4));
                                }
                            });
                            // Sleep to allow other threads to unlock
                            drop(iter);
                            debug!("unlocked cpu");
                            std::thread::sleep(std::time::Duration::from_millis(20));
                            iter = queue.lock().unwrap();
                            debug!("acquired cpu");
                        }
                        debug!("CPU FINISH");
                    });
                });
                drop(queue);
                bases_res
            }

            #[cfg(not(feature = "cuda"))]
            Vec::new()
        }
    }
}