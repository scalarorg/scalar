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
            rx_committed_transactions: UnboundedReceiverStream::new(rx_committed_transactions)
                .fuse(),
            pending_transaction_queue: Default::default(),
            pending_committed_transactions: Default::default(),
            expected_nonces: Default::default(),
            deferred_commited_transactions: Default::default(),
            map_pending_trans: Default::default(),
            committed_trans: 0,
            sealed_trans: 0,
            total_pending: 0,
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
    expected_nonces: HashMap<Address, u64>,
    // Các transaction trước đó chưa được đưa vào block
    deferred_commited_transactions: Vec<Arc<ExternalTransaction>>,
    pending_transaction_queue:
        VecDeque<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
    map_pending_trans:
        HashMap<FixedBytes<32>, Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
    pending_committed_transactions: VecDeque<Vec<ExternalTransaction>>,
    committed_trans: u64,
    sealed_trans: u64,
    total_pending: u64,
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
        loop {
            let mut hasIncoming = false;
            if let Poll::Ready(Some(comitted_transactions)) =
                Pin::new(&mut self.rx_committed_transactions).poll_next(cx)
            {
                self.committed_trans += comitted_transactions.len() as u64;
                info!(
                    "Push pending commited transactions {:?}. Total committed_trans {}",
                    comitted_transactions.len(),
                    &self.committed_trans
                );
                if comitted_transactions.len() > 0 {
                    self.pending_committed_transactions
                        .push_back(comitted_transactions);
                }
                hasIncoming = true;
            }
            info!("drain the notification stream");
            if let Poll::Ready(Some(tx_hash)) =
                Pin::new(&mut self.rx_pending_transaction).poll_next(cx)
            {
                //let guard = self.pending_transaction_queue.write().await;
                if let Some(transaction) = pool.get(&tx_hash) {
                    let nonce = transaction.nonce();
                    let sender = transaction.sender();
                    self.map_pending_trans
                        .insert(transaction.hash().clone(), transaction.clone());
                    self.total_pending += 1;
                    //self.pending_transaction_queue.push_back(transaction);
                    info!(
                        "Add new transaction {:?} from sender {} into queue. Pending queue size {:?}. Total pending trans {:?}",
                        nonce,
                        &sender,
                        self.map_pending_trans.len(),
                        &self.total_pending,
                    );
                }
                hasIncoming = true;
            }
            if !hasIncoming {
                break;
            }
        }
        if self.pending_committed_transactions.is_empty()
            && self.deferred_commited_transactions.is_empty()
        {
            info!(
                "Both pending commited_transactions and deferred_commited_transactions are empty"
            );
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
        // let mut committed_txs_set: HashSet<Vec<u8>> = self
        //     .deferred_commited_transactions
        //     .iter()
        //     .map(|tran| tran.tx_bytes.clone())
        //     .collect();
        let current_commited_transactions = self
            .pending_committed_transactions
            .pop_front()
            .unwrap_or_else(|| vec![]);
        // current_commited_transactions.iter().for_each(|tran| {
        //     committed_txs_set.insert(tran.tx_bytes.clone());
        // });
        for transaction in current_commited_transactions.into_iter() {
            self.deferred_commited_transactions
                .push(Arc::new(transaction));
        }

        //let mut new_pending_queue = VecDeque::default();
        let mut sealed_txs: HashSet<Vec<u8>> = Default::default();
        /*
         * 2. Loop throught commited transaction, each iteration find transaction with expected nonce.
         */
        let mut iter_trans = vec![];
        loop {
            let mut deferred_commited_transactions = vec![];
            info!(
                "Start new loop with deffered commited transaction size {}",
                self.deferred_commited_transactions.len()
            );
            for commited_tran in self.deferred_commited_transactions.iter() {
                let key = commited_tran.tx_bytes.clone();
                let tx_hash = FixedBytes::from_slice(commited_tran.tx_bytes.as_slice());
                if let Some(pending_tx) = pool.get(&tx_hash) {
                    let nonce = pending_tx.nonce();
                    let sender = pending_tx.sender();
                    let expected_nonce = self
                        .expected_nonces
                        .get(&sender)
                        .map(|nonce| nonce.clone())
                        .unwrap_or(0_u64);
                    if nonce == expected_nonce {
                        info!(
                            "Push transaction {} from sender {} into new block",
                            nonce, &sender
                        );
                        sealed_txs.insert(key);
                        iter_trans.push(pending_tx.clone());
                        self.expected_nonces.insert(sender, nonce + 1);
                        //self.map_pending_trans.remove(&tx_hash);
                    } else {
                        info!("Transaction {} from sender {} not matches with expected nonce {}. Put it back to the deffered queue", nonce, &sender, expected_nonce);
                        deferred_commited_transactions.push(commited_tran.clone());
                    }
                } else {
                    // Missing transaction in pending queue.
                    // Khong xay ra do consensus chay sau layer execution 1 khoang thoi gian
                    info!("Missing pending transaction with commited {:?} Put it back into the deffered queue.", commited_tran);
                    deferred_commited_transactions.push(commited_tran.clone());
                }
            }
            self.deferred_commited_transactions = deferred_commited_transactions;
            if iter_trans.len() > 0 {
                transactions.append(&mut iter_trans);
            } else {
                // Không tìm được transaction nào phù hợp với expected nonce
                break;
            }
        }

        self.sealed_trans += sealed_txs.len() as u64;

        info!("Expected nonces {:?}. Pool size {:?}. Deffered commited transaction size {}. Block size {}, Total sealed trans {}", 
            &self.expected_nonces,
            // self.map_pending_trans.len(), 
            pool.pool_size(),
            self.deferred_commited_transactions.len(),
            transactions.len(),
            &self.sealed_trans
        );
        transactions
    }
}
