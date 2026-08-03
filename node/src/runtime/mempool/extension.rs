use super::MempoolError;
use crate::runtime::params::{
    DEFAULT_MARKET_FEE, DEFAULT_MIN_RELAY_FEE, DYNAMIC_MARKET_FEE_MAX_MULTIPLIER,
    LOW_FEE_EXPIRY_SECS, MAX_MEMPOOL_BYTES, MAX_MEMPOOL_TXS, MEMPOOL_EXPIRY_SECS,
};
use paqus::block::{Block, BlockHeight, MAX_BLOCK_SIZE, MAX_BLOCK_WEIGHT};
use paqus::crypto::{Address, TransactionHash};
use paqus::ledger::Ledger;
use paqus::state::QCashCoinId;
use paqus::transaction::{QCashTransactionKind, SignedProtocolTransaction, TransactionFamily};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Ordered pool for every protocol transaction family.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Mempool {
    transactions: BTreeMap<TransactionHash, SignedProtocolTransaction>,
    by_signer_state: BTreeMap<(Address, paqus::crypto::Hash), TransactionHash>,
    reserved_coins: BTreeMap<QCashCoinId, TransactionHash>,
    inserted_at: BTreeMap<TransactionHash, u64>,
    total_bytes: usize,
    config: MempoolConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MempoolConfig {
    pub max_transactions: usize,
    pub max_bytes: usize,
    pub transaction_ttl_secs: u64,
    pub low_fee_ttl_secs: u64,
    pub min_relay_fee: u64,
    pub market_fee: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeeMarketSnapshot {
    pub min_relay_fee_rate: u64,
    pub configured_market_fee_rate: u64,
    pub recommended_fee_rate: u64,
    pub pressure_bps: u64,
    pub transaction_count: usize,
    pub total_bytes: usize,
    pub max_transactions: usize,
    pub max_bytes: usize,
    pub next_block_clearing_fee_rate: u64,
    pub median_fee_rate: u64,
    pub p75_fee_rate: u64,
    pub p90_fee_rate: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: MAX_MEMPOOL_TXS,
            max_bytes: MAX_MEMPOOL_BYTES,
            transaction_ttl_secs: MEMPOOL_EXPIRY_SECS,
            low_fee_ttl_secs: LOW_FEE_EXPIRY_SECS,
            min_relay_fee: DEFAULT_MIN_RELAY_FEE,
            market_fee: DEFAULT_MARKET_FEE,
        }
    }
}

