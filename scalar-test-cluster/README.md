## Consensus Commit
Sau khi make consensus, consensus output được gửi ra ngoài bởi function 
TransactionManager::certificate_ready(..., PendingCertificate)

Các logic itegration được xử lý trong consensus_service bao gồm:
Init a bidirection transaction stream từ Reth tới Scalar

SenderSignedData contain exact 1 transaction???