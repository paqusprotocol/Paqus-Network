use wallet_core::{
    AuthorizationInput, AuthorizationKeys, PAQUS_MNEMONIC_DEFAULT_WORDS, Wallet,
    generate_paqus_mnemonic, wallet_file_bytes, wallet_from_paqus_mnemonic,
};

fn wallet_address_string(wallet: &Wallet) -> String {
    address_to_string(&wallet.address)
}

fn save_wallet(path: &str, wallet: &Wallet) -> Result<(), String> {
    let bytes = wallet_file_bytes(wallet)?;
    write_new_synced_file(std::path::Path::new(path), &bytes)
}

fn save_wallet_overwrite(path: &str, wallet: &Wallet) -> Result<(), String> {
    let bytes = wallet_file_bytes(wallet)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("failed to update wallet file {path}: {error}"))?;
    file.write_all(&bytes)
        .map_err(|error| format!("failed to write wallet file {path}: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("failed to sync wallet file {path}: {error}"))
}

fn create_mnemonic_wallet_file(
    path: &str,
    words: usize,
    auth_password: &str,
) -> Result<(Wallet, Zeroizing<String>), String> {
    let mnemonic = generate_paqus_mnemonic(words)?;
    let mut wallet = wallet_from_paqus_mnemonic(&mnemonic, auth_password)?;
    wallet.mnemonic = Some(mnemonic.to_string());
    save_wallet(path, &wallet)?;
    Ok((wallet, mnemonic))
}

fn restore_mnemonic_wallet_file(
    path: &str,
    mnemonic: &str,
    auth_password: &str,
) -> Result<Wallet, String> {
    let mut wallet = wallet_from_paqus_mnemonic(mnemonic, auth_password)?;
    wallet.mnemonic = Some(mnemonic.to_string());
    save_wallet(path, &wallet)?;
    Ok(wallet)
}
