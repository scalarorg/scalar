// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::authority::authority_per_epoch_store::{
    AuthorityPerEpochStore, ConsensusStats, ConsensusStatsAPI, ExecutionIndicesWithStats,
};
use crate::authority::epoch_start_configuration::EpochStartConfigTrait;
use crate::authority::AuthorityMetrics;
use crate::checkpoints::CheckpointServiceNotify;
use crate::consensus_throughput_calculator::ConsensusThroughputCalculator;
use crate::scoring_decision::update_low_scoring_authorities;
use crate::transaction_manager::TransactionManager;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use fastcrypto::hash::Hash as _Hash;
use fastcrypto::traits::ToFromBytes;
use lru::LruCache;
use mysten_metrics::{monitored_scope, spawn_monitored_task};
use narwhal_config::Committee;
use narwhal_executor::{ExecutionIndices, ExecutionState};
use narwhal_test_utils::latest_protocol_version;
use narwhal_types::{BatchAPI, Certificate, CertificateAPI, ConsensusOutput, HeaderAPI};
use scalar_types::authenticator_state::ActiveJwk;
use scalar_types::base_types::{AuthorityName, EpochId, TransactionDigest};
use scalar_types::executable_transaction::VerifiedExecutableTransaction;
use scalar_types::messages_consensus::{
    ConsensusTransaction, ConsensusTransactionKey, ConsensusTransactionKind,
};
use scalar_types::storage::ObjectStore;
use scalar_types::transaction::{SenderSignedData, VerifiedTransaction};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace_span};

pub struct ConsensusHandler<T, C> {
    /// A store created for each epoch. ConsensusHandler is recreated each epoch, with the
    /// corresponding store. This store is also used to get the current epoch ID.
    epoch_store: Arc<AuthorityPerEpochStore>,
    /// Holds the indices, hash and stats after the last consensus commit
    /// It is used for avoiding replaying already processed transactions,
    /// checking chain consistency, and accumulating per-epoch consensus output stats.
    last_consensus_stats: ExecutionIndicesWithStats,
    checkpoint_service: Arc<C>,
    /// parent_sync_store is needed when determining the next version to assign for shared objects.
    object_store: T,
    /// Reputation scores used by consensus adapter that we update, forwarded from consensus
    low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
    /// The narwhal committee used to do stake computations for deciding set of low scoring authorities
    committee: Committee,
    // TODO: ConsensusHandler doesn't really share metrics with AuthorityState. We could define
    // a new metrics type here if we want to.
    metrics: Arc<AuthorityMetrics>,
    /// Lru cache to quickly discard transactions processed by consensus
    processed_cache: LruCache<SequencedConsensusTransactionKey, ()>,
    transaction_scheduler: AsyncTransactionScheduler,
    /// Using the throughput calculator to record the current consensus throughput
    throughput_calculator: Arc<ConsensusThroughputCalculator>,
}

const PROCESSED_CACHE_CAP: usize = 1024 * 1024;

impl<T, C> ConsensusHandler<T, C> {
    pub fn new(
        epoch_store: Arc<AuthorityPerEpochStore>,
        checkpoint_service: Arc<C>,
        transaction_manager: Arc<TransactionManager>,
        object_store: T,
        low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
        committee: Committee,
        metrics: Arc<AuthorityMetrics>,
        throughput_calculator: Arc<ConsensusThroughputCalculator>,
    ) -> Self {
        // Recover last_consensus_stats so it is consistent across validators.
        let mut last_consensus_stats = epoch_store
            .get_last_consensus_stats()
            .expect("Should be able to read last consensus index");
        // stats is empty at the beginning of epoch.
        if !last_consensus_stats.stats.is_initialized() {
            last_consensus_stats.stats = ConsensusStats::new(committee.size());
        }
        let transaction_scheduler =
            AsyncTransactionScheduler::start(transaction_manager, epoch_store.clone());
        Self {
            epoch_store,
            last_consensus_stats,
            checkpoint_service,
            object_store,
            low_scoring_authorities,
            committee,
            metrics,
            processed_cache: LruCache::new(NonZeroUsize::new(PROCESSED_CACHE_CAP).unwrap()),
            transaction_scheduler,
            throughput_calculator,
        }
    }

