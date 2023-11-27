use ethers::{core::k256::ecdsa::SigningKey, prelude::*, signers::coins_bip39::English};
pub struct WalletClient {
    wallet: Wallet<SigningKey>,
    address: String,
}

// TODO: Implement wallet client methods