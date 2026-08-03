fn wallet_new(args: &[String]) -> Result<(), String> {
    let show_secret = args.iter().any(|arg| arg == "--show-secret");
    let mut output_path = DEFAULT_WALLET_PATH.to_string();
    let mut mnemonic_words = PAQUS_MNEMONIC_DEFAULT_WORDS;
    let mut auth_password = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--show-secret" => {}
            "--words" | "--mnemonic-words" => {
                index += 1;
                mnemonic_words = parse_mnemonic_words(args.get(index))?;
            }
            "--auth-password" => {
                index += 1;
                auth_password = Some(required_option(args, index, "--auth-password")?);
            }
            value if value.starts_with("-") => {
                return Err(format!("unknown wallet new option `{value}`"));
            }
            value => output_path = value.to_string(),
        }
        index += 1;
    }
    let auth_password = match auth_password {
        Some(password) => Zeroizing::new(password),
        None => prompt_hidden("Authorization password")?,
    };
    if auth_password.is_empty() {
        return Err("authorization password must not be empty".to_string());
    }
    let result = create_mnemonic_wallet_file(&output_path, mnemonic_words, &auth_password);
    let (wallet, mnemonic) = result?;

    let address_str = wallet_address_string(&wallet).to_string();

    println!("Wallet successfully saved to `{output_path}`");
    println!("address: {address_str}");
    println!("mnemonic: {}", mnemonic.as_str());
    println!("authorization: bound to this dual-key address");
    println!("keys: saved to wallet file");
    if show_secret {
        let secret_key_hex = Zeroizing::new(hex::encode(wallet.secret_key.0));
        println!("secret_key: {}", secret_key_hex.as_str());
    }
    Ok(())
}

fn wallet_new_mnemonic(args: &[String]) -> Result<(), String> {
    wallet_new(args)
}

fn parse_mnemonic_words(value: Option<&String>) -> Result<usize, String> {
    let value = value.ok_or_else(|| "missing value for --words".to_string())?;
    match value.as_str() {
        "12" => Ok(12),
        "24" => Ok(24),
        _ => Err("mnemonic words must be 12 or 24".to_string()),
    }
}

fn wallet_restore_mnemonic(args: &[String]) -> Result<(), String> {
    let mut mnemonic = None;
    let mut auth_password = None;
    let mut output_path = DEFAULT_IMPORTED_WALLET_PATH.to_string();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--mnemonic" => {
                index += 1;
                mnemonic = Some(required_option(args, index, "--mnemonic")?);
            }
            "--auth-password" => {
                index += 1;
                auth_password = Some(required_option(args, index, "--auth-password")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown wallet restore-mnemonic option `{value}`"));
            }
            value => output_path = value.to_string(),
        }
        index += 1;
    }
    let mnemonic = Zeroizing::new(match mnemonic {
        Some(value) => value,
        None => prompt("Mnemonic")?,
    });
    let auth_password = match auth_password {
        Some(password) => Zeroizing::new(password),
        None => prompt_hidden("Authorization password")?,
    };
    if auth_password.is_empty() {
        return Err("authorization password must not be empty".to_string());
    }
    let result = restore_mnemonic_wallet_file(&output_path, &mnemonic, &auth_password);
    let wallet = result?;
    println!("Wallet successfully restored to `{output_path}`");
    println!("address: {}", wallet_address_string(&wallet));
    Ok(())
}

fn wallet_address(args: &[String]) -> Result<(), String> {
    let secret_key = parse_secret_key(args.first())?;
    let public_key = derive_public_key(&secret_key);
    let auth_password = prompt_hidden("Authorization password")?;
    let authorization = authorization_from_password(&auth_password, &public_key)?;
    let address = dual_address_from_public_keys(&public_key, &authorization.public_key);
    println!("{}", address_to_string(&address));
    Ok(())
}

fn wallet_balance(args: &[String]) -> Result<(), String> {
    let mut address = None;
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut cash_dir = "./cash".to_string();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            "--cash-dir" | "--cash" => {
                index += 1;
                cash_dir = args
                    .get(index)
                    .ok_or_else(|| "missing value for --cash-dir".to_string())?
                    .clone();
            }
            value if !value.starts_with('-') && address.is_none() => {
                address = Some(parse_address(args.get(index))?);
            }
            value => return Err(format!("unknown wallet balance option `{value}`")),
        }
        index += 1;
    }

    let address = match address {
        Some(address) => address,
        None => load_wallet_address(&wallet_path)?,
    };

    print_wallet_balance_summary(&rpc_addr, &address, &cash_dir)
}

fn wallet_global_stats(args: &[String]) -> Result<(), String> {
    let mut rpc_addr = default_rpc_addr();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            value => return Err(format!("unknown wallet stats option `{value}`")),
        }
        index += 1;
    }

    print_global_stats(&rpc_addr)
}

fn print_global_stats(rpc_addr: &str) -> Result<(), String> {
    let body = http_get(rpc_addr, "/stats")?;
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse stats rpc response: {error}: {body}"))?;
    print_chain_stats(&value);
    Ok(())
}

fn wallet_address_stats(args: &[String]) -> Result<(), String> {
    let mut address = None;
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            value if !value.starts_with('-') && address.is_none() => {
                address = Some(parse_address(args.get(index))?);
            }
            value => return Err(format!("unknown wallet address-stats option `{value}`")),
        }
        index += 1;
    }

    let address = match address {
        Some(address) => address,
        None => load_wallet_address(&wallet_path)?,
    };

    print_wallet_stats(&rpc_addr, &address)
}

fn print_wallet_stats(rpc_addr: &str, address: &Address) -> Result<(), String> {
    let address_hex = address_to_string(address);
    let body = http_get(rpc_addr, &format!("/address/{address_hex}"))?;
    let response: AddressRpcResponse = serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse address rpc response: {error}: {body}"))?;
    let stats = WalletStats::from_response(&response);

    println!("Wallet Tracking");
    print_field("Address", short_text(&response.address));
    print_field("Height", response.balance.height);
    print_field(
        "Confirmed",
        amount_units_text(&response.balance.confirmed.to_string()),
    );
    print_field(
        "Available",
        amount_units_text(&response.balance.available.to_string()),
    );
    print_field(
        "Unspendable",
        amount_units_text(&response.balance.unspendable.to_string()),
    );
    print_field(
        "Authorization",
        if response.balance.authorization_registered {
            "registered"
        } else {
            "registration required"
        },
    );
    print_field(
        "Incoming",
        amount_units_text(&response.balance.pending_incoming.to_string()),
    );
    print_field(
        "Outgoing",
        amount_units_text(&response.balance.pending_outgoing.to_string()),
    );
    if let Some(statement) = &response.balance.statement {
        print_field("Statement", short_text(statement));
    }
    println!();
    print_field("Mined blocks", stats.mined_blocks);
    print_field("Maturity", format!("{BLOCK_REWARD_MATURITY} blocks"));
    print_field(
        "Mined total",
        amount_units_text(&stats.mined_total.to_string()),
    );
    print_field(
        "Matured mined",
        amount_units_text(&stats.matured_mined.to_string()),
    );
    print_field(
        "Immature mined",
        amount_units_text(&stats.immature_mined.to_string()),
    );
    print_field(
        "Mining fees",
        amount_units_text(&stats.mining_fees.to_string()),
    );
    print_field(
        "Next maturity",
        optional_u64_text(stats.next_maturity_height),
    );
    println!();
    print_field("Tx count", stats.total_transactions);
    print_field("Received tx", stats.received_transactions);
    print_field("Sent tx", stats.sent_transactions);
    print_field(
        "Received total",
        amount_units_text(&stats.received_total.to_string()),
    );
    print_field(
        "Sent total",
        amount_units_text(&stats.sent_total.to_string()),
    );
    print_field("Fees sent", amount_units_text(&stats.sent_fees.to_string()));
    print_field("Pending tx", stats.pending_transactions);
    Ok(())
}

fn wallet_hashrate(args: &[String]) -> Result<(), String> {
    let mut rpc_addr = default_rpc_addr();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            value => return Err(format!("unknown wallet hashrate option `{value}`")),
        }
        index += 1;
    }

    print_hashrate(&status_value(&rpc_addr)?);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AddressRpcResponse {
    address: String,
    balance: AddressBalanceRpcResponse,
    #[serde(default)]
    mined_blocks: Vec<MinedBlockRpcResponse>,
    #[serde(default)]
    transactions: Vec<TransactionRpcResponse>,
}

#[derive(Debug, Deserialize)]
struct AddressBalanceRpcResponse {
    height: u64,
    confirmed: u64,
    available: u64,
    pending_incoming: u64,
    pending_outgoing: u64,
    statement: Option<String>,
    #[serde(default)]
    authorization_registered: bool,
    #[serde(default)]
    unspendable: u64,
}

#[derive(Debug, Deserialize)]
struct WalletBalanceRpcResponse {
    address: String,
    height: u64,
    confirmed: u64,
    available: u64,
    pending_incoming: u64,
    pending_outgoing: u64,
    statement: Option<String>,
    #[serde(default)]
    authorization_registered: bool,
    #[serde(default)]
    unspendable: u64,
}

#[derive(Debug, Deserialize)]
struct MinedBlockRpcResponse {
    #[serde(default)]
    maturity_height: u64,
    #[serde(default = "default_block_reward_maturity")]
    matured: bool,
    #[serde(default)]
    fees: u64,
    #[serde(default)]
    total: u64,
}

#[derive(Debug, Deserialize)]
struct TransactionRpcResponse {
    from: String,
    to: String,
    amount: u64,
    fee: u64,
    status: String,
}

#[derive(Debug, Default)]
struct WalletStats {
    mined_blocks: u64,
    mined_total: u64,
    matured_mined: u64,
    immature_mined: u64,
    mining_fees: u64,
    next_maturity_height: Option<u64>,
    total_transactions: u64,
    received_transactions: u64,
    sent_transactions: u64,
    received_total: u64,
    sent_total: u64,
    sent_fees: u64,
    pending_transactions: u64,
}

impl WalletStats {
    fn from_response(response: &AddressRpcResponse) -> Self {
        let mut stats = Self {
            mined_blocks: response.mined_blocks.len() as u64,
            ..Self::default()
        };
        for block in &response.mined_blocks {
            stats.mined_total = stats.mined_total.saturating_add(block.total);
            stats.mining_fees = stats.mining_fees.saturating_add(block.fees);
            if block.matured {
                stats.matured_mined = stats.matured_mined.saturating_add(block.total);
            } else {
                stats.immature_mined = stats.immature_mined.saturating_add(block.total);
                stats.next_maturity_height = match stats.next_maturity_height {
                    Some(height) => Some(height.min(block.maturity_height)),
                    None => Some(block.maturity_height),
                };
            }
        }

        for transaction in &response.transactions {
            stats.total_transactions = stats.total_transactions.saturating_add(1);
            if transaction.status == "pending" {
                stats.pending_transactions = stats.pending_transactions.saturating_add(1);
            }
            if transaction.to == response.address {
                stats.received_transactions = stats.received_transactions.saturating_add(1);
                stats.received_total = stats.received_total.saturating_add(transaction.amount);
            }
            if transaction.from == response.address {
                stats.sent_transactions = stats.sent_transactions.saturating_add(1);
                stats.sent_total = stats.sent_total.saturating_add(transaction.amount);
                stats.sent_fees = stats.sent_fees.saturating_add(transaction.fee);
            }
        }

        stats
    }
}

fn default_block_reward_maturity() -> bool {
    false
}

fn optional_u64_text(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn wallet_pay(args: &[String]) -> Result<(), String> {
    let to = parse_address(args.first())?;
    let amount = parse_amount(args.get(1), "amount")?;
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut fee = Amount(DEFAULT_TRANSACTION_FEE);
    let mut authorization = None;
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            "--fee" => {
                index += 1;
                fee = parse_fee(args.get(index))?;
            }
            "--auth-secret-key" => {
                index += 1;
                authorization = Some(AuthorizationInput::Keys(Box::new(
                    authorization_from_secret_key(parse_secret_key(args.get(index))?),
                )));
            }
            "--auth-password" => {
                index += 1;
                authorization = Some(AuthorizationInput::Password(Zeroizing::new(
                    required_option(args, index, "--auth-password")?,
                )));
            }
            value => return Err(format!("unknown wallet pay option `{value}`")),
        }
        index += 1;
    }

    submit_wallet_payment(&wallet_path, to, amount, fee, &rpc_addr, authorization)
}

