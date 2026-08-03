async fn rpc_tx(
    State(state): State<RpcState>,
    AxumPath(hash): AxumPath<String>,
) -> impl IntoResponse {
    let hash = match parse_hash_hex(&hash) {
        Ok(hash) => TransactionHash(hash.0),
        Err(error) => return rpc_error(StatusCode::BAD_REQUEST, error),
    };
    match state.node.lock() {
        Ok(node) => {
            for transaction in node.mempool.transactions() {
                if transaction.hash().is_ok_and(|txid| txid == hash) {
                    return match protocol_tx_response(transaction, None, None, None) {
                        Ok(response) => Json(response).into_response(),
                        Err(error) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, error),
                    };
                }
            }
            match node.storage.load_protocol_transaction(&hash) {
                Ok(Some((location, transaction))) => {
                    match protocol_tx_response(
                        &transaction,
                        Some(location.block_height),
                        Some(location.block_hash),
                        node.tip_height(),
                    ) {
                        Ok(response) => Json(response).into_response(),
                        Err(error) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, error),
                    }
                }
                Ok(None) => rpc_error(StatusCode::NOT_FOUND, "transaction_not_found"),
                Err(error) => rpc_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to load transaction: {error}"),
                ),
            }
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_address(
    State(state): State<RpcState>,
    AxumPath(address): AxumPath<String>,
) -> impl IntoResponse {
    let address = match parse_address_string(&address) {
        Ok(address) => address,
        Err(error) => return rpc_error(StatusCode::BAD_REQUEST, error),
    };
    match state.node.lock() {
        Ok(node) => {
            Json(AddressResponse {
                address: address_to_string(&address),
                balance: balance_value(&node, &address),
                mined_blocks: Vec::new(),
                transactions: Vec::new(),
            })
            .into_response()
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_accounts(State(state): State<RpcState>) -> impl IntoResponse {
    match state.node.lock() {
        Ok(node) => {
            let height = node.tip_height().unwrap_or(Height(0));
            let accounts = node
                .ledger
                .accounts()
                .values()
                .map(|account| {
                    let pending = node.pending_balance(&account.address);
                    account_response(account, height, pending, true)
                })
                .collect::<Vec<_>>();
            Json(accounts).into_response()
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_account_by_statement(
    State(state): State<RpcState>,
    AxumPath(statement): AxumPath<String>,
) -> impl IntoResponse {
    let statement = match parse_hash_hex(&statement) {
        Ok(statement) => statement,
        Err(error) => return rpc_error(StatusCode::BAD_REQUEST, error),
    };
    match state.node.lock() {
        Ok(node) => {
            match find_account_statement(&node, statement) {
                Ok(Some((account, height, current))) => {
                    let pending = if current {
                        node.pending_balance(&account.address)
                    } else {
                        crate::runtime::node::node::PendingBalance::default()
                    };
                    Json(account_response(&account, height, pending, current)).into_response()
                }
                Ok(None) => rpc_error(StatusCode::NOT_FOUND, "account_statement_not_found"),
                Err(error) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, error),
            }
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

fn account_response(
    account: &paqus::state::Account,
    height: Height,
    pending: crate::runtime::node::node::PendingBalance,
    statement_current: bool,
) -> AccountResponse {
    AccountResponse {
        address: address_to_string(&account.address),
        confirmed: account.balance.0,
        available: account.available_balance_at(height).0,
        unspendable: account.immature_balance_at(height).0,
        pending_incoming: pending.incoming.0,
        pending_outgoing: pending.outgoing.0,
        nonce: 0,
        statement: hex::encode(account.statement.0),
        statement_height: account.statement_height.0,
        statement_current,
        authorization_registered: account.authorization.is_some(),
        credits: account.credits.len(),
    }
}

fn find_account_statement(
    node: &Node,
    statement: paqus::crypto::Hash,
) -> Result<Option<(paqus::state::Account, Height, bool)>, String> {
    if let Some(account) = node
        .ledger
        .accounts()
        .values()
        .find(|account| account.statement == statement)
    {
        return Ok(Some((
            account.clone(),
            node.tip_height().unwrap_or(account.statement_height),
            true,
        )));
    }

    let mut replay = paqus::genesis::genesis_ledger().map_err(|error| error.to_string())?;
    if let Some(account) = replay
        .accounts()
        .values()
        .find(|account| account.statement == statement)
    {
        return Ok(Some((account.clone(), Height(0), false)));
    }
    let tip = node.tip_height().unwrap_or(Height(0));
    for height in 1..=tip.0 {
        let block = node
            .storage
            .load_block_by_height(Height(height))
            .map_err(|error| format!("failed to load account statement history: {error}"))?
            .ok_or_else(|| format!("canonical block {height} is unavailable"))?;
        replay
            .apply_block(block)
            .map_err(|error| format!("failed to replay account statement history: {error}"))?;
        if let Some(account) = replay
            .accounts()
            .values()
            .find(|account| account.statement == statement)
        {
            return Ok(Some((account.clone(), Height(height), false)));
        }
    }
    Ok(None)
}

async fn rpc_mempool(State(state): State<RpcState>) -> impl IntoResponse {
    match state.node.lock() {
        Ok(node) => {
            let transactions = node
                .mempool
                .transactions()
                .filter_map(|transaction| protocol_tx_response(transaction, None, None, None).ok())
                .collect::<Vec<_>>();
            Json(MempoolResponse {
                size: transactions.len(),
                transactions,
            })
            .into_response()
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_qcash_mempool(State(_state): State<RpcState>) -> impl IntoResponse {
    Json(serde_json::json!({ "size": 0, "transactions": [] })).into_response()
}

fn balance_json(node: &Node, address: &Address) -> String {
    balance_value(node, address).to_string()
}

fn balance_value(node: &Node, address: &Address) -> serde_json::Value {
    let height = node.tip_height().unwrap_or(Height(0));
    let pending = node.pending_balance(address);
    match node.ledger.account(address) {
        Some(account) => serde_json::json!({
            "address": address_to_string(address),
            "height": height.0,
            "exists": true,
            "confirmed": account.balance.0,
            "available": account.available_balance_at(height).0,
            "pending_incoming": pending.incoming.0,
            "pending_outgoing": pending.outgoing.0,
            "nonce": null,
            "statement": hex::encode(account.statement.0),
            "statement_height": account.statement_height.0,
            "authorization_registered": account.authorization.is_some(),
            "unspendable": account.immature_balance_at(height).0
        }),
        None => serde_json::json!({
            "address": address_to_string(address),
            "height": height.0,
            "exists": false,
            "confirmed": 0,
            "available": 0,
            "pending_incoming": 0,
            "pending_outgoing": 0,
            "nonce": null,
            "statement": null,
            "statement_height": null,
            "authorization_registered": false,
            "unspendable": 0
        }),
    }
}

fn chain_stats(node: &Node) -> Result<ChainStatsResponse, String> {
    let height = node.tip_height().map(|height| height.0).unwrap_or(0);
    let onchain_supply = node
        .ledger
        .total_supply()
        .map_err(|error| format!("failed to calculate on-chain supply: {error}"))?
        .0;
    let qcash_offchain_supply = node
        .ledger
        .qcash_utxos
        .total_value()
        .map_err(|error| format!("failed to calculate QCash supply: {error}"))?
        .0;
    let qcash_redeemable_supply = node
        .ledger
        .qcash_utxos
        .redeemable_balance_at(Height(height))
        .map_err(|error| format!("failed to calculate redeemable QCash supply: {error}"))?
        .0;
    let qcash_pending_supply = qcash_offchain_supply
        .checked_sub(qcash_redeemable_supply)
        .ok_or_else(|| "redeemable QCash supply exceeds total QCash supply".to_string())?;
    let total_known_supply = onchain_supply
        .checked_add(qcash_offchain_supply)
        .ok_or_else(|| "total known supply overflow".to_string())?;
    let genesis_premine = node
        .ledger
        .chain
        .block(&Height(0))
        .ok_or_else(|| "genesis block is unavailable".to_string())?
        .genesis_allocations()
        .iter()
        .try_fold(0_u64, |total, allocation| {
            total.checked_add(allocation.amount.0)
        })
        .ok_or_else(|| "genesis premine overflow".to_string())?;
    let mined_supply = total_known_supply
        .checked_sub(genesis_premine)
        .ok_or_else(|| "genesis premine exceeds current supply".to_string())?;
    Ok(ChainStatsResponse {
        chain: CHAIN_NAME,
        coin: COIN_NAME,
        height,
        blocks: height.saturating_add(1),
        genesis_premine,
        mined_supply,
        onchain_supply,
        qcash_offchain_supply,
        qcash_redeemable_supply,
        qcash_pending_supply,
        total_known_supply,
        current_supply: total_known_supply,
        miner_income: 0,
        service_revenue: 0,
        total_transactions: 0,
        transfer_transactions: 0,
        pending_transactions: node.mempool.len() as u64,
        total_transfer_volume: 0,
        total_transaction_fees: 0,
        average_transfer_amount: 0,
    })
}

async fn rpc_qcash_utxos(State(state): State<RpcState>) -> impl IntoResponse {
    match state.node.lock() {
        Ok(node) => {
            let height = node.tip_height().unwrap_or(Height(0));
            let utxos = node
                .ledger
                .qcash_utxos
                .coins()
                .map(|coin| qcash_utxo_value(coin, height))
                .collect::<Vec<_>>();
            Json(serde_json::json!({
                "height": height.0,
                "total": utxos.len(),
                "utxos": utxos,
            }))
            .into_response()
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_qcash_coin(
    State(state): State<RpcState>,
    AxumPath(coin_id): AxumPath<String>,
) -> impl IntoResponse {
    let hash = match parse_hash_hex(&coin_id) {
        Ok(hash) => hash,
        Err(error) => return rpc_error(StatusCode::BAD_REQUEST, error),
    };
    match state.node.lock() {
        Ok(node) => {
            let height = node.tip_height().unwrap_or(Height(0));
            match node
                .ledger
                .qcash_utxos
                .coin(paqus::state::QCashCoinId(hash.0))
            {
                Some(coin) => Json(qcash_utxo_value(coin, height)).into_response(),
                None => Json(serde_json::json!({
                    "coin_id": coin_id.to_ascii_lowercase(),
                    "height": height.0,
                    "status": "spent_or_unknown",
                }))
                .into_response(),
            }
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

async fn rpc_qcash_file(
    State(state): State<RpcState>,
    AxumPath(name): AxumPath<String>,
) -> impl IntoResponse {
    let prefix = match qcash_file_lookup_prefix(&name) {
        Ok(prefix) => prefix,
        Err(error) => return rpc_error(StatusCode::BAD_REQUEST, error),
    };
    match state.node.lock() {
        Ok(node) => {
            let height = node.tip_height().unwrap_or(Height(0));
            let matches = node
                .ledger
                .qcash_utxos
                .coins()
                .filter(|coin| {
                    hex::encode_upper(coin.id.0).starts_with(&prefix.to_ascii_uppercase())
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [coin] => {
                    let mut value = qcash_utxo_value(coin, height);
                    if let Some(object) = value.as_object_mut() {
                        object.insert("lookup".into(), serde_json::json!(name));
                    }
                    Json(value).into_response()
                }
                [] => Json(serde_json::json!({
                    "lookup": name,
                    "coin_id_prefix": prefix,
                    "height": height.0,
                    "status": "spent_or_unknown",
                    "matches": 0,
                }))
                .into_response(),
                _ => Json(serde_json::json!({
                    "lookup": name,
                    "coin_id_prefix": prefix,
                    "height": height.0,
                    "status": "ambiguous",
                    "matches": matches.len(),
                }))
                .into_response(),
            }
        }
        Err(_) => rpc_error(StatusCode::INTERNAL_SERVER_ERROR, "state_lock_failed"),
    }
}

pub(crate) fn qcash_file_lookup_prefix(name: &str) -> Result<String, String> {
    let stem = name
        .strip_suffix(".QCash")
        .or_else(|| name.strip_suffix(".XPQ"))
        .unwrap_or(name);
    let prefix = stem.rsplit_once('_').map_or(stem, |(_, suffix)| suffix);
    if !(9..=64).contains(&prefix.len()) || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid_qcash_file_name_or_coin_id_prefix".to_string());
    }
    Ok(prefix.to_ascii_uppercase())
}

pub(crate) fn qcash_utxo_value(
    coin: &paqus::state::QCashUtxo,
    height: Height,
) -> serde_json::Value {
    let redeemable_height = coin
        .issued_height
        .0
        .saturating_add(paqus::ledger::QCASH_REDEEM_DELAY as u64);
    let status = if height.0 >= redeemable_height {
        "redeemable"
    } else {
        "pending"
    };
    serde_json::json!({
        "coin_id": hex::encode(coin.id.0),
        "short_coin_id": coin.id.short_id(),
        "file_name": coin.id.file_name(coin.denomination),
        "denomination": coin.denomination.xpq(),
        "status": status,
        "height": height.0,
        "issued_height": coin.issued_height.0,
        "maturity_height": redeemable_height,
        "redeemable_height": redeemable_height,
        "remaining_redeem_delay_blocks": redeemable_height.saturating_sub(height.0),
        "output_index": coin.outpoint.output_index,
        "withdraw_tx_hash": hex::encode(coin.outpoint.transaction_hash.0),
        "withdrawer": address_to_string(&coin.withdrawer),
    })
}

pub(crate) fn block_response(node: &Node, block: &Block) -> Result<BlockResponse, String> {
    let hash = block.hash().map_err(|error| error.to_string())?;
    let tip = node.tip_height().unwrap_or(block.height());
    let confirmations = tip.0.saturating_sub(block.height().0).saturating_add(1);
    let transactions = block
        .transactions()
        .iter()
        .map(|transaction| {
            protocol_tx_response(
                transaction,
                Some(block.height()),
                Some(hash),
                Some(tip),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value_moved = block
        .transactions()
        .iter()
        .filter_map(|transaction| match transaction {
            SignedProtocolTransaction::BatchTransfer(tx) => tx.transaction.total_amount().ok(),
            _ => None,
        })
        .map(|amount| amount.0)
        .sum();
    Ok(BlockResponse {
        version: block.header.version,
        height: block.height().0,
        hash: hex::encode(hash.0),
        short_hash: short_hash(Some(hash)),
        previous_hash: hex::encode(block.previous_hash().0),
        merkle_root: hex::encode(block.header.merkle_root.0),
        state_root: hex::encode(block.state_root().0),
        miner_address: address_to_string(&block.miner_address()),
        difficulty: block.difficulty(),
        confirmations,
        value_moved,
        nonce: block.proof.nonce.0,
        tx_count: block.transaction_count(),
        size: block_bytes(block).map_err(|error| error.to_string())?.len(),
        payload_size: block_bytes(block).map_err(|error| error.to_string())?.len(),
        proof_size: 0,
        weight: block.block_weight() as usize,
        coinbase: block.coinbase().as_ref().map(|coinbase| CoinbaseResponse {
            to: address_to_string(&coinbase.to),
            subsidy: coinbase.subsidy.0,
            fees: 0,
            total: coinbase.total().0,
        }),
        genesis_allocations: block
            .genesis_allocations()
            .iter()
            .map(|allocation| GenesisAllocationResponse {
                to: address_to_string(&allocation.to),
                amount: allocation.amount.0,
            })
            .collect(),
        transactions,
    })
}

pub(crate) fn protocol_tx_response(
    transaction: &SignedProtocolTransaction,
    block_height: Option<Height>,
    block_hash: Option<BlockHash>,
    tip_height: Option<Height>,
) -> Result<ProtocolTxResponse, String> {
    let txid = transaction.hash().map_err(|error| error.to_string())?;
    let signer = transaction.signer();
    let (operation, recipient, amount, outputs) = match transaction {
        SignedProtocolTransaction::BatchTransfer(tx) => {
            let outputs = tx
                .transaction
                .outputs()
                .map(|output| TransferOutputResponse {
                    to: output
                        .to
                        .address()
                        .map(|address| address_to_string(&address))
                        .unwrap_or_else(|| "block_miner".to_string()),
                    amount: output.amount.0,
                })
                .collect::<Vec<_>>();
            (
                "transfer",
                outputs.first().map(|output| output.to.clone()),
                tx.transaction.total_amount().ok().map(|amount| amount.0),
                outputs,
            )
        }
        SignedProtocolTransaction::QCash(tx) => match &tx.transaction.kind {
            paqus::transaction::QCashTransactionKind::Withdraw { amount, .. } => {
                ("qcash_withdraw", None, Some(amount.0), Vec::new())
            }
            paqus::transaction::QCashTransactionKind::Redeem { recipient, .. } => (
                "qcash_redeem",
                Some(address_to_string(recipient)),
                None,
                Vec::new(),
            ),
            paqus::transaction::QCashTransactionKind::RecoverRedeem { claimant, .. } => (
                "qcash_recover_redeem",
                Some(address_to_string(claimant)),
                None,
                Vec::new(),
            ),
        },
    };
    let depth = block_height
        .zip(tip_height)
        .map(|(height, tip)| tip.0.saturating_sub(height.0).saturating_add(1))
        .unwrap_or(0);
    let lifecycle = if block_height.is_some() {
        canonical_transaction_lifecycle(depth)
    } else {
        paqus::ledger::TransactionLifecycle::Pending
    };
    Ok(ProtocolTxResponse {
        family: match transaction.family() {
            paqus::transaction::TransactionFamily::BatchTransfer => "transfer",
            paqus::transaction::TransactionFamily::QCash => "qcash",
        },
        operation,
        txid: hex::encode(txid.0),
        signer: address_to_string(&signer),
        authorization_addresses: transaction
            .authorization_proof_addresses()
            .into_iter()
            .map(|address| address_to_string(&address))
            .collect(),
        recipient,
        amount,
        outputs,
        fee: 0,
        nonce: 0,
        payload_size: transaction.to_bytes().map_err(|error| error.to_string())?.len(),
        proof_size: 0,
        virtual_size: transaction.to_bytes().map_err(|error| error.to_string())?.len(),
        block_height: block_height.map(|height| height.0),
        block_hash: block_hash.map(|hash| hex::encode(hash.0)),
        confirmations: depth,
        depth,
        confirmation_depth: CONFIRMATION_DEPTH,
        finality_depth: FINALITY_DEPTH,
        confirmed: lifecycle != paqus::ledger::TransactionLifecycle::Pending,
        finalized: lifecycle == paqus::ledger::TransactionLifecycle::Finalized,
        status: lifecycle.as_str(),
    })
}