    /// Updates the execution indexes based on the provided input. Some is returned when the indexes
    /// are updated which means that the transaction has been seen for first time. None is returned
    /// otherwise.
    fn update_index_and_hash(&mut self, index: ExecutionIndices, v: &[u8]) -> bool {
        update_index_and_hash(&mut self.last_consensus_stats, index, v)
    }
}

fn update_index_and_hash(
    last_consensus_stats: &mut ExecutionIndicesWithStats,
    index: ExecutionIndices,
    v: &[u8],
) -> bool {
    if last_consensus_stats.index >= index {
        return false;
    }

    let previous_hash = last_consensus_stats.hash;
    let mut hasher = DefaultHasher::new();
    previous_hash.hash(&mut hasher);
    v.hash(&mut hasher);
    let hash = hasher.finish();
    // Log hash every 1000th transaction of the subdag
    if index.transaction_index % 1000 == 0 {
        info!(
            "Integrity hash for consensus output at subdag {} transaction {} is {:016x}",
            index.sub_dag_index, index.transaction_index, hash
        );
    }

    last_consensus_stats.index = index;
    last_consensus_stats.hash = hash;
    true
}

#[async_trait]
impl<T: ObjectStore + Send + Sync, C: CheckpointServiceNotify + Send + Sync> ExecutionState
    for ConsensusHandler<T, C>
{
    /// This function will be called by Narwhal, after Narwhal sequenced this certificate.
    #[instrument(level = "debug", skip_all)]
    async fn handle_consensus_output(&mut self, consensus_output: ConsensusOutput) {
        let _scope = monitored_scope("HandleConsensusOutput");

        // This code no longer supports old protocol versions.
        assert!(self
            .epoch_store
            .protocol_config()
            .consensus_order_end_of_epoch_last());

        let last_committed_round = self.last_consensus_stats.index.last_committed_round;

        let round = consensus_output.sub_dag.leader_round();

        assert!(round >= last_committed_round);
        if last_committed_round == round {
            // we can receive the same commit twice after restart
            // It is critical that the writes done by this function are atomic - otherwise we can
            // lose the later parts of a commit if we restart midway through processing it.
            info!(
                "Ignoring consensus output for round {} as it is already committed",
                round
            );
            return;
        }

        /* (serialized, transaction, output_cert) */
        let mut transactions = vec![];
        let timestamp = consensus_output.sub_dag.commit_timestamp();
        let leader_author = consensus_output.sub_dag.leader.header().author();

        let epoch_start = self
            .epoch_store
            .epoch_start_config()
            .epoch_start_timestamp_ms();
        let timestamp = if timestamp < epoch_start {
            error!(
                "Unexpected commit timestamp {timestamp} less then epoch start time {epoch_start}, author {leader_author}, round {round}",
            );
            epoch_start
        } else {
            timestamp
        };

        info!(
            "Received consensus output {:?} at leader round {}, subdag index {}, timestamp {} epoch {}",
            consensus_output.digest(),
            round,
            consensus_output.sub_dag.sub_dag_index,
            timestamp,
            self.epoch_store.epoch(),
        );

        let prologue_transaction = self.consensus_commit_prologue_transaction(round, timestamp);
        transactions.push((
            vec![],
            SequencedConsensusTransactionKind::System(prologue_transaction),
            Arc::new(consensus_output.sub_dag.leader.clone()),
        ));

        // Load all jwks that became active in the previous round, and commit them in this round.
        // We want to delay one round because none of the transactions in the previous round could
        // have been authenticated with the jwks that became active in that round.
        //
        // Because of this delay, jwks that become active in the last round of the epoch will
        // never be committed. That is ok, because in the new epoch, the validators should
        // immediately re-submit these jwks, and they can become active then.
        let new_jwks = self
            .epoch_store
            .get_new_jwks(last_committed_round)
            .expect("Unrecoverable error in consensus handler");

        if !new_jwks.is_empty() {
            debug!("adding AuthenticatorStateUpdate tx: {:?}", new_jwks);
            let authenticator_state_update_transaction =
                self.authenticator_state_update_transaction(round, new_jwks);

            transactions.push((
                vec![],
                SequencedConsensusTransactionKind::System(authenticator_state_update_transaction),
                Arc::new(consensus_output.sub_dag.leader.clone()),
            ));
        }

        update_low_scoring_authorities(
            self.low_scoring_authorities.clone(),
            &self.committee,
            consensus_output.sub_dag.reputation_score.clone(),
            &self.metrics,
            self.epoch_store
                .protocol_config()
                .consensus_bad_nodes_stake_threshold(),
        );

        self.metrics
            .consensus_committed_subdags
            .with_label_values(&[&leader_author.to_string()])
            .inc();

        let mut bytes = 0usize;
        for (cert, batches) in consensus_output
            .sub_dag
            .certificates
            .iter()
            .zip(consensus_output.batches.iter())
        {
            let span = trace_span!("process_consensus_cert");
            let _guard = span.enter();

            assert_eq!(cert.header().payload().len(), batches.len());
            let author = cert.header().author();
            let num_certs = self
                .last_consensus_stats
                .stats
                .inc_narwhal_certificates(author.0 as usize);
            self.metrics
                .consensus_committed_certificates
                .with_label_values(&[&author.to_string()])
                .set(num_certs as i64);
            let output_cert = Arc::new(cert.clone());
            for batch in batches {
                let span = trace_span!("process_consensus_batch");
                let _guard = span.enter();

                assert!(output_cert.header().payload().contains_key(&batch.digest()));
                self.metrics.consensus_handler_processed_batches.inc();
                for serialized_transaction in batch.transactions() {
                    bytes += serialized_transaction.len();

                    let transaction = match bcs::from_bytes::<ConsensusTransaction>(
                        serialized_transaction,
                    ) {
                        Ok(transaction) => transaction,
                        Err(err) => {
                            // This should have been prevented by Narwhal batch verification.
                            panic!(
                                "Unexpected malformed transaction (failed to deserialize): {}\nCertificate={:?} BatchDigest={:?} Transaction={:?}",
                                err, output_cert, batch.digest(), serialized_transaction
                            );
                        }
                    };
                    self.metrics
                        .consensus_handler_processed
                        .with_label_values(&[classify(&transaction)])
                        .inc();
                    if matches!(
                        &transaction.kind,
                        ConsensusTransactionKind::UserTransaction(_)
                    ) {
                        let num_txns = self
                            .last_consensus_stats
                            .stats
                            .inc_user_transactions(author.0 as usize);
                        self.metrics
                            .consensus_committed_user_transactions
                            .with_label_values(&[&author.to_string()])
                            .set(num_txns as i64);
                    }
                    let transaction = SequencedConsensusTransactionKind::External(transaction);
                    transactions.push((
                        serialized_transaction.clone(),
                        transaction,
                        output_cert.clone(),
                    ));
                }
            }
        }
        self.metrics
            .consensus_handler_processed_bytes
            .inc_by(bytes as u64);

        let mut all_transactions = Vec::new();
        {
            // We need a set here as well, since the processed_cache is a LRU cache and can drop
            // entries while we're iterating over the sequenced transactions.
            let mut processed_set = HashSet::new();

            for (seq, (serialized, transaction, output_cert)) in
                transactions.into_iter().enumerate()
            {
                let index = ExecutionIndices {
                    last_committed_round: round,
                    sub_dag_index: consensus_output.sub_dag.sub_dag_index,
                    transaction_index: seq as u64,
                };

                let index_with_stats = if self.update_index_and_hash(index, &serialized) {
                    self.last_consensus_stats.clone()
                } else {
                    debug!(
                        "Ignore consensus transaction at index {:?} as it appear to be already processed",
                        index
                    );
                    continue;
                };

                let certificate_author = AuthorityName::from_bytes(
                    self.committee
                        .authority_safe(&output_cert.header().author())
                        .protocol_key_bytes()
                        .0
                        .as_ref(),
                )
                .unwrap();

                let sequenced_transaction = SequencedConsensusTransaction {
                    certificate: output_cert.clone(),
                    certificate_author,
                    consensus_index: index_with_stats.index,
                    transaction,
                };

                let key = sequenced_transaction.key();
                let in_set = !processed_set.insert(key);
                let in_cache = self
                    .processed_cache
                    .put(sequenced_transaction.key(), ())
                    .is_some();

                if in_set || in_cache {
                    self.metrics.skipped_consensus_txns_cache_hit.inc();
                    continue;
                }

                all_transactions.push(sequenced_transaction);
            }
        }

        let transactions_to_schedule = self
            .epoch_store
            .process_consensus_transactions_and_commit_boundary(
                all_transactions,
                &self.last_consensus_stats,
                &self.checkpoint_service,
                &self.object_store,
                round,
                timestamp,
                &self.metrics.skipped_consensus_txns,
            )
            .await
            .expect("Unrecoverable error in consensus handler");

        // update the calculated throughput
        self.throughput_calculator
            .add_transactions(timestamp, transactions_to_schedule.len() as u64);

        self.transaction_scheduler
            .schedule(transactions_to_schedule)
            .await;
    }

    async fn last_executed_sub_dag_index(&self) -> u64 {
        self.last_consensus_stats.index.sub_dag_index
    }
}

