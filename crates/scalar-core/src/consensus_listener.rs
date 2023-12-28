use std::sync::Arc;

use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    RwLock,
};

use super::consensus_types::NsTransaction;

pub type ConsensusOut = Vec<NsTransaction>;
pub type ConsensusSender = UnboundedSender<ConsensusOut>;
// pub type ConsensusListeners = Vec<ConsensusListener>;

pub struct ConsensusListener {
    senders: Arc<RwLock<Vec<ConsensusSender>>>,
}
impl Default for ConsensusListener {
    fn default() -> Self {
        Self {
            senders: Default::default(),
        }
    }
}
impl ConsensusListener {
    pub async fn add_listener(&self, sender: ConsensusSender) {
        let mut guard = self.senders.write().await;
        guard.push(sender);
    }
    pub async fn start_notifier(
        &self,
        mut rx_commited_transactions: UnboundedReceiver<ConsensusOut>,
    ) {
        let senders = self.senders.clone();
        let _handle = tokio::spawn(async move {
            while let Some(consensus_res) = rx_commited_transactions.recv().await {
                let guard = senders.read().await;
                for sender in guard.iter() {
                    let _res = sender.send(consensus_res.clone());
                }
            }
        });
    }
}
