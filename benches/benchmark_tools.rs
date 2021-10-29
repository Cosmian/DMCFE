use bls12_381::{pairing, G1Affine, G1Projective, G2Affine, Scalar};
use dmcfe::dlp;
use eyre::Result;
use rand::Rng;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

/// size of a SHA256 hash
const SHA256_SIZE: usize = 32;

/// Hash table used to store precomputed distinguished points.
type Table = HashMap<[u8; SHA256_SIZE], Scalar>;
type Jumps = Vec<Scalar>;

/// Get the time of a random DLP solving.
/// - `m`:      `x < m`
pub fn get_time_dlp(m: u64) -> Result<u128> {
    eyre::ensure!(
        m > (std::u32::MAX as u64) * (std::u32::MAX as u64),
        "Size of m is too big to be computable!"
    );

    // Get `x`, a random `u64` such that `x < m`
    let x: u64 = rand::thread_rng().gen_range(0..m);
    let n: u32 = (m as f64).sqrt() as u32 + 1; // conversion is OK

    // create the DLP
    let p: G1Projective = G1Projective::generator() * Scalar::from_raw([x, 0, 0, 0]);

    // solve it
    let timer = Instant::now();
    let res = dlp::bsgs::solve(&p, n, n)?;
    let timer = timer.elapsed();

    // check the result
    eyre::ensure!(x == res, "Wrong DLP solution: {} != {}", x, res);

    Ok(timer.as_millis())
}

fn get_precomputations(
    l: u64,
    t: usize,
    u: usize,
    k: usize,
    w: usize,
    n: usize,
) -> Result<(Jumps, Table)> {
    let table_filename = Path::new("benches").join("table");
    let jumps_filename = Path::new("benches").join("jumps");

    if table_filename.exists() && jumps_filename.exists() {
        Ok((
            dlp::kangaroo::read_jumps(&jumps_filename)?,
            dlp::kangaroo::read_table(&table_filename)?,
        ))
    } else {
        let jumps = dlp::kangaroo::gen_jumps(l, k)?;
        let table = dlp::kangaroo::gen_improved_table(l, t, w, u, n, &jumps)?;
        // write tables for futur uses
        dlp::kangaroo::write_jumps(&jumps_filename, &jumps)?;
        dlp::kangaroo::write_table(&table_filename, &table)?;
        Ok((jumps, table))
    }
}

/// used to generate table and jumps
pub fn gen() -> Result<()> {
    // Upper bound of the DLP intervalle
    const L: u64 = 2u64.pow(32);
    // Table size
    const T: usize = 2usize.pow(10);
    // number of tables to generate before selecting the best points
    const U: usize = 2usize.pow(4);
    // Number of random jumps
    // Chosen based on https://www.jstor.org/stable/2698783
    const K: usize = 16;
    // Alpha constant
    const ALPHA: f64 = 1f64 / 16f64;
    // number of threads to launch
    const N: usize = 2usize.pow(4);

    // Compute the walk size
    let w: usize = (ALPHA * (L as f64 / (T as f64)).sqrt()) as usize;

    println!("L: {}, T: {}, U: {}, K: {}, W: {}, N: {}", L, T, U, K, w, N);

    let (jumps, table) = get_precomputations(L, T, U, K, w, N)?;

    let h = Scalar::from_raw([rand::thread_rng().gen_range(1..L), 0, 0, 0]);

    eyre::ensure!(
        h == dlp::kangaroo::solve(
            &table,
            &jumps,
            &(pairing(&G1Affine::generator(), &G2Affine::generator()) * h),
            w,
            N,
        ),
        "Wrong DLP solution!"
    );
    println!("Success!");
    Ok(())
}