fn wallet_send(args: &[String]) -> Result<(), String> {
    let short_form = args.len() >= 2 && !args[0].starts_with('-') && !args[1].starts_with('-');
    if short_form {
        return wallet_send_short(args);
    }

    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut to = None;
    let mut amount = None;
    let mut rpc_addr = default_rpc_addr();
    let mut fee = Amount(DEFAULT_TRANSACTION_FEE);
    let mut submit = false;
    let mut authorization = None;
    let mut outputs = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--to" => {
                index += 1;
                to = Some(parse_address(args.get(index))?);
            }
            "--amount" => {
                index += 1;
                amount = Some(parse_amount(args.get(index), "--amount")?);
            }
            "--output" => {
                index += 1;
                outputs.extend(parse_transfer_output_specs(
                    args.get(index)
                        .ok_or_else(|| "missing value for --output".to_string())?,
                )?);
            }
            "--nonce" => {
                return Err("--nonce has been removed; wallet uses account statement last_state".to_string());
            }
            "--auth-secret-key" => {
                index += 1;
                authorization = Some(AuthorizationInput::Keys(Box::new(
                    authorization_from_secret_key(parse_secret_key(args.get(index))?),
                )));
            }
            "--auth-password" => {
                index += 1;
                authorization = Some(AuthorizationInput::Password(Zeroizing::new(
                    required_option(args, index, "--auth-password")?,
                )));
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            "--fee" => {
                index += 1;
                fee = parse_fee(args.get(index))?;
            }
            "--submit" => submit = true,
            value => return Err(format!("unknown wallet send option `{value}`")),
        }
        index += 1;
    }

    match (to, amount) {
        (Some(to), Some(amount)) => outputs.insert(
            0,
            TransferOutput {
                to: to.into(),
                amount,
            },
        ),
        (None, None) if !outputs.is_empty() => {}
        (None, _) => return Err("missing --to address".to_string()),
        (_, None) => return Err("missing --amount".to_string()),
    }
    submit_wallet_outputs(
        &wallet_path,
        outputs,
        fee,
        &rpc_addr,
        submit,
        authorization,
    )
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PoolAccountingRound {
    pool_address: String,
    height: u64,
    block_hash: String,
    maturity_height: u64,
    gross_reward: u64,
    payouts: Vec<PoolWorkerPayout>,
}

#[derive(Debug, Deserialize)]
struct PoolWorkerPayout {
    worker: String,
    address: String,
    amount: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PoolPayoutReceipt {
    round_block_hash: String,
    round_height: u64,
    worker: String,
    address: String,
    amount: u64,
    last_state: String,
    tx_hash: String,
    submitted_at_height: u64,
}

fn wallet_pool_payout(args: &[String]) -> Result<(), String> {
    let mut ledger = "pool-accounting.jsonl".to_string();
    let mut receipts = "pool-payout-receipts.jsonl".to_string();
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut execute = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--ledger" => {
                index += 1;
                ledger = required_option(args, index, "--ledger")?;
            }
            "--receipts" => {
                index += 1;
                receipts = required_option(args, index, "--receipts")?;
            }
            "--wallet" => {
                index += 1;
                wallet_path = required_option(args, index, "--wallet")?;
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = required_option(args, index, "--rpc")?;
            }
            "--execute" => execute = true,
            value => return Err(format!("unknown pool-payout option `{value}`")),
        }
        index += 1;
    }

    let height = status_value(&rpc_addr)?
        .get("height")
        .and_then(serde_json::Value::as_u64)
        .ok_or("rpc status is missing height")?;
    let rounds = read_json_lines::<PoolAccountingRound>(&ledger)?;
    let prior_receipts = if std::path::Path::new(&receipts).exists() {
        read_json_lines::<PoolPayoutReceipt>(&receipts)?
    } else {
        Vec::new()
    };
    let paid = prior_receipts
        .iter()
        .map(receipt_key)
        .collect::<HashSet<_>>();
    let wallet_address = address_to_string(&load_wallet_address(&wallet_path)?);
    let mut pending = Vec::new();
    for round in rounds
        .iter()
        .filter(|round| round.maturity_height <= height)
    {
        if round.pool_address != wallet_address {
            return Err(format!(
                "round {} belongs to pool {}, but wallet address is {}",
                round.block_hash, round.pool_address, wallet_address
            ));
        }
        let _payout_total = round
            .payouts
            .iter()
            .try_fold(0u64, |total, payout| total.checked_add(payout.amount))
            .ok_or("round payout total overflow")?;
        for payout in &round.payouts {
            if payout.amount > 0 && !paid.contains(&payout_key(round, payout)) {
                let address = parse_address_string(&payout.address).map_err(|error| {
                    format!(
                        "invalid payout address for worker {}: {error}",
                        payout.worker
                    )
                })?;
                pending.push((round, payout, address));
            }
        }
    }

    if !execute {
        println!(
            "{}",
            serde_json::json!({
                "execute": false,
                "height": height,
                "mature_unpaid_payouts": pending.len(),
                "amount": pending.iter().map(|(_, payout, _)| payout.amount).sum::<u64>(),
                "hint": "review this preview, then repeat with --execute"
            })
        );
        return Ok(());
    }

    let mut wallet = load_wallet(&wallet_path)?;
    let authorization = resolve_authorization_for_wallet_file(&wallet_path, &mut wallet, None)?;
    if pending.len() > 1 {
        return Err(
            "pool-payout needs wallet-side draft AccountStatement chaining before multi-payout"
                .to_string(),
        );
    }
    let account_state = resolve_wallet_account_state(&wallet.address, &rpc_addr)?;
    ensure_no_pending_outgoing(&account_state)?;
    for (round, payout, address) in pending {
        let transaction = Transaction::new(
            wallet.address,
            vec![TransferOutput {
                to: (address).into(),
                amount: Amount(payout.amount),
            }],
        )
        .with_last_state(account_state.last_state);
        let signed = wallet.sign_transaction(
            transaction,
            Some(authorization.clone()),
            account_state.authorization_registered,
        )?;
        let tx_hash = hex::encode(signed.hash().map_err(|error| error.to_string())?.0);
        let body = format!("{{\"tx\":\"{}\"}}", signed_transaction_to_hex(&signed)?);
        let response = http_post_json(&rpc_addr, "/tx", &body)?;
        let accepted = serde_json::from_str::<serde_json::Value>(&response)
            .ok()
            .and_then(|value| value.get("accepted").and_then(serde_json::Value::as_bool));
        if accepted != Some(true) {
            return Err(format!(
                "node rejected payout for {}: {response}",
                payout.worker
            ));
        }
        append_payout_receipt(
            &receipts,
            &PoolPayoutReceipt {
                round_block_hash: round.block_hash.clone(),
                round_height: round.height,
                worker: payout.worker.clone(),
                address: payout.address.clone(),
                amount: payout.amount,
                last_state: hex::encode(account_state.last_state.0),
                tx_hash,
                submitted_at_height: height,
            },
        )?;
    }
    println!("{{\"accepted\":true,\"height\":{height}}}");
    Ok(())
}

fn read_json_lines<T: for<'de> Deserialize<'de>>(path: &str) -> Result<Vec<T>, String> {
    let file = fs::File::open(path).map_err(|error| format!("failed to open {path}: {error}"))?;
    BufReader::new(file)
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match line {
            Ok(line) if line.trim().is_empty() => None,
            result => Some((index, result)),
        })
        .map(|(index, line)| {
            let line = line.map_err(|error| format!("failed to read {path}: {error}"))?;
            serde_json::from_str(&line)
                .map_err(|error| format!("invalid JSON in {path} line {}: {error}", index + 1))
        })
        .collect()
}

fn payout_key(round: &PoolAccountingRound, payout: &PoolWorkerPayout) -> String {
    format!(
        "{}:{}:{}:{}",
        round.block_hash, payout.worker, payout.address, payout.amount
    )
}

fn receipt_key(receipt: &PoolPayoutReceipt) -> String {
    format!(
        "{}:{}:{}:{}",
        receipt.round_block_hash, receipt.worker, receipt.address, receipt.amount
    )
}

fn append_payout_receipt(path: &str, receipt: &PoolPayoutReceipt) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("open receipt file: {error}"))?;
    serde_json::to_writer(&mut file, receipt).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_data().map_err(|error| error.to_string())
}

fn wallet_cash(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("inspect") => {
            let path = args
                .get(1)
                .ok_or_else(|| "usage: cash inspect <coin.XPQ>".to_string())?;
            let file = load_cash_coin_file(path)?;
            println!(
                "{{\"version\":{},\"coin_id\":\"{}\",\"denomination\":{},\"file\":\"{}\"}}",
                file.version,
                hex::encode(file.coin_id),
                file.denomination.xpq(),
                path
            );
            Ok(())
        }
        Some("withdraw") => wallet_cash_withdraw(&args[1..]),
        Some("redeem") => wallet_cash_redeem(&args[1..]),
        Some("track") | Some("status") => wallet_cash_track(&args[1..]),
        Some("utxos") | Some("explorer") => wallet_cash_utxos(&args[1..]),
        Some("list") => wallet_cash_list(&args[1..]),
        Some("backup") => wallet_cash_backup(&args[1..]),
        Some("recover") => wallet_cash_recover(&args[1..]),
        Some(command) => Err(format!(
            "unknown cash command `{command}`; use withdraw, inspect, redeem, track, utxos, list, backup, or recover"
        )),
        None => Err(
            "usage: cash <withdraw|inspect|redeem|track|utxos|list|backup|recover> ..."
                .to_string(),
        ),
    }
}

fn wallet_events(args: &[String]) -> Result<(), String> {
    let (scope, value, options) = match args {
        [scope, value, options @ ..] => (scope.as_str(), value.as_str(), options),
        _ => {
            return Err(
                "usage: events <block|tx|address|id> <value> [--kind event-kind] [--offset n] [--limit n] [--from-height n] [--to-height n] [--rpc host:port]"
                    .to_string(),
            );
        }
    };
    let mut rpc_addr = default_rpc_addr();
    let mut query = Vec::new();
    let mut index = 0;
    while index < options.len() {
        match options[index].as_str() {
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = required_option(options, index, "--rpc")?;
            }
            flag @ ("--kind" | "--offset" | "--limit" | "--from-height" | "--to-height") => {
                index += 1;
                let value = required_option(options, index, flag)?;
                query.push(format!(
                    "{}={value}",
                    flag.trim_start_matches("--").replace('-', "_")
                ));
            }
            option => return Err(format!("unknown events option `{option}`")),
        }
        index += 1;
    }
    let base = match scope {
        "block" | "height" => format!("/blocks/{value}/events"),
        "tx" | "transaction" => format!("/tx/{value}/events"),
        "address" | "addr" => format!("/address/{value}/events"),
        "id" | "event" => {
            if !query.is_empty() {
                return Err("event id lookup does not accept filters".to_string());
            }
            format!("/events/{value}")
        }
        _ => return Err(format!("unknown event scope `{scope}`")),
    };
    let path = if query.is_empty() {
        base
    } else {
        format!("{base}?{}", query.join("&"))
    };
    print_rpc_get(&rpc_addr, &path)
}

#[derive(Debug, Serialize, Deserialize)]
struct WalletCheckpointFile {
    version: u8,
    height: u64,
    block_hash: String,
    checkpoint: String,
}

fn checkpoint_path(wallet_path: &str) -> String {
    format!("{wallet_path}.checkpoint")
}