struct AsyncTransactionScheduler {
    sender: tokio::sync::mpsc::Sender<Vec<VerifiedExecutableTransaction>>,
}

impl AsyncTransactionScheduler {
    pub fn start(
        transaction_manager: Arc<TransactionManager>,
        epoch_store: Arc<AuthorityPerEpochStore>,
    ) -> Self {
        let (sender, recv) = tokio::sync::mpsc::channel(16);
        spawn_monitored_task!(Self::run(recv, transaction_manager, epoch_store));
        Self { sender }
    }

    pub async fn schedule(&self, transactions: Vec<VerifiedExecutableTransaction>) {
        self.sender.send(transactions).await.ok();
    }

    pub async fn run(
        mut recv: tokio::sync::mpsc::Receiver<Vec<VerifiedExecutableTransaction>>,
        transaction_manager: Arc<TransactionManager>,
        epoch_store: Arc<AuthorityPerEpochStore>,
    ) {
        while let Some(transactions) = recv.recv().await {
            let _guard = monitored_scope("ConsensusHandler::enqueue");
            transaction_manager
                .enqueue(transactions, &epoch_store)
                .expect("transaction_manager::enqueue should not fail");
        }
    }
}

impl<T, C> ConsensusHandler<T, C> {
    fn consensus_commit_prologue_transaction(
        &self,
        round: u64,
        commit_timestamp_ms: u64,
    ) -> VerifiedExecutableTransaction {
        let transaction = VerifiedTransaction::new_consensus_commit_prologue(
            self.epoch(),
            round,
            commit_timestamp_ms,
        );
        VerifiedExecutableTransaction::new_system(transaction, self.epoch())
    }

