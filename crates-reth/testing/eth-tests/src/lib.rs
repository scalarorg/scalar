#![cfg(test)]
mod tests {
    use ethers::{core::k256::ecdsa::SigningKey, prelude::*, signers::coins_bip39::English};

    /// Node rpc url
    ///
    /// Must provide the RPC_URL from env or run a local node before running tests
    const DEFAULT_RPC_URL: &str = "http://127.0.0.1:8545";

    /// Test account, must be prefunded
    ///
    /// This account is the first account derived from the mnemonic
    const TEST_ACCOUNT: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

    const TEST_ACCOUNT_2: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";

    #[tokio::test]
    async fn send_raw_transaction() -> Result<(), Box<dyn std::error::Error>> {
        let (provider, chain_id) = create_provider().await?;
        let (tx, wallet) = create_transaction(chain_id).await?;

        let tx = tx.into();

        // Get the signature
        let signature = wallet.sign_transaction(&tx).await?;

        // Get the raw transaction
        let raw_tx = tx.rlp_signed(&signature);

        // Send the raw transaction
        let result = provider.send_raw_transaction(raw_tx).await?;

        assert!(
            result.tx_hash().to_string().starts_with("0x"),
            "invalid tx hash"
        );

        Ok(())
    }

    #[tokio::test]
    async fn get_balance() -> Result<(), Box<dyn std::error::Error>> {
        let provider = create_provider().await?.0;

        // Valid address
        let balance = provider.get_balance(TEST_ACCOUNT, None).await?;

        assert!(
            balance > U256::zero(),
            "balance should be greater than zero"
        );

        // Random address
        let balance = provider
            .get_balance("0x70997970C51812dc3A010C7d01b50e0d17dc79C7", None)
            .await?;

        assert!(balance == U256::zero(), "balance should be zero");

        Ok(())
    }

    #[tokio::test]
    async fn estimate_gas() -> Result<(), Box<dyn std::error::Error>> {
        let (provider, chain_id) = create_provider().await?;

        // Create a transaction
        let tx = create_transaction(chain_id).await?.0.into();

        // Estimate the gas
        // TODO: This is failing with "insufficient funds for transfer" error
        let gas = provider.estimate_gas(&tx, None).await?;

        assert!(gas > U256::zero(), "gas should be greater than zero");

        Ok(())
    }

    async fn create_provider() -> Result<(Provider<Http>, u64), Box<dyn std::error::Error>> {
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| DEFAULT_RPC_URL.to_string());
        let provider = Provider::<Http>::try_from(rpc_url)?;
        let chain_id = provider.get_chainid().await?.as_u64();
        Ok((provider, chain_id))
    }

    async fn create_test_wallet(
        chain_id: u64,
    ) -> Result<Wallet<SigningKey>, Box<dyn std::error::Error>> {
        /// Includes 20 prefunded accounts with 10_000 ETH each derived from mnemonic "test test test test
        /// test test test test test test test junk".
        /// See crates-reth/primitives/src/chain/spec.rs [DEV] for more details
        const PHRASE: &str = "test test test test test test test test test test test junk";
        // Use first account
        const INDEX: u32 = 0u32;

        // instantiate the wallet with the phrase and the index of the account we want to use
        Ok(MnemonicBuilder::<English>::default()
            .phrase(PHRASE)
            .index(INDEX)?
            .build()?
            .with_chain_id(chain_id))
    }

    async fn create_transaction(
        chain_id: u64,
    ) -> Result<(TransactionRequest, Wallet<SigningKey>), Box<dyn std::error::Error>> {
        let wallet = create_test_wallet(chain_id).await?;

        // Random nonce
        let nonce = rand::random::<u64>();

        // Create a transaction
        Ok((
            TransactionRequest::new()
                .to(TEST_ACCOUNT_2) // this will use ENS
                .nonce(nonce)
                .value(10001)
                .gas(0x76c000)
                .gas_price(0x43423422)
                .chain_id(wallet.chain_id()),
            wallet,
        ))
    }
}
