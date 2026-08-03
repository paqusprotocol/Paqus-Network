use super::Node;
use crate::runtime::params::{BASE_FEE, BLOCK_REWARD_MATURITY, CONFIRMATION_DEPTH};
use crate::test_support::BlockTestExt;
use paqus::block::{Block, BlockError, GenesisAllocation, Height, Nonce};
use paqus::consensus::supply::Amount;
use paqus::consensus::{Consensus, ConsensusConfig, ConsensusError, MIN_DIFFICULTY};
use paqus::crypto::{
    Address, HASH_SIZE, Hash, KeyPair, PreviousHash, dual_address_from_public_keys,
    generate_keypair, sign,
};
use paqus::ledger::Ledger;
use paqus::state::Account;
use paqus::transaction::{SignedTransaction, Transaction};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn address(byte: u8) -> Address {
    Address([byte; 20])
}

fn signed_transaction_to(to: Address, amount: u64, nonce: u64) -> SignedTransaction {
    let keypair = generate_keypair();
    signed_transaction_from_keypair(&keypair, to, amount, nonce)
}

fn signed_transaction_from_keypair(
    keypair: &KeyPair,
    to: Address,
    amount: u64,
    nonce: u64,
) -> SignedTransaction {
    let from = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
    let template = Transaction::new(
        from,
        vec![paqus::transaction::TransferOutput {
            to: to.into(),
            amount: Amount(amount),
        }],
        Nonce(nonce),
    );
    let template_signature = sign(&keypair.secret_key, &template.signing_bytes().unwrap());
    let template_auth_signature = sign(&keypair.secret_key, &template.signing_bytes().unwrap());
    let virtual_size = SignedTransaction::new_authorized(
        template,
        keypair.public_key,
        template_signature,
        keypair.public_key,
        template_auth_signature,
    )
    .virtual_size()
    .unwrap();
    let payload = Transaction::new(
        from,
        vec![paqus::transaction::TransferOutput {
            to: to.into(),
            amount: Amount(amount),
        }],
        Nonce(nonce),
    );
    let signature = sign(&keypair.secret_key, &payload.signing_bytes().unwrap());
    {
        let auth_signature = sign(&keypair.secret_key, &payload.signing_bytes().unwrap());
        SignedTransaction::new_authorized(
            payload,
            keypair.public_key,
            signature,
            keypair.public_key,
            auth_signature,
        )
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn mine_for_test(mut block: Block) -> Block {
    while Consensus::validate_proof_of_work_at_difficulty(&block, block.difficulty()).is_err() {
        block.header.nonce = Nonce(block.header.nonce.0.saturating_add(1));
    }
    block
}

fn block(
    height: u64,
    previous_hash: impl Into<PreviousHash>,
    difficulty: u32,
    nonce: u64,
) -> Block {
    block_with_transactions(height, previous_hash, difficulty, nonce, vec![])
}

fn block_with_transactions(
    height: u64,
    previous_hash: impl Into<PreviousHash>,
    difficulty: u32,
    nonce: u64,
    transactions: Vec<SignedTransaction>,
) -> Block {
    Block::with_difficulty(
        Height(height),
        previous_hash.into(),
        address(9),
        difficulty,
        1_700_000_000,
        Nonce(nonce),
        transactions,
    )
}

#[test]
fn submits_transaction_to_mempool() {
    let transaction = signed_transaction_to(address(2), 10, 0);
    let sender = transaction.transaction.from;
    let mut ledger = Ledger::new();
    ledger.create_account(sender, Amount(1_000_000)).unwrap();
    ledger.create_account(address(2), Amount(0)).unwrap();
    let mut node = Node::temporary(
        ledger,
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
    )
    .unwrap();

    let hash = transaction.hash().unwrap();

    assert_eq!(node.submit_transaction(transaction).unwrap(), hash);
    assert!(node.mempool.contains(&hash));
}

#[test]
fn mines_and_applies_block_from_mempool() {
    let transaction = signed_transaction_to(address(2), 10, 0);
    let sender = transaction.transaction.from;
    let miner = address(9);
    let initial_balance = paqus::consensus::block_reward(Height(1));
    let mut accounts = BTreeMap::new();
    accounts.insert(sender, Account::new(sender, initial_balance));
    let mut ledger = Ledger::from_accounts_and_chain(accounts, Default::default()).unwrap();
    let genesis = mine_for_test(Block::genesis(miner, 1_700_000_000, vec![]).unwrap());
    let mut funding = Block::new(
        Height(1),
        genesis.hash().unwrap(),
        sender,
        1_700_000_001,
        Nonce(0),
        vec![],
    );
    funding.set_state_root(ledger.state_root());
    let funding = mine_for_test(funding);
    ledger.chain.insert_block(genesis).unwrap();
    ledger.chain.insert_block(funding).unwrap();
    let mut node = Node::temporary(
        ledger,
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
    )
    .unwrap();

    node.submit_transaction(transaction).unwrap();
    let result = node.mine_block(miner, 1_700_000_002, 2_000, 10).unwrap();

    assert_eq!(result.block.height(), Height(2));
    assert_eq!(node.tip_height(), Some(Height(2)));
    assert!(node.mempool.is_empty());
    let expected_sender = initial_balance.0.saturating_sub(10);
    assert_eq!(node.balance(&sender), Some(Amount(expected_sender)));
    assert_eq!(node.balance(&address(2)), Some(Amount(10)));
}

#[test]
fn initializes_new_storage_with_frozen_genesis() {
    let dir = tempfile_dir();
    let consensus = Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap();
    let mut node = Node::init_or_load(&dir, consensus).unwrap();
    let miner = address(7);
    let timestamp = 1_700_000_123;

    assert_eq!(node.tip_height(), Some(Height(0)));
    assert_eq!(
        node.tip_hash().map(|hash| hash.0),
        Some(paqus::genesis::GENESIS_HASH)
    );
    assert_eq!(node.next_difficulty().unwrap(), MIN_DIFFICULTY);

    let result = node.mine_block(miner, timestamp, 100, 10).unwrap();
    assert_eq!(result.block.height(), Height(1));
    assert_eq!(result.block.miner_address(), miner);
    assert_eq!(result.block.timestamp(), timestamp);
    assert_eq!(node.tip_height(), Some(Height(1)));
}

#[test]
fn stores_side_fork_without_changing_active_tip_when_work_is_lower() {
    let genesis = Block::new(
        Height(0),
        Hash([0; HASH_SIZE]),
        address(9),
        1_700_000_000,
        Nonce(0),
        vec![],
    );
    let mut active = block(1, genesis.hash().unwrap(), 1, 1);
    let mut side = block(1, genesis.hash().unwrap(), 1, 2);
    let genesis_accounts = BTreeMap::new();
    let mut ledger =
        Ledger::from_accounts_and_chain(genesis_accounts.clone(), Default::default()).unwrap();
    ledger.chain.insert_block(genesis).unwrap();
    active = mine_for_test(active);
    side = mine_for_test(side);
    active.set_state_root(ledger.state_root_after_block(&active).unwrap());
    side.set_state_root(ledger.state_root_after_block(&side).unwrap());
    active = mine_for_test(active);
    side = mine_for_test(side);
    let side_hash = side.hash().unwrap();
    let now = active.timestamp();
    ledger.apply_block_at(active, now).unwrap();
    let mut active_child = block(2, ledger.tip_hash().unwrap(), 1, 3);
    active_child = mine_for_test(active_child);
    active_child.set_state_root(ledger.state_root_after_block(&active_child).unwrap());
    active_child = mine_for_test(active_child);
    let active_hash = active_child.hash().unwrap();
    let now = active_child.timestamp();
    ledger.apply_block_at(active_child, now).unwrap();
    let mut node = Node::with_genesis_accounts(
        ledger,
        crate::runtime::storage::Storage::temporary().unwrap(),
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
        genesis_accounts,
    );

    assert!(node.apply_block(side).is_ok());
    assert!(node.fork_choice.contains(&side_hash));
    assert_eq!(node.tip_hash(), Some(active_hash));
}

#[test]
fn rejects_block_with_unexpected_difficulty() {
    let genesis = Block::new(
        Height(0),
        Hash([0; HASH_SIZE]),
        address(9),
        1_700_000_000,
        Nonce(0),
        vec![],
    );
    let mut ledger = Ledger::new();
    ledger.chain.insert_block(genesis.clone()).unwrap();
    let storage = crate::runtime::storage::Storage::temporary().unwrap();
    storage.save_ledger(&ledger).unwrap();
    let mut node = Node::new(ledger, storage, Consensus::with_default_config());
    let block = block(1, genesis.hash().unwrap(), 2, 1);

    let error = node.apply_block(block).unwrap_err();

    assert!(matches!(
        error,
        crate::runtime::node::NodeError::Consensus(ConsensusError::UnexpectedDifficulty)
    ));
}

#[test]
fn accepts_future_timestamp_as_non_rhythm_metadata() {
    let genesis = Block::new(
        Height(0),
        Hash([0; HASH_SIZE]),
        address(9),
        1_700_000_000,
        Nonce(0),
        vec![],
    );
    let mut ledger = Ledger::new();
    ledger.chain.insert_block(genesis.clone()).unwrap();
    let mut node = Node::temporary(
        ledger,
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
    )
    .unwrap();
    let result = node.mine_block(address(9), u64::MAX, 100_000, 10).unwrap();
    assert_eq!(result.block.timestamp(), u64::MAX);
    assert_eq!(node.tip_height(), Some(Height(1)));
}

#[test]
#[cfg(not(feature = "mainnet"))]
fn reorgs_state_when_side_fork_becomes_best_tip() {
    let active_keypair = generate_keypair();
    let side_keypair = generate_keypair();
    let active_sender =
        dual_address_from_public_keys(&active_keypair.public_key, &active_keypair.public_key);
    let side_sender =
        dual_address_from_public_keys(&side_keypair.public_key, &side_keypair.public_key);
    let active_receiver = address(3);
    let side_receiver = address(4);
    let active_transaction =
        signed_transaction_from_keypair(&active_keypair, active_receiver, 10, 0);
    let side_transaction = signed_transaction_from_keypair(&side_keypair, side_receiver, 10, 0);
    let mut genesis = Block::genesis(
        address(9),
        1_700_000_000,
        vec![
            GenesisAllocation::new(active_sender, Amount(1_000_000_000)),
            GenesisAllocation::new(side_sender, Amount(1_000_000_000)),
        ],
    )
    .unwrap();
    let mut active = block_with_transactions(
        1,
        genesis.hash().unwrap(),
        1,
        1,
        vec![active_transaction.clone()],
    );
    let mut side =
        block_with_transactions(1, genesis.hash().unwrap(), 1, 2, vec![side_transaction]);
    let mut genesis_accounts = BTreeMap::new();
    genesis_accounts.insert(
        active_sender,
        Account::new(active_sender, Amount(1_000_000_000)),
    );
    genesis_accounts.insert(
        side_sender,
        Account::new(side_sender, Amount(1_000_000_000)),
    );
    genesis_accounts.insert(active_receiver, Account::new(active_receiver, Amount(0)));
    genesis_accounts.insert(side_receiver, Account::new(side_receiver, Amount(0)));
    genesis_accounts.insert(address(9), Account::new(address(9), Amount(0)));
    let mut ledger =
        Ledger::from_accounts_and_chain(genesis_accounts.clone(), Default::default()).unwrap();
    genesis.set_state_root(ledger.state_root());
    genesis = mine_for_test(genesis);
    active.header.previous_hash = genesis.hash().unwrap().into();
    side.header.previous_hash = genesis.hash().unwrap().into();
    ledger.chain.insert_block(genesis).unwrap();
    active = mine_for_test(active);
    side = mine_for_test(side);
    active.set_state_root(ledger.state_root_after_block(&active).unwrap());
    side.set_state_root(ledger.state_root_after_block(&side).unwrap());
    active = mine_for_test(active);
    side = mine_for_test(side);
    let mut side_ledger = ledger.clone();
    side_ledger
        .apply_block_at(side.clone(), side.timestamp())
        .unwrap();
    let mut side_child = block(2, side.hash().unwrap(), 1, 3);
    side_child = mine_for_test(side_child);
    side_child.set_state_root(side_ledger.state_root_after_block(&side_child).unwrap());
    side_child = mine_for_test(side_child);
    let side_hash = side_child.hash().unwrap();
    let now = active.timestamp();
    ledger.apply_block_at(active, now).unwrap();
    let mut node = Node::with_genesis_accounts(
        ledger,
        crate::runtime::storage::Storage::temporary().unwrap(),
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
        genesis_accounts,
    );

    node.apply_block(side)
        .expect("side block should enter fork choice");
    node.apply_block(side_child)
        .expect("side child should trigger the reorganization");
    assert_eq!(
        node.fork_choice.best_tip().map(|node| node.hash),
        Some(side_hash)
    );
    assert_eq!(node.tip_hash(), Some(side_hash));
    assert_eq!(node.balance(&active_sender), Some(Amount(1_000_000_000)));
    assert_eq!(node.balance(&active_receiver), Some(Amount(0)));
    let expected_side_sender = 1_000_000_000_u64.saturating_sub(10).saturating_sub(
        node.ledger.block(&Height(1)).unwrap().transactions[0]
            .total_amount()
            .unwrap_or(Amount(0))()
        .0,
    );
    assert_eq!(
        node.balance(&side_sender),
        Some(Amount(expected_side_sender))
    );
    assert_eq!(node.balance(&side_receiver), Some(Amount(10)));
    assert!(node.mempool.contains(&active_transaction.hash().unwrap()));
}

#[test]
fn reports_confirmed_available_and_pending_balances() {
    let keypair = generate_keypair();
    let sender = dual_address_from_public_keys(&keypair.public_key, &keypair.public_key);
    let receiver = address(2);
    let miner = address(9);
    let mut genesis_accounts = BTreeMap::new();
    let fee_per_transaction = signed_transaction_from_keypair(&keypair, receiver, 1, 0)
        .transaction
        .total_amount()
        .unwrap_or(Amount(0))
        .0;
    let initial_sender_balance = 1_000_000_000;
    genesis_accounts.insert(sender, Account::new(sender, Amount(initial_sender_balance)));
    genesis_accounts.insert(receiver, Account::new(receiver, Amount(0)));
    genesis_accounts.insert(miner, Account::new(miner, Amount(0)));
    let mut genesis = Block::genesis(
        miner,
        1_700_000_000,
        vec![GenesisAllocation::new(
            sender,
            Amount(initial_sender_balance),
        )],
    )
    .unwrap();

    let mut ledger =
        Ledger::from_accounts_and_chain(genesis_accounts.clone(), Default::default()).unwrap();
    genesis.set_state_root(ledger.state_root());
    genesis = mine_for_test(genesis);
    ledger.chain.insert_block(genesis).unwrap();

    for height in 1..=11 {
        let transaction = signed_transaction_from_keypair(&keypair, receiver, 1, height - 1);
        let mut block = block_with_transactions(
            height,
            ledger.tip_hash().unwrap(),
            1,
            height,
            vec![transaction],
        );
        block = mine_for_test(block);
        block.set_state_root(ledger.state_root_after_block(&block).unwrap());
        block = mine_for_test(block);
        let now = block.timestamp();
        ledger.apply_block_at(block, now).unwrap();
    }

    let mut node = Node::with_genesis_accounts(
        ledger,
        crate::runtime::storage::Storage::temporary().unwrap(),
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
        genesis_accounts,
    );
    let pending_transaction = signed_transaction_from_keypair(&keypair, receiver, 5, 11);

    node.submit_transaction(pending_transaction).unwrap();

    let confirmed_sender = initial_sender_balance - 11;
    assert_eq!(
        node.confirmed_balance(&sender),
        Some(Amount(confirmed_sender))
    );
    let available_receiver = 11_u64.saturating_sub(CONFIRMATION_DEPTH as u64);
    assert_eq!(
        node.confirmed_balance(&receiver),
        Some(Amount(available_receiver))
    );
    assert_eq!(
        node.available_balance(&sender),
        Some(Amount(confirmed_sender))
    );
    assert_eq!(
        node.available_balance(&receiver),
        Some(Amount(available_receiver))
    );
    assert_eq!(
        node.account_view(&receiver),
        Some(crate::runtime::node::AccountView {
            balance: Amount(available_receiver),
            unspendable: Amount(11 - available_receiver),
            nonce: Nonce(0),
        })
    );

    let sender_pending = node.pending_balance(&sender);
    assert_eq!(sender_pending.incoming, Amount(0));
    assert_eq!(sender_pending.outgoing, Amount(5));

    let receiver_pending = node.pending_balance(&receiver);
    assert_eq!(receiver_pending.incoming, Amount(5));
    assert_eq!(receiver_pending.outgoing, Amount(0));

    assert_eq!(
        node.account_view(&sender),
        Some(crate::runtime::node::AccountView {
            balance: Amount(confirmed_sender),
            unspendable: Amount(0),
            nonce: Nonce(11),
        })
    );
}

#[test]
fn keeps_mining_rewards_unspendable_until_maturity() {
    let miner = address(9);
    let reward = paqus::consensus::block_reward(Height(1));
    let mut miner_account = Account::new(miner, Amount(0));
    miner_account
        .credit_at_maturity(
            reward,
            Height(BLOCK_REWARD_MATURITY as u64),
            paqus::state::CreditSource::MiningReward,
        )
        .unwrap();
    let mut genesis_accounts = BTreeMap::new();
    genesis_accounts.insert(miner, miner_account);
    let mut genesis = Block::genesis(
        miner,
        1_700_000_000,
        vec![GenesisAllocation::new(miner, reward)],
    )
    .unwrap();

    let mut ledger =
        Ledger::from_accounts_and_chain(genesis_accounts.clone(), Default::default()).unwrap();
    genesis.set_state_root(ledger.state_root());
    genesis = mine_for_test(genesis);
    ledger.chain.insert_block(genesis).unwrap();

    let mut node = Node::with_genesis_accounts(
        ledger,
        crate::runtime::storage::Storage::temporary().unwrap(),
        Consensus::new(ConsensusConfig::new(paqus::consensus::MIN_DIFFICULTY)).unwrap(),
        genesis_accounts,
    );

    assert_eq!(
        node.account_view(&miner).unwrap(),
        crate::runtime::node::AccountView {
            balance: Amount(0),
            unspendable: reward,
            nonce: Nonce(0),
        }
    );

    node.ledger.chain.tip_height = Some(Height(BLOCK_REWARD_MATURITY as u64));
    assert_eq!(
        node.account_view(&miner).unwrap(),
        crate::runtime::node::AccountView {
            balance: reward,
            unspendable: Amount(0),
            nonce: Nonce(0),
        }
    );
}

fn tempfile_dir() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "full-node-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    path
}
