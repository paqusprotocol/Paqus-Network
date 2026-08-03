use crate::runtime::params::{
    CHAIN_NAME, COIN_NAME, PROTOCOL_STAGE, PROTOCOL_VERSION, SIGNATURE_SCHEME,
};
use paqus::consensus::DIFFICULTY_START;
use paqus::crypto::Hash;
use paqus::genesis::CURRENT_CHAIN_PARAMS;
use paqus::ledger::{CONFIRMATION_DEPTH, FINALITY_DEPTH};
use paqus_app_config::{
    BOOTSTRAP_PEERS_ENV, DEFAULT_P2P_PORT, DEFAULT_RPC_PORT, NODE_P2P_LISTEN_ADDR_ENV,
    NODE_RPC_LISTEN_ADDR_ENV, PUBLIC_ADDR_ENV,
};

pub fn format_hash<T>(hash: Option<T>) -> String
where
    T: Into<Hash>,
{
    hash.map(|hash| hex::encode(hash.into().0))
        .unwrap_or_else(|| "none".to_string())
}

pub fn short_hash<T>(hash: Option<T>) -> String
where
    T: Into<Hash>,
{
    let hash = format_hash(hash);
    if hash.len() <= 16 {
        return hash;
    }
    format!("{}..{}", &hash[..8], &hash[hash.len() - 8..])
}

pub fn format_difficulty(difficulty: Result<u32, impl std::fmt::Display>) -> String {
    difficulty
        .map(|difficulty| difficulty.to_string())
        .unwrap_or_else(|error| format!("error:{error}"))
}

pub fn print_help() {
    println!(
        "\
paqus-node

Usage:
  paqus-node                         Run the node; auto-mines when ../wallet.json or wallet.json exists
  paqus-node --help
  paqus-node version
  paqus-node mine [wallet-path] [db-path]
  paqus-node node info
  paqus-node node config [config-path]
  paqus-node node init [db-path] [miner-address]
  paqus-node node db check [db-path]
  paqus-node node db backup <db-path> <backup-path>
  paqus-node node db restore <backup-path> <db-path>
  paqus-node node snapshot export <db-path> <bundle-path>
  paqus-node node snapshot import <new-db-path> <bundle-path>
  paqus-node node run [db-path] [--network devnet|testnet|mainnet] [--fast-sync] [--config path] [--listen addr] [--rpc-listen addr] [--grpc-listen addr] [--rpc-admin-listen addr --rpc-admin-token token] [--rpc-tls-cert path --rpc-tls-key path] [--rpc-cors-origin origin] [--peer addr] [--peers-file path] [--dns-seed host[:port]] [--gateway host:port] [--public-addr host:port] [--nat-traversal] [--nat-lease-secs n] [--min-relay-fee paqus-per-byte] [--market-fee paqus-per-byte] [--miner-min-fee-rate paqus-per-byte] [--low-fee-expiry-secs n] [--mempool-expiry-secs n] [--wallet path] [--miner address] [--miner-secret-key key-hex] [--mine]

Network defaults and environment:
  P2P listen: [::]:{DEFAULT_P2P_PORT} or ${NODE_P2P_LISTEN_ADDR_ENV}
  RPC listen: 127.0.0.1:{DEFAULT_RPC_PORT} or ${NODE_RPC_LISTEN_ADDR_ENV}
  Public P2P address: ${PUBLIC_ADDR_ENV}
  Bootstrap peers: built-in network peers or ${BOOTSTRAP_PEERS_ENV} (comma-separated)
  Config file values override defaults; environment overrides the config file;
  command-line options override both.

RPC:
  GET  /status
  GET  /health
  GET  /metrics
  GET  /chain
  GET  /stats
  GET  /peers
  GET  /balance/<address>
  GET  /proof/account/<address>
  GET  /proof/qcash/<coin-id>
  GET  /blocks/latest
  GET  /blocks/<height>
  GET  /blocks/hash/<block-hash>
  GET  /tx/<tx-hash>
  GET  /address/<address>
  GET  /draft-basis/<address>
  GET  /accounts
  GET  /accounts/statement/<statement-hash>
  GET  /mempool
  GET  /qcash/mempool
  POST /draft/transfer  JSON: unsigned transfer draft request
  POST /tx              JSON: {{\"tx\":\"signed-transaction-hex\"}}
  POST /qcash/tx        JSON: {{\"tx\":\"signed-qcash-transaction-hex\"}}
  POST /protocol/transaction
                         JSON: {{\"tx\":\"signed-protocol-transaction-hex\"}}

Mempool:
  Transactions do not expire at consensus level. Local mempool age eviction is
  disabled when --mempool-expiry-secs is 0; nodes and miners may still evict or
  ignore low-bid transactions by local policy.

To bootstrap mining with your own account:
  1. Create a wallet: wallet-cli new wallet.json
  2. Start with the editable config: paqus-node node config; paqus-node
  3. Or explicitly: paqus-node mine wallet.json ./data/mainnet
"
    );
}

pub fn print_version() {
    println!(
        "{} {} ({}, protocol {})",
        CHAIN_NAME,
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_STAGE,
        PROTOCOL_VERSION
    );
}

pub fn print_network_info() {
    println!("chain: {CHAIN_NAME}");
    println!("coin: {COIN_NAME}");
    println!("stage: {PROTOCOL_STAGE}");
    println!("signature_scheme: {SIGNATURE_SCHEME}");
    println!("protocol_version: {PROTOCOL_VERSION}");
    println!(
        "pow_argon2_memory_kib: {}",
        CURRENT_CHAIN_PARAMS.pow_memory_kib
    );
    println!(
        "pow_argon2_iterations: {}",
        CURRENT_CHAIN_PARAMS.pow_iterations
    );
    println!("pow_argon2_lanes: {}", CURRENT_CHAIN_PARAMS.pow_lanes);
    println!("confirmation_depth: {CONFIRMATION_DEPTH}");
    println!("finality_depth: {FINALITY_DEPTH}");
    println!("difficulty_start: {DIFFICULTY_START}");
}
