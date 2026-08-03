# Paqus Wallet CLI

`wallet-cli` is the command-line wallet for Paqus. It manages mnemonic-backed
wallets, balances, transfers, QCash files, explorer queries, protocol events,
proofs, and rollback recovery through a Paqus node RPC.

## Build

From the repository root:

```bash
cargo build --release -p wallet-cli
```

The binary is written to:

```text
target/release/wallet-cli
```

## Interactive menu

Start without arguments:

```bash
./target/release/wallet-cli
```

The menu includes:

```text
1. Create wallet
2. Import wallet
3. Accounts
4. Global chain stats
5. Send coin
6. QCash
7. RPC
8. Block explorer
9. Mempool
10. Hashrate
11. Protocol events
12. Rollback recovery
13. Trusted proof/checkpoint
14. Exit
```

The Accounts menu includes My Accounts, Global Accounts, Address Explorer,
and Account Statement Explorer. The statement explorer resolves the account
whose current statement hash matches the entered value.

When withdrawing QCash with selected denominations, the CLI displays a
numbered list ordered from `1 XPQ` through `1,000,000 XPQ`. Enter the menu
numbers, not the denomination values. Exact counts use `MENU_NUMBERxCOUNT`,
for example `3x2,1x5` selects two `5 XPQ` outputs and five `1 XPQ` outputs.

## Wallet creation and recovery

Create a wallet:

```bash
./target/release/wallet-cli new wallet.json
```

Import a mnemonic interactively:

```bash
./target/release/wallet-cli import wallet.json
```

Keep `wallet.json`, its password, and the recovery phrase private. Do not add
wallet files to Git.

## RPC

The mainnet default is:

```text
127.0.0.1:6666
```

Run `paqus-node` first, then verify the connection from menu `RPC -> Health`,
or use a command directly:

```bash
./target/release/wallet-cli balance --rpc 127.0.0.1:6666
```

For a temporary override:

```bash
PAQUS_RPC_ADDR=127.0.0.1:6666 ./target/release/wallet-cli
```

The wallet currently uses plain HTTP RPC. Keep RPC on the same trusted machine
or trusted private network. Do not expose an unencrypted wallet RPC workflow to
the public internet.

## Common commands

```bash
./target/release/wallet-cli balance
./target/release/wallet-cli stats
./target/release/wallet-cli address-stats
./target/release/wallet-cli hashrate
./target/release/wallet-cli pay ADDRESS AMOUNT_XPQ
./target/release/wallet-cli send ADDRESS AMOUNT_XPQ
./target/release/wallet-cli cash list cash
./target/release/wallet-cli proof status
```

Run the complete built-in reference with:

```bash
./target/release/wallet-cli --help
```

For QCash operations and backup guidance, see [QCASH.md](QCASH.md).
