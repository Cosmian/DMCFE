use bls12_381::{G1Affine, G1Projective};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::ErrorKind;

use crate::interface;

const SHA256_SIZE: usize = 32;

/// This algorithm implements the precomputation step of the BSGS algorithms.
/// It returns a hashed map containing all the precomputed pairs.
///
/// - `m`:  number of pairs to precompute
///
/// See [the notes on DLP](crate::notes::dlp) for more explanations
fn precomputation(m: u32) -> HashMap<[u8; SHA256_SIZE], u32> {
    let G = G1Projective::generator();
    let mut pairs = HashMap::new();
    let mut P_i = G1Projective::identity();
    for i in 0..m {
        let mut hasher = Sha256::new();
        hasher.update(G1Affine::to_compressed(&G1Affine::from(P_i)));
        let res = hasher.finalize().into();
        let res = pairs.insert(res, i);
        if let Some(j) = res {
            panic!("Collision for {} and {}", i, j);
        }
        P_i = P_i + G;
    }
    pairs
}

/// This algorithm imlements the iteration fonction of the BSGS algorithm.
///
/// - `P`:      right term of the DLP
/// - `Q`:      inverse of `g^m`
/// - `n`:      the number of iterations
/// - `pairs`:  hash table containg the precomputed values
fn iterate(
    P: &G1Projective,
    Q: &G1Projective,
    n: u32,
    pairs: &mut HashMap<[u8; SHA256_SIZE], u32>,
) -> Option<(u32, u32)> {
    let mut res = None;
    let mut k = 0;
    let mut Q_k = G1Projective::identity();
    let mut P_k = Q_k + P;
    while k < n {
        let mut hasher = Sha256::new();
        hasher.update(G1Affine::to_compressed(&G1Affine::from(P_k)));
        let hash_res: [u8; SHA256_SIZE] = hasher.finalize().into();
        res = pairs.get(&hash_res);
        if res.is_some() {
            break;
        } else {
            k += 1;
            Q_k = Q_k + Q;
            P_k = Q_k + P;
        }
    }
    res.map(|value| (k, *value))
}

/// This algorithm implements the BSGS algorithm. It aims to find `x` such that
/// `x.g = p`, where `g` is the generator of `G1`, `p` is given and `x < M`
/// with `M = mn` is not too big.
///
/// `P`:  right member of the DLP equation
/// `m`:  integer such that `x < mn`
/// `n`:  integer such that `x < mn`
pub fn bsgs(P: &G1Projective, m: u32, n: u32) -> Result<u64, ErrorKind> {
    // define some heuristics
    // e.g. test the case where the solution is 1

    let mut pairs = precomputation(m);
    let Q = interface::get_inverse(&G1Projective::generator(), m as u64);

    let res = iterate(&P, &Q, n, &mut pairs);
    if let Some((k, i)) = res {
        Ok((k as u64) * (m as u64) + (i as u64))
    } else {
        Err(ErrorKind::NotFound)
    }
}
