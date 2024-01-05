//! The mode the auto seal miner is operating in.

use futures_util::{stream::Fuse, StreamExt};
use reth_primitives::{revm_primitives::FixedBytes, Address, TxHash};
use reth_transaction_pool::{TransactionPool, ValidPoolTransaction};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::mpsc::{Receiver, UnboundedReceiver},
    time::Interval,
};
use tokio_stream::{
    wrappers::{ReceiverStream, UnboundedReceiverStream},
    Stream,
};
use tracing::{info, warn};

use crate::proto::ExternalTransaction;

/// Mode of operations for the `Miner`
#[derive(Debug)]
pub enum ScalarMiningMode<Pool: TransactionPool> {
    /// A miner that does nothing
    None,
    /// A miner that listens for new transactions that are ready.
    ///
    /// Either one transaction will be mined per block, or any number of transactions will be
    /// allowed
    Auto(ReadyTransactionMiner),
    /// A miner that listens commited transactions from external Narwhal consensus
    Narwhal(NarwhalTransactionMiner<Pool>),
    /// A miner that constructs a new block every `interval` tick
    FixedBlockTime(FixedBlockTimeMiner),
}

// === impl MiningMode ===

impl<Pool: TransactionPool> ScalarMiningMode<Pool> {
    /// Creates a new instant mining mode that listens for new transactions and tries to build
    /// non-empty blocks as soon as transactions arrive.
    pub fn instant(max_transactions: usize, listener: Receiver<TxHash>) -> Self {
        ScalarMiningMode::Auto(ReadyTransactionMiner {
            max_transactions,
            has_pending_txs: None,
            rx: ReceiverStream::new(listener).fuse(),
        })
    }

    /// Creates a new narwhal mining mode that listens for commited transactions from Narwhal consensus
    /// and tries to build non-empty blocks as soon as transactions arrive.
    pub fn narwhal(
        rx_pending_transaction: Receiver<TxHash>,
        rx_committed_transactions: UnboundedReceiver<Vec<ExternalTransaction>>,
    ) -> Self {
        ScalarMiningMode::Narwhal(NarwhalTransactionMiner {
            max_transactions: usize::MAX,
            // has_pending_txs: None,
            rx_pending_transaction: ReceiverStream::new(rx_pending_transaction).fuse(),
            rx_committed_transactions: UnboundedReceiverStream::new(rx_committed_transactions).fuse(),
            pending_transaction_queue: Default::default(),
            pending_committed_transactions: Default::default(),
            expected_nonces: Default::default(),
            deferred_commited_transactions: Default::default(),
            committed_trans: 0,
            sealed_trans: 0
        })
    }

    /// Creates a new interval miner that builds a block ever `duration`.
    pub fn interval(duration: Duration) -> Self {
        ScalarMiningMode::FixedBlockTime(FixedBlockTimeMiner::new(duration))
    }

    /// polls the Pool and returns those transactions that should be put in a block, if any.
    pub(crate) fn poll(
        &mut self,
        pool: &Pool,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>>
    where
        Pool: TransactionPool,
    {
        match self {
            ScalarMiningMode::None => Poll::Pending,
            ScalarMiningMode::Auto(miner) => miner.poll(pool, cx),
            ScalarMiningMode::Narwhal(miner) => miner.poll(pool, cx),
            ScalarMiningMode::FixedBlockTime(miner) => miner.poll(pool, cx),
        }
    }
}

/// A miner that's supposed to create a new block every `interval`, mining all transactions that are
/// ready at that time.
///
/// The default blocktime is set to 6 seconds
#[derive(Debug)]
pub struct FixedBlockTimeMiner {
    /// The interval this fixed block time miner operates with
    interval: Interval,
}

// === impl FixedBlockTimeMiner ===

impl FixedBlockTimeMiner {
    /// Creates a new instance with an interval of `duration`
    pub(crate) fn new(duration: Duration) -> Self {
        let start = tokio::time::Instant::now() + duration;
        Self {
            interval: tokio::time::interval_at(start, duration),
        }
    }

