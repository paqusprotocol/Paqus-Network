# Paqus 0.2.20 Architecture

Paqus 0.2.20 is split into a small application workspace and a published core
protocol library. The application workspace builds the node and wallet CLI,
while consensus, block, ledger, transaction, QCash, and proof logic come from
the `paqus` crate version `0.2.20`.

```text
Paqus Network/
|-- app-config/      Shared default ports, env names, and app constants
|-- node/            paqus-node binary: P2P, RPC, storage, mining, sync
|-- wallet-core/     Wallet file encryption and wallet data helpers
|-- wallet-cli/      Interactive and command-line wallet application
`-- Cargo.toml       Workspace using paqus = 0.2.20 from crates.io
```

## Protocol Core

The `paqus` crate is the consensus source of truth. Application code imports it
rather than duplicating protocol rules locally.

Core responsibilities:

- Blocks and headers, including canonical block serialization and block weight.
- Ledger state, account balances, account statements, rollback state, and roots.
- Signed protocol transactions, including batch transfer and QCash transaction
  envelopes.
- QCash UTXO state, bearer coin files, redeem rules, and state proofs.
- Monetary policy, block rewards, XPQ decimals, and WBDA difficulty/reward
  adjustment.
- Account and QCash proof verification.

Key consensus parameters in `paqus 0.2.20`:

```text
Coin decimals       : 6
1 XPQ               : 1,000,000 units
Base block reward   : 10 XPQ
Reward bounds       : 1 XPQ .. 20 XPQ
Max block size      : 5 MiB
Max block weight    : 5 MiB
WBDA window         : 2048 blocks
WBDA target weight  : 5 MiB average block weight
WBDA low threshold  : 30%
WBDA high threshold : 70%
PoW algorithm label : argon2id-wbda-weight-v1
QCash redeem delay  : 1 block
```

## Node

`paqus-node` is the network and chain runtime around the core protocol.

Main responsibilities:

- Load or initialize the chain database.
- Maintain the active ledger and canonical chain tip.
- Validate blocks and transactions through the `paqus` core ledger.
- Mine candidate blocks when mining is enabled.
- Serve HTTP RPC for wallets, explorers, proofs, and monitoring.
- Maintain P2P connections and synchronize with peers.
- Store blocks, transaction indexes, protocol events, and rollback data.

Important node modules:

```text
node/src/main.rs                 Process entrypoint and node runtime loop
node/src/command/config.rs       Editable node configuration and CLI flags
node/src/runtime/node/           Ledger orchestration and block application
node/src/runtime/storage/        LMDB-backed block and index storage
node/src/runtime/mempool/        Pending transaction policy and block assembly
node/src/runtime/miner/          Candidate block preparation and mining
node/src/p2p/                    Peer handshake, gossip, sync, inventory
node/src/rpc/                    Public/admin RPC handlers
node/src/snapshot.rs             Authenticated snapshot import/export support
```

## Node Configuration

The node can run from CLI flags or from an editable JSON config.

Generate a config:

```bash
./target/release/paqus-node node config node.json
```

Typical mainnet config:

```json
{
  "network": "mainnet",
  "db_path": "./data/mainnet",
  "listen_addr": ["[::]:5555"],
  "rpc_addr": "127.0.0.1:6666",
  "bootstrap_peers": [],
  "peers": [],
  "public_addr": ["PUBLIC_IP_OR_IPV6:5555"],
  "wallet": "wallet.json",
  "mine": true
}
```

`listen_addr` is the local bind address. `public_addr` is the externally
reachable P2P address announced to peers. RPC should normally stay on loopback.

## P2P Layer

P2P is used for peer discovery, handshake, inventory propagation, block sync,
and transaction gossip.

Node identity on the network is advertised with `public_addr`. Bootstrap peers
are only entry points for discovery and synchronization; they are not chain
authority. Fork choice is based on protocol chainwork and consensus validation.

Default ports:

```text
P2P : 5555
RPC : 6666
```

## RPC Layer

The public RPC is used by wallet CLI, local tooling, and explorers.

Main public RPC groups:

```text
Status       : /status, /health, /metrics, /chain, /stats
Peers        : /peers
Accounts     : /balance/<address>, /accounts, /accounts/statement/<hash>
Blocks       : /blocks/latest, /blocks/<height>, /blocks/hash/<hash>
Transactions : /tx/<hash>, /draft/transfer, /tx, /protocol/transaction
QCash        : /qcash/utxos, /qcash/file/<name>, /qcash/coin/<id>, /qcash/tx
Proofs       : /proof/account/<address>, /proof/qcash/<coin-id>
Events       : /events, /events/<id>, block/tx/address event routes
Mempool      : /mempool, /qcash/mempool
```

The admin RPC is separate and requires explicit admin listen address and token.
It is used for privileged operations such as peer insertion and mining template
submission.

## Wallet

The wallet is split between `wallet-core` and `wallet-cli`.

`wallet-core` handles wallet file primitives. `wallet-cli` provides both
interactive menus and direct commands.

Wallet responsibilities:

- Create and import wallet files.
- Read account state from node RPC.
- Build and sign transfer transactions.
- Build and sign QCash withdraw, redeem, and recovery transactions.
- Store local QCash bearer coin files.
- Track QCash files through node QCash explorer RPC.
- Verify account and QCash proofs/checkpoints.

The wallet never needs direct database access. It talks to the node over RPC.

## Account Model

Accounts are keyed by address and contain:

- Confirmed balance.
- Immature credits.
- Authorization state.
- Current account statement hash.
- Statement height.

Transactions include the previous active account statement as `last_state`.
When a transaction is applied, the account statement advances. This gives each
account a state chain and rejects stale transactions.

The Account Statement Explorer can find both current and historical statements
by replaying canonical blocks when the statement is no longer current.

## QCash Model

QCash is bearer-style off-chain value represented on-chain as QCash UTXOs.

Withdraw flow:

```text
wallet signs QCash withdraw
-> node accepts /qcash/tx
-> miner includes transaction in a block
-> ledger debits account
-> ledger inserts QCash UTXOs
-> wallet stores local .QCash bearer files
```

Redeem flow:

```text
wallet loads .QCash file
-> wallet signs redeem transaction
-> node validates bearer proof and QCash UTXO
-> ledger removes QCash UTXO
-> recipient receives mature on-chain credit
```

QCash explorer endpoints read the live QCash UTXO set:

```text
/qcash/utxos          Global active QCash UTXO list
/qcash/coin/<id>      Lookup by full coin ID
/qcash/file/<name>    Lookup by .QCash filename or short coin ID
```

QCash supply is counted separately from account balances:

```text
total known supply = on-chain account supply + off-chain QCash UTXO supply
```

## Storage

The node storage layer persists:

- Canonical and side blocks.
- Block indexes by height and hash.
- Transaction locations.
- Protocol events.
- QCash and account proof support data.
- Snapshot and rollback support data.

The active ledger is loaded from storage, validated against canonical state
roots, and updated through core ledger transitions.

## Mining

Mining uses the node wallet or configured miner address. The miner builds a
candidate block from:

- Parent tip.
- Current difficulty.
- Current block reward.
- Valid mempool transactions.
- Coinbase output.
- Canonical state root after applying the block.

WBDA adjusts difficulty and reward only at epoch boundaries:

```text
height 2049, 4097, 6145, ...
```

The adjustment uses the previous 2048 completed block weights.

## Supply Accounting

There is no mainnet genesis premine in the current chain parameters.

Supply displayed by the node is:

```text
current_supply = onchain_supply + qcash_offchain_supply
total_known    = onchain_supply + qcash_offchain_supply
mined_supply   = total_known - genesis_premine
```

This means a QCash withdraw does not create new XPQ. It moves value from
on-chain account balance into off-chain QCash UTXO supply.

## Build Outputs

Release binaries are built from the workspace root:

```bash
cargo build --release --locked --bin paqus-node --bin wallet-cli
```

Output:

```text
target/release/paqus-node
target/release/wallet-cli
```

Run from the workspace root:

```bash
./target/release/paqus-node
./target/release/wallet-cli
```
