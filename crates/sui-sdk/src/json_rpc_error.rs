// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
use jsonrpsee::types::{error::UNKNOWN_ERROR_CODE, ErrorObject, ErrorObjectOwned};
pub use sui_json_rpc::error::{TRANSACTION_EXECUTION_CLIENT_ERROR_CODE, TRANSIENT_ERROR_CODE};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub struct Error {
    pub code: i32,
    pub message: String,
    // TODO: as this SDK is specialized for the Sui JSON RPC implementation, we should define structured representation for the data field if applicable
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "code: '{}', message: '{}'", self.code, self.message)
    }
}

impl Error {
    pub fn is_call_error(&self) -> bool {
        self.code != UNKNOWN_ERROR_CODE
    }

    pub fn is_client_error(&self) -> bool {
        use jsonrpsee::types::error::{
            BATCHES_NOT_SUPPORTED_CODE, INVALID_PARAMS_CODE, INVALID_REQUEST_CODE,
            METHOD_NOT_FOUND_CODE, OVERSIZED_REQUEST_CODE, PARSE_ERROR_CODE,
        };
        matches!(
            self.code,
            PARSE_ERROR_CODE
                | OVERSIZED_REQUEST_CODE
                | INVALID_PARAMS_CODE
                | INVALID_REQUEST_CODE
                | METHOD_NOT_FOUND_CODE
                | BATCHES_NOT_SUPPORTED_CODE
                | TRANSACTION_EXECUTION_CLIENT_ERROR_CODE
        )
    }

    pub fn is_execution_error(&self) -> bool {
        self.code == TRANSACTION_EXECUTION_CLIENT_ERROR_CODE
    }

    pub fn is_transient_error(&self) -> bool {
        self.code == TRANSIENT_ERROR_CODE
    }
}

impl From<jsonrpsee::core::Error> for Error {
    fn from(err: jsonrpsee::core::Error) -> Self {
        // The following code relies on jsonrpsee's From<Error> for ErrorObjectOwned implementation
        // It converts any variant that is not Error::Call into an ErrorObject with UNKNOWN_ERROR_CODE
        match err {
            jsonrpsee::core::Error::Call(err) => Error {
                code: err.code(),
                message: err.message().to_string(),
                data: None,
            },
            err => Error {
                code: UNKNOWN_ERROR_CODE,
                message: format!("{:?}", err),
                data: None,
            }
            // jsonrpsee::core::Error::Transport(_) => todo!(),
            // jsonrpsee::core::Error::InvalidResponse(_) => todo!(),
            // jsonrpsee::core::Error::RestartNeeded(_) => todo!(),
            // jsonrpsee::core::Error::ParseError(_) => todo!(),
            // jsonrpsee::core::Error::InvalidSubscriptionId => todo!(),
            // jsonrpsee::core::Error::InvalidRequestId(_) => todo!(),
            // jsonrpsee::core::Error::UnregisteredNotification(_) => todo!(),
            // jsonrpsee::core::Error::DuplicateRequestId => todo!(),
            // jsonrpsee::core::Error::MethodAlreadyRegistered(_) => todo!(),
            // jsonrpsee::core::Error::MethodNotFound(_) => todo!(),
            // jsonrpsee::core::Error::SubscriptionNameConflict(_) => todo!(),
            // jsonrpsee::core::Error::RequestTimeout => todo!(),
            // jsonrpsee::core::Error::MaxSlotsExceeded => todo!(),
            // jsonrpsee::core::Error::AlreadyStopped => todo!(),
            // jsonrpsee::core::Error::EmptyAllowList(_) => todo!(),
            // jsonrpsee::core::Error::HttpHeaderRejected(_, _) => todo!(),
            // jsonrpsee::core::Error::Custom(_) => todo!(),
            // jsonrpsee::core::Error::HttpNotImplemented => todo!(),
            // jsonrpsee::core::Error::EmptyBatchRequest => todo!(),
        }

        // let error_object_owned: ErrorObjectOwned = err.into();
        // Error {
        //     code: error_object_owned.code(),
        //     message: error_object_owned.message().to_string(),
        //     data: None,
        // }
    }
}