    fn poll<Pool>(
        &mut self,
        pool: &Pool,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>>
    where
        Pool: TransactionPool,
    {
        if self.interval.poll_tick(cx).is_ready() {
            // drain the pool
            return Poll::Ready(pool.best_transactions().collect());
        }
        Poll::Pending
    }
}

impl Default for FixedBlockTimeMiner {
    fn default() -> Self {
        Self::new(Duration::from_secs(6))
    }
}

/// A miner that Listens for new ready transactions
pub struct ReadyTransactionMiner {
    /// how many transactions to mine per block
    max_transactions: usize,
    /// stores whether there are pending transactions (if known)
    has_pending_txs: Option<bool>,
    /// Receives hashes of transactions that are ready
    rx: Fuse<ReceiverStream<TxHash>>,
}

// === impl ReadyTransactionMiner ===

impl ReadyTransactionMiner {
    fn poll<Pool>(
        &mut self,
        pool: &Pool,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>>
    where
        Pool: TransactionPool,
    {
        // drain the notification stream
        while let Poll::Ready(Some(_hash)) = Pin::new(&mut self.rx).poll_next(cx) {
            self.has_pending_txs = Some(true);
        }

        if self.has_pending_txs == Some(false) {
            return Poll::Pending;
        }

        let transactions = pool
            .best_transactions()
            .take(self.max_transactions)
            .collect::<Vec<_>>();

        // there are pending transactions if we didn't drain the pool
        self.has_pending_txs = Some(transactions.len() >= self.max_transactions);

        if transactions.is_empty() {
            return Poll::Pending;
        }

        Poll::Ready(transactions)
    }
}

impl fmt::Debug for ReadyTransactionMiner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadyTransactionMiner")
            .field("max_transactions", &self.max_transactions)
            .finish_non_exhaustive()
    }
}

/// A miner that Listens for new ready transactions
pub struct NarwhalTransactionMiner<Pool: TransactionPool> {
    /// how many transactions to mine per block
    max_transactions: usize,
    // /// stores whether there are pending transactions (if known)
    // has_pending_txs: Option<bool>,
    /// Receives hashes of transactions that are ready
    rx_pending_transaction: Fuse<ReceiverStream<TxHash>>,
    rx_committed_transactions: Fuse<UnboundedReceiverStream<Vec<ExternalTransaction>>>,
    // Todo: store and load this map from the db
    expected_nonces: Mutex<HashMap<Address, u64>>,
    // Các transaction trước đó chưa được đưa vào block
    deferred_commited_transactions: Vec<ExternalTransaction>,
    pending_transaction_queue:
        VecDeque<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
    pending_committed_transactions: VecDeque<Vec<ExternalTransaction>>,
    committed_trans: u64,
    sealed_trans: u64
}

// === impl NarwhalTransactionMiner ===

impl<Pool: TransactionPool> NarwhalTransactionMiner<Pool> {
    fn poll(
        &mut self,
        pool: &Pool,
        cx: &mut Context<'_>,
    ) -> Poll<Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>>
    where
        Pool: TransactionPool,
    {
        info!("Poll next commited transactions");
        while let Poll::Ready(Some(comitted_transactions)) =
            Pin::new(&mut self.rx_committed_transactions).poll_next(cx)
        {
            self.committed_trans += comitted_transactions.len() as u64;
            info!("Push pending commited transactions {:?}. Total committed_trans {}", comitted_transactions.len(), &self.committed_trans);
            if comitted_transactions.len() > 0 {                
                self.pending_committed_transactions
                    .push_back(comitted_transactions);
            }
        }
        info!("drain the notification stream");
        while let Poll::Ready(Some(tx_hash)) =
            Pin::new(&mut self.rx_pending_transaction).poll_next(cx)
        {
            //let guard = self.pending_transaction_queue.write().await;
            if let Some(transaction) = pool.get(&tx_hash) {
                let nonce = transaction.nonce();
                self.pending_transaction_queue.push_back(transaction);
                info!(
                    "Push pending transaction {:?}. New pending queue size {:?}",
                    nonce,
                    self.pending_transaction_queue.len()
                );
            }
        }

        if self.pending_committed_transactions.is_empty()
            && self.deferred_commited_transactions.is_empty()
        {
            info!("Both pending commited_transactions and deferred_commited_transactions are empty");
            return Poll::Pending;
        }
        info!("Process next block");
        let transactions = self.process_next_block(pool);
        if transactions.is_empty() {
            return Poll::Pending;
        }
        return Poll::Ready(transactions);
    }
}

impl<Pool: TransactionPool> fmt::Debug for NarwhalTransactionMiner<Pool> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NarwhalTransactionMiner")
            .field("max_transactions", &self.max_transactions)
            .finish_non_exhaustive()
    }
}
/*
 * TaiVV 2023 Jan 04
 * Mỗi commited nhận được từ consensus sẽ kết hợp với các commited trước đó để chuẩn bị tạo 1 block.
 * Loop through all pending transactions
 * Các transaction trong pool được đưa vào block tiếp theo nếu thoả mãn 2 điều kiện
 * 1. Số nonce trùng với expected nonce
 * 2. Transaction hash được committed
 * Todo: Thuật toán này phục vụ nhu cầu demo,
 * Sẽ phải update cho các version tiếp theo
 */

