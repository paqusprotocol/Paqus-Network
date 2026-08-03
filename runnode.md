# Running a Paqus Node

This guide covers the recommended binary-based setup for a local node, miner,
and public bootstrap node. Node settings are stored in an editable JSON file,
so normal operation does not require a long command line.

## 1. Build the binaries

From the project root:

```bash
cargo build --release -p paqus-node -p wallet-cli
```

The resulting binaries are:

```text
target/release/paqus-node
target/release/wallet-cli
```

If the binaries are copied to another directory, make them executable once:

```bash
chmod +x paqus-node wallet-cli
```

## 2. Create a mining wallet

Create `wallet.json` in the directory from which the node will be started:

```bash
./wallet-cli new wallet.json
```

To restore an existing mnemonic:

```bash
./wallet-cli import wallet.json
```

Keep the wallet file and recovery phrase private. The public wallet address is
safe to use as the mining payout address, but private wallet material must
never be placed in a public configuration repository.

## 3. Generate the editable node configuration

Run once:

```bash
./paqus-node node config
```

For a mainnet binary this creates:

```text
data/mainnet/node.json
```

The binary automatically reads this file on later starts. The generated file
already contains:

```json
"wallet": "wallet.json",
"mine": true
```

After configuration, normal startup is simply:

```bash
./paqus-node
```

The node also auto-mines when `wallet.json` or `../wallet.json` exists, even if
no configuration file has been generated.

## 4. Local node configuration

For a node used only by applications on the same machine, keep RPC bound to
loopback:

```json
{
  "network": "mainnet",
  "db_path": "./data/mainnet",
  "listen_addr": [
    "[::]:5555"
  ],
  "rpc_addr": "127.0.0.1:6666",
  "bootstrap_peers": [
    "BOOTSTRAP_PUBLIC_IPV4:5555",
    "[BOOTSTRAP_PUBLIC_IPV6]:5555"
  ],
  "peers": [],
  "public_addr": null,
  "wallet": "wallet.json",
  "mine": true
}
```

The generated configuration contains additional safety, fee, peer, and
resource-limit fields. Keep those fields unless their behavior is understood.

Default ports are network-specific:

| Network | P2P | RPC |
| --- | ---: | ---: |
| Mainnet | 5555 | 6666 |
| Testnet | 15555 | 16666 |
| Devnet | 25555 | 26666 |

The wallet CLI uses the matching local RPC address automatically.

## 5. Public IPv4 and IPv6 node

To accept incoming peers over both IP families, edit `listen_addr` and
`public_addr`:

```json
{
  "listen_addr": [
    "0.0.0.0:5555",
    "[::]:5555"
  ],
  "rpc_addr": "127.0.0.1:6666",
  "bootstrap_peers": [],
  "peers": [],
  "public_addr": [
    "YOUR_PUBLIC_IPV4:5555",
    "[YOUR_PUBLIC_IPV6]:5555"
  ],
  "wallet": "wallet.json",
  "mine": true
}
```

For the first bootstrap laptop, both `"bootstrap_peers": []` and `"peers": []`
are correct because it is the initial node. Other nodes should point
`bootstrap_peers` to that laptop after its public addresses are reachable:

```json
"bootstrap_peers": [
  "BOOTSTRAP_PUBLIC_IPV4:5555",
  "[BOOTSTRAP_PUBLIC_IPV6]:5555"
]
```

IPv6 socket addresses must use brackets. For example:

```text
[2001:db8:1234::10]:5555
```

`listen_addr` selects local interfaces. `public_addr` is the externally
reachable P2P address announced to other nodes. Do not put the RPC address in
`public_addr`.

Open the P2P port in the host firewall:

```bash
sudo ufw allow 5555/tcp
```

For IPv4 behind a router, forward TCP port `5555` to the node's local IPv4
address. An ISP using CGNAT normally requires a public IP, VPS, or suitable
tunnel before inbound IPv4 connections can work. For IPv6, allow inbound TCP
port `5555` in both the host and router firewalls.

Keep RPC on `127.0.0.1` unless TLS and the RPC security controls have been
configured. Public P2P and public RPC are separate concerns.

## 6. Bootstrap node configuration

