# Paqus Node

`paqus-node` is the full-node binary for the Paqus proof-of-work network. It
provides LMDB storage, Argon2id mining, P2P synchronization, mempool handling,
HTTP RPC, optional TLS/admin RPC, gRPC status, authenticated snapshots, and
QCash support.

For the complete operator guide, see [Running a Paqus Node](../runnode.md).

## Quick start

Build the node and wallet binaries from the repository root:

```bash
cargo build --release -p paqus-node -p wallet-cli
```

Create a miner wallet:

```bash
./target/release/wallet-cli new wallet.json
```

Generate an editable configuration:

```bash
./target/release/paqus-node node config
```

Edit `data/mainnet/node.json`, then start the node:

```bash
./target/release/paqus-node
```

Mainnet defaults:

```text
P2P: [::]:5555
RPC: 127.0.0.1:6666
DB:  ./data/mainnet
```

Check the local RPC:

```bash
curl http://127.0.0.1:6666/health
curl http://127.0.0.1:6666/status
curl http://127.0.0.1:6666/peers
```

## Configuration

Persistent settings belong in `data/mainnet/node.json`. Important fields are:

```json
{
  "network": "mainnet",
  "db_path": "./data/mainnet",
  "listen_addr": ["0.0.0.0:5555", "[::]:5555"],
  "rpc_addr": "127.0.0.1:6666",
  "bootstrap_peers": [],
  "peers": [],
  "public_addr": null,
  "wallet": "wallet.json",
  "mine": true
}
```

- `bootstrap_peers` are initial network entry points.
- `peers` are additional operator-selected static peers.
- `public_addr` contains externally reachable P2P addresses, not RPC addresses.
- `wallet` selects the mining payout wallet.

Configuration precedence is:

```text
network defaults -> node.json -> environment -> command-line options
```

Run `paqus-node --help` for supported environment variables, command-line
overrides, and RPC endpoints.

## Network safety

Bootstrap peers provide connectivity only. They do not define the canonical
chain. Blocks and chain selection remain subject to consensus validation and
cumulative work.

Keep RPC on loopback unless TLS and the RPC security controls are configured.
Only the P2P port needs to be publicly reachable for a normal public node.

## Shutdown

Run the node in the foreground and press `Ctrl+C` for graceful shutdown. From
another terminal:

```bash
pgrep -af paqus-node
kill -INT PID
```
