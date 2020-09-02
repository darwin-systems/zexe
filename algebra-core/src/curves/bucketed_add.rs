use crate::{//cfg_iter_mut,
    curves::BatchGroupArithmeticSlice, log2, AffineCurve
};

use std::collections::HashMap;

// #[cfg(feature = "parallel")]
// use rayon::prelude::*;

const RATIO_MULTIPLIER: usize = 2;

const BATCH_ADD_SIZE: usize = 4096;

#[inline]
pub fn batch_bucketed_add<C: AffineCurve>(
    buckets: usize,
    elems: &mut [C],
    bucket_assign: &[usize],
) -> Vec<C> {
    let num_split = 2i32.pow(log2(buckets) / 2 + 2) as usize;
    let split_size = (buckets - 1) / num_split + 1;
    let mut bucket_split = vec![Vec::with_capacity(split_size); num_split];

    // Get the inverted index for the positions assigning to each buckets
    let now = std::time::Instant::now();

    for (position, &bucket) in bucket_assign.iter().enumerate() {
        if bucket < buckets {
            bucket_split[bucket / split_size].push((bucket as u32, position as u32));
        }
    }
    // println!("Splitting bucket: {:?}", now.elapsed().as_micros());

    let offset = ((elems.len() - 1) / buckets + 1) * RATIO_MULTIPLIER;
    let mut index = vec![0u32; offset * buckets];
    let mut assign_hash = HashMap::<usize, Vec<u32>>::new();

    for split in bucket_split {
        for (bucket, position) in split {
            let bucket = bucket as usize;
            let idx = bucket * offset;
            let n_assignments = index[idx] as usize;
            index[idx] += 1;
            // If we have run out of space for the fixed sized offsets, we add the assignments
            // to a dynamically sized vector stored in a hashmap
            if n_assignments >= offset - 1 {
                let assign_vec = assign_hash
                    .entry(bucket)
                    .or_insert(Vec::with_capacity(offset));
                if n_assignments == offset - 1 {
                    assign_vec.extend_from_slice(&index[idx + 1..idx + offset]);
                }
                assign_vec.push(position);
            } else {
                index[idx + n_assignments + 1] = position;
            }
        }
    }
    println!("Generate Inverted Index: {:?}", now.elapsed().as_micros());

    // Instructions for indexes for the in place addition tree
    let mut instr: Vec<Vec<(usize, usize)>> = vec![];
    // Find the maximum depth of the addition tree
    let max_depth = index
        .iter()
        .step_by(offset)
        .map(|x| log2(*x as usize))
        .max()
        .unwrap() as usize;

    let now = std::time::Instant::now();

    // for bucket in 0..buckets {
    //     for assign in 0..offset {
    //         print!("{:?},", index[bucket * offset + assign]);
    //     }
    //     println!("");
    // }
    // println!("---");
    // Generate in-place addition instructions that implement the addition tree
    // for each bucket from the leaves to the root
    for i in 0..max_depth {
        let mut instr_row = Vec::<(usize, usize)>::with_capacity(buckets);
        for bucket in 0..buckets {
            let idx = bucket * offset;
            let len = index[idx] as usize;

            if len > 1 << (max_depth - i - 1) {
                let new_len = (len - 1) / 2 + 1;
                // We must deal with vector
                if len > offset - 1 {
                    // println!("OVERFLOW: {}", len);
                    let assign_vec = assign_hash.entry(bucket).or_default();
                    if new_len <= offset - 1 {
                        for j in 0..len / 2 {
                            index[idx + j + 1] = assign_vec[2 * j];
                            instr_row
                                .push((assign_vec[2 * j] as usize, assign_vec[2 * j + 1] as usize));
                        }
                        if len % 2 == 1 {
                            index[idx + new_len] = assign_vec[len - 1];
                        }
                        // println!("{:?}", assign_vec);
                        assign_hash.remove(&bucket);
                    } else {
                        for j in 0..len / 2 {
                            assign_vec[j] = assign_vec[2 * j];
                            instr_row
                                .push((assign_vec[2 * j] as usize, assign_vec[2 * j + 1] as usize));
                        }
                        if len % 2 == 1 {
                            assign_vec[new_len - 1] = assign_vec[len - 1];
                        }
                    }
                } else {
                    for j in 0..len / 2 {
                        index[idx + j + 1] = index[idx + 2 * j + 1];
                        instr_row.push((
                            index[idx + 2 * j + 1] as usize,
                            index[idx + 2 * j + 2] as usize,
                        ));
                    }
                    if len % 2 == 1 {
                        index[idx + new_len] = index[idx + len];
                    }
                }
                // New length is the ceil of (old_length / 2)
                index[idx] = new_len as u32;
            }
        }
        if instr_row.len() > 0 {
            instr.push(instr_row);
        }

        // for bucket in 0..buckets {
        //     for assign in 0..offset {
        //         print!("{:?},", index[bucket * offset + assign]);
        //     }
        //     println!("");
        // }
        // println!("---");
    }
    // println!("offset: {}, max depth {}", offset, max_depth);
    // println!("{:?}", instr);
    println!("Generate Instr: {:?}", now.elapsed().as_micros());

    let now = std::time::Instant::now();
    // let mut elems_mut_1 = elems.to_vec();

    for instr_row in instr.iter() {
        for instr_chunk in
            C::get_chunked_instr::<(usize, usize)>(&instr_row[..], BATCH_ADD_SIZE).iter()
        {
            elems[..].batch_add_in_place_same_slice(&instr_chunk[..]);
        }
    }
    println!("Batch add in place: {:?}", now.elapsed().as_micros());

    let now = std::time::Instant::now();
    let zero = C::zero();
    let mut res = vec![zero; buckets];

    for bucket in 0..buckets {
        if index[offset * bucket] > 1 {
            panic!("Did not successfully reduce to_add");
        } else if index[offset * bucket] == 1 {
            res[bucket] = elems[index[offset * bucket + 1] as usize];
        }
    }

    println!("Reassign: {:?}", now.elapsed().as_micros());
    res
}

