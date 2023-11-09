// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[macro_export]
macro_rules! with_tracing {
    ($time_spent_threshold:expr, $future:expr) => {{
        use jsonrpsee::core::{Error as RpcError, RpcResult};
        use jsonrpsee::types::error::{
            CALL_EXECUTION_FAILED_CODE, INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE,
        };
        use jsonrpsee::types::{ErrorObject, ErrorObjectOwned};
        use tracing::{error, info, Instrument, Span};
        //use jsonrpsee::types::error::{CallError};
        use anyhow::anyhow;
        use $crate::error::RpcInterimResult;

        async move {
            let start = std::time::Instant::now();
            let interim_result: RpcInterimResult<_> = $future.await;
            let elapsed = start.elapsed();
            let result: RpcResult<_> = interim_result.map_err(|e: Error| {
                let anyhow_error = anyhow!("{:?}", e);

                let rpc_error: RpcError = e.into();
                /*
                 * 23-11-09 TaiVV upgrade to v 0.20.3 with reth
                 */
                // if !matches!(rpc_error, RpcError::Call(CallError::InvalidParams(_))) {
                //     error!(error=?anyhow_error);
                // }
                // rpc_error
                match rpc_error {
                    RpcError::Call(err) => err,
                    _ => {
                        ErrorObjectOwned::owned(INVALID_PARAMS_CODE, format!("{:?}", e), None::<()>)
                    }
                }
                // if !matches!(rpc_error, RpcError::Call(err)) {
                //     //error!(error=?anyhow_error);
                //     err
                // } else {
                //     ErrorObjectOwned::owned(INVALID_PARAMS_CODE, e.into(), None::<()>)
                // }
            });

            if elapsed > $time_spent_threshold {
                info!(?elapsed, "RPC took longer than threshold to complete.");
            }
            result
        }
        .instrument(Span::current())
        .await
    }};

    ($future:expr) => {{
        with_tracing!(std::time::Duration::from_secs(1), $future)
    }};
}