fn load_wallet_checkpoint(wallet_path: &str) -> Result<Option<TrustedHeaderCheckpoint>, String> {
    let path = checkpoint_path(wallet_path);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("failed to read checkpoint {path}: {error}")),
    };
    let file: WalletCheckpointFile = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse checkpoint {path}: {error}"))?;
    if file.version != 1 {
        return Err("unsupported wallet checkpoint version".to_string());
    }
    let encoded = hex::decode(&file.checkpoint)
        .map_err(|error| format!("invalid checkpoint encoding: {error}"))?;
    let checkpoint: TrustedHeaderCheckpoint = canonical_deserialize(&encoded)
        .map_err(|error| format!("invalid trusted checkpoint: {error}"))?;
    let hash = checkpoint
        .header
        .hash()
        .map_err(|error| error.to_string())?;
    if checkpoint.header.height.0 != file.height || hex::encode(hash.0) != file.block_hash {
        return Err("wallet checkpoint metadata mismatch".to_string());
    }
    Ok(Some(checkpoint))
}

fn save_wallet_checkpoint(
    wallet_path: &str,
    checkpoint: &TrustedHeaderCheckpoint,
) -> Result<(), String> {
    let path = checkpoint_path(wallet_path);
    let hash = checkpoint
        .header
        .hash()
        .map_err(|error| error.to_string())?;
    let file = WalletCheckpointFile {
        version: 1,
        height: checkpoint.header.height.0,
        block_hash: hex::encode(hash.0),
        checkpoint: hex::encode(canonical_bytes(checkpoint).map_err(|error| error.to_string())?),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| format!("failed to encode checkpoint: {error}"))?;
    let temporary = format!("{path}.tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to write checkpoint {temporary}: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("failed to activate checkpoint {path}: {error}"))
}

fn proof_request_path(
    base: String,
    checkpoint: Option<&TrustedHeaderCheckpoint>,
) -> Result<String, String> {
    let Some(checkpoint) = checkpoint else {
        return Ok(base);
    };
    let hash = checkpoint
        .header
        .hash()
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "{base}?checkpoint_height={}&checkpoint_hash={}",
        checkpoint.header.height.0,
        hex::encode(hash.0)
    ))
}

fn update_checkpoint_from_headers(
    current: Option<&TrustedHeaderCheckpoint>,
    headers: &[paqus::block::BlockHeader],
) -> Result<TrustedHeaderCheckpoint, String> {
    if let Some(current) = current {
        verify_header_chain_extension(current, headers)
            .map_err(|error| format!("checkpoint header extension rejected: {error}"))?;
        advance_trusted_header_checkpoint(current, headers)
            .map_err(|error| format!("failed to advance checkpoint: {error}"))
    } else {
        trusted_header_checkpoint(headers)
            .map_err(|error| format!("full header proof rejected: {error}"))
    }
}

fn wallet_proof(args: &[String]) -> Result<(), String> {
    let action = args.first().map(String::as_str).unwrap_or("account");
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut value = args
        .get(1)
        .filter(|value| !value.starts_with("--"))
        .cloned();
    let mut index = if value.is_some() { 2 } else { 1 };
    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = required_option(args, index, "--wallet")?;
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = required_option(args, index, "--rpc")?;
            }
            option => return Err(format!("unknown proof option `{option}`")),
        }
        index += 1;
    }
    if action == "status" {
        return match load_wallet_checkpoint(&wallet_path)? {
            Some(checkpoint) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "wallet": wallet_path,
                        "height": checkpoint.header.height.0,
                        "block_hash": hex::encode(checkpoint.header.hash().map_err(|error| error.to_string())?.0),
                        "cumulative_work": checkpoint.cumulative_work.to_be_limbs(),
                    })
                );
                Ok(())
            }
            None => Err("wallet has no trusted checkpoint; run `proof account` first".to_string()),
        };
    }

    let current = load_wallet_checkpoint(&wallet_path)?;
    let response = match action {
        "account" => {
            let address = match value.take() {
                Some(address) => address,
                None => address_to_string(&load_wallet_address(&wallet_path)?),
            };
            let path = proof_request_path(
                format!("/proof/account/{address}"),
                current.as_ref(),
            )?;
            http_get(&rpc_addr, &path)?
        }
        "qcash" => {
            let coin_id = value.ok_or_else(|| {
                "usage: proof qcash <coin-id> [--wallet path] [--rpc host:port]".to_string()
            })?;
            let path =
                proof_request_path(format!("/proof/qcash/{coin_id}"), current.as_ref())?;
            http_get(&rpc_addr, &path)?
        }
        _ => return Err("usage: proof <account|qcash|status> ...".to_string()),
    };
    let json: serde_json::Value = serde_json::from_str(&response)
        .map_err(|error| format!("invalid proof response: {error}: {response}"))?;
    let encoded = json
        .get("bundle")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "proof response has no bundle".to_string())?;
    let bytes = hex::decode(encoded).map_err(|error| format!("invalid proof bundle hex: {error}"))?;

    let next = if action == "qcash" {
        let bundle = decode_qcash_state_proof_bundle(&bytes)
            .map_err(|error| format!("QCash proof decode failed: {error}"))?;
        let next = update_checkpoint_from_headers(current.as_ref(), &bundle.canonical_headers)?;
        let tip_hash = next.header.hash().map_err(|error| error.to_string())?;
        bundle
            .verify_state_binding(&next.header, tip_hash)
            .map_err(|error| format!("QCash state proof rejected: {error}"))?;
        next
    } else if json.get("proof_kind").and_then(serde_json::Value::as_str)
        == Some("membership")
    {
        let bundle = decode_account_state_proof_bundle(&bytes)
            .map_err(|error| format!("account proof decode failed: {error}"))?;
        let next = update_checkpoint_from_headers(current.as_ref(), &bundle.canonical_headers)?;
        let tip_hash = next.header.hash().map_err(|error| error.to_string())?;
        bundle
            .verify_state_binding(&next.header, tip_hash)
            .map_err(|error| format!("account state proof rejected: {error}"))?;
        next
    } else {
        let bundle = decode_account_non_membership_proof_bundle(&bytes)
            .map_err(|error| format!("account absence proof decode failed: {error}"))?;
        let next = update_checkpoint_from_headers(current.as_ref(), &bundle.canonical_headers)?;
        let tip_hash = next.header.hash().map_err(|error| error.to_string())?;
        bundle
            .verify_state_binding(&next.header, tip_hash)
            .map_err(|error| format!("account absence proof rejected: {error}"))?;
        next
    };
    save_wallet_checkpoint(&wallet_path, &next)?;
    println!(
        "{}",
        serde_json::json!({
            "verified": true,
            "proof_kind": json.get("proof_kind"),
            "height": next.header.height.0,
            "block_hash": hex::encode(next.header.hash().map_err(|error| error.to_string())?.0),
            "checkpoint_file": checkpoint_path(&wallet_path),
            "headers_received": json.get("header_count"),
        })
    );
    Ok(())
}

fn wallet_rollback(args: &[String]) -> Result<(), String> {
    let (action, value, options) = match args {
        [action, value, options @ ..] => (action.as_str(), value.as_str(), options),
        _ => {
            return Err(
                "usage: rollback <list address|show issue-id|verify issue-id|retry issue-id> [--rpc host:port]"
                    .to_string(),
            );
        }
    };
    let mut rpc_addr = default_rpc_addr();
    let mut index = 0;
    while index < options.len() {
        match options[index].as_str() {
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = required_option(options, index, "--rpc")?;
            }
            option => return Err(format!("unknown rollback option `{option}`")),
        }
        index += 1;
    }
    match action {
        "list" => print_rpc_get(&rpc_addr, &format!("/account/{value}/rollback-issues")),
        "show" | "inspect" => {
            let (body, _) = fetch_and_verify_rollback_proof(&rpc_addr, value)?;
            print_rpc_response(&format!("/rollback-issues/{value}"), &body)
        }
        "verify" => {
            let (_, verified) = fetch_and_verify_rollback_proof(&rpc_addr, value)?;
            println!("Rollback proof verified");
            println!(
                "Transaction   : {}",
                hex::encode(verified.transaction_hash.0)
            );
            println!(
                "Disconnected  : {}",
                hex::encode(verified.disconnected_block_hash.0)
            );
            println!(
                "Ancestor      : {}",
                hex::encode(verified.common_ancestor.0)
            );
            println!("Losing tip    : {}", hex::encode(verified.losing_tip.0));
            println!("Canonical tip : {}", hex::encode(verified.canonical_tip.0));
            Ok(())
        }
        "retry" | "claim" => {
            let _ = fetch_and_verify_rollback_proof(&rpc_addr, value)?;
            let path = format!("/rollback-issues/{value}/retry");
            let body = http_post_json(&rpc_addr, &path, "{}")?;
            print_rpc_response(&path, &body)
        }
        _ => Err(format!("unknown rollback action `{action}`")),
    }
}

fn fetch_and_verify_rollback_proof(
    rpc_addr: &str,
    issue_id: &str,
) -> Result<(String, paqus::qcash::recovery::VerifiedRollbackProof), String> {
    let path = format!("/rollback-issues/{issue_id}");
    let body = http_get(rpc_addr, &path)?;
    let response: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("failed to parse rollback issue response: {error}"))?;
    let proof_hex = response
        .get("rollback_proof_bundle")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "node response is missing rollback_proof_bundle".to_string())?;
    let proof_bytes =
        hex::decode(proof_hex).map_err(|error| format!("invalid rollback proof hex: {error}"))?;
    if proof_bytes.len() > MAX_ROLLBACK_PROOF_BYTES {
        return Err("rollback proof exceeds the wallet verification limit".to_string());
    }
    let bundle: RollbackProofBundle = canonical_deserialize(&proof_bytes)
        .map_err(|error| format!("invalid rollback proof encoding: {error}"))?;
    let verified = bundle
        .verify()
        .map_err(|error| format!("rollback proof verification failed: {error}"))?;
    if response.get("status").and_then(serde_json::Value::as_str) != Some("reconfirmed") {
        let status = status_value(rpc_addr)?;
        let reported_tip = status
            .get("tip_hash")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "node status is missing tip_hash".to_string())?;
        if !reported_tip.eq_ignore_ascii_case(&hex::encode(verified.canonical_tip.0)) {
            return Err(
                "rollback proof canonical tip does not match the node's current canonical tip"
                    .to_string(),
            );
        }
    }
    Ok((body, verified))
}