impl<Pool: TransactionPool> NarwhalTransactionMiner<Pool> {
    fn process_next_block(
        &mut self,
        pool: &Pool,
    ) -> Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>> {
        let mut transactions = vec![];
        //1. Join defferred commited transaction with current commited transactions to form unique set
        let mut committed_txs_set: HashSet<Vec<u8>> = self
            .deferred_commited_transactions
            .iter()
            .map(|tran| tran.tx_bytes.clone())
            .collect();
        let current_commited_transactions = self
            .pending_committed_transactions
            .pop_front()
            .unwrap_or_else(|| vec![]);
        current_commited_transactions.iter().for_each(|tran| {
            committed_txs_set.insert(tran.tx_bytes.clone());
        });
        let mut new_pending_queue = VecDeque::default();
        let mut sealed_txs: HashSet<Vec<u8>> = Default::default();
        //2. Loop throught pending transactions. Push transaction to block if its' nonce is expected
        while let Some(pending_tx) = self.pending_transaction_queue.pop_front() {
            if committed_txs_set.is_empty() {
                break;
            }
            let key = pending_tx.hash().as_slice().to_vec();
            let nonce = pending_tx.nonce();
            let sender = pending_tx.sender();
            // Nếu transaction chưa được committed, đưa vào hàng đợi tạm để xử lý transaction tiếp theo
            if !committed_txs_set.contains(&key) {
                // Các message gửi tới consensus layer trước nhưng chưa nhận lại trong commit
                warn!("Pending transaction {} from sender {} not found in committed cache. Stop to seal new block", &nonce, &sender);
                new_pending_queue.push_back(pending_tx);
                break;
            } else if let Ok(mut expected_nonces) = self.expected_nonces.lock() {
                let expected_nonce = expected_nonces.get(&sender)
                    .map(|nonce| nonce.clone())
                    .unwrap_or(0_u64);
                if nonce == expected_nonce {
                    sealed_txs.insert(key);
                    transactions.push(pending_tx);
                    (*expected_nonces).insert(sender, nonce + 1);
                } else {
                    warn!("Pending transaction {} from sender {} not matches with expected nonce {}", &nonce, &sender, &expected_nonce);
                    new_pending_queue.push_back(pending_tx);
                }
            } else {
                // Không thể aquired lock
                warn!("Unable acquire lock to current nonces. Put transaction {} from ther sender {} back to the queue for next processing.", &nonce, &sender);
                new_pending_queue.push_back(pending_tx);
            }
        }
        // Nếu có transactions chưa được xử lý thì đưa ngược trở lại vào queue với đúng thứ tự cũ
        while let Some(tx) = new_pending_queue.pop_back() {
            self.pending_transaction_queue.push_front(tx);
        }
        self.sealed_trans += sealed_txs.len() as u64;
        //3. Remove sealed transactions
        self.deferred_commited_transactions
            .retain(|key| !sealed_txs.contains(&key.tx_bytes));
        current_commited_transactions.into_iter().for_each(|tx| {
            if !sealed_txs.contains(&tx.tx_bytes) {
                self.deferred_commited_transactions.push(tx);
            }
        });
        info!("Expected nonces {:?}. Pending transaction queue size {}. Deffered commited transaction size {}. Block size {}, Total sealed trans {}", 
            &self.expected_nonces,
            self.pending_transaction_queue.len(), 
            self.deferred_commited_transactions.len(),
            transactions.len(),
            &self.sealed_trans
        );
        transactions
    }
}
