use std::{sync::Arc, time::Duration};

use futures::future::{select, Either, Future};
use mysten_common::sync::notify_read::NotifyRead;
use prometheus::Registry;
use scalar_core::{
    authority::AuthorityState,
    authority_aggregator::{AuthAggMetrics, AuthorityAggregator},
    authority_client::NetworkAuthorityClient,
    quorum_driver::{QuorumDriverHandler, QuorumDriverHandlerBuilder, QuorumDriverMetrics},
    safe_client::SafeClientMetricsBase,
};
use scalar_types::{
    error::SuiResult,
    quorum_driver_types::{ExecuteTransactionRequestType, QuorumDriverError, QuorumDriverResult},
    transaction::VerifiedTransaction,
};
use tokio::time::timeout;
use tonic::{Response, Status};
use tracing::{debug, warn};

use crate::{api::ConsensusMetrics, types::EthTransaction};
type ConsensusServiceResult<T> = Result<Response<T>, Status>;

// How long to wait for local execution (including parents) before a timeout
// is returned to client.
const LOCAL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);

const WAIT_FOR_FINALITY_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct TransactionOrchestrator<A: Clone> {
    quorum_driver_handler: Arc<QuorumDriverHandler<A>>,
    validator_state: Arc<AuthorityState>,
    metrics: Arc<ConsensusMetrics>,
}

impl TransactionOrchestrator<NetworkAuthorityClient> {
    pub fn new(
        validator_state: Arc<AuthorityState>,
        prometheus_registry: &Registry,
        metrics: Arc<ConsensusMetrics>,
    ) -> Self {
        let safe_client_metrics_base = SafeClientMetricsBase::new(prometheus_registry);
        let auth_agg_metrics = AuthAggMetrics::new(prometheus_registry);
        let validators = AuthorityAggregator::new_from_local_system_state(
            &validator_state.db(),
            validator_state.committee_store(),
            safe_client_metrics_base.clone(),
            auth_agg_metrics.clone(),
        )?;
        let notifier = Arc::new(NotifyRead::new());
        let quorum_driver_handler = Arc::new(
            QuorumDriverHandlerBuilder::new(
                validators,
                Arc::new(QuorumDriverMetrics::new(prometheus_registry)),
            )
            .with_notifier(notifier.clone())
            .with_reconfig_observer(Arc::new(reconfig_observer))
            .start(),
        );
        Self {
            validator_state,
            metrics,
        }
    }

    pub async fn handle_eth_transaction(
        &self,
        eth_transaction: EthTransaction,
    ) -> ConsensusServiceResult<()> {
        let response = Response::new(());
        let transaction = eth_transaction.into();
        let transaction = self
            .validator_state
            .verify_transaction(transaction)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;
        let tx_digest = *transaction.digest();
        debug!(?tx_digest, "TO Received transaction execution request.");
        let ticket = self.submit(transaction.clone()).await.map_err(|e| {
            warn!(?tx_digest, "QuorumDriverInternalError: {e:?}");
            Status::invalid_argument(e.to_string())
        })?;

        let wait_for_local_execution = matches!(
            request.request_type,
            ExecuteTransactionRequestType::WaitForLocalExecution
        );

        let Ok(result) = timeout(WAIT_FOR_FINALITY_TIMEOUT, ticket).await else {
            debug!(?tx_digest, "Timeout waiting for transaction finality.");
            self.metrics.wait_for_finality_timeout.inc();
            return Err(Status::deadline_exceeded("TimeoutBeforeFinality"));
        };
        Ok(response)
    }

    /// Submits the transaction to Quorum Driver for execution.
    /// Returns an awaitable Future.
    async fn submit(
        &self,
        transaction: VerifiedTransaction,
    ) -> SuiResult<impl Future<Output = SuiResult<QuorumDriverResult>> + '_> {
        let tx_digest = *transaction.digest();
        let ticket = self.notifier.register_one(&tx_digest);
        if self
            .pending_tx_log
            .write_pending_transaction_maybe(&transaction)
            .await?
        {
            debug!(?tx_digest, "no pending request in flight, submitting.");
            self.quorum_driver()
                .submit_transaction_no_ticket(transaction.clone().into())
                .await?;
        }
        // It's possible that the transaction effects is already stored in DB at this point.
        // So we also subscribe to that. If we hear from `effects_await` first, it means
        // the ticket misses the previous notification, and we want to ask quorum driver
        // to form a certificate for us again, to serve this request.
        let effects_await = self
            .validator_state
            .database
            .notify_read_executed_effects(vec![tx_digest]);
        let qd = self.clone_quorum_driver();
        Ok(async move {
            match select(ticket, effects_await.boxed()).await {
                Either::Left((quorum_driver_response, _)) => Ok(quorum_driver_response),
                Either::Right((_, unfinished_quorum_driver_task)) => {
                    debug!(
                        ?tx_digest,
                        "Effects are available in DB, use quorum driver to get a certificate"
                    );
                    qd.submit_transaction_no_ticket(transaction.into()).await?;
                    Ok(unfinished_quorum_driver_task.await)
                }
            }
        })
    }
    pub fn quorum_driver(&self) -> &Arc<QuorumDriverHandler<A>> {
        &self.quorum_driver_handler
    }

    pub fn clone_quorum_driver(&self) -> Arc<QuorumDriverHandler<A>> {
        self.quorum_driver_handler.clone()
    }

    pub fn clone_authority_aggregator(&self) -> Arc<AuthorityAggregator<A>> {
        self.quorum_driver().authority_aggregator().load_full()
    }

    pub fn subscribe_to_effects_queue(&self) -> Receiver<QuorumDriverEffectsQueueResult> {
        self.quorum_driver_handler.subscribe_to_effects()
    }
}