A bootstrap node is an ordinary public node that stays online and has a stable,
reachable address. Once its IPv4 and IPv6 addresses have been verified, place
them in the shared application configuration:

```rust
pub const BOOTSTRAP_PEER_IPV4: &str = "PUBLIC_IPV4:5555";
pub const BOOTSTRAP_PEER_IPV6: &str = "[PUBLIC_IPV6]:5555";
```

These constants are located in:

```text
app-config/src/lib.rs
```

Changing Rust constants requires rebuilding the distributed binaries. For a
temporary bootstrap list, use `bootstrap_peers` in `node.json` instead:

```json
"bootstrap_peers": [
  "PUBLIC_IPV4:5555",
  "[PUBLIC_IPV6]:5555"
]
```

The two fields have distinct purposes:

```json
"bootstrap_peers": ["INITIAL_DISCOVERY_NODE:5555"],
"peers": ["OPERATOR_STATIC_PEER:5555"]
```

`bootstrap_peers` provides initial network entry points. `peers` contains
additional static peers selected by the operator. Both are connectivity hints;
chain selection still follows validated consensus and cumulative work.

The first bootstrap node does not need another peer to start. It must remain
online so new nodes can discover and synchronize from it. For production,
operate at least two bootstrap nodes on separate machines or networks.

## 7. Start and monitor the node

Start in the foreground so activity logs remain visible:

```bash
./paqus-node
```

Typical startup messages include:

```text
[P2P] listening ...
[RPC] listening label=public addr=127.0.0.1:6666 tls=false
```

From another terminal, verify the process and listeners:

```bash
pgrep -af paqus-node
ss -ltnp | grep -E ':(5555|6666)\b'
```

Verify local RPC:

```bash
curl http://127.0.0.1:6666/health
curl http://127.0.0.1:6666/status
curl http://127.0.0.1:6666/peers
curl http://127.0.0.1:6666/chain
```

Test public P2P reachability from a different internet connection:

```bash
nc -vz YOUR_PUBLIC_IPV4 5555
nc -6 -vz YOUR_PUBLIC_IPV6 5555
```

A successful local listener check does not prove that the port is reachable
from the internet; an external test is required.

## 8. Connect the wallet CLI

On the same machine:

```bash
./wallet-cli
```

The mainnet wallet defaults to `127.0.0.1:6666`. A temporary override is:

```bash
PAQUS_RPC_ADDR=127.0.0.1:6666 ./wallet-cli
```

Inside the interactive wallet menu, select `RPC`, then `Health` or `Status` to
verify the connection.

## 9. Runtime environment overrides

JSON is recommended for persistent settings. Environment variables are useful
for containers and temporary changes:

```bash
PAQUS_NODE_P2P_LISTEN_ADDR='0.0.0.0:5555,[::]:5555' \
PAQUS_NODE_RPC_LISTEN_ADDR='127.0.0.1:6666' \
PAQUS_PUBLIC_ADDR='PUBLIC_IPV4:5555,[PUBLIC_IPV6]:5555' \
PAQUS_BOOTSTRAP_PEERS='PEER_IPV4:5555,[PEER_IPV6]:5555' \
./paqus-node
```

Configuration precedence is:

```text
network defaults -> node.json -> environment -> command-line options
```

## 10. Stop the node safely

When running in the foreground, press:

```text
Ctrl+C
```

The node observes the shutdown signal and exits its main loop. If the node was
started from another terminal, locate its PID and send `SIGINT`:

```bash
pgrep -af paqus-node
kill -INT PID
```

Avoid force-killing the process during database writes unless graceful shutdown
is no longer possible.

## 11. Troubleshooting

### Wallet cannot connect to RPC

Confirm that the node logs show RPC on `127.0.0.1:6666`, then run:

```bash
curl http://127.0.0.1:6666/health
```

Make sure `PAQUS_RPC_ADDR` is not overriding the wallet with an old address:

```bash
printenv PAQUS_RPC_ADDR
```

### Node is listening locally but unreachable publicly

Check the host firewall, router port forwarding, IPv6 firewall, and CGNAT
status. Test from a different network rather than from the node itself.

### Node starts without mining

Verify these settings:

```json
"wallet": "wallet.json",
"mine": true
```

Also confirm that the wallet path is relative to the directory from which the
binary is launched.
