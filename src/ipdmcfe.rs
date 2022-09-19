use crate::{
    dsum, tools,
    types::{DVec, Label, TMat},
};
use cosmian_bls12_381::{pairing, G1Affine, G1Projective, G2Affine, G2Projective, Gt, Scalar};
use rand_core::{CryptoRng, RngCore};

/// DMCFE cyphertext type
#[derive(Clone, Copy)]
pub struct CypherText(G1Projective);

/// DMCFE private key type
#[derive(Clone)]
pub struct PrivateKey {
    /// - `s`:  two dimensional scalar vector
    pub s: DVec<Scalar>,
    /// - `t`:  2x2 scalar matrix
    pub t: TMat<dsum::CypherText>,
}

/// DMCFE partial decryption key type: `di`
#[derive(Clone)]
pub struct PartialDecryptionKey(DVec<G2Projective>);

/// DMCFE decryption key type: `(y, d)`
#[derive(Clone)]
pub struct DecryptionKey {
    /// - `y`:  decryption function
    pub y: Vec<Scalar>,
    /// - `d`:  functional decryption key
    d: DVec<G2Projective>,
}

/// Create `Ti`, such that `Sum(Ti) = 0`.
/// - `dski`: DSum secret key
/// - `dpk` : DSum public keys from all clients
fn t_gen(dski: &dsum::PrivateKey, dpk: &[dsum::PublicKey]) -> TMat<dsum::CypherText> {
    let mut res = [Default::default(); 4];
    for (i, res) in res.iter_mut().enumerate() {
        let mut l = Label::from("Setup");
        l.aggregate((i as u8).to_be_bytes());
        *res = dsum::encode(&Scalar::zero(), dski, dpk, &l);
    }
    TMat::new(res[0], res[1], res[2], res[3])
}

/// Return the DMCFE secret key.
/// - `dski`: DSum secret key
/// - `dpk` : DSum public keys from all clients
/// - `rng` : random number generator
pub fn setup<R: CryptoRng + RngCore>(
    dski: &dsum::PrivateKey,
    dpk: &[dsum::PublicKey],
    rng: &mut R,
) -> PrivateKey {
    PrivateKey {
        s: DVec::new(tools::random_scalar(rng), tools::random_scalar(rng)),
        t: t_gen(dski, dpk),
    }
}

/// Compute the DMCFE partial decryption key.
/// - `id`  : client ID
/// - `ski` : private key
/// - `y`   : decryption function
pub fn dkey_gen_share(id: usize, ski: &PrivateKey, y: &[Scalar]) -> PartialDecryptionKey {
    let v = DVec::from(tools::double_hash_to_curve_in_g2(&Label::from(y)));
    PartialDecryptionKey(&(&ski.s * &y[id]) * &G2Projective::generator() + &(&ski.t * &v))
}

/// Combine the partial decryption keys to return the final decryption key.
/// - `y`   : decryption function
/// - `pdk` : partial decryption keys
pub fn key_comb(y: &[Scalar], pdk: &[PartialDecryptionKey]) -> DecryptionKey {
    DecryptionKey {
        y: y.to_vec(),
        d: pdk
            .iter()
            .map(|PartialDecryptionKey(di)| di)
            .fold(DVec::default(), |acc, e| acc + e),
    }
}

/// Encrypts the data of a client `i` for a given label and encryption key.
/// - `xi`  : contribution
/// - `ski` : encryption key
/// - `l`   : label
pub fn encrypt(xi: &Scalar, ski: &PrivateKey, l: &Label) -> CypherText {
    let u = DVec::from(tools::double_hash_to_curve_in_g1(l));
    CypherText(u.inner_product(&ski.s) + tools::smul_in_g1(xi))
}

/// Decrypt the given cyphertexts with a given label and decryption key.
/// - `c`  : cyphertexts
/// - `dk` : decryption key
/// - `l`  : label
pub fn decrypt(c: &[CypherText], dk: &DecryptionKey, l: &Label) -> Gt {
    let u = DVec::from(tools::double_hash_to_curve_in_g1(l));

    c.iter()
        .zip(dk.y.iter())
        .map(|(CypherText(ci), yi)| {
            pairing(&G1Affine::from(ci), &G2Affine::from(tools::smul_in_g2(yi)))
        })
        .sum::<Gt>()
        - u.iter()
            .zip(dk.d.iter())
            .map(|(ui, di)| pairing(&G1Affine::from(ui), &G2Affine::from(di)))
            .sum::<Gt>()
}
