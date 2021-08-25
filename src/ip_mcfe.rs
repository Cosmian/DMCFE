use crate::{ipfe, tools};
use bls12_381::{G1Projective, Scalar};
use eyre::Result;

/// MCFE encryption key type
pub type EncryptionKey = (Vec<Vec<Scalar>>, Vec<Scalar>);

/// MCFE partial decryption key type
pub type PartialDecryptionKey = (Vec<Scalar>, [Scalar; 2], Scalar);

/// MCFE decryption key type
pub type DecryptionKey = (Vec<Vec<Scalar>>, Vec<Scalar>, Vec<Scalar>);

/// Compute the client encryption keys.
/// - `n`: number of clients
/// - `m`: number of contributions per client
pub fn setup(n: usize, m: usize) -> Vec<EncryptionKey> {
    (0..n)
        .map(|_| {
            let (ip_msk, _) = ipfe::setup(m);
            (tools::random_mat_gen(m, 2), ip_msk)
        })
        .collect()
}

/// Encrypts the data of a client `i` using its encryption key for a given label.
/// - `eki`:    client encription key;
/// - `xi`:     client contribution;
/// - `l`:      label
pub fn encrypt(eki: &EncryptionKey, xi: &[Scalar], l: usize) -> Result<Vec<G1Projective>> {
    let (Si, mski) = eki;
    let Ul = tools::hash_to_curve(l);
    let R1 = tools::mat_mul(&Si, &tools::double_hash_to_curve(l))?;
    let R2: Vec<G1Projective> = mski.iter().map(|si| Ul * si).collect();
    let ci: Vec<G1Projective> = xi
        .iter()
        .zip(R1.iter())
        .map(|(xij, r)| r + G1Projective::generator() * xij)
        .collect();

    Ok(ci.iter().zip(R2.iter()).map(|(cij, r)| r + cij).collect())
}

/// Compute the partial decryption key for a client `i`.
/// - `eki`: the encryption key of a client `i`;
/// - `yi`: the vector associated to the decryption function for a client `i`.
pub fn dkey_gen(eki: &EncryptionKey, yi: &[Scalar]) -> Result<PartialDecryptionKey> {
    let (Si, mski) = eki;
    let dky_i = tools::scal_mat_mul_dim_2(&tools::transpose(Si)?, yi)?;
    eyre::ensure!(
        2 == dky_i.len(),
        "Wrong size for dky_i: {}, should be 2!",
        dky_i.len()
    );
    Ok((
        (*yi).to_vec(),
        [dky_i[0], dky_i[1]],
        yi.iter().zip(mski.iter()).map(|(yij, sij)| yij * sij).sum(),
    ))
}

/// Compute the decryption key given the `n` partial decryption keys from the clients.
/// - `dk`: partial decryption keys generated by the clients
pub fn key_comb(dki: &[PartialDecryptionKey]) -> Result<DecryptionKey> {
    let mut y = Vec::new();
    let mut d = vec![Scalar::zero(); 2];
    let mut ip_dk = Vec::new();

    dki.iter().for_each(|(yi, di, ip_dki)| {
        y.push(yi.to_vec());
        d[0] += di[0];
        d[1] += di[1];
        ip_dk.push(*ip_dki);
    });

    Ok((y, d, ip_dk))
}

/// Decrypt the given cyphertexts of a given label using the decryption key.
/// - `C`:  the cyphertexts
/// - `dk`: the decryption key
/// - `l`:  the label
pub fn decrypt(C: &[Vec<G1Projective>], dk: &DecryptionKey, l: usize) -> G1Projective {
    let Ul = tools::hash_to_curve(l);
    let dl: G1Projective = C
        .iter()
        .zip(dk.0.iter())
        .zip(dk.2.iter())
        .map(|((Ci, yi), ip_dki)| {
            let di: G1Projective = Ci
                .iter()
                .zip(yi.iter())
                .map(|(Cij, yij)| Cij * yij)
                .sum::<G1Projective>()
                - Ul * ip_dki;
            di
        })
        .sum();
    let d: G1Projective =
        dk.1.iter()
            .zip(tools::double_hash_to_curve(l).iter())
            .map(|(di, ui)| ui * di)
            .sum();
    dl - d
}
