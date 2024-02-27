#[derive(Debug, Clone)]
struct ConsensusClient<Pool: TransactionPool + 'static> {
    socket_addr: SocketAddr,
    tx_pool: Pool,
    tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
}

impl<Pool: TransactionPool + 'static> ConsensusClient<Pool> {
    fn new(
        socket_addr: SocketAddr,
        tx_pool: Pool,
        tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
    ) -> Self {
        Self {
            socket_addr,
            tx_pool,
            tx_commited_transactions,
        }
    }
    async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            socket_addr,
            tx_pool,
            tx_commited_transactions,
        } = self;
        let url = format!("http://{}", socket_addr);
        let mut client = ConsensusApiClient::connect(url).await?;
        //let handler = ScalarConsensus { beacon_consensus_engine_handle };
        info!(
            "Connected to the grpc consensus server at {:?}",
            &socket_addr
        );
        let mut rx_pending_transaction = tx_pool.pending_transactions_listener();
        //let in_stream = tokio_stream::wrappers::ReceiverStream::new(transaction_rx)
        let stream = async_stream::stream! {
            while let Some(tx_hash) = rx_pending_transaction.recv().await {
                /*
                 * 231129 TaiVV
                 * Scalar TODO: convert transaction to ConsensusTransactionIn
                 */
                let tx_bytes = tx_hash.as_slice().to_vec();
                info!("Receive a pending transaction hash, send it into narwhal consensus {:?}", &tx_bytes);
                let consensus_transaction = ExternalTransaction { namespace: NAMESPACE.to_owned(), tx_bytes };
                yield consensus_transaction;
            }
        };
        //pin_mut!(stream);
        let stream = Box::pin(stream);
        let response = client.init_transaction(stream).await?;
        let mut resp_stream = response.into_inner();

        while let Some(received) = resp_stream.next().await {
            match received {
                Ok(CommitedTransactions { transactions }) => {
                    info!("Received {:?} commited transactions.", transactions.len());
                    if let Err(err) = tx_commited_transactions.send(transactions) {
                        error!("{:?}", err);
                    }
                    //let _ = handler.handle_commited_transactions(transactions).await;
                }
                Err(err) => {
                    return Err(Box::new(err));
                }
            }
        }
        Ok(())
    }
}
