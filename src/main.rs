use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

type Address = [u8; 20];
type U256 = [u8; 32];

#[derive(Clone)]
struct Transaction {
    nonce: u64,
    gas_price: U256,
    gas_limit: u64,
    to: Option<Address>,
    value: U256,
    data: Vec<u8>,
    v: u8,
    r: U256,
    s: U256,
}

#[derive(Clone)]
struct Account {
    nonce: u64,
    balance: U256,
}

#[derive(Clone)]
struct RollupState {
    accounts: HashMap<Address, Account>,
}

struct StateUpdate {
    transactions: Vec<Transaction>,
    old_state_root: Vec<u8>,
    new_state_root: Vec<u8>,
}

struct OptimisticRollup {
    state: RollupState,
    state_updates: Vec<StateUpdate>,
}

impl OptimisticRollup {
    fn new() -> Self {
        OptimisticRollup {
            state: RollupState { accounts: HashMap::new() },
            state_updates: Vec::new(),
        }
    }

    fn process_transaction_batch(&mut self, transactions: Vec<Transaction>) {
        let old_state_root = self.calculate_state_root();

        for tx in &transactions {
            self.apply_transaction(tx);
        }

        let new_state_root = self.calculate_state_root();

        self.state_updates.push(StateUpdate {
            transactions,
            old_state_root,
            new_state_root,
        });
    }

    fn apply_transaction(&mut self, tx: &Transaction) {
        if let Some(to) = tx.to {
            // Transfer transaction
            let from = self.recover_signer(tx);
            self.transfer(&from, &to, &tx.value);
        } else {
            // Contract creation (simplified)
            println!("Contract creation not implemented in this example");
        }

        // Update nonce
        if let Some(account) = self.state.accounts.get_mut(&self.recover_signer(tx)) {
            account.nonce += 1;
        }
    }

    fn transfer(&mut self, from: &Address, to: &Address, value: &U256) {
        let mut from_account = self.state.accounts.entry(*from).or_insert_with(|| Account { nonce: 0, balance: [0; 32] }).clone();
        let mut to_account = self.state.accounts.entry(*to).or_insert_with(|| Account { nonce: 0, balance: [0; 32] }).clone();

        // Simplified balance update (doesn't handle overflow)
        for i in 0..32 {
            from_account.balance[i] = from_account.balance[i].saturating_sub(value[i]);
            to_account.balance[i] = to_account.balance[i].saturating_add(value[i]);
        }

        self.state.accounts.insert(*from, from_account);
        self.state.accounts.insert(*to, to_account);
    }

    fn calculate_state_root(&self) -> Vec<u8> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut sorted_accounts: Vec<_> = self.state.accounts.iter().collect();
        sorted_accounts.sort_by(|a, b| a.0.cmp(b.0));
        for (address, account) in sorted_accounts {
            address.hash(&mut hasher);
            account.nonce.hash(&mut hasher);
            account.balance.hash(&mut hasher);
        }
        hasher.finish().to_be_bytes().to_vec()
    }

    fn recover_signer(&self, _tx: &Transaction) -> Address {
        // In a real implementation, this would recover the signer's address from the transaction signature
        // For simplicity, we'll return a dummy address
        [0; 20]
    }

    fn generate_fraud_proof(&self, update_index: usize, fraudulent_tx_index: usize) -> Option<FraudProof> {
        let update = self.state_updates.get(update_index)?;
        let fraudulent_tx = update.transactions.get(fraudulent_tx_index)?;

        // Recreate the state just before the fraudulent transaction
        let mut pre_fraud_state = RollupState { accounts: HashMap::new() };
        for i in 0..update_index {
            for tx in &self.state_updates[i].transactions {
                Self::apply_transaction_to_state(&mut pre_fraud_state, tx);
            }
        }
        for i in 0..fraudulent_tx_index {
            Self::apply_transaction_to_state(&mut pre_fraud_state, &update.transactions[i]);
        }

        // Generate the proof
        let pre_fraud_root = Self::calculate_state_root_for(&pre_fraud_state);
        let mut post_fraud_state = pre_fraud_state.clone();
        Self::apply_transaction_to_state(&mut post_fraud_state, fraudulent_tx);
        let post_fraud_root = Self::calculate_state_root_for(&post_fraud_state);

        Some(FraudProof {
            update_index,
            fraudulent_tx_index,
            pre_fraud_root,
            post_fraud_root,
            fraudulent_tx: fraudulent_tx.clone(),
        })
    }

    fn apply_transaction_to_state(state: &mut RollupState, tx: &Transaction) {
        if let Some(to) = tx.to {
            let from = [0; 20]; // Dummy address, should be recovered from signature
            Self::transfer_in_state(state, &from, &to, &tx.value);
        }
        // Update nonce (simplified)
        if let Some(account) = state.accounts.get_mut(&[0; 20]) { // Should use recovered address
            account.nonce += 1;
        }
    }

    fn transfer_in_state(state: &mut RollupState, from: &Address, to: &Address, value: &U256) {
        let mut from_account = state.accounts.entry(*from).or_insert_with(|| Account { nonce: 0, balance: [0; 32] }).clone();
        let mut to_account = state.accounts.entry(*to).or_insert_with(|| Account { nonce: 0, balance: [0; 32] }).clone();

        // Simplified balance update (doesn't handle overflow)
        for i in 0..32 {
            from_account.balance[i] = from_account.balance[i].saturating_sub(value[i]);
            to_account.balance[i] = to_account.balance[i].saturating_add(value[i]);
        }

        state.accounts.insert(*from, from_account);
        state.accounts.insert(*to, to_account);
    }

    fn calculate_state_root_for(state: &RollupState) -> Vec<u8> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut sorted_accounts: Vec<_> = state.accounts.iter().collect();
        sorted_accounts.sort_by(|a, b| a.0.cmp(b.0));
        for (address, account) in sorted_accounts {
            address.hash(&mut hasher);
            account.nonce.hash(&mut hasher);
            account.balance.hash(&mut hasher);
        }
        hasher.finish().to_be_bytes().to_vec()
    }

    fn verify_fraud_proof(&self, proof: &FraudProof) -> bool {
        let update = &self.state_updates[proof.update_index];
        
        // Recreate the state just before the fraudulent transaction
        let mut pre_fraud_state = RollupState { accounts: HashMap::new() };
        for i in 0..proof.update_index {
            for tx in &self.state_updates[i].transactions {
                Self::apply_transaction_to_state(&mut pre_fraud_state, tx);
            }
        }
        for i in 0..proof.fraudulent_tx_index {
            Self::apply_transaction_to_state(&mut pre_fraud_state, &update.transactions[i]);
        }

        // Verify pre-fraud root
        let calculated_pre_fraud_root = Self::calculate_state_root_for(&pre_fraud_state);
        if calculated_pre_fraud_root != proof.pre_fraud_root {
            return false;
        }

        // Apply the fraudulent transaction
        Self::apply_transaction_to_state(&mut pre_fraud_state, &proof.fraudulent_tx);

        // Verify post-fraud root
        let calculated_post_fraud_root = Self::calculate_state_root_for(&pre_fraud_state);
        calculated_post_fraud_root == proof.post_fraud_root
    }
}