    fn authenticator_state_update_transaction(
        &self,
        round: u64,
        mut new_active_jwks: Vec<ActiveJwk>,
    ) -> VerifiedExecutableTransaction {
        new_active_jwks.sort();

        info!("creating authenticator state update transaction");
        assert!(self.epoch_store.authenticator_state_enabled());
        let transaction = VerifiedTransaction::new_authenticator_state_update(
            self.epoch(),
            round,
            new_active_jwks,
            self.epoch_store
                .epoch_start_config()
                .authenticator_obj_initial_shared_version()
                .expect("authenticator state obj must exist"),
        );
        VerifiedExecutableTransaction::new_system(transaction, self.epoch())
    }

    fn epoch(&self) -> EpochId {
        self.epoch_store.epoch()
    }
}

pub(crate) fn classify(transaction: &ConsensusTransaction) -> &'static str {
    match &transaction.kind {
        ConsensusTransactionKind::UserTransaction(certificate) => {
            if certificate.contains_shared_object() {
                "shared_certificate"
            } else {
                "owned_certificate"
            }
        }
        ConsensusTransactionKind::CheckpointSignature(_) => "checkpoint_signature",
        ConsensusTransactionKind::EndOfPublish(_) => "end_of_publish",
        ConsensusTransactionKind::CapabilityNotification(_) => "capability_notification",
        ConsensusTransactionKind::NewJWKFetched(_, _, _) => "new_jwk_fetched",
    }
}

pub struct SequencedConsensusTransaction {
    pub certificate: Arc<narwhal_types::Certificate>,
    pub certificate_author: AuthorityName,
    pub consensus_index: ExecutionIndices,
    pub transaction: SequencedConsensusTransactionKind,
}