fn wallet_cash_withdraw(args: &[String]) -> Result<(), String> {
    let requested_amount = parse_amount(args.first(), "cash amount")?;
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut output_dir = "./cash".to_string();
    let mut selected_denominations = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = required_option(args, index, "--wallet")?;
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = required_option(args, index, "--rpc")?;
            }
            "--out" | "--output-dir" => {
                index += 1;
                output_dir = required_option(args, index, "--out")?;
            }
            "--fee" => {
                index += 1;
                let _ = parse_fee(args.get(index))?;
            }
            "--nonce" => {
                return Err("--nonce has been removed; wallet uses account statement last_state".to_string());
            }
            "--denoms" | "--denominations" => {
                index += 1;
                selected_denominations = Some(parse_qcash_denomination_selection(
                    args.get(index)
                        .ok_or_else(|| "missing value for --denoms".to_string())?,
                )?);
            }
            value => return Err(format!("unknown cash withdraw option `{value}`")),
        }
        index += 1;
    }

    let (qcash_amount, remainder, denominations) = if let Some(selection) = selected_denominations {
        match selection {
            QCashDenominationSelection::Allowed(denominations) => {
                plan_selected_qcash_denominations(requested_amount, &denominations)?
            }
            QCashDenominationSelection::Exact(denominations) => {
                plan_exact_qcash_denominations(requested_amount, denominations)?
            }
        }
    } else {
        let plan = QCashWithdrawalMetadata::plan_automatic(requested_amount)
            .map_err(|error| format!("cash amount cannot be withdrawn: {error}"))?;
        (plan.qcash_amount, plan.remainder, plan.denominations)
    };
    let mut redeem_secrets = Zeroizing::new(Vec::with_capacity(denominations.len()));
    let mut commitments = Vec::with_capacity(denominations.len());
    for _ in &denominations {
        let mut redeem_secret = [0u8; 32];
        getrandom::fill(&mut redeem_secret)
            .map_err(|error| format!("secure random generation failed: {error}"))?;
        commitments.push(qcash_redeem_key_commitment_from_secret(&redeem_secret));
        redeem_secrets.push(redeem_secret);
    }
    let metadata = QCashWithdrawalMetadata::with_selected_denominations(&denominations, &commitments)
        .map_err(|error| format!("failed to build withdraw outputs: {error}"))?;
    let mut wallet = load_wallet(&wallet_path)?;
    let authorization = resolve_authorization_for_wallet_file(&wallet_path, &mut wallet, None)?;
    let account_state = resolve_wallet_account_state(&wallet.address, &rpc_addr)?;
    ensure_no_pending_outgoing(&account_state)?;
    let transaction = QCashTransaction::withdraw(wallet.address, qcash_amount, metadata.clone())
        .with_last_state(account_state.last_state);
    let withdraw_hash = transaction.hash().map_err(|error| error.to_string())?;
    let signed = wallet.sign_qcash_transaction(
        transaction,
        &authorization,
        account_state.authorization_registered,
    )?;

    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create cash output directory {output_dir}: {error}"))?;
    let mut pending_cash_files = PendingCashFiles::new(metadata.outputs.len());
    for (output, redeem_secret) in metadata.outputs.iter().zip(redeem_secrets.iter()) {
        let cash_file = QCashCoinFile::new(withdraw_hash, output, *redeem_secret)
            .map_err(|error| format!("failed to create cash file: {error}"))?;
        let file_name = QCashCoinId(cash_file.coin_id).file_name(output.denomination);
        let final_path = std::path::Path::new(&output_dir).join(file_name);
        let encoded_cash = encode_qcash_coin_file(&cash_file).map_err(|error| {
            format!(
                "failed to encode cash file {}: {error}",
                final_path.display()
            )
        })?;
        write_new_synced_file(&final_path, &encoded_cash)?;
        pending_cash_files.track(final_path);
    }

    let body = format!(
        "{{\"tx\":\"{}\"}}",
        hex::encode(signed.to_bytes().map_err(|error| error.to_string())?)
    );
    let response = http_post_json(&rpc_addr, "/qcash/tx", &body)?;
    let accepted = serde_json::from_str::<serde_json::Value>(&response)
        .ok()
        .and_then(|value| value.get("accepted").and_then(serde_json::Value::as_bool))
        == Some(true);
    if !accepted {
        return Err(format!(
            "node rejected cash withdraw; cash files removed: {response}"
        ));
    }
    let cash_files = pending_cash_files.commit();
    println!(
        "{{\"accepted\":true,\"lifecycle\":\"offchain-active\",\"hash\":\"{}\",\"cash_amount\":{},\"remainder\":{},\"coins\":{},\"redeem_delay_blocks\":{},\"output_dir\":\"{}\",\"next\":\"cash track {}\"}}",
        hex::encode(withdraw_hash.0),
        qcash_amount.0,
        remainder.0,
        cash_files.len(),
        QCASH_REDEEM_DELAY,
        output_dir,
        output_dir
    );
    Ok(())
}

struct PendingCashFiles {
    paths: Vec<std::path::PathBuf>,
    committed: bool,
}

impl PendingCashFiles {
    fn new(capacity: usize) -> Self {
        Self {
            paths: Vec::with_capacity(capacity),
            committed: false,
        }
    }

    fn track(&mut self, path: std::path::PathBuf) {
        self.paths.push(path);
    }

    fn commit(mut self) -> Vec<std::path::PathBuf> {
        self.committed = true;
        std::mem::take(&mut self.paths)
    }
}

impl Drop for PendingCashFiles {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn parse_qcash_denominations(value: &str) -> Result<Vec<QCashDenomination>, String> {
    let mut denominations = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| match value {
            "1" => Ok(QCashDenomination::One),
            "2" => Ok(QCashDenomination::Two),
            "5" => Ok(QCashDenomination::Five),
            "10" => Ok(QCashDenomination::Ten),
            "20" => Ok(QCashDenomination::Twenty),
            "50" => Ok(QCashDenomination::Fifty),
            "100" => Ok(QCashDenomination::OneHundred),
            "500" => Ok(QCashDenomination::FiveHundred),
            "1000" => Ok(QCashDenomination::OneThousand),
            "5000" => Ok(QCashDenomination::FiveThousand),
            "10000" => Ok(QCashDenomination::TenThousand),
            "50000" => Ok(QCashDenomination::FiftyThousand),
            "100000" => Ok(QCashDenomination::OneHundredThousand),
            "500000" => Ok(QCashDenomination::FiveHundredThousand),
            "1000000" => Ok(QCashDenomination::OneMillion),
            _ => Err(format!(
                "unsupported QCash denomination `{value}`; use 1,2,5,10,20,50,100,500,1000,5000,10000,50000,100000,500000,1000000"
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if denominations.is_empty() {
        return Err("at least one QCash denomination is required".to_string());
    }
    if denominations.len() > paqus::qcash::MAX_QCASH_WITHDRAWAL_OUTPUTS {
        return Err("too many QCash denominations selected".to_string());
    }
    denominations.sort_by_key(|denomination| std::cmp::Reverse(denomination.xpq()));
    denominations.dedup();
    Ok(denominations)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum QCashDenominationSelection {
    Allowed(Vec<QCashDenomination>),
    Exact(Vec<QCashDenomination>),
}

fn parse_qcash_denomination_selection(value: &str) -> Result<QCashDenominationSelection, String> {
    if !value.to_ascii_lowercase().contains('x') {
        return parse_qcash_denominations(value).map(QCashDenominationSelection::Allowed);
    }

    let mut outputs = Vec::new();
    for item in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (denomination, count) = item
            .split_once(['x', 'X'])
            .ok_or_else(|| format!("invalid denomination count `{item}`; use DENOMxCOUNT"))?;
        let denomination = parse_qcash_denominations(denomination)?
            .into_iter()
            .next()
            .ok_or_else(|| format!("missing denomination in `{item}`"))?;
        let count = count
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("invalid denomination count in `{item}`"))?;
        if count == 0 {
            return Err(format!("denomination count must be positive in `{item}`"));
        }
        if outputs.len().saturating_add(count) > paqus::qcash::MAX_QCASH_WITHDRAWAL_OUTPUTS {
            return Err(format!(
                "denomination counts require more than {} cash files",
                paqus::qcash::MAX_QCASH_WITHDRAWAL_OUTPUTS
            ));
        }
        outputs.extend(std::iter::repeat_n(denomination, count));
    }
    if outputs.is_empty() {
        return Err("at least one denomination count is required".to_string());
    }
    outputs.sort_by_key(|denomination| std::cmp::Reverse(denomination.xpq()));
    Ok(QCashDenominationSelection::Exact(outputs))
}

fn plan_selected_qcash_denominations(
    requested_amount: Amount,
    allowed_denominations: &[QCashDenomination],
) -> Result<(Amount, Amount, Vec<QCashDenomination>), String> {
    let mut remaining = requested_amount.0;
    let mut outputs = Vec::new();
    for denomination in allowed_denominations {
        let unit = denomination.amount().0;
        let count = remaining / unit;
        let count = usize::try_from(count)
            .map_err(|_| "selected QCash output count is too large".to_string())?;
        if outputs.len().saturating_add(count) > paqus::qcash::MAX_QCASH_WITHDRAWAL_OUTPUTS {
            return Err(format!(
                "selected denominations require more than {} cash files",
                paqus::qcash::MAX_QCASH_WITHDRAWAL_OUTPUTS
            ));
        }
        outputs.extend(std::iter::repeat_n(*denomination, count));
        remaining %= unit;
    }
    let qcash_amount = Amount(requested_amount.0.saturating_sub(remaining));
    if outputs.is_empty() || qcash_amount.0 == 0 {
        return Err("selected denominations cannot represent the requested amount".to_string());
    }
    Ok((qcash_amount, Amount(remaining), outputs))
}

fn plan_exact_qcash_denominations(
    requested_amount: Amount,
    denominations: Vec<QCashDenomination>,
) -> Result<(Amount, Amount, Vec<QCashDenomination>), String> {
    let qcash_amount = denominations
        .iter()
        .try_fold(Amount(0), |total, denomination| {
            total
                .0
                .checked_add(denomination.amount().0)
                .map(Amount)
                .ok_or_else(|| "explicit QCash denomination total overflowed".to_string())
        })?;
    let requested_cash_amount = Amount(requested_amount.0 - (requested_amount.0 % XPQ));
    if qcash_amount != requested_cash_amount {
        return Err(format!(
            "explicit denominations total {} XPQ, but requested cash amount is {} XPQ",
            qcash_amount.0 / XPQ,
            requested_cash_amount.0 / XPQ
        ));
    }
    Ok((
        qcash_amount,
        Amount(requested_amount.0 - qcash_amount.0),
        denominations,
    ))
}

fn required_option(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn write_new_synced_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("failed to write {}: {error}", path.display()));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("failed to sync {}: {error}", path.display()));
    }
    Ok(())
}

fn wallet_cash_redeem(args: &[String]) -> Result<(), String> {
    let coin_path = args
        .first()
        .filter(|value| !value.starts_with('-'))
        .ok_or_else(|| "usage: cash redeem <coin.XPQ> --to <address>".to_string())?
        .clone();
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut recipient = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--to" => {
                index += 1;
                recipient = Some(parse_address(args.get(index))?);
            }
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            "--fee" => {
                index += 1;
                let _ = parse_fee(args.get(index))?;
            }
            "--nonce" => {
                return Err("--nonce has been removed; wallet uses account statement last_state".to_string());
            }
            value => return Err(format!("unknown cash redeem option `{value}`")),
        }
        index += 1;
    }

    let recipient = recipient.ok_or_else(|| "missing --to address".to_string())?;
    let mut wallet = load_wallet(&wallet_path)?;
    let authorization = resolve_authorization_for_wallet_file(&wallet_path, &mut wallet, None)?;
    let account_state = resolve_wallet_account_state(&wallet.address, &rpc_addr)?;
    ensure_no_pending_outgoing(&account_state)?;
    let file = load_cash_coin_file(&coin_path)?;
    let transaction = QCashTransaction::redeem_from_files_at(
        wallet.address,
        recipient,
        &[file],
        account_state.last_state
    )
    .map(|transaction| transaction.with_last_state(account_state.last_state))
    .map_err(|error| format!("failed to authorize cash coin: {error}"))?;
    let signed = wallet.sign_qcash_transaction(
        transaction,
        &authorization,
        account_state.authorization_registered,
    )?;
    let body = format!(
        "{{\"tx\":\"{}\"}}",
        hex::encode(signed.to_bytes().map_err(|error| error.to_string())?)
    );
    let response = http_post_json(&rpc_addr, "/qcash/tx", &body)?;
    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|error| format!("invalid node response: {error}: {response}"))?;
    if value.get("accepted").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(format!(
            "node rejected cash redeem; original file retained: {response}"
        ));
    }
    let redeem_hash = value
        .get("hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("accepted redeem response has no hash: {response}"))?;
    println!(
        "{{\"accepted\":true,\"lifecycle\":\"ledger-pending\",\"hash\":\"{}\",\"file\":\"{}\",\"next\":\"cash track {}\"}}",
        redeem_hash, coin_path, coin_path
    );
    Ok(())
}

fn wallet_cash_track(args: &[String]) -> Result<(), String> {
    let lookup = args
        .first()
        .ok_or_else(|| "usage: cash track <file-name-or-short-id> [--rpc host:port]".to_string())?;
    let mut rpc_addr = default_rpc_addr();
    if let Some(index) = args
        .iter()
        .position(|value| value == "--rpc" || value == "--rpc-addr")
    {
        rpc_addr = required_option(args, index + 1, "--rpc")?;
    }
    let name = qcash_lookup_name(lookup)?;
    let response = http_get(&rpc_addr, &format!("/qcash/file/{name}"))?;
    print_qcash_file_lookup(&response)?;
    Ok(())
}

