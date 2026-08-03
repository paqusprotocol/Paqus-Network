# Paqus Network Roadmap

## Transaction Fee Market

### Vision

Paqus should evolve from a simple static fee policy into a transaction-native `MarketFee` model.
The node should be able to publish a current fee-rate recommendation, while every user can attach
an explicit fee offer to each transaction. Wallets can then present fees like a marketplace:
users place a bid, nodes/miners expose an ask, and the mempool/miner policy matches transactions
by effective fee rate.

### Goals

- Introduce `MarketFee` as the canonical fee policy vocabulary for wallet, node, mempool, RPC,
  gRPC, and future GUI flows.
- Let the node calculate fee-rate recommendations from local mempool pressure, competing
  transaction fee rates, and next-block clearing estimates.
- Let users choose an explicit per-transaction fee offer instead of relying only on automatic
  defaults.
- Expose a wallet/UI fee marketplace view with slow/normal/fast recommendations, current mempool
  pressure, node ask, and user bid.
- Keep fee policy separate from consensus until the protocol intentionally standardizes a
  network-wide fee rule.

### Proposed Model

- **Ask**: the node/miner policy rate. This is the minimum fee rate the node is willing to relay
  or the miner is willing to include.
- **Bid**: the user's offered transaction fee rate. This is selected automatically by wallet
  policy or manually by the user.
- **Clearing rate**: the estimated fee rate needed for near-term inclusion based on the current
  mempool.
- **Effective fee rate**: the miner-visible fee paid by the transaction, denominated in
  `paqus/vByte`.

### Transaction Design

Current Paqus transfers pay the miner through an output target such as `block_miner`. That means
the transaction can express miner compensation directly as part of its outputs.

The roadmap target is to make this explicit at the model/API level:

- Add a `MarketFee` draft field describing selected rate, estimated virtual size, and estimated
  total fee.
- Let wallets construct a transfer as `outputs + MarketFee`, where the fee may be encoded as a
  block-miner output or as a future dedicated fee field if the protocol migrates that way.
- Preserve canonical signing bytes so the wallet signs the exact draft created or verified by the
  node.
- Reject or deprioritize transactions whose effective fee rate is below node/miner policy.

### Miner Fee Accounting

The block miner should receive the transaction fees for transactions included in the block.
In the current output-based model, this is represented by transaction value directed to
`block_miner`; when the block is executed, that value is paid to the actual miner of that block.

Open design questions:

- Should all fees always go to the block miner, or should the protocol later split fees between
  miner, treasury, burn, or protocol services?
- Should Paqus keep fee payment as an explicit `block_miner` output, or migrate to a dedicated
  transaction fee field?
- Should fee rules stay local policy only, or should there be a consensus-enforced minimum fee
  after the network matures?

### Implementation Phases

1. **Phase 1: Local fee market policy**
   - Keep current `paqus/vByte` fee-rate unit.
   - Use mempool pressure, fee-rate percentiles, and next-block clearing estimates to recommend
     a dynamic fee.
   - Expose detailed fee market data through `/fee-policy`, `/status`, and gRPC.

2. **Phase 2: Wallet bid flow**
   - Add wallet CLI/GUI controls for automatic, slow, normal, fast, and custom fee bids.
   - Show estimated fee, estimated inclusion priority, and current node ask.
   - Include the selected fee offer in node-created transaction drafts.

3. **Phase 3: Mempool fee auction behavior**
   - Sort mempool and block candidates by effective fee rate.
   - Add replacement policy for higher-fee replacement where account state and safety rules allow it.
   - Add low-fee eviction when mempool pressure rises.

4. **Phase 4: Network-wide fee discovery**
   - Gossip compact fee market snapshots between peers.
   - Compare local ask against peer fee markets without making peer policy authoritative.
   - Use multiple peers to produce a better wallet recommendation.

5. **Phase 5: Protocol decision**
   - Decide whether `MarketFee` remains wallet/node policy or becomes part of a future consensus
     transaction version.
   - If consensus-level fees are introduced, define activation, serialization, validation, and
     backward compatibility rules.