pub enum SequencedConsensusTransactionKind {
    External(ConsensusTransaction),
    System(VerifiedExecutableTransaction),
}

#[derive(Serialize, Deserialize, Clone, Hash, PartialEq, Eq, Debug)]
pub enum SequencedConsensusTransactionKey {
    External(ConsensusTransactionKey),
    System(TransactionDigest),
}

impl SequencedConsensusTransactionKind {
    pub fn key(&self) -> SequencedConsensusTransactionKey {
        match self {
            SequencedConsensusTransactionKind::External(ext) => {
                SequencedConsensusTransactionKey::External(ext.key())
            }
            SequencedConsensusTransactionKind::System(txn) => {
                SequencedConsensusTransactionKey::System(*txn.digest())
            }
        }
    }

    pub fn get_tracking_id(&self) -> u64 {
        match self {
            SequencedConsensusTransactionKind::External(ext) => ext.get_tracking_id(),
            SequencedConsensusTransactionKind::System(_txn) => 0,
        }
    }

    pub fn is_executable_transaction(&self) -> bool {
        match self {
            SequencedConsensusTransactionKind::External(ext) => ext.is_user_certificate(),
            SequencedConsensusTransactionKind::System(_) => true,
        }
    }

    pub fn executable_transaction_digest(&self) -> Option<TransactionDigest> {
        match self {
            SequencedConsensusTransactionKind::External(ext) => {
                if let ConsensusTransactionKind::UserTransaction(txn) = &ext.kind {
                    Some(*txn.digest())
                } else {
                    None
                }
            }
            SequencedConsensusTransactionKind::System(txn) => Some(*txn.digest()),
        }
    }

    pub fn is_end_of_publish(&self) -> bool {
        match self {
            SequencedConsensusTransactionKind::External(ext) => {
                matches!(ext.kind, ConsensusTransactionKind::EndOfPublish(..))
            }
            SequencedConsensusTransactionKind::System(_) => false,
        }
    }
}

impl SequencedConsensusTransaction {
    pub fn sender_authority(&self) -> AuthorityName {
        self.certificate_author
    }

    pub fn key(&self) -> SequencedConsensusTransactionKey {
        self.transaction.key()
    }

    pub fn is_end_of_publish(&self) -> bool {
        if let SequencedConsensusTransactionKind::External(ref transaction) = self.transaction {
            matches!(transaction.kind, ConsensusTransactionKind::EndOfPublish(..))
        } else {
            false
        }
    }

    pub fn as_shared_object_txn(&self) -> Option<&SenderSignedData> {
        match &self.transaction {
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransaction(certificate),
                ..
            }) if certificate.contains_shared_object() => Some(certificate.data()),
            SequencedConsensusTransactionKind::System(txn) if txn.contains_shared_object() => {
                Some(txn.data())
            }
            _ => None,
        }
    }
}

pub struct VerifiedSequencedConsensusTransaction(pub SequencedConsensusTransaction);

#[cfg(test)]
impl VerifiedSequencedConsensusTransaction {
    pub fn new_test(transaction: ConsensusTransaction) -> Self {
        Self(SequencedConsensusTransaction::new_test(transaction))
    }
}

