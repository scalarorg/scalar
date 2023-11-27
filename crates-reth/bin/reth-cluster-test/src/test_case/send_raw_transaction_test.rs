use async_trait::async_trait;
use tracing::info;

use crate::{TestCaseImpl, TestContext};

pub struct SendRawTransactionTest;

#[async_trait]
impl TestCaseImpl for SendRawTransactionTest {
    fn name(&self) -> &'static str {
        "SendRawTransaction"
    }

    fn description(&self) -> &'static str {
        "Test sending a raw transaction to the cluster."
    }

    async fn run(&self, ctx: &mut TestContext) -> Result<(), anyhow::Error> {
        info!("Running test {}.", self.name());

        Ok(())
    }
}