fn wallet_cash_utxos(args: &[String]) -> Result<(), String> {
    let mut rpc_addr = default_rpc_addr();
    if let Some(index) = args
        .iter()
        .position(|value| value == "--rpc" || value == "--rpc-addr")
    {
        rpc_addr = required_option(args, index + 1, "--rpc")?;
    }
    let response = http_get(&rpc_addr, "/qcash/utxos")?;
    let value: serde_json::Value = serde_json::from_str(&response)
        .map_err(|error| format!("failed to parse QCash UTXO explorer: {error}: {response}"))?;
    println!("QCash UTXO Explorer");
    println!(
        "Height        : {}",
        value
            .get("height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    );
    println!(
        "Total UTXO    : {}",
        value
            .get("total")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    );
    let Some(utxos) = value.get("utxos").and_then(serde_json::Value::as_array) else {
        return Err("QCash UTXO response has no utxos array".to_string());
    };
    for (index, utxo) in utxos.iter().enumerate() {
        println!();
        println!("UTXO #{}", index + 1);
        if let Some(coin_id) = json_str(utxo, "coin_id") {
            println!("Coin id       : {coin_id}");
        }
        if let Some(denomination) = utxo.get("denomination").and_then(serde_json::Value::as_u64) {
            println!("Denomination  : {denomination} XPQ");
        }
        if let Some(status) = json_str(utxo, "status") {
            println!("Status        : {}", qcash_status_label(status));
        }
        if let Some(issued_height) = utxo
            .get("issued_height")
            .and_then(serde_json::Value::as_u64)
        {
            println!("Issued height : {issued_height}");
        }
        if let Some(maturity_height) = utxo
            .get("maturity_height")
            .and_then(serde_json::Value::as_u64)
        {
            println!("Maturity      : height {maturity_height}");
        }
    }
    Ok(())
}

fn print_qcash_file_lookup(response: &str) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(response)
        .map_err(|error| format!("failed to parse QCash file lookup: {error}: {response}"))?;
    let status = json_str(&value, "status").unwrap_or("unknown");
    println!("QCash file status");
    println!("Status        : {}", qcash_status_label(status));
    if let Some(file_name) = json_str(&value, "file_name").or_else(|| json_str(&value, "lookup")) {
        println!("File          : {file_name}");
    }
    if let Some(denomination) = value
        .get("denomination")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Denomination  : {denomination} XPQ");
    }
    if let Some(short_id) = json_str(&value, "short_coin_id") {
        println!("Short coin id : {short_id}");
    } else if let Some(prefix) = json_str(&value, "coin_id_prefix") {
        println!("Coin id prefix: {prefix}");
    }
    if let Some(coin_id) = json_str(&value, "coin_id") {
        println!("Coin id       : {coin_id}");
    }
    if let Some(height) = value.get("height").and_then(serde_json::Value::as_u64) {
        println!("Node height   : {height}");
    }
    if let Some(issued_height) = value
        .get("issued_height")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Issued height : {issued_height}");
    }
    if let Some(redeemable_height) = value
        .get("redeemable_height")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Redeemable   : height {redeemable_height}");
    }
    if let Some(remaining) = value
        .get("remaining_redeem_delay_blocks")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Remaining     : {remaining} block(s)");
    }
    if let Some(output_index) = value
        .get("output_index")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Output index  : {output_index}");
    }
    if let Some(tx_hash) = json_str(&value, "withdraw_tx_hash") {
        println!("Withdraw tx   : {tx_hash}");
    }
    if let Some(withdrawer) = json_str(&value, "withdrawer") {
        println!("Withdrawer    : {withdrawer}");
    }
    if let Some(matches) = value.get("matches").and_then(serde_json::Value::as_u64) {
        println!("Matches       : {matches}");
    }
    if let Some(operation) = json_str(&value, "spent_operation") {
        println!("Spent by      : {operation}");
    }
    if let Some(operation) = json_str(&value, "pending_operation") {
        println!("Pending use   : {operation}");
    }
    if let Some(txid) = json_str(&value, "pending_redeem_txid") {
        println!("Pending txid  : {txid}");
    }
    if let Some(txid) = json_str(&value, "pending_redeem_txid") {
        println!("Pending txid : {txid}");
    }
    if let Some(txid) = json_str(&value, "spent_txid") {
        println!("Spent txid    : {txid}");
    }
    if let Some(txid) = json_str(&value, "spent_txid") {
        println!("Spent txid   : {txid}");
    }
    if let Some(height) = value
        .get("spent_height")
        .and_then(serde_json::Value::as_u64)
    {
        println!("Spent height  : {height}");
    }
    if let Some(block) = json_str(&value, "spent_block_hash") {
        println!("Spent block   : {block}");
    }
    if let Some(signer) = json_str(&value, "redeem_signer") {
        println!("Redeem signer: {signer}");
    }
    if let Some(recipient) = json_str(&value, "recipient") {
        println!("Recipient     : {recipient}");
    }
    Ok(())
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn qcash_status_label(status: &str) -> &'static str {
    match status {
        "redeemable" => "redeemable",
        "pending" => "active — waiting for redeem eligibility",
        "redeem_pending" => "redeem pending confirmation",
        "spent" => "spent",
        "unknown" => "unknown",
        "spent_or_unknown" => "spent or unknown (legacy node)",
        _ => "unknown",
    }
}

