#![allow(non_snake_case)]

mod bus;

use bus::{Bus, BusTx};

use bls12_381::{pairing, G1Affine, G2Affine, Gt, Scalar};
use dmcfe::{dsum, ipdmcfe, label::Label};
use eyre::Result;
use rand::Rng;
use std::thread;

/// Number of decryption keys asked by the user
const NB_DK: u8 = 2;

/// structure containing all the buses used for the simulation
/// - `n`:  number of bus clients
/// - `yi`: decryption function components channel
/// - `pk`: public key bus channel
/// - `dk`: partial decryption key bus channel
/// - `ci`: cyphertext bus channel
struct SimuBus {
    n: usize,
    yi: Bus<Scalar>,
    pk: Bus<dsum::PublicKey>,
    dk: Bus<ipdmcfe::PartialDecryptionKey>,
    ci: Bus<((ipdmcfe::CypherText, Label), usize)>,
}

impl SimuBus {
    /// Create a new simulation bus
    /// - `n`:  number of clients
    fn new(n: usize) -> Self {
        SimuBus {
            n,
            yi: Bus::<Scalar>::open(n),
            pk: Bus::<dsum::PublicKey>::open(n),
            dk: Bus::<ipdmcfe::PartialDecryptionKey>::open(n),
            ci: Bus::<((ipdmcfe::CypherText, Label), usize)>::open(n),
        }
    }

    /// Close all buses
    fn close(self) -> Result<()> {
        self.pk.close()?;
        self.dk.close()?;
        self.ci.close()?;
        Ok(())
    }

    /// Get the transmission channels
    fn get_tx(&self) -> SimuTx {
        SimuTx {
            n: self.n,
            yi: self.yi.tx.clone(),
            pk: self.pk.tx.clone(),
            dk: self.dk.tx.clone(),
            ci: self.ci.tx.clone(),
        }
    }
}

/// Bus transmission channels used in the simulation
/// - `n`:  number of bus clients
/// - `yi`: decryption function components channel
/// - `pk`: public key bus channel
/// - `dk`: partial decryption key bus channel
/// - `ci`: cyphertext bus channel
#[derive(Clone)]
struct SimuTx {
    n: usize,
    yi: BusTx<Scalar>,
    pk: BusTx<dsum::PublicKey>,
    dk: BusTx<ipdmcfe::PartialDecryptionKey>,
    ci: BusTx<((ipdmcfe::CypherText, Label), usize)>,
}

/// Generate a random scalar
fn random_scalar() -> Scalar {
    Scalar::from_raw([
        rand::random(),
        rand::random(),
        rand::random(),
        rand::random(),
    ])
}

/// Reorder the given vector of `(T, i)` elements given `i`.
/// Return the vector of `T` elements.
/// - `v`:  vector to sort
fn reorder<T: Clone>(v: &mut [(T, usize)]) -> Vec<T> {
    v.sort_by_key(|vi| vi.1);
    v.iter().map(|vi| vi.0.clone()).collect()
}

/// Check labels used to encrypt cyphertexts are identical.
/// - `c`:  list of cyphertexts along with their labels
fn check_labels(c: &[(ipdmcfe::CypherText, Label)]) -> Result<(Vec<ipdmcfe::CypherText>, Label)> {
    let mut iter = c.iter();
    let mut c = Vec::with_capacity(c.len());
    let (ct, label) = iter.next().unwrap();
    c.push(*ct);
    for (ct, l) in iter {
        eyre::ensure!(
            *l.as_ref() == *label.as_ref(),
            "Cyphertexts are using different labels!"
        );
        c.push(*ct);
    }
    Ok((c, label.clone()))
}

/// Send decryption function to the clients, wait for the associated partial
/// decryption keys and compute the final decryption key.
/// - `tx`: bus
fn get_decryption_key(tx: &SimuTx) -> Result<ipdmcfe::DecryptionKey> {
    // generate a new decrytion function
    let y: Vec<Scalar> = (0..(tx.n - 1)).map(|_| random_scalar()).collect();
    // broadcast it to the clients
    for &yi in &y {
        bus::broadcast(&tx.yi, yi)?;
    }
    // wait for the partial decryption keys
    let dk_list = bus::wait_n(&tx.dk, tx.n - 1, tx.n - 1)?;
    // return the final decryption key
    Ok(ipdmcfe::key_comb(&y, &dk_list))
}

/// Setup step of the DMCFE algorithm.
/// - `id`: client network id
/// - `tx`: bus transmission channels
fn client_setup(id: usize, tx: &SimuTx) -> Result<ipdmcfe::PrivateKey> {
    // generate the DSum keys to create the `T` matrix
    let dsum::KeyPair(ski, pki) = dsum::client_setup();
    bus::broadcast(&tx.pk, pki)?;
    let pk_list = bus::wait_n(&tx.pk, tx.n - 1, id)?;

    // generate the DMCFE secret key
    Ok(ipdmcfe::setup(&ski, &pk_list))
}

/// Simulate a client:
/// - compute the cyphered contributions;
/// - compute the partial decryption key upon reception of a decryption function;
/// - send cyphertexts and partial decryption keys to the decryption client.
/// Return the contribution used, for test purpose only. In real life
/// applications, the contribution should never be shared!
///
/// - `id`: client network ID
/// - `tx`: bus transmission channels
fn client_simulation(id: usize, tx: &SimuTx) -> Result<Scalar> {
    // Generate setup variables
    let ski = client_setup(id, tx)?;

    // Send cyphered contribution to the user.
    let c_handle = {
        let (ski, tx) = (ski.clone(), tx.clone());
        thread::spawn(move || -> Result<Scalar> {
            let l = Label::new()?;
            let xi: Scalar = random_scalar();
            let cij = ipdmcfe::encrypt(&xi, &ski, &l);
            bus::unicast(&tx.ci, tx.n - 1, ((cij, l), id))?;
            Ok(xi)
        })
    };

    // Note: this loop should run until the thread is closed. For
    // testing purposes, we need it to terminate in order to return
    // the thread data `xi` to check the final result
    for _ in 0..NB_DK {
        let y = bus::wait_n(&tx.yi, tx.n - 1, id)?;
        let dki = ipdmcfe::dkey_gen_share(&ski, &y[id], &y);
        bus::unicast(&tx.dk, tx.n - 1, dki)?;
    }

    // We return the `xi` for testing purpose only: in real aplications, the
    // contribution should never be shared!
    Ok(c_handle
        .join()
        .map_err(|err| eyre::eyre!("Error while sending the cyphertext: {:?}", err))??)
}

/// Simulate the final user. Ask for partial decryption keys from the clients,
/// get the cyphertexts, decrypt data using the received decryption keys.
/// - `tx`: bus transmission channels
fn decrypt_simulation(tx: &SimuTx) -> Result<Vec<(ipdmcfe::DecryptionKey, Gt)>> {
    // Listen to the clients and wait for the cyphertexts.
    let c_handle = {
        let tx = tx.clone();
        thread::spawn(
            move || -> Result<Vec<((ipdmcfe::CypherText, Label), usize)>> {
                bus::wait_n(&tx.ci, tx.n - 1, tx.n - 1)
            },
        )
    };

    // Ask for some decryption keys.
    let mut dk_list = Vec::with_capacity(NB_DK as usize);
    for _ in 0..NB_DK {
        dk_list.push(get_decryption_key(&tx)?);
    }

    // Ensure we got all the cyphertexts
    let c = reorder(
        &mut c_handle
            .join()
            .map_err(|err| eyre::eyre!("Error while getting the cyphertexts: {:?}", err))??,
    );

    let (c, l) = check_labels(&c)?;

    // Decrypt the set of cyphertexts with each decryption key.
    Ok(dk_list
        .iter()
        .map(
            |dk: &ipdmcfe::DecryptionKey| -> (ipdmcfe::DecryptionKey, Gt) {
                (dk.clone(), ipdmcfe::decrypt(&c, dk, &l))
            },
        )
        .collect())
}

/// Simulate a complete MCFE encryption and decryption process. The encryption
/// of `x`, a given `(m,n)` matrix, for a given label `l` is done by `n` clients
/// with `m contributions. The decryption is done by another client who gathers
/// the partial encription keys and cyphertexts and compute the complete
/// decryption key.
/// - `n`:  number of clients
/// It returns the result of the MCFE in G1.
fn simulation(n: usize) -> Result<()> {
    // open the bus
    let bus = SimuBus::new(n + 1);

    // Launch the user
    let res = {
        let tx = bus.get_tx();
        thread::spawn(move || decrypt_simulation(&tx))
    };

    // Launch the clients
    let children: Vec<thread::JoinHandle<Result<Scalar>>> = (0..n)
        .map(|id| {
            let bus = bus.get_tx();
            thread::spawn(move || client_simulation(id, &bus))
        })
        .collect();

    // Get the contributions used by the children
    let x = children
        .into_iter()
        .map(|child| -> Result<Scalar> {
            child
                .join()
                .map_err(|err| eyre::eyre!("Error in client thread: {:?}", err))?
        })
        .collect::<Result<Vec<Scalar>>>()?;

    // Get the results from the user
    let res = res
        .join()
        .map_err(|err| eyre::eyre!("Error in the receiver thread: {:?}", err))??;

    // Check the results
    for (dk, res) in res {
        eyre::ensure!(
            res == pairing(&G1Affine::generator(), &G2Affine::generator())
                * x.iter()
                    .zip(dk.y.iter())
                    .map(|(xi, yi)| yi * xi)
                    .sum::<Scalar>(),
            "Wrong result!"
        )
    }

    bus.close()?;

    Ok(())
}

#[test]
fn test_dmcfe() -> Result<()> {
    simulation(rand::thread_rng().gen_range(2..20))
}
