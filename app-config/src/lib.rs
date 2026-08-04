#[cfg(any(
    all(feature = "mainnet", feature = "testnet"),
    all(feature = "mainnet", feature = "devnet"),
    all(feature = "testnet", feature = "devnet"),
))]
compile_error!("mainnet, testnet, and devnet features are mutually exclusive");

#[cfg(not(any(feature = "mainnet", feature = "testnet", feature = "devnet")))]
compile_error!("one network feature must be enabled");

pub const RPC_ADDR_ENV: &str = "PAQUS_RPC_ADDR";
pub const NODE_RPC_LISTEN_ADDR_ENV: &str = "PAQUS_NODE_RPC_LISTEN_ADDR";
pub const NODE_P2P_LISTEN_ADDR_ENV: &str = "PAQUS_NODE_P2P_LISTEN_ADDR";
pub const PUBLIC_ADDR_ENV: &str = "PAQUS_PUBLIC_ADDR";
pub const BOOTSTRAP_PEERS_ENV: &str = "PAQUS_BOOTSTRAP_PEERS";
pub const BOOTSTRAP_PEER_IPV6: &str = "[2404:8000:1044:4d8:e5c4:5b9:93bc:656d]:5555";

#[cfg(feature = "mainnet")]
pub const BOOTSTRAP_PEERS: &[&str] = &[BOOTSTRAP_PEER_IPV6];
#[cfg(any(feature = "testnet", feature = "devnet"))]
pub const BOOTSTRAP_PEERS: &[&str] = &[];

#[cfg(feature = "mainnet")]
pub const DEFAULT_P2P_PORT: u16 = 5555;
#[cfg(feature = "testnet")]
pub const DEFAULT_P2P_PORT: u16 = 15555;
#[cfg(feature = "devnet")]
pub const DEFAULT_P2P_PORT: u16 = 25555;

#[cfg(feature = "mainnet")]
pub const DEFAULT_RPC_PORT: u16 = 6666;
#[cfg(feature = "testnet")]
pub const DEFAULT_RPC_PORT: u16 = 16666;
#[cfg(feature = "devnet")]
pub const DEFAULT_RPC_PORT: u16 = 26666;

#[cfg(feature = "mainnet")]
pub const DEFAULT_WALLET_RPC_ADDR: &str = "127.0.0.1:6666";
#[cfg(feature = "testnet")]
pub const DEFAULT_WALLET_RPC_ADDR: &str = "127.0.0.1:16666";
#[cfg(feature = "devnet")]
pub const DEFAULT_WALLET_RPC_ADDR: &str = "127.0.0.1:26666";