fn qcash_lookup_name(value: &str) -> Result<String, String> {
    let name = std::path::Path::new(value)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(value)
        .trim();
    if name.is_empty() {
        return Err("cash file name or short coin id is required".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    {
        return Err("cash file lookup contains unsupported characters".to_string());
    }
    Ok(name.to_string())
}

fn cash_lifecycle(path: &std::path::Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    if name.ends_with(".QCash") || name.ends_with(".XPQ") {
        Some("ready")
    } else {
        None
    }
}

fn cash_files_in(directory: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "refusing symbolic link in QCash directory: {}",
                entry.path().display()
            ));
        }
        if file_type.is_file() && cash_lifecycle(&entry.path()).is_some() {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

#[derive(Default)]
struct QCashLocalTotals {
    files: usize,
    known: u64,
    redeemable: u64,
    pending: u64,
    redeem_pending: u64,
    spent_or_unknown: u64,
}

fn qcash_local_totals(
    directory: &std::path::Path,
    rpc_addr: &str,
) -> Result<QCashLocalTotals, String> {
    if !directory.exists() {
        return Ok(QCashLocalTotals::default());
    }
    let files = cash_files_in(directory)?;
    let mut totals = QCashLocalTotals::default();
    for path in files {
        let file = load_cash_coin_file(
            path.to_str()
                .ok_or_else(|| "cash path is not valid UTF-8".to_string())?,
        )?;
        totals.files += 1;
        let amount = file.denomination.amount().0;
        let coin_id = hex::encode(file.coin_id);
        let response = http_get(rpc_addr, &format!("/qcash/coin/{coin_id}"))?;
        let status = serde_json::from_str::<serde_json::Value>(&response)
            .ok()
            .and_then(|value| {
                value
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "spent_or_unknown".to_string());
        match status.as_str() {
            "redeemable" => {
                totals.redeemable = totals.redeemable.saturating_add(amount);
                totals.known = totals.known.saturating_add(amount);
            }
            "pending" => {
                totals.pending = totals.pending.saturating_add(amount);
                totals.known = totals.known.saturating_add(amount);
            }
            "redeem_pending" => {
                totals.redeem_pending = totals.redeem_pending.saturating_add(amount);
                totals.known = totals.known.saturating_add(amount);
            }
            _ => {
                totals.spent_or_unknown = totals.spent_or_unknown.saturating_add(amount);
            }
        }
    }
    Ok(totals)
}

fn wallet_cash_list(args: &[String]) -> Result<(), String> {
    let directory = std::path::Path::new(args.first().map(String::as_str).unwrap_or("./cash"));
    let files = cash_files_in(directory)?;
    let mut totals = std::collections::BTreeMap::<&str, (usize, u64)>::new();
    for path in &files {
        let file = load_cash_coin_file(
            path.to_str()
                .ok_or_else(|| "cash path is not valid UTF-8".to_string())?,
        )?;
        let lifecycle = cash_lifecycle(path).ok_or_else(|| {
            format!(
                "QCash file no longer has a recognized lifecycle: {}",
                path.display()
            )
        })?;
        let total = totals.entry(lifecycle).or_default();
        total.0 += 1;
        total.1 = total.1.saturating_add(file.denomination.amount().0);
        println!(
            "{{\"file\":\"{}\",\"lifecycle\":\"{}\",\"coin_id\":\"{}\",\"denomination\":{}}}",
            path.display(),
            lifecycle,
            hex::encode(file.coin_id),
            file.denomination.xpq()
        );
    }
    let coins: usize = totals.values().map(|(count, _)| *count).sum();
    let value: u64 = totals.values().map(|(_, amount)| *amount).sum();
    println!(
        "{{\"directory\":\"{}\",\"coins\":{},\"value\":{},\"states\":{}}}",
        directory.display(),
        coins,
        value,
        serde_json::to_string(&totals).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn create_private_directory(path: &std::path::Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|error| {
        format!(
            "failed to create private directory {}: {error}",
            path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to secure {}: {error}", path.display()))?;
    }
    Ok(())
}

fn copy_cash_file_exclusive(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let bytes = fs::read(source)
        .map_err(|error| format!("failed to read {}: {error}", source.display()))?;
    write_new_synced_file(destination, &bytes)
}

fn wallet_cash_backup(args: &[String]) -> Result<(), String> {
    let source = args
        .first()
        .ok_or_else(|| "usage: cash backup <cash-directory> <new-backup-directory>".to_string())?;
    let destination = args
        .get(1)
        .ok_or_else(|| "usage: cash backup <cash-directory> <new-backup-directory>".to_string())?;
    let source = std::path::Path::new(source);
    let destination = std::path::Path::new(destination);
    let files = cash_files_in(source)?;
    if files.is_empty() {
        return Err("cash directory contains no QCash files".to_string());
    }
    for path in &files {
        load_cash_coin_file(
            path.to_str()
                .ok_or_else(|| "cash path is not valid UTF-8".to_string())?,
        )?;
    }
    create_private_directory(destination)?;
    let mut copied = 0_usize;
    for path in files {
        let name = path
            .file_name()
            .ok_or_else(|| "cash file has no name".to_string())?;
        copy_cash_file_exclusive(&path, &destination.join(name))?;
        copied += 1;
    }
    println!(
        "{{\"backup\":true,\"source\":\"{}\",\"destination\":\"{}\",\"coins\":{},\"warning\":\"unencrypted bearer backup\"}}",
        source.display(),
        destination.display(),
        copied
    );
    Ok(())
}

fn wallet_cash_recover(args: &[String]) -> Result<(), String> {
    let backup = args
        .first()
        .ok_or_else(|| "usage: cash recover <backup-directory> <cash-directory>".to_string())?;
    let destination = args
        .get(1)
        .ok_or_else(|| "usage: cash recover <backup-directory> <cash-directory>".to_string())?;
    let backup = std::path::Path::new(backup);
    let destination = std::path::Path::new(destination);
    let files = cash_files_in(backup)?;
    if files.is_empty() {
        return Err("backup contains no QCash files".to_string());
    }
    for path in &files {
        load_cash_coin_file(
            path.to_str()
                .ok_or_else(|| "cash path is not valid UTF-8".to_string())?,
        )?;
        let name = path
            .file_name()
            .ok_or_else(|| "cash file has no name".to_string())?;
        if destination.join(name).exists() {
            return Err(format!(
                "recovery would overwrite existing file {}",
                destination.join(name).display()
            ));
        }
    }
    if !destination.exists() {
        create_private_directory(destination)?;
    } else if !destination.is_dir() {
        return Err("cash recovery destination is not a directory".to_string());
    }
    let mut restored = 0_usize;
    for path in files {
        load_cash_coin_file(
            path.to_str()
                .ok_or_else(|| "cash path is not valid UTF-8".to_string())?,
        )?;
        let name = path
            .file_name()
            .ok_or_else(|| "cash file has no name".to_string())?;
        copy_cash_file_exclusive(&path, &destination.join(name))?;
        restored += 1;
    }
    println!(
        "{{\"recovered\":true,\"backup\":\"{}\",\"destination\":\"{}\",\"coins\":{}}}",
        backup.display(),
        destination.display(),
        restored
    );
    Ok(())
}

fn load_cash_coin_file(path: &str) -> Result<QCashCoinFile, String> {
    let bytes = Zeroizing::new(
        fs::read(path).map_err(|error| format!("failed to read cash file {path}: {error}"))?,
    );
    decode_qcash_coin_file(&bytes).map_err(|error| format!("invalid cash file {path}: {error}"))
}

fn wallet_send_short(args: &[String]) -> Result<(), String> {
    let to = parse_address(args.first())?;
    let amount = parse_amount(args.get(1), "amount")?;
    let mut wallet_path = DEFAULT_WALLET_PATH.to_string();
    let mut rpc_addr = default_rpc_addr();
    let mut fee = Amount(DEFAULT_TRANSACTION_FEE);
    let mut authorization = None;
    let mut extra_outputs = Vec::new();
    let mut index = 2;

    while index < args.len() {
        match args[index].as_str() {
            "--wallet" => {
                index += 1;
                wallet_path = args
                    .get(index)
                    .ok_or_else(|| "missing value for --wallet".to_string())?
                    .clone();
            }
            "--rpc" | "--rpc-addr" => {
                index += 1;
                rpc_addr = args
                    .get(index)
                    .ok_or_else(|| "missing value for --rpc".to_string())?
                    .clone();
            }
            "--fee" => {
                index += 1;
                fee = parse_fee(args.get(index))?;
            }
            "--output" => {
                index += 1;
                extra_outputs.extend(parse_transfer_output_specs(
                    args.get(index)
                        .ok_or_else(|| "missing value for --output".to_string())?,
                )?);
            }
            "--nonce" => return Err(
                "--nonce has been removed; wallet uses account statement last_state".to_string(),
            ),
            "--auth-secret-key" => {
                index += 1;
                authorization = Some(AuthorizationInput::Keys(Box::new(
                    authorization_from_secret_key(parse_secret_key(args.get(index))?),
                )));
            }
            "--auth-password" => {
                index += 1;
                authorization = Some(AuthorizationInput::Password(Zeroizing::new(
                    required_option(args, index, "--auth-password")?,
                )));
            }
            value => return Err(format!("unknown wallet send option `{value}`")),
        }
        index += 1;
    }

    let mut outputs = vec![TransferOutput {
        to: to.into(),
        amount,
    }];
    outputs.extend(extra_outputs);
    submit_wallet_outputs(
        &wallet_path,
        outputs,
        fee,
        &rpc_addr,
        true,
        authorization,
    )
}

fn submit_wallet_payment(
    wallet_path: &str,
    to: Address,
    amount: Amount,
    fee: Amount,
    rpc_addr: &str,
    authorization: Option<AuthorizationInput>,
) -> Result<(), String> {
    submit_wallet_outputs(
        wallet_path,
        vec![TransferOutput {
            to: to.into(),
            amount,
        }],
        fee,
        rpc_addr,
        true,
        authorization,
    )
}

fn submit_wallet_outputs(
    wallet_path: &str,
    mut outputs: Vec<TransferOutput>,
    fee: Amount,
    rpc_addr: &str,
    submit: bool,
    authorization: Option<AuthorizationInput>,
) -> Result<(), String> {
    if outputs.is_empty() {
        return Err("at least one transfer output is required".to_string());
    }
    if outputs.len() > MAX_BATCH_OUTPUTS {
        return Err(format!(
            "transfer supports at most {MAX_BATCH_OUTPUTS} outputs"
        ));
    }
    let mut wallet = load_wallet(wallet_path)?;
    let authorization =
        resolve_authorization_for_wallet_file(wallet_path, &mut wallet, authorization)?;
    reject_multiple_block_miner_outputs(&outputs)?;
    let account_state = resolve_wallet_account_state(&wallet.address, rpc_addr)?;
    ensure_no_pending_outgoing(&account_state)?;
    append_market_fee_output(
        &wallet,
        &authorization,
        account_state.authorization_registered,
        &mut outputs,
        fee,
        account_state.last_state,
        rpc_addr,
    )?;
    let (transaction, authorization_registered) =
        match draft_wallet_transfer(&wallet.address, &outputs, rpc_addr) {
            Ok(draft) => draft,
            Err(error) if transfer_draft_fallback_allowed(&error) => {
                let transaction = Transaction::new(wallet.address, outputs)
                    .with_last_state(account_state.last_state);
                transaction
                    .validate()
                    .map_err(|error| format!("invalid transfer outputs: {error}"))?;
                (transaction, account_state.authorization_registered)
            }
            Err(error) => return Err(error),
        };
    let signed = wallet.sign_transaction(
        transaction,
        Some(authorization),
        authorization_registered,
    )?;
    let tx_hex = signed_transaction_to_hex(&signed)?;
    if submit {
        let body = format!("{{\"tx\":\"{tx_hex}\"}}");
        println!("{}", http_post_json(rpc_addr, "/tx", &body)?);
    } else {
        let outputs = signed
            .transaction
            .outputs()
            .map(|output| {
                serde_json::json!({
                    "to": output_target_to_string(output.to),
                    "amount": output.amount.0
                })
            })
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::json!({
                "tx": tx_hex,
                "hash": hex::encode(signed.hash().map_err(|error| error.to_string())?.0),
                "from": address_to_string(&signed.transaction.from),
                "outputs": outputs,
                "total_amount": signed.transaction.total_amount()
                    .map_err(|error| error.to_string())?.0,
                "last_state": hex::encode(signed.transaction.last_state.0),
            })
        );
    }

    Ok(())
}

fn append_market_fee_output(
    wallet: &Wallet,
    authorization: &AuthorizationKeys,
    authorization_registered: bool,
    outputs: &mut Vec<TransferOutput>,
    requested_fee: Amount,
    last_state: Hash,
    rpc_addr: &str,
) -> Result<(), String> {
    if outputs
        .iter()
        .any(|output| output.to == paqus::transaction::OutputTarget::BlockMiner)
    {
        return Ok(());
    }
    if outputs.len() >= MAX_BATCH_OUTPUTS {
        return Err(format!(
            "automatic MarketFee output requires one free output slot; transfer supports at most {MAX_BATCH_OUTPUTS} outputs"
        ));
    }
    let fee = if requested_fee.0 == DEFAULT_TRANSACTION_FEE {
        let estimate = Transaction::new(wallet.address, outputs.to_vec()).with_last_state(last_state);
        let signed =
            wallet.sign_transaction(estimate, Some(authorization.clone()), authorization_registered)?;
        policy_fee_for_virtual_size(
            rpc_addr,
            SignedProtocolTransaction::from(signed)
                .virtual_size()
                .map_err(|error| error.to_string())?,
        )?
    } else {
        requested_fee
    };
    if fee.0 == 0 {
        return Ok(());
    }
    outputs.push(TransferOutput {
        to: paqus::transaction::OutputTarget::BlockMiner,
        amount: fee,
    });
    Ok(())
}

fn reject_multiple_block_miner_outputs(outputs: &[TransferOutput]) -> Result<(), String> {
    if block_miner_output_count(outputs) > 1 {
        return Err("transfer supports at most one block_miner fee output".to_string());
    }
    Ok(())
}

fn block_miner_output_count(outputs: &[TransferOutput]) -> usize {
    outputs
        .iter()
        .filter(|output| output.to == paqus::transaction::OutputTarget::BlockMiner)
        .count()
}

fn draft_wallet_transfer(
    signer: &Address,
    outputs: &[TransferOutput],
    rpc_addr: &str,
) -> Result<(Transaction, bool), String> {
    let request_outputs = outputs
        .iter()
        .map(|output| DraftTransferRpcOutput {
            to: output_target_to_string(output.to),
            amount: output.amount.0,
        })
        .collect::<Vec<_>>();
    let request = DraftTransferRpcRequest {
        signer: address_to_string(signer),
        outputs: request_outputs,
    };
    let body = serde_json::to_string(&request)
        .map_err(|error| format!("failed to encode draft request: {error}"))?;
    let response = http_post_json(rpc_addr, "/draft/transfer", &body)?;
    let draft: DraftTransferRpcResponse = serde_json::from_str(&response)
        .map_err(|error| format!("failed to parse transfer draft rpc response: {error}"))?;
    let transaction_bytes =
        hex::decode(&draft.transaction).map_err(|error| format!("invalid draft tx hex: {error}"))?;
    let transaction: Transaction = canonical_deserialize(&transaction_bytes)
        .map_err(|error| format!("invalid draft transaction payload: {error}"))?;
    verify_transfer_draft(signer, outputs, &transaction, &draft)?;
    Ok((transaction, draft.authorization_registered))
}

fn transfer_draft_fallback_allowed(error: &str) -> bool {
    error.contains("HTTP 404") || error.to_ascii_lowercase().contains("not found")
}

fn verify_transfer_draft(
    signer: &Address,
    expected_outputs: &[TransferOutput],
    transaction: &Transaction,
    draft: &DraftTransferRpcResponse,
) -> Result<(), String> {
    if &transaction.from != signer {
        return Err("node draft signer does not match wallet address".to_string());
    }
    let actual_outputs = transaction.outputs().collect::<Vec<_>>();
    if actual_outputs.len() != expected_outputs.len() {
        return Err("node draft output count does not match wallet request".to_string());
    }
    for (actual, expected) in actual_outputs.iter().zip(expected_outputs) {
        if actual.to != expected.to || actual.amount != expected.amount {
            return Err("node draft outputs do not match wallet request".to_string());
        }
    }
    let encoded =
        transaction_bytes(transaction).map_err(|error| format!("failed to verify draft: {error}"))?;
    if hex::encode(encoded) != draft.transaction {
        return Err("node draft canonical encoding mismatch".to_string());
    }
    if hex::encode(transaction.last_state.0) != draft.last_state {
        return Err("node draft last_state metadata mismatch".to_string());
    }
    let signing_bytes = transaction
        .signing_bytes()
        .map_err(|error| format!("failed to verify draft signing bytes: {error}"))?;
    if hex::encode(signing_bytes) != draft.signing_bytes {
        return Err("node draft signing bytes mismatch".to_string());
    }
    transaction
        .validate()
        .map_err(|error| format!("invalid node transfer draft: {error}"))?;
    Ok(())
}

fn output_target_to_string(target: paqus::transaction::OutputTarget) -> String {
    match target {
        paqus::transaction::OutputTarget::Address(address) => address_to_string(&address),
        paqus::transaction::OutputTarget::BlockMiner => "block_miner".to_string(),
    }
}

fn parse_transfer_output_specs(value: &str) -> Result<Vec<TransferOutput>, String> {
    let outputs = value
        .split(',')
        .filter(|item| !item.trim().is_empty())
        .map(|item| {
            let item = item.trim();
            let (target, amount) = item
                .split_once(':')
                .or_else(|| item.split_once('='))
                .ok_or_else(|| format!("invalid output `{item}`; expected recipient:amount-xpq"))?;
            let target = match target.trim() {
                "block_miner" | "miner" => paqus::transaction::OutputTarget::BlockMiner,
                address => parse_address_string(address)?.into(),
            };
            Ok(TransferOutput {
                to: target,
                amount: parse_xpq_amount(amount.trim())
                    .map_err(|error| format!("invalid amount in output `{item}`: {error}"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if outputs.is_empty() {
        return Err("--output requires at least one recipient:amount pair".to_string());
    }
    Ok(outputs)
}

#[derive(Debug)]
struct WalletAccountState {
    last_state: Hash,
    authorization_registered: bool,
    pending_outgoing: u64,
    pending_outgoing_hashes: Vec<String>,
}

fn resolve_wallet_account_state(
    address: &Address,
    rpc_addr: &str,
) -> Result<WalletAccountState, String> {
    let address_hex = address_to_string(address);
    let balance_body = http_get(rpc_addr, &format!("/balance/{address_hex}"))?;
    let balance: BalanceRpcResponse = serde_json::from_str(&balance_body)
        .map_err(|error| format!("failed to parse balance rpc response: {error}"))?;
    let draft = http_get(rpc_addr, &format!("/draft-basis/{address_hex}"))
        .ok()
        .and_then(|body| serde_json::from_str::<DraftBasisRpcResponse>(&body).ok());
    let statement = draft
        .as_ref()
        .map(|basis| basis.last_state.as_str())
        .or(balance.statement.as_deref())
        .ok_or_else(|| "balance rpc response is missing account statement".to_string())?;
    let pending_outgoing = draft
        .as_ref()
        .map(|basis| basis.pending_outgoing)
        .unwrap_or(balance.pending_outgoing);
    let pending_outgoing_hashes = draft
        .as_ref()
        .map(|basis| basis.pending_outgoing_hashes.clone())
        .unwrap_or_default();

    Ok(WalletAccountState {
        last_state: parse_hash_hex(statement)?,
        authorization_registered: balance.authorization_registered,
        pending_outgoing,
        pending_outgoing_hashes,
    })
}

fn ensure_no_pending_outgoing(account_state: &WalletAccountState) -> Result<(), String> {
    if account_state.pending_outgoing == 0 && account_state.pending_outgoing_hashes.is_empty() {
        return Ok(());
    }
    let hashes = if account_state.pending_outgoing_hashes.is_empty() {
        "unknown pending tx".to_string()
    } else {
        account_state.pending_outgoing_hashes.join(",")
    };
    Err(format!(
        "account has pending outgoing transaction(s): {hashes}; wait until they are included or dropped before creating another tx from this wallet"
    ))
}

#[derive(Debug, Deserialize)]
struct DraftBasisRpcResponse {
    last_state: String,
    #[serde(default)]
    spendable_after_pending: u64,
    #[serde(default)]
    finalized_height: u64,
    #[serde(default)]
    pending_outgoing: u64,
    #[serde(default)]
    pending_outgoing_hashes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DraftTransferRpcRequest {
    signer: String,
    outputs: Vec<DraftTransferRpcOutput>,
}

#[derive(Debug, Serialize)]
struct DraftTransferRpcOutput {
    to: String,
    amount: u64,
}

#[derive(Debug, Deserialize)]
struct DraftTransferRpcResponse {
    transaction: String,
    signing_bytes: String,
    last_state: String,
    #[serde(default)]
    authorization_registered: bool,
}

#[derive(Debug, Deserialize)]
struct BalanceRpcResponse {
    statement: Option<String>,
    #[serde(default)]
    authorization_registered: bool,
    #[serde(default)]
    pending_outgoing: u64,
}

#[derive(Debug, Deserialize, Serialize, Zeroize, ZeroizeOnDrop)]
struct WalletFile {
    version: u8,
    address: String,
    #[serde(default)]
    mnemonic: Option<String>,
    public_key: String,
    secret_key: String,
    #[serde(default)]
    auth_public_key: Option<String>,
    #[serde(default)]
    auth_secret_key: Option<String>,
}

fn load_wallet(path: &str) -> Result<Wallet, String> {
    let contents = Zeroizing::new(
        fs::read(path).map_err(|error| format!("failed to read wallet file {path}: {error}"))?,
    );
    load_wallet_bytes(path, &contents)
}

fn load_wallet_address(path: &str) -> Result<Address, String> {
    load_wallet(path).map(|wallet| wallet.address)
}

fn load_wallet_bytes(path: &str, contents: &[u8]) -> Result<Wallet, String> {
    let mut wallet_file: WalletFile = serde_json::from_slice(contents)
        .map_err(|error| format!("failed to parse wallet file {path}: {error}"))?;
    if wallet_file.version != WALLET_VERSION {
        return Err("unsupported wallet format".to_string());
    }
    let address = parse_address_string(&wallet_file.address)
        .map_err(|error| format!("invalid wallet address `{}`: {error}", wallet_file.address))?;
    let secret_key = parse_secret_key(Some(&wallet_file.secret_key))?;
    let auth_public_key = wallet_file
        .auth_public_key
        .as_deref()
        .map(parse_public_key_hex)
        .transpose()?
        .ok_or_else(|| {
            "wallet is missing auth_public_key and is incompatible with dual-key addresses"
                .to_string()
        })?;
    let auth_secret_key = wallet_file
        .auth_secret_key
        .as_ref()
        .map(|value| parse_secret_key(Some(value)))
        .transpose()?;
    let mut wallet = Wallet::from_keys_with_authorization(
        derive_public_key(&secret_key),
        secret_key,
        auth_public_key,
        auth_secret_key,
    );
    wallet.mnemonic = wallet_file.mnemonic.take();
    if wallet.address != address {
        return Err("wallet address does not match secret key".to_string());
    }
    if hex::encode(wallet.public_key.0) != wallet_file.public_key {
        return Err("wallet public key does not match secret key".to_string());
    }
    Ok(wallet)
}

fn signed_transaction_to_hex(transaction: &SignedTransaction) -> Result<String, String> {
    Ok(hex::encode(
        transaction.to_bytes().map_err(|error| error.to_string())?,
    ))
}

#[allow(dead_code)]
fn submit_or_print_protocol_transaction(
    transaction: SignedProtocolTransaction,
    submit: bool,
    rpc_addr: &str,
) -> Result<(), String> {
    let tx_hex = hex::encode(
        signed_protocol_transaction_bytes(&transaction).map_err(|error| error.to_string())?,
    );
    if submit {
        let body = format!("{{\"tx\":\"{tx_hex}\"}}");
        let response = http_post_json(rpc_addr, "/protocol/transaction", &body)?;
        println!("{response}");
    } else {
        println!(
            "{}",
            serde_json::json!({
                "tx": tx_hex,
                "family": protocol_family_name(&transaction),
                "hash": hex::encode(transaction.hash().map_err(|error| error.to_string())?.0),
                "signer": address_to_string(&transaction.signer()),
            })
        );
    }
    Ok(())
}

#[allow(dead_code)]
fn protocol_family_name(transaction: &SignedProtocolTransaction) -> &'static str {
    match transaction {
        SignedProtocolTransaction::BatchTransfer(_) => "transfer",
        SignedProtocolTransaction::QCash(_) => "qcash",
    }
}

#[allow(dead_code)]
fn qcash_policy_fee(
    wallet: &Wallet,
    transaction: QCashTransaction,
    authorization: &AuthorizationKeys,
    rpc_addr: &str,
) -> Result<Amount, String> {
    let signed = wallet.sign_qcash_transaction(transaction, authorization, false)?;
    policy_fee_for_virtual_size(
        rpc_addr,
        SignedProtocolTransaction::from(signed)
            .virtual_size()
            .map_err(|error| error.to_string())?,
    )
}

fn parse_address(value: Option<&String>) -> Result<Address, String> {
    parse_address_string(value.ok_or_else(|| "missing address".to_string())?)
}

fn parse_address_string(value: &str) -> Result<Address, String> {
    address_from_string(value).map_err(|error| format!("invalid address `{value}`: {error}"))
}

fn parse_secret_key(value: Option<&String>) -> Result<SecretKey, String> {
    let value = value.ok_or_else(|| "missing secret key".to_string())?;
    let bytes = hex::decode(value).map_err(|error| format!("invalid secret key hex: {error}"))?;
    if bytes.len() != paqus::crypto::SECRET_KEY_SIZE {
        return Err(format!(
            "secret key must be {} bytes",
            paqus::crypto::SECRET_KEY_SIZE
        ));
    }
    let mut key = [0_u8; paqus::crypto::SECRET_KEY_SIZE];
    key.copy_from_slice(&bytes);
    Ok(SecretKey(key))
}

fn parse_public_key_hex(value: &str) -> Result<PublicKey, String> {
    let bytes = hex::decode(value).map_err(|error| format!("invalid public key hex: {error}"))?;
    if bytes.len() != paqus::crypto::PUBLIC_KEY_SIZE {
        return Err(format!(
            "public key must be {} bytes",
            paqus::crypto::PUBLIC_KEY_SIZE
        ));
    }
    let mut key = [0_u8; paqus::crypto::PUBLIC_KEY_SIZE];
    key.copy_from_slice(&bytes);
    Ok(PublicKey(key))
}

fn parse_hash_hex(value: &str) -> Result<Hash, String> {
    let bytes = hex::decode(value).map_err(|error| format!("invalid hash hex: {error}"))?;
    if bytes.len() != paqus::crypto::HASH_SIZE {
        return Err(format!("hash must be {} bytes", paqus::crypto::HASH_SIZE));
    }
    let mut hash = [0_u8; paqus::crypto::HASH_SIZE];
    hash.copy_from_slice(&bytes);
    Ok(Hash(hash))
}

fn authorization_from_secret_key(secret_key: SecretKey) -> AuthorizationKeys {
    AuthorizationKeys {
        public_key: derive_public_key(&secret_key),
        secret_key,
    }
}

fn authorization_from_password(
    password: &str,
    primary_public_key: &PublicKey,
) -> Result<AuthorizationKeys, String> {
    let keypair = authorization_keypair_from_password(password.as_bytes(), primary_public_key)
        .map_err(|error| format!("authorization key derivation failed: {error}"))?;
    Ok(AuthorizationKeys {
        public_key: keypair.public_key,
        secret_key: keypair.secret_key,
    })
}

fn resolve_authorization_for_wallet_file(
    wallet_path: &str,
    wallet: &mut Wallet,
    input: Option<AuthorizationInput>,
) -> Result<AuthorizationKeys, String> {
    let keys = match input {
        Some(AuthorizationInput::Keys(keys)) => *keys,
        Some(AuthorizationInput::Password(password)) => {
            authorization_from_password(&password, &wallet.public_key)?
        }
        None => match wallet.stored_authorization_keys() {
            Some(keys) => keys,
            None => {
                let password = prompt_hidden("Authorization password")?;
                authorization_from_password(&password, &wallet.public_key)?
            }
        },
    };
    if wallet.auth_public_key != Some(keys.public_key) {
        wallet.auth_public_key = Some(keys.public_key);
        wallet.auth_secret_key = None;
        save_wallet_overwrite(wallet_path, wallet)?;
    }
    Ok(keys)
}

fn parse_amount(value: Option<&String>, flag: &str) -> Result<Amount, String> {
    let value = value.ok_or_else(|| format!("missing value for {flag}"))?;
    parse_xpq_amount(value).map_err(|error| format!("invalid XPQ amount for {flag}: {error}"))
}

fn prompt_hidden(label: &str) -> Result<Zeroizing<String>, String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush stdout: {error}"))?;
    let masked = Command::new("stty")
        .args(["-echo", "-icanon", "min", "1", "time", "0"])
        .status()
        .is_ok_and(|status| status.success());
    if masked {
        let mut value = Zeroizing::new(Vec::new());
        let mut byte = Zeroizing::new([0_u8; 1]);
        let read_result = loop {
            match io::stdin().read_exact(&mut byte[..]) {
                Ok(()) if byte[0] == b'\n' || byte[0] == b'\r' => break Ok(()),
                Ok(()) if byte[0] == 8 || byte[0] == 127 => {
                    if value.pop().is_some() {
                        print!("\u{8} \u{8}");
                        let _ = io::stdout().flush();
                    }
                }
                Ok(()) => {
                    value.push(byte[0]);
                    print!("*");
                    let _ = io::stdout().flush();
                }
                Err(error) => break Err(error),
            }
        };
        let _ = Command::new("stty").args(["echo", "icanon"]).status();
        println!();
        read_result.map_err(|error| format!("failed to read password: {error}"))?;
        return String::from_utf8(value.to_vec())
            .map(Zeroizing::new)
            .map_err(|_| "authorization password must be valid UTF-8".to_string());
    }

    let mut value = Zeroizing::new(String::new());
    io::stdin()
        .read_line(&mut value)
        .map_err(|error| format!("failed to read input: {error}"))?;
    let trimmed_len = value.trim_end_matches(['\r', '\n']).len();
    value.truncate(trimmed_len);
    Ok(value)
}

#[allow(dead_code)]
fn policy_fee_for_virtual_size(rpc_addr: &str, virtual_size: usize) -> Result<Amount, String> {
    let policy = http_get(rpc_addr, "/fee-policy")
        .ok()
        .and_then(|response| serde_json::from_str::<serde_json::Value>(&response).ok());
    let fee_rate = match policy
        .as_ref()
        .and_then(|value| value.get("dynamic_market_fee_rate"))
        .and_then(serde_json::Value::as_u64)
    {
        Some(rate) => rate.max(1),
        None => fee_rate_from_status(&status_value(rpc_addr)?)?,
    };
    fee_for_rate(fee_rate, virtual_size)
}

fn fee_rate_from_status(status: &serde_json::Value) -> Result<u64, String> {
    let dynamic_rate = status
        .get("dynamic_market_fee_rate_per_byte")
        .and_then(serde_json::Value::as_u64);
    let min_relay_rate = status
        .get("min_relay_fee_rate_per_byte")
        .and_then(serde_json::Value::as_u64);
    match (dynamic_rate, min_relay_rate) {
        (Some(dynamic), Some(minimum)) => Ok(dynamic.max(minimum).max(1)),
        (Some(dynamic), None) => Ok(dynamic.max(1)),
        (None, Some(minimum)) => Ok(minimum.max(1)),
        (None, None) => Err("node status is missing dynamic_market_fee_rate_per_byte".to_string()),
    }
}

fn fee_for_rate(fee_rate: u64, virtual_size: usize) -> Result<Amount, String> {
    let virtual_size = u64::try_from(virtual_size)
        .map_err(|_| "transaction virtual size exceeds supported range".to_string())?;
    fee_rate
        .checked_mul(virtual_size.max(1))
        .map(Amount)
        .ok_or_else(|| "automatic transaction fee overflow".to_string())
}

fn parse_fee(value: Option<&String>) -> Result<Amount, String> {
    let value = value.ok_or_else(|| "missing value for --fee".to_string())?;
    if value.eq_ignore_ascii_case("auto") {
        return Ok(Amount(DEFAULT_TRANSACTION_FEE));
    }
    parse_xpq_amount(value).map_err(|error| format!("invalid XPQ amount for --fee: {error}"))
}

fn parse_xpq_amount(value: &str) -> Result<Amount, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("amount is empty".to_string());
    }
    if value.starts_with('-') {
        return Err("amount cannot be negative".to_string());
    }

    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fractional = parts.next();
    if parts.next().is_some() {
        return Err("amount has more than one decimal point".to_string());
    }
    if whole.is_empty() && fractional.is_none_or(str::is_empty) {
        return Err("amount is empty".to_string());
    }
    if !whole.chars().all(|character| character.is_ascii_digit()) {
        return Err("whole XPQ part must contain digits only".to_string());
    }

    let whole_units = if whole.is_empty() {
        0u64
    } else {
        whole
            .parse::<u64>()
            .map_err(|error| format!("whole XPQ part is too large: {error}"))?
    };

    let fractional_units = match fractional {
        Some("") | None => 0u64,
        Some(value) => {
            let decimals = usize::from(DECIMALS);
            if value.len() > decimals {
                return Err(format!("XPQ supports at most {DECIMALS} decimal places"));
            }
            if !value.chars().all(|character| character.is_ascii_digit()) {
                return Err("fractional XPQ part must contain digits only".to_string());
            }
            let mut padded = value.to_string();
            while padded.len() < decimals {
                padded.push('0');
            }
            padded
                .parse::<u64>()
                .map_err(|error| format!("fractional XPQ part is invalid: {error}"))?
        }
    };

    let units = whole_units
        .checked_mul(XPQ)
        .and_then(|units| units.checked_add(fractional_units))
        .ok_or_else(|| "amount is too large".to_string())?;
    Ok(Amount(units))
}


#[cfg(test)]
fn unix_timestamp() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is before unix epoch".to_string())
}

fn http_post_json(addr: &str, path: &str, body: &str) -> Result<String, String> {
    let addr = addr
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid rpc address: {error}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|error| format!("failed to connect rpc: {error}"))?;
    configure_stream(&stream)?;
    let request = format!(
        "POST {path} HTTP/1.1\r\nhost: {addr}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write rpc request: {error}"))?;
    read_http_response(stream)
}

fn http_get(addr: &str, path: &str) -> Result<String, String> {
    let addr = addr
        .parse::<SocketAddr>()
        .map_err(|error| format!("invalid rpc address: {error}"))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))
        .map_err(|error| format!("failed to connect rpc: {error}"))?;
    configure_stream(&stream)?;
    let request = format!("GET {path} HTTP/1.1\r\nhost: {addr}\r\nconnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write rpc request: {error}"))?;
    read_http_response(stream)
}

fn configure_stream(stream: &TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(RPC_HTTP_TIMEOUT))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(RPC_HTTP_TIMEOUT))
        .map_err(|error| format!("failed to set write timeout: {error}"))?;
    Ok(())
}

