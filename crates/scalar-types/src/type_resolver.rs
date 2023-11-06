// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*
 * 2023-11-02 TaiVV
 * copy and modify from sui-types/src/object.rs
 * Tags: SCALAR_OBJECT, SCALAR_MOVE_LANGUAGE
 * Thay the deprecated MultiSigLegacy boi MultiSing
 */

use crate::move_types::runtime_types::Type;
use crate::move_types::{language_storage::TypeTag, value::MoveStructLayout};
use crate::{
    error::{ExecutionError, SuiError},
    object::{MoveObject, ObjectFormatOptions},
};

pub trait LayoutResolver {
    fn get_layout(
        &mut self,
        object: &MoveObject,
        format: ObjectFormatOptions,
    ) -> Result<MoveStructLayout, SuiError>;
}

pub trait TypeTagResolver {
    fn get_type_tag(&self, type_: &Type) -> Result<TypeTag, ExecutionError>;
}