impl SequencedConsensusTransaction {
    pub fn new_test(transaction: ConsensusTransaction) -> Self {
        Self {
            transaction: SequencedConsensusTransactionKind::External(transaction),
            certificate: Arc::new(Certificate::default(&latest_protocol_version())),
            certificate_author: AuthorityName::ZERO,
            consensus_index: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::authority_per_epoch_store::{ConsensusStats, ConsensusStatsAPI};
    use crate::authority::test_authority_builder::TestAuthorityBuilder;
    use crate::checkpoints::CheckpointServiceNoop;
    use crate::consensus_adapter::consensus_tests::{test_certificates, test_gas_objects};
    use crate::post_consensus_tx_reorder::PostConsensusTxReorder;
    use narwhal_config::AuthorityIdentifier;
    use narwhal_test_utils::latest_protocol_version;
    use narwhal_types::{
        Batch, Certificate, CommittedSubDag, Header, HeaderV1Builder, ReputationScores,
    };
    use prometheus::Registry;
    use scalar_types::base_types::{random_object_ref, AuthorityName, SuiAddress};
    use scalar_types::committee::Committee;
    use scalar_types::messages_consensus::{
        AuthorityCapabilities, ConsensusTransaction, ConsensusTransactionKind,
    };
    use scalar_types::object::Object;
    use scalar_types::sui_system_state::epoch_start_sui_system_state::EpochStartSystemStateTrait;
    use scalar_types::transaction::{
        CertifiedTransaction, SenderSignedData, TransactionData, TransactionDataAPI,
    };
    use shared_crypto::intent::Intent;
    use std::collections::BTreeSet;
    use sui_protocol_config::{ConsensusTransactionOrdering, SupportedProtocolVersions};

    #[tokio::test]
    pub async fn test_consensus_handler() {
        // GIVEN
        let mut objects = test_gas_objects();
        objects.push(Object::shared_for_testing());

        let latest_protocol_config = &latest_protocol_version();

        let network_config =
            sui_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(objects.clone())
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config)
            .build()
            .await;

        let epoch_store = state.epoch_store_for_testing().clone();
        let new_epoch_start_state = epoch_store.epoch_start_state();
        let committee = new_epoch_start_state.get_narwhal_committee();

        let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));

        let throughput_calculator = ConsensusThroughputCalculator::new(None, metrics.clone());

        let mut consensus_handler = ConsensusHandler::new(
            epoch_store,
            Arc::new(CheckpointServiceNoop {}),
            state.transaction_manager().clone(),
            state.db(),
            Arc::new(ArcSwap::default()),
            committee.clone(),
            metrics,
            Arc::new(throughput_calculator),
        );

        // AND
        // Create test transactions
        let transactions = test_certificates(&state).await;
        let mut certificates = Vec::new();
        let mut batches = Vec::new();

        for transaction in transactions.iter() {
            let transaction_bytes: Vec<u8> = bcs::to_bytes(
                &ConsensusTransaction::new_certificate_message(&state.name, transaction.clone()),
            )
            .unwrap();

            let batch = Batch::new(vec![transaction_bytes], latest_protocol_config);

            batches.push(vec![batch.clone()]);

            // AND make batch as part of a commit
            let header = HeaderV1Builder::default()
                .author(AuthorityIdentifier(0))
                .round(5)
                .epoch(0)
                .parents(BTreeSet::new())
                .with_payload_batch(batch.clone(), 0, 0)
                .build()
                .unwrap();

            let certificate = Certificate::new_unsigned(
                latest_protocol_config,
                &committee,
                Header::V1(header),
                vec![],
            )
            .unwrap();

            certificates.push(certificate);
        }

        // AND create the consensus output
        let consensus_output = ConsensusOutput {
            sub_dag: Arc::new(CommittedSubDag::new(
                certificates.clone(),
                certificates[0].clone(),
                10,
                ReputationScores::default(),
                None,
            )),
            batches,
        };

        // AND processing the consensus output once
        consensus_handler
            .handle_consensus_output(consensus_output.clone())
            .await;

        // AND capturing the consensus stats
        let num_certificates = certificates.len();
        let num_transactions = transactions.len();
        let last_consensus_stats_1 = consensus_handler.last_consensus_stats.clone();
        assert_eq!(
            last_consensus_stats_1.index.transaction_index,
            num_transactions as u64
        );
        assert_eq!(last_consensus_stats_1.index.sub_dag_index, 10_u64);
        assert_eq!(last_consensus_stats_1.index.last_committed_round, 5_u64);
        assert_ne!(last_consensus_stats_1.hash, 0);
        assert_eq!(
            last_consensus_stats_1.stats.get_narwhal_certificates(0),
            num_certificates as u64
        );
        assert_eq!(
            last_consensus_stats_1.stats.get_user_transactions(0),
            num_transactions as u64
        );