fn read_http_response(mut stream: TcpStream) -> Result<String, String> {
    let mut response = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                response.extend_from_slice(&buffer[..bytes_read]);
                if response_body_complete(&response)? {
                    break;
                }
            }
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                if response_body_complete(&response)? {
                    break;
                }
                return Err(
                    "failed to read rpc response: timed out waiting for node response".to_string(),
                );
            }
            Err(error) => return Err(format!("failed to read rpc response: {error}")),
        }
    }
    let response = String::from_utf8(response)
        .map_err(|error| format!("failed to decode rpc response: {error}"))?;
    let (headers, body) = match response.split_once("\r\n\r\n") {
        Some((headers, body)) => (headers, body.to_string()),
        None => ("", response.clone()),
    };
    let status_code = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(200);
    if status_code >= 400 {
        return Err(rpc_error_alert(status_code, &body));
    }
    Ok(body)
}

fn rpc_error_alert(status_code: u16, body: &str) -> String {
    let value = serde_json::from_str::<serde_json::Value>(body).ok();
    let lifecycle = value
        .as_ref()
        .and_then(|value| value.get("status"))
        .and_then(serde_json::Value::as_str);
    let detail = value
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(serde_json::Value::as_str)
        .filter(|detail| !detail.trim().is_empty())
        .or_else(|| (!body.trim().is_empty()).then_some(body))
        .map(str::to_string)
        .unwrap_or_else(|| format!("HTTP {status_code}"));
    let alert = match lifecycle {
        Some("expired") => "Transaction expired",
        Some("dropped") => "Transaction dropped from mempool",
        Some("reverted") => "Transaction reverted by chain reorganization",
        Some("conflicted") => "Transaction conflicted on the canonical chain",
        Some("rejected") => "Transaction rejected",
        _ if status_code < 500 => "Request rejected by node",
        _ => "Node request error",
    };
    format!("{alert}: {detail}")
}

fn response_body_complete(response: &[u8]) -> Result<bool, String> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(false);
    };
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|error| format!("failed to decode rpc response headers: {error}"))?;
    let Some(content_length) = headers.lines().find_map(content_length) else {
        return Ok(false);
    };
    Ok(response.len() >= header_end + 4 + content_length)
}

fn content_length(line: &str) -> Option<usize> {
    let (name, value) = line.split_once(':')?;
    name.eq_ignore_ascii_case("content-length")
        .then(|| value.trim().parse().ok())
        .flatten()
}

fn default_rpc_addr() -> String {
    env::var(RPC_ADDR_ENV).unwrap_or_else(|_| DEFAULT_WALLET_RPC_ADDR.to_string())
}

fn default_wallet_address_or_empty() -> String {
    load_wallet_address(DEFAULT_WALLET_PATH)
        .map(|address| address_to_string(&address))
        .unwrap_or_default()
}

fn prompt(label: &str) -> Result<String, String> {
    print!("{label}: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush stdout: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("failed to read input: {error}"))?;
    Ok(line.trim().to_string())
}

fn prompt_back(label: &str) -> Result<Option<String>, String> {
    let value = prompt(&format!("{label} (b/back to menu)"))?;
    if is_back(&value) {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn prompt_default(label: &str, default: &str) -> Result<String, String> {
    print!("{label} [{default}]: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush stdout: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("failed to read input: {error}"))?;
    let value = line.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn prompt_default_back(label: &str, default: &str) -> Result<Option<String>, String> {
    let value = prompt_default(&format!("{label} (b/back to menu)"), default)?;
    if is_back(&value) {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn pause_for_menu() -> Result<(), String> {
    let _ = prompt("Press Enter or type b/back to return to menu")?;
    Ok(())
}

fn is_back(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "b" | "back")
}

fn print_help() {
    println!(
        "\
wallet-cli

Usage:
  wallet-cli
  wallet-cli menu
  wallet-cli new [wallet-path] [--words 12|24] [--auth-password password] [--show-secret]
  wallet-cli new-mnemonic [wallet-path] [--words 12|24] [--auth-password password] [--show-secret]
  wallet-cli import [wallet-path] [--mnemonic words]
  wallet-cli restore-mnemonic [wallet-path] [--mnemonic words] [--auth-password password]
  wallet-cli address <secret-key-hex>
  wallet-cli balance [address] [--wallet path] [--rpc host:port]
  wallet-cli stats [--rpc host:port]
  wallet-cli address-stats [address] [--wallet path] [--rpc host:port]
  wallet-cli hashrate [--rpc host:port]
  wallet-cli pay <address> <amount-xpq> [--wallet path] [--fee xpq] [--auth-secret-key hex | --auth-password text] [--rpc host:port]
  wallet-cli send <address> <amount-xpq> [--output address:amount ...] [--wallet path] [--fee xpq] [--auth-secret-key hex | --auth-password text] [--rpc host:port]
  wallet-cli send [--wallet path] [--to <address> --amount xpq] --output address:amount [--output address:amount ...] [--fee xpq] [--auth-secret-key hex | --auth-password text] [--submit] [--rpc host:port]
  wallet-cli pool-payout [--ledger file] [--receipts file] [--wallet path] [--fee xpq] [--rpc host:port] [--execute]
  wallet-cli cash withdraw <amount-xpq> [--denoms 1000,500,100 | 1000x1,500x2] [--out directory] [--wallet path] [--fee xpq] [--rpc host:port]
  wallet-cli cash inspect <coin.XPQ>
  wallet-cli cash redeem <coin.XPQ> --to <address> [--wallet path] [--fee xpq] [--rpc host:port]
  wallet-cli cash track <coin-file-or-directory> [--rpc host:port]
  wallet-cli cash track <file-name-or-short-id> [--rpc host:port]
  wallet-cli cash list [cash-directory]
  wallet-cli cash backup <cash-directory> <new-backup-directory>
  wallet-cli cash recover <backup-directory> <cash-directory>
  wallet-cli events <block|tx|address|id> <value> [--kind event-kind] [--offset n] [--limit n] [--from-height n] [--to-height n] [--rpc host:port]
  wallet-cli rollback list <address> [--rpc host:port]
  wallet-cli rollback show <issue-id> [--rpc host:port]
  wallet-cli rollback verify <issue-id> [--rpc host:port]
  wallet-cli rollback retry <issue-id> [--rpc host:port]
  wallet-cli proof account [address] [--wallet path] [--rpc host:port]
  wallet-cli proof qcash <coin-id> [--wallet path] [--rpc host:port]
  wallet-cli proof status [--wallet path]

Defaults:
  Wallet path: wallet.json
  RPC address: ${RPC_ADDR_ENV} or {DEFAULT_WALLET_RPC_ADDR}
  New wallets are mnemonic-backed and start with authorization inactive; authorized actions ask for the password when needed.
"
    );
}
