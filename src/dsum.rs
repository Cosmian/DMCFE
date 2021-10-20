use crate::{label::Label, tools};
use bls12_381::{G1Projective, Scalar};
use std::ops::Deref;

#[derive(Clone, Copy)]
pub struct CypherText(Scalar);

#[derive(Clone, Copy)]
pub struct PrivateKey(Scalar);

impl Deref for PrivateKey {
    type Target = Scalar;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy)]
pub struct PublicKey(G1Projective);

impl Deref for PublicKey {
    type Target = G1Projective;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy)]
pub struct KeyPair(pub PrivateKey, pub PublicKey);

/// Creates the private and public keys for a DSum client.
pub fn client_setup() -> KeyPair {
    let t = tools::random_scalar();
    KeyPair(PrivateKey(t), PublicKey(tools::smul_in_g1(&t)))
}

/// Encrypt the given data using the given keys and label.
/// - `x`:      data to encrypt
/// - `ski`:    client private key
/// - `pk`:     list of all public keys
/// - `l`:      label
pub fn encode(x: &Scalar, ski: &PrivateKey, pk_list: &[PublicKey], label: &Label) -> CypherText {
    CypherText(
        pk_list
            .iter()
            .fold(*x, |acc, pkj| acc + tools::h(label.as_ref(), ski, pkj)),
    )
}

/// Decrypt the given data.
/// - `c`:  list of all encrypted data
pub fn combine(c: &[CypherText]) -> Scalar {
    c.iter().map(|&CypherText(ci)| ci).sum()
}