        // WHEN processing the same output multiple times
        // THEN the consensus stats do not update
        for _ in 0..2 {
            consensus_handler
                .handle_consensus_output(consensus_output.clone())
                .await;
            let last_consensus_stats_2 = consensus_handler.last_consensus_stats.clone();
            assert_eq!(last_consensus_stats_1, last_consensus_stats_2);
        }
    }

    #[test]
    pub fn test_update_index_and_hash() {
        let index0 = ExecutionIndices {
            sub_dag_index: 0,
            transaction_index: 0,
            last_committed_round: 0,
        };
        let index1 = ExecutionIndices {
            sub_dag_index: 0,
            transaction_index: 1,
            last_committed_round: 0,
        };
        let index2 = ExecutionIndices {
            sub_dag_index: 1,
            transaction_index: 0,
            last_committed_round: 0,
        };

        let mut last_seen = ExecutionIndicesWithStats {
            index: index1,
            hash: 1000,
            stats: ConsensusStats::default(),
        };

        let tx = &[0];
        assert!(!update_index_and_hash(&mut last_seen, index0, tx));
        assert!(!update_index_and_hash(&mut last_seen, index1, tx));
        assert!(update_index_and_hash(&mut last_seen, index2, tx));
    }

    #[test]
    fn test_order_by_gas_price() {
        let mut v = vec![cap_txn(10), user_txn(42), user_txn(100), cap_txn(1)];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "cap(1)".to_string(),
                "user(100)".to_string(),
                "user(42)".to_string(),
            ]
        );

        let mut v = vec![
            user_txn(1200),
            cap_txn(10),
            user_txn(12),
            user_txn(1000),
            user_txn(42),
            user_txn(100),
            cap_txn(1),
            user_txn(1000),
        ];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "cap(1)".to_string(),
                "user(1200)".to_string(),
                "user(1000)".to_string(),
                "user(1000)".to_string(),
                "user(100)".to_string(),
                "user(42)".to_string(),
                "user(12)".to_string(),
            ]
        );

        // If there are no user transactions, the order should be preserved.
        let mut v = vec![
            cap_txn(10),
            eop_txn(12),
            eop_txn(10),
            cap_txn(1),
            eop_txn(11),
        ];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "eop(12)".to_string(),
                "eop(10)".to_string(),
                "cap(1)".to_string(),
                "eop(11)".to_string(),
            ]
        );
    }

    fn extract(v: Vec<VerifiedSequencedConsensusTransaction>) -> Vec<String> {
        v.into_iter().map(extract_one).collect()
    }

    fn extract_one(t: VerifiedSequencedConsensusTransaction) -> String {
        match t.0.transaction {
            SequencedConsensusTransactionKind::External(ext) => match ext.kind {
                ConsensusTransactionKind::EndOfPublish(authority) => {
                    format!("eop({})", authority.0[0])
                }
                ConsensusTransactionKind::CapabilityNotification(cap) => {
                    format!("cap({})", cap.generation)
                }
                ConsensusTransactionKind::UserTransaction(txn) => {
                    format!("user({})", txn.transaction_data().gas_price())
                }
                _ => unreachable!(),
            },
            SequencedConsensusTransactionKind::System(_) => unreachable!(),
        }
    }

    fn eop_txn(a: u8) -> VerifiedSequencedConsensusTransaction {
        let mut authority = AuthorityName::default();
        authority.0[0] = a;
        txn(ConsensusTransactionKind::EndOfPublish(authority))
    }

    fn cap_txn(generation: u64) -> VerifiedSequencedConsensusTransaction {
        txn(ConsensusTransactionKind::CapabilityNotification(
            AuthorityCapabilities {
                authority: Default::default(),
                generation,
                supported_protocol_versions: SupportedProtocolVersions::SYSTEM_DEFAULT,
                available_system_packages: vec![],
            },
        ))
    }

    fn user_txn(gas_price: u64) -> VerifiedSequencedConsensusTransaction {
        let (committee, keypairs) = Committee::new_simple_test_committee();
        let data = SenderSignedData::new(
            TransactionData::new_transfer(
                SuiAddress::default(),
                random_object_ref(),
                SuiAddress::default(),
                random_object_ref(),
                1000 * gas_price,
                gas_price,
            ),
            Intent::sui_transaction(),
            vec![],
        );
        txn(ConsensusTransactionKind::UserTransaction(Box::new(
            CertifiedTransaction::new_from_keypairs_for_testing(data, &keypairs, &committee),
        )))
    }

    fn txn(kind: ConsensusTransactionKind) -> VerifiedSequencedConsensusTransaction {
        VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
            kind,
            tracking_id: Default::default(),
        })
    }
}