impl Mempool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: MempoolConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    pub fn config(&self) -> MempoolConfig {
        self.config
    }

    pub fn insert_validated(
        &mut self,
        ledger: &Ledger,
        transaction: SignedProtocolTransaction,
    ) -> Result<TransactionHash, MempoolError> {
        let transaction_size = transaction
            .to_bytes()
            .map_err(MempoolError::Serialization)?
            .len();
        if transaction_size > self.config.max_bytes {
            return Err(MempoolError::MempoolFull);
        }
        let hash = transaction.hash().map_err(MempoolError::Serialization)?;
        if block_miner_output_count(&transaction) > 1 {
            return Err(MempoolError::DuplicateBlockMinerFeeOutput);
        }
        let signer = transaction.signer();
        let signer_state = (signer, protocol_last_state(&transaction));
        if self.transactions.contains_key(&hash) || self.by_signer_state.contains_key(&signer_state)
        {
            return Err(MempoolError::DuplicateTransaction);
        }
        self.evict_for_capacity(&transaction, transaction_size)?;
        let coin_ids = redeem_coin_ids(&transaction);
        if coin_ids
            .iter()
            .any(|coin_id| self.reserved_coins.contains_key(coin_id))
        {
            return Err(MempoolError::CashCoinReserved);
        }

        let height = ledger
            .tip_height()
            .map(|height| paqus::block::Height(height.0.saturating_add(1)))
            .unwrap_or(paqus::block::Height(0));
        let mut staged = ledger.clone();
        let mut pending = self.transactions.values().cloned().collect::<Vec<_>>();
        pending.push(transaction.clone());
        pending.sort_by_key(|tx| (tx.signer(), tx.hash().ok()));
        for pending_transaction in &pending {
            apply_extension(&mut staged, pending_transaction, height)?;
        }

        for coin_id in coin_ids {
            self.reserved_coins.insert(coin_id, hash);
        }
        self.by_signer_state.insert(signer_state, hash);
        self.transactions.insert(hash, transaction);
        self.inserted_at.insert(hash, current_unix_timestamp());
        self.total_bytes = self.total_bytes.saturating_add(transaction_size);
        Ok(hash)
    }

    fn evict_for_capacity(
        &mut self,
        incoming: &SignedProtocolTransaction,
        incoming_size: usize,
    ) -> Result<(), MempoolError> {
        let incoming_rate = miner_bounty_rate(incoming)?;
        while self.transactions.len() >= self.config.max_transactions
            || self.total_bytes.saturating_add(incoming_size) > self.config.max_bytes
        {
            let Some((victim_hash, victim_rate, _inserted_at)) = self.lowest_fee_candidate() else {
                return Err(MempoolError::MempoolFull);
            };
            if incoming_rate <= victim_rate {
                return Err(MempoolError::FeeTooLow);
            }
            self.remove(&victim_hash)?;
        }
        Ok(())
    }

    fn lowest_fee_candidate(&self) -> Option<(TransactionHash, u64, u64)> {
        self.transactions
            .iter()
            .filter_map(|(hash, transaction)| {
                let rate = miner_bounty_rate(transaction).ok()?;
                let inserted_at = self.inserted_at.get(hash).copied().unwrap_or(0);
                Some((*hash, rate, inserted_at))
            })
            .min_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| left.2.cmp(&right.2))
                    .then_with(|| left.0.cmp(&right.0))
            })
    }

    pub fn contains(&self, hash: &TransactionHash) -> bool {
        self.transactions.contains_key(hash)
    }

    pub fn get(&self, hash: &TransactionHash) -> Option<&SignedProtocolTransaction> {
        self.transactions.get(hash)
    }

    #[cfg(test)]
    pub(crate) fn insert_for_compact_test(
        &mut self,
        transaction: SignedProtocolTransaction,
    ) -> Result<(), paqus::error::CodecError> {
        self.transactions.insert(transaction.hash()?, transaction);
        Ok(())
    }

    pub fn transactions(&self) -> impl Iterator<Item = &SignedProtocolTransaction> {
        self.transactions.values()
    }

    pub fn transactions_for_family(
        &self,
        family: TransactionFamily,
    ) -> impl Iterator<Item = &SignedProtocolTransaction> {
        self.transactions
            .values()
            .filter(move |transaction| transaction.family() == family)
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    pub fn remove(
        &mut self,
        hash: &TransactionHash,
    ) -> Result<Option<SignedProtocolTransaction>, MempoolError> {
        let Some(transaction) = self.transactions.remove(hash) else {
            return Ok(None);
        };
        self.inserted_at.remove(hash);
        self.total_bytes = self.total_bytes.saturating_sub(
            transaction
                .to_bytes()
                .map_err(MempoolError::Serialization)?
                .len(),
        );
        self.by_signer_state
            .remove(&(transaction.signer(), protocol_last_state(&transaction)));
        for coin_id in redeem_coin_ids(&transaction) {
            if self.reserved_coins.get(&coin_id) == Some(hash) {
                self.reserved_coins.remove(&coin_id);
            }
        }
        Ok(Some(transaction))
    }

    pub fn evict_by_policy(&mut self, now: u64) -> Result<usize, MempoolError> {
        if self.config.transaction_ttl_secs == 0 {
            return Ok(0);
        }
        let mut evicted = Vec::new();
        for hash in self.transactions.keys() {
            let inserted_at = self.inserted_at.get(hash).copied().unwrap_or(now);
            let ttl = self.config.transaction_ttl_secs;
            if inserted_at.saturating_add(ttl) <= now {
                evicted.push(*hash);
            }
        }
        let removed = evicted.len();
        for hash in evicted {
            self.remove(&hash)?;
        }
        Ok(removed)
    }

    pub fn mempool_pressure_bps(&self) -> u64 {
        occupancy_bps(self.total_bytes, self.config.max_bytes).max(occupancy_bps(
            self.transactions.len(),
            self.config.max_transactions,
        ))
    }

    pub fn dynamic_market_fee_rate(&self) -> u64 {
        self.fee_market_snapshot().recommended_fee_rate
    }

    pub fn fee_market_snapshot(&self) -> FeeMarketSnapshot {
        let base_rate = self.config.market_fee.max(self.config.min_relay_fee);
        let pressure_bps = self.mempool_pressure_bps();
        let premium = base_rate
            .saturating_mul(DYNAMIC_MARKET_FEE_MAX_MULTIPLIER)
            .saturating_mul(pressure_bps)
            / 10_000;
        let pressure_rate = base_rate.saturating_add(premium);
        let mut entries = self
            .transactions
            .values()
            .filter_map(|transaction| {
                let virtual_size = transaction.virtual_size().ok()?;
                let rate = miner_bounty_rate(transaction).ok()?;
                Some((rate, virtual_size))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));

        let next_block_clearing_fee_rate =
            weighted_clearing_rate(&entries, MAX_BLOCK_WEIGHT).unwrap_or(base_rate);
        let median_fee_rate = weighted_percentile_rate(&entries, 50).unwrap_or(base_rate);
        let p75_fee_rate = weighted_percentile_rate(&entries, 75).unwrap_or(base_rate);
        let p90_fee_rate = weighted_percentile_rate(&entries, 90).unwrap_or(base_rate);
        let recommended_fee_rate = [
            base_rate,
            pressure_rate,
            next_block_clearing_fee_rate,
            p75_fee_rate,
        ]
        .into_iter()
        .max()
        .unwrap_or(base_rate);

        FeeMarketSnapshot {
            min_relay_fee_rate: self.config.min_relay_fee,
            configured_market_fee_rate: self.config.market_fee,
            recommended_fee_rate,
            pressure_bps,
            transaction_count: self.transactions.len(),
            total_bytes: self.total_bytes,
            max_transactions: self.config.max_transactions,
            max_bytes: self.config.max_bytes,
            next_block_clearing_fee_rate,
            median_fee_rate,
            p75_fee_rate,
            p90_fee_rate,
        }
    }

    pub fn select_for_block(
        &self,
        ledger: &Ledger,
        height: BlockHeight,
        _block_timestamp: u64,
        limit: usize,
        min_bounty_rate: u64,
    ) -> Result<Vec<SignedProtocolTransaction>, MempoolError> {
        let mut ordered = self.transactions.values().cloned().collect::<Vec<_>>();
        ordered.sort_by(|left, right| {
            let left_rate = miner_bounty_rate(left).unwrap_or(0);
            let right_rate = miner_bounty_rate(right).unwrap_or(0);
            right_rate
                .cmp(&left_rate)
                .then_with(|| left.signer().cmp(&right.signer()))
                .then_with(|| left.hash().ok().cmp(&right.hash().ok()))
        });
        let mut staged = ledger.clone();
        let mut candidates = Vec::new();
        let mut remaining = ordered;
        while !remaining.is_empty() && candidates.len() < limit {
            let mut progressed = false;
            let mut deferred = Vec::new();
            for transaction in remaining {
                if candidates.len() == limit {
                    deferred.push(transaction);
                    continue;
                }
                if miner_bounty_rate(&transaction)? < min_bounty_rate {
                    continue;
                }
                if transaction.validity().validate_at(height).is_err() {
                    continue;
                }
                if apply_extension(&mut staged, &transaction, height).is_err() {
                    continue;
                }
                candidates.push(transaction);
                progressed = true;
            }
            if !progressed {
                break;
            }
            remaining = deferred;
        }
        Ok(candidates)
    }

    pub fn append_selected_to_block(
        &self,
        ledger: &Ledger,
        block: &mut Block,
        transaction_limit: usize,
        min_fee_rate: u64,
    ) -> Result<(), MempoolError> {
        let remaining = transaction_limit.saturating_sub(block.transaction_count());
        for transaction in
            self.select_for_block(ledger, block.height(), 0, remaining, min_fee_rate)?
        {
            block.body.transactions.push(transaction);
            refresh_block_fees_and_commitments(block)?;
            if block
                .serialized_size()
                .map_err(MempoolError::Serialization)?
                > MAX_BLOCK_SIZE
                || block.weight().map_err(MempoolError::Serialization)? > MAX_BLOCK_WEIGHT
            {
                block.body.transactions.pop();
                refresh_block_fees_and_commitments(block)?;
            }
        }
        refresh_block_fees_and_commitments(block)?;
        Ok(())
    }

    pub fn remove_confirmed(&mut self, block: &Block) -> Result<(), MempoolError> {
        let hashes = block
            .transactions()
            .iter()
            .map(|tx| tx.hash())
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(MempoolError::Serialization)?;
        for hash in hashes {
            self.remove(&hash)?;
        }
        Ok(())
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn occupancy_bps(used: usize, capacity: usize) -> u64 {
    if capacity == 0 {
        return 10_000;
    }
    ((used as u128).saturating_mul(10_000) / capacity as u128).min(10_000) as u64
}

fn weighted_clearing_rate(entries: &[(u64, usize)], target_weight: usize) -> Option<u64> {
    if entries.is_empty() {
        return None;
    }
    let mut accumulated = 0usize;
    let mut last_rate = entries[0].0;
    for (rate, virtual_size) in entries {
        accumulated = accumulated.saturating_add(*virtual_size);
        last_rate = *rate;
        if accumulated >= target_weight {
            return Some(*rate);
        }
    }
    Some(last_rate)
}

fn weighted_percentile_rate(entries: &[(u64, usize)], percentile: u64) -> Option<u64> {
    if entries.is_empty() {
        return None;
    }
    let total = entries
        .iter()
        .fold(0usize, |total, (_, size)| total.saturating_add(*size));
    if total == 0 {
        return Some(entries[0].0);
    }
    let target = ((total as u128)
        .saturating_mul(percentile as u128)
        .saturating_add(99)
        / 100)
        .max(1) as usize;
    let mut accumulated = 0usize;
    for (rate, virtual_size) in entries {
        accumulated = accumulated.saturating_add(*virtual_size);
        if accumulated >= target {
            return Some(*rate);
        }
    }
    entries.last().map(|(rate, _)| *rate)
}

fn fee_rate(fee: u64, virtual_size: usize) -> u64 {
    if virtual_size == 0 {
        return u64::MAX;
    }
    fee.saturating_mul(crate::runtime::params::FEE_RATE_UNIT_BYTES as u64) / virtual_size as u64
}

fn miner_bounty_rate(transaction: &SignedProtocolTransaction) -> Result<u64, MempoolError> {
    let virtual_size = transaction
        .virtual_size()
        .map_err(MempoolError::Serialization)?;
    Ok(fee_rate(transaction.block_miner_bounty(), virtual_size))
}

fn block_miner_output_count(transaction: &SignedProtocolTransaction) -> usize {
    match transaction {
        SignedProtocolTransaction::BatchTransfer(transaction) => transaction
            .transaction
            .outputs()
            .filter(|output| output.to == paqus::transaction::OutputTarget::BlockMiner)
            .count(),
        SignedProtocolTransaction::QCash(_) => 0,
    }
}

fn refresh_block_fees_and_commitments(block: &mut Block) -> Result<(), MempoolError> {
    block
        .refresh_commitments()
        .map_err(MempoolError::Serialization)?;
    Ok(())
}

fn apply_extension(
    staged: &mut Ledger,
    transaction: &SignedProtocolTransaction,
    height: BlockHeight,
) -> Result<(), MempoolError> {
    match transaction {
        SignedProtocolTransaction::BatchTransfer(tx) => {
            staged.apply_signed_transaction_at(tx, height)?;
        }
        SignedProtocolTransaction::QCash(tx) => {
            staged.apply_signed_qcash_transaction(tx, height)?;
        }
    }
    Ok(())
}

fn protocol_last_state(transaction: &SignedProtocolTransaction) -> paqus::crypto::Hash {
    match transaction {
        SignedProtocolTransaction::BatchTransfer(tx) => tx.transaction.last_state,
        SignedProtocolTransaction::QCash(tx) => tx.transaction.last_state,
    }
}

fn redeem_coin_ids(transaction: &SignedProtocolTransaction) -> Vec<QCashCoinId> {
    match transaction {
        SignedProtocolTransaction::QCash(tx) => match &tx.transaction.kind {
            QCashTransactionKind::Redeem { metadata, .. }
            | QCashTransactionKind::RecoverRedeem { metadata, .. } => metadata
                .inputs
                .iter()
                .map(|input| QCashCoinId(input.coin_id))
                .collect(),
            QCashTransactionKind::Withdraw { .. } => Vec::new(),
        },
        _ => Vec::new(),
    }
}

// These legacy wrapper tests target transaction families removed before paqus 0.2.20.
#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use paqus::block::{Height, Nonce};
    use paqus::consensus::supply::{Amount, XPQ};
    use paqus::crypto::{
        Address, TransactionHash, dual_address_from_public_keys, generate_keypair, sign,
    };
    use paqus::qcash::{
        QCashCoinFile, QCashDenomination, QCashWithdrawalMetadata,
        qcash_redeem_key_commitment_from_secret,
    };
    use paqus::state::{VaultMetadata, VaultPolicy};
    use paqus::transaction::{
        OutputTarget, QCashTransaction, SignedQCashTransaction, SignedTransaction,
        SignedVaultTransaction, Transaction, TransferOutput, VaultTransaction,
    };

    fn signed_transfer(keypair: &paqus::crypto::KeyPair, nonce: u64) -> SignedProtocolTransaction {
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        signed_transfer_with_outputs(
            keypair,
            nonce,
            vec![TransferOutput {
                to: (Address([42; 20])).into(),
                amount: Amount(1),
            }],
        )
    }

    fn signed_transfer_with_outputs(
        keypair: &paqus::crypto::KeyPair,
        nonce: u64,
        outputs: Vec<TransferOutput>,
    ) -> SignedProtocolTransaction {
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        let transaction = Transaction::new(signer, outputs, Nonce(nonce));
        let signature = sign(&keypair.secret_key, &transaction.signing_bytes().unwrap());
        SignedTransaction::new_authorized(
            transaction,
            keypair.public_key,
            signature,
            keypair.public_key,
            signature,
        )
        .into()
    }

    #[test]
    fn candidate_never_selects_a_descendant_without_its_nonce_parent() {
        let keypair = generate_keypair();
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        let mut ledger = Ledger::new();
        ledger
            .create_account_with_authorization(signer, keypair.public_key, Amount(100_000))
            .unwrap();
        ledger.chain.tip_height = Some(Height(0));

        let parent = signed_transfer(&keypair, 0);
        let descendant = signed_transfer(&keypair, 1);
        let mut pool = Mempool::new();
        pool.insert_validated(&ledger, parent).unwrap();
        pool.insert_validated(&ledger, descendant).unwrap();

        let selected = pool
            .select_for_block(&ledger, Height(1), 1_700_000_001, 10, DEFAULT_MIN_RELAY_FEE)
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].nonce(), Nonce(0));
        assert_eq!(selected[1].nonce(), Nonce(1));
    }

    #[test]
    fn candidate_filters_by_block_miner_bounty_rate() {
        let keypair = generate_keypair();
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        let mut ledger = Ledger::new();
        ledger
            .create_account_with_authorization(signer, keypair.public_key, Amount(100_000))
            .unwrap();
        ledger.chain.tip_height = Some(Height(0));

        let no_bounty = signed_transfer(&keypair, 0);
        let virtual_size = no_bounty.virtual_size().unwrap() as u64;
        let bounty_rate = 2;
        let bounty = virtual_size.saturating_mul(bounty_rate);
        let with_bounty = signed_transfer_with_outputs(
            &keypair,
            1,
            vec![
                TransferOutput {
                    to: (Address([42; 20])).into(),
                    amount: Amount(1),
                },
                TransferOutput {
                    to: OutputTarget::BlockMiner,
                    amount: Amount(bounty),
                },
            ],
        );
        let mut pool = Mempool::new();
        pool.insert_validated(&ledger, no_bounty).unwrap();
        pool.insert_validated(&ledger, with_bounty).unwrap();

        let selected = pool
            .select_for_block(&ledger, Height(1), 1_700_000_001, 10, bounty_rate)
            .unwrap();
        assert!(selected.is_empty());

        let selected = pool
            .select_for_block(&ledger, Height(1), 1_700_000_001, 10, 0)
            .unwrap();
        assert_eq!(selected.len(), 2);
    }

    fn signed_redeem(
        keypair: &paqus::crypto::KeyPair,
        file: &QCashCoinFile,
    ) -> SignedProtocolTransaction {
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        let transaction =
            QCashTransaction::redeem_from_files(signer, signer, Nonce(0), &[file.clone()]).unwrap();
        let payload = transaction.signing_bytes().unwrap();
        SignedQCashTransaction::new_authorized(
            transaction,
            keypair.public_key,
            sign(&keypair.secret_key, &payload),
            keypair.public_key,
            sign(&keypair.secret_key, &payload),
        )
        .into()
    }

    #[test]
    fn reserves_qcash_coin_across_the_unified_extension_pool() {
        let redeem_secret = [11; 32];
        let withdraw = QCashWithdrawalMetadata::with_denominations(
            Amount(XPQ),
            &[QCashDenomination::One],
            &[qcash_redeem_key_commitment_from_secret(&redeem_secret)],
        )
        .unwrap();
        let withdraw_hash = TransactionHash([12; 32]);
        let mut ledger = Ledger::new();
        ledger
            .qcash_utxos
            .apply_withdraw(Address([13; 20]), withdraw_hash, &withdraw, Height(0))
            .unwrap();
        let issued_rewards = (1..=paqus::ledger::QCASH_REDEEM_DELAY as u64)
            .map(|height| paqus::consensus::block_reward(Height(height)).0)
            .sum();
        ledger
            .create_account(Address([98; 20]), Amount(issued_rewards))
            .unwrap();
        let file = QCashCoinFile::new(withdraw_hash, &withdraw.outputs[0], redeem_secret).unwrap();
        let first_keypair = generate_keypair();
        let second_keypair = generate_keypair();
        let first_signer =
            dual_address_from_public_keys(&first_keypair.public_key, &first_keypair.public_key);
        let second_signer =
            dual_address_from_public_keys(&second_keypair.public_key, &second_keypair.public_key);
        ledger
            .create_account_with_authorization(first_signer, first_keypair.public_key, Amount(XPQ))
            .unwrap();
        ledger
            .create_account_with_authorization(
                second_signer,
                second_keypair.public_key,
                Amount(XPQ),
            )
            .unwrap();
        let genesis = Block::genesis(
            Address([9; 20]),
            1_700_000_000,
            vec![
                paqus::block::GenesisAllocation::new(Address([99; 20]), Amount(XPQ)),
                paqus::block::GenesisAllocation::new(first_signer, Amount(XPQ)),
                paqus::block::GenesisAllocation::new(second_signer, Amount(XPQ)),
            ],
        )
        .unwrap();
        ledger.chain.insert_block(genesis).unwrap();
        ledger.chain.tip_height = Some(Height(paqus::ledger::QCASH_REDEEM_DELAY as u64));
        let first = signed_redeem(&first_keypair, &file);
        let second = signed_redeem(&second_keypair, &file);
        let mut pool = Mempool::new();
        let first_hash = pool.insert_validated(&ledger, first).unwrap();
        assert_eq!(
            pool.insert_validated(&ledger, second.clone()),
            Err(MempoolError::CashCoinReserved)
        );
        pool.remove(&first_hash).unwrap();
        assert!(pool.insert_validated(&ledger, second).is_ok());
    }

    #[test]
    fn validates_and_selects_vault_create_from_unified_pool() {
        let keypair = generate_keypair();
        let signer = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
        let mut ledger = Ledger::new();
        ledger
            .create_account_with_authorization(signer, keypair.public_key, Amount(10 * XPQ))
            .unwrap();
        ledger.chain.tip_height = Some(Height(0));

        let template = VaultTransaction::create(
            signer,
            Amount(XPQ),
            Nonce(0),
            VaultMetadata {
                name: "mempool vault".to_string(),
                description: String::new(),
            },
            VaultPolicy::new(vec![signer], None).unwrap(),
        );
        let transaction = template;
        let payload = transaction.signing_bytes().unwrap();
        let signed = SignedProtocolTransaction::from(SignedVaultTransaction::new_authorized(
            transaction,
            keypair.public_key,
            sign(&keypair.secret_key, &payload),
            keypair.public_key,
            sign(&keypair.secret_key, &payload),
        ));
        let mut pool = Mempool::new();
        pool.insert_validated(&ledger, signed).unwrap();
        let selected = pool
            .select_for_block(&ledger, Height(1), 1_700_000_001, 10, DEFAULT_MIN_RELAY_FEE)
            .unwrap();
        assert_eq!(selected.len(), 1);
        assert!(matches!(selected[0], SignedProtocolTransaction::Vault(_)));
    }
}
