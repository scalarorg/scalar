use once_cell::sync::OnceCell;
use scalar_types::digests::ChainIdentifier;
pub static CHAIN_IDENTIFIER: OnceCell<ChainIdentifier> = OnceCell::new();
