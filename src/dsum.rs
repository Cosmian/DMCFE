use crate::tools;
use bls12_381::{G1Projective, Scalar};
use std::cmp::Ordering;

pub type CypherText = Scalar;
pub type PrivateKey = Scalar;
pub type PublicKey = G1Projective;
pub type KeyPair = (PrivateKey, PublicKey);

fn h(l: &[u8], ti: (usize, &Scalar), tj: (usize, &G1Projective)) -> Scalar {
    let (i, sk) = ti;
    let (j, pk) = tj;
    match j.cmp(&i) {
        Ordering::Less => Scalar::neg(&tools::hash_to_scalar(
            pk,
            &tools::smul_in_g1(sk),
            &(pk * sk),
            l,
        )),
        Ordering::Equal => Scalar::zero(),
        Ordering::Greater => tools::hash_to_scalar(&tools::smul_in_g1(sk), pk, &(pk * sk), l),
    }
}

/// Creates the private and public keys for a DSum client.
pub fn client_setup() -> KeyPair {
    let t = tools::random_scalar();
    (t, tools::smul_in_g1(&t))
}

/// Encrypt the given data using the given keys and label.
/// - `i`:      client id
/// - `x`:      data to encrypt
/// - `ski`:    client private key
/// - `pk`:     list of `(client, id)`, where `client` is the client `id` and `pki` is his public key
/// - `l`:      label
pub fn encode(
    i: usize,
    x: &Scalar,
    ski: &PrivateKey,
    pk: &[(usize, PublicKey)],
    l: &[u8],
) -> CypherText {
    pk.iter()
        .fold(*x, |acc, (j, pkj)| acc + h(l, (i, ski), (*j, pkj)))
}

/// Decrypt the given data.
/// - `c`:  list of all encrypted data
pub fn combine(c: &[CypherText]) -> Scalar {
    c.iter().sum()
}
