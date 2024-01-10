use crate::{TestCaseImpl, TestContext};
use async_trait::async_trait;
use ethers::{
    core::k256::ecdsa::SigningKey,
    contract::{abigen, ContractFactory},
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    types::{Address},
};
use ethers_solc::Solc;
use ethers::prelude::*;
use eyre::Result;
use std::{path::Path, sync::Arc,};
use tracing::info;

pub struct DeployAndCallSmartContractTest;

#[async_trait]
impl TestCaseImpl for DeployAndCallSmartContractTest {
    fn name(&self) -> &'static str {
        "DeployAndCallSmartContractTest"
    }

    fn description(&self) -> &'static str {
        "Test deploy and call smart contract to a cluster."
    }

    async fn run(&self, ctx: &mut TestContext) -> Result<(), anyhow::Error> {
        let client = &ctx.clients[0];
        let wallet = client.get_wallet();
        let provider = client.get_fullnode_client();
        let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet.clone()));

        // Deploy an ERC20 token contract on the freshly started local Anvil node that doesn't have any
        // tokens deployed on it. This will also allocate tokens to the `from_wallet` address.
        // You don't need to do this to transfer an already deployed ERC20 token, in that case you only
        // need the token address.
        let contract_address = deploy_token_contract(client.clone()).await.expect("Should deploy contract");
        info!("Deployed ERC20 contract");

        // 1. Generate the ABI for the ERC20 contract. This is will define an `ERC20Contract` struct in
        // this scope that will let us call the methods of the contract.
        abigen!(
            ERC20Contract,
            r#"[
                function balanceOf(address account) external view returns (uint256)
                function decimals() external view returns (uint8)
                function symbol() external view returns (string memory)
                function transfer(address to, uint256 amount) external returns (bool)
                event Transfer(address indexed from, address indexed to, uint256 value)
            ]"#,
        );

        // 2. Create the contract instance to let us call methods of the contract and let it sign
        // transactions with the sender wallet.
        let contract = ERC20Contract::new(contract_address, client);

        // 3. Fetch the decimals used by the contract so we can compute the decimal amount to send.
        let whole_amount: u64 = 1000;
        let decimals = contract.decimals().call().await?;
        let decimal_amount = U256::from(whole_amount) * U256::exp10(decimals as usize);
        let receiver_address = ctx.options.receiver_address().parse().unwrap();

        // 4. Transfer the desired amount of tokens to the `to_address`
        info!("Transferring {} tokens to {}", whole_amount, receiver_address);
        let tx = contract.transfer(receiver_address, decimal_amount);
        let pending_tx = tx.send().await?;
        let _mined_tx = pending_tx.await?;

        // 5. Fetch the balance of the recipient.
        let balance = contract.balance_of(receiver_address).call().await?;
        assert_eq!(balance, decimal_amount);

        // 6. Query the balance of the recipient on the other node.
        let client = &ctx.clients[1];
        let wallet = client.get_wallet();
        let provider = client.get_fullnode_client();
        let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet.clone()));
        let contract = ERC20Contract::new(contract_address, client);

        // 7. Check that the balance of the recipient is the same on the other node.
        let balance = contract.balance_of(receiver_address).call().await?;
        info!("Balance of {} is {}", receiver_address, balance);
        assert_eq!(balance, decimal_amount);


        Ok(())
    }
}

/// Helper function to deploy a contract on a local anvil node. See
/// `examples/contracts/examples/deploy_from_solidity.rs` for detailed explanation on how to deploy
/// a contract.
/// Allocates tokens to the deployer address and returns the ERC20 contract address.
async fn deploy_token_contract(
    client: Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
) -> Result<Address> {
    let source = Path::new(&env!("CARGO_MANIFEST_DIR"))
        .join("contracts/ERC20Example.sol");
    info!("Compiling contract from {:?}", source);
    let compiled = Solc::default().compile_source(source).expect("Could not compile contract");
    let (abi, bytecode, _runtime_bytecode) =
        compiled.find("ERC20Example").expect("could not find contract").into_parts_or_default();

    let factory = ContractFactory::new(abi, bytecode, client.clone());
    let mut contract = factory.deploy(())?;
    contract.tx.set_gas(10000000);
    contract.tx.set_gas_price(0x43423422);
    // contract.tx.gas(100000)
    //         .gas_price(0x43423422);
    let contract = contract.send().await?;

    Ok(contract.address())
}