// We make the batch bucket add cache-oblivious by splitting the problem
// into sub problems recursively
pub fn batch_bucketed_add_split<C: AffineCurve>(
    buckets: usize,
    elems: &[C],
    bucket_assign: &[usize],
    bucket_size: usize,
) -> Vec<C> {
    let split_size = if buckets >= 1 << 26 {
        1 << 16
    } else {
        1 << bucket_size
    };
    let num_split = (buckets - 1) / split_size + 1;
    println!("{}, {}", split_size, num_split);
    let mut elem_split = vec![vec![]; num_split];
    let mut bucket_split = vec![vec![]; num_split];

    let now = std::time::Instant::now();

    let split_window = 1 << 5;
    let split_split = (num_split - 1) / split_window + 1;

    let mut res = vec![];
    for i in 0..split_split {
        let then = std::time::Instant::now();
        for (position, &bucket) in bucket_assign.iter().enumerate() {
            let split_index = bucket / split_size;
            // // Check the bucket assignment is valid
            if bucket < buckets
                && split_index >= i * split_window
                && split_index < (i + 1) * split_window
            {
                bucket_split[split_index].push(bucket % split_size);
                elem_split[split_index].push(elems[position]);
            }
        }

        // println!(
        //     "\nAssign bucket and elem split: {:?}",
        //     now.elapsed().as_micros()
        // );

        let now = std::time::Instant::now();

        for (elems, buckets) in elem_split[i * split_window..(i + 1) * split_window]
            .iter_mut()
            .zip(bucket_split[i * split_window..(i + 1) * split_window]
            .iter())
        {
            if elems.len() > 0 {
                 res.append(&mut batch_bucketed_add(split_size, &mut elems[..], &buckets[..]));
            }
        }
        // println!("{}: time: {}", i, then.elapsed().as_micros());
    }


    // let res = if split_size < 1 << (bucket_size + 1) {
    // let res = cfg_iter_mut!(elem_split)
    //     .zip(cfg_iter_mut!(bucket_split))
    //     .filter(|(e, b)| e.len() > 0)
    //     .map(|(elems, buckets)| batch_bucketed_add(split_size, &mut elems[..], &buckets[..]))
    //     .flatten()
        // .collect();
    // } else {
    //     // println!("CALLING RECURSIVE");
    //     elem_split
    //         .iter()
    //         .zip(bucket_split.iter())
    //         .map(|(elems, bucket)| {
    //             batch_bucketed_add_split(split_size, &elems[..], &bucket[..], bucket_size)
    //         })
    //         .flatten()
    //         .collect()
    // };

    // println!("Bucketed add: {:?}", now.elapsed().as_micros());
    res
}

pub fn batch_bucketed_add_old<C: AffineCurve>(
    buckets: usize,
    elems: &mut [C],
    bucket_assign: &[usize],
) -> Vec<C> {
    let num_split = 2i32.pow(log2(buckets) / 2 + 2) as usize;
    let split_size = (buckets - 1) / num_split + 1;
    let ratio = elems.len() / buckets * 2;
    // Get the inverted index for the positions assigning to each bucket
    let now = std::time::Instant::now();
    let mut bucket_split = vec![vec![]; num_split];
    let mut index = vec![Vec::with_capacity(ratio); buckets];

    // We use two levels of assignments to help with cache locality.
    // #[cfg(feature = "prefetch")]
    // let mut prefetch_iter = bucket_assign.iter();
    // #[cfg(feature = "prefetch")]
    // {
    //     // prefetch_iter.next();
    // }

    for (position, &bucket) in bucket_assign.iter().enumerate() {
        // #[cfg(feature = "prefetch")]
        // {
        //     if let Some(next) = prefetch_iter.next() {
        //         prefetch(&mut index[*next]);
        //     }
        // }
        // Check the bucket assignment is valid
        if bucket < buckets {
            // index[bucket].push(position);
            bucket_split[bucket / split_size].push((bucket, position));
        }
    }

    for split in bucket_split {
        for (bucket, position) in split {
            index[bucket].push(position);
        }
    }
    println!("\nGenerate Inverted Index: {:?}", now.elapsed().as_micros());

    // Instructions for indexes for the in place addition tree
    let mut instr: Vec<Vec<(usize, usize)>> = vec![];
    // Find the maximum depth of the addition tree
    let max_depth = index.iter()
        // log_2
        .map(|x| log2(x.len()))
        .max().unwrap();

    let now = std::time::Instant::now();
    // Generate in-place addition instructions that implement the addition tree
    // for each bucket from the leaves to the root
    for i in 0..max_depth {
        let mut instr_row = Vec::<(usize, usize)>::with_capacity(buckets);
        for to_add in index.iter_mut() {
            if to_add.len() > 1 << (max_depth - i - 1) {
                let mut new_to_add = vec![];
                for j in 0..(to_add.len() / 2) {
                    new_to_add.push(to_add[2 * j]);
                    instr_row.push((to_add[2 * j], to_add[2 * j + 1]));
                }
                if to_add.len() % 2 == 1 {
                    new_to_add.push(*to_add.last().unwrap());
                }
                *to_add = new_to_add;
            }
        }
        instr.push(instr_row);
    }
    println!("Generate Instr: {:?}", now.elapsed().as_micros());

    let now = std::time::Instant::now();
    // let mut elems_mut_1 = elems.to_vec();

    for instr_row in instr.iter() {
        for instr in C::get_chunked_instr::<(usize, usize)>(&instr_row[..], BATCH_ADD_SIZE).iter() {
            elems[..].batch_add_in_place_same_slice(&instr[..]);
        }
    }
    println!("Batch add in place: {:?}", now.elapsed().as_micros());

    let now = std::time::Instant::now();
    let zero = C::zero();
    let mut res = vec![zero; buckets];

    for (i, to_add) in index.iter().enumerate() {
        if to_add.len() > 1 {
            panic!("Did not successfully reduce to_add");
        } else if to_add.len() == 1 {
            res[i] = elems[to_add[0]];
        }
    }

    println!("Reassign: {:?}", now.elapsed().as_micros());
    res
}
