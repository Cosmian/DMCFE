//! # Discrete Logarithm
//!
//! Description of the DLP

#![allow(non_snake_case)]
use bls12_381::{G1Projective, Scalar};
use dmcfe::dlp;
use eyre::Result;
use rand::Rng;

#[test]
fn test_dlp() -> Result<()> {
    // Do not use a big number, it will take useless time
    const M: u64 = 10 ^ 4;
    let m: u32 = (M as f64).sqrt() as u32 + 1;
    let x: u64 = rand::thread_rng().gen_range(0..M);

    // create the DLP
    let P: G1Projective = G1Projective::generator() * Scalar::from_raw([x, 0, 0, 0]);

    // solve it
    let res = dlp::bsgs(&P, m, m)?;

    eyre::ensure!(x == res, "Wrong DLP solution!");
    Ok(())
}