#[derive(Clone)]
struct FraudProof {
    update_index: usize,
    fraudulent_tx_index: usize,
    pre_fraud_root: Vec<u8>,
    post_fraud_root: Vec<u8>,
    fraudulent_tx: Transaction,
}

fn main() {
    let mut rollup = OptimisticRollup::new();

    // Initialize an account with some balance
    let initial_account = Account {
        nonce: 0,
        balance: {
            let mut balance = [0; 32];
            balance[31] = 200; // Set balance to 200 wei
            balance
        },
    };
    rollup.state.accounts.insert([0; 20], initial_account);

    // Create some example transactions
    let tx1 = Transaction {
        nonce: 0,
        gas_price: [0; 32],
        gas_limit: 21000,
        to: Some([1; 20]),
        value: {
            let mut value = [0; 32];
            value[31] = 100; // Transfer 100 wei
            value
        },
        data: vec![],
        v: 0,
        r: [0; 32],
        s: [0; 32],
    };

    let tx2 = Transaction {
        nonce: 1,
        gas_price: [0; 32],
        gas_limit: 21000,
        to: Some([2; 20]),
        value: {
            let mut value = [0; 32];
            value[31] = 150; // Transfer 150 wei (fraudulent: not enough balance)
            value
        },
        data: vec![],
        v: 0,
        r: [0; 32],
        s: [0; 32],
    };

    // Process first batch of transactions
    rollup.process_transaction_batch(vec![tx1]);
    println!("Processed first batch with one valid transaction");

    // Process second batch with a fraudulent transaction
    rollup.process_transaction_batch(vec![tx2]);
    println!("Processed second batch with a fraudulent transaction");

    // Generate a fraud proof for the fraudulent transaction
    if let Some(fraud_proof) = rollup.generate_fraud_proof(1, 0) {
        println!("Fraud Proof Generated:");
        println!("Update Index: {}", fraud_proof.update_index);
        println!("Fraudulent Transaction Index: {}", fraud_proof.fraudulent_tx_index);
        println!("Pre-fraud State Root: {:?}", fraud_proof.pre_fraud_root);
        println!("Post-fraud State Root: {:?}", fraud_proof.post_fraud_root);

        // Verify the fraud proof
        let is_valid = rollup.verify_fraud_proof(&fraud_proof);
        println!("Fraud Proof is valid: {}", is_valid);

        if is_valid {
            println!("Fraudulent state update detected. In a real system, this would trigger:");
            println!("1. Reversion of the fraudulent state update");
            println!("2. Penalty for the submitter of the fraudulent update");
            println!("3. Reward for the challenger who submitted the fraud proof");
        }
    } else {
        println!("Failed to generate fraud proof");
    }
}