mod db;
mod storage;
mod tendermint;

use std::path::Path;

use anoma::types::{Message, Transaction};
use byteorder::{BigEndian, ByteOrder};
use rand::{prelude::ThreadRng, thread_rng};
use storage::ValidatorAccount;
use ed25519_dalek::Keypair;

pub fn run() {
    // run our shell via Tendermint ABCI
    let shell = Shell::new("./.anoma/store.db");
    let addr = "127.0.0.1:26658".parse().unwrap();
    tendermint::run(addr, shell)
}

pub fn reset() {
    tendermint::reset()
}

// Simple counter application. Its only state is a u64 count
// We use BigEndian to serialize the data across transactions calls
pub struct Shell {
    store: db::Store,
    count: u64,
}

pub enum MempoolTxType {
    /// A transaction that has not been validated by this node before
    NewTransaction,
    /// A transaction that has been validated at some previous level that may
    /// need to be validated again
    RecheckTransaction,
}
pub type MempoolValidationResult<'a> = Result<(), String>;
pub type ApplyResult<'a> = Result<(), String>;

pub struct MerkleRoot(pub Vec<u8>);

impl Shell {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Self {
        Self {
            store: db::Store::new(db_path),
            count: 0,
        }
    }
}

pub struct InitialParameters {
    validators: Vec<ValidatorAccount>,
}

impl Shell {
    pub fn init_chain() -> InitialParameters {
        let mut rng: ThreadRng = thread_rng();
        let validators_count = 10;
        let mut validators = vec![];
        for _ in 0..validators_count {
            let keypair = Keypair::generate(&mut rng);
            validators.push(ValidatorAccount {
                pk: keypair.public,
                voting_power: 10,
                vp: (),
            });
        }
        println!("validators: {:#?}", validators);
        InitialParameters { validators }
    }

    /// Validate a transaction request. On success, the transaction will
    /// included in the mempool and propagated to peers, otherwise it will be
    /// rejected.
    pub fn mempool_validate(
        &mut self,
        tx_bytes: &[u8],
        _prevalidation_type: MempoolTxType,
    ) -> MempoolValidationResult {
        let tx = Transaction::decode(&tx_bytes[..]).map_err(|e| {
            format!(
                "Error decoding a transaction: {}, from bytes  from bytes {:?}",
                e, tx_bytes
            )
        })?;
        let c = tx.count;

        // Validation logic.
        // Rule: Transactions must be incremental: 1,2,3,4...
        if c != self.count + 1 {
            return Err(String::from("Count must be incremental!"));
        }
        // Update state to keep state correct for next check_tx call
        self.count = c;
        Ok(())
    }

    /// Validate and apply a transaction.
    pub fn apply_tx(&mut self, tx_bytes: &[u8]) -> ApplyResult {
        let tx = Transaction::decode(&tx_bytes[..]).map_err(|e| {
            format!(
                "Error decoding a transaction: {}, from bytes  from bytes {:?}",
                e, tx_bytes
            )
        })?;
        // Update state
        self.count = tx.count;
        // Return default code 0 == bueno
        Ok(())
    }

    /// Persist the application state and return the Merkle root hash.
    pub fn commit(&mut self) -> MerkleRoot {
        // Convert count to bits
        let mut buf = [0; 8];
        BigEndian::write_u64(&mut buf, self.count);
        // Set data so last state is included in the block
        MerkleRoot(buf.to_vec())
    }
}
