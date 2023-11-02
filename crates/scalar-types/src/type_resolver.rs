// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*
 * 2023-11-02 TaiVV
 * copy and modify from sui-types/src/object.rs
 * Tags: SCALAR_OBJECT, SCALAR_MOVE_LANGUAGE
 * Thay the deprecated MultiSigLegacy boi MultiSing
 */

use crate::{
    error::{ExecutionError, SuiError},
    object::{MoveObject, ObjectFormatOptions},
};
use crate::{language_storage::TypeTag, value::MoveStructLayout};
// use move_vm_types::loaded_data::runtime_types::Type;

pub trait LayoutResolver {
    fn get_layout(
        &mut self,
        object: &MoveObject,
        format: ObjectFormatOptions,
    ) -> Result<MoveStructLayout, SuiError>;
}
/*
 * 2023-11-02 TaiVV
 * Tam thoi comment out code lien quan toi Move
 * Se add lai vao cac package doc lap xu ly Move logic
 * Tags: SCALAR_RESOLVER, SCALAR_MOVE_LANGUAGE, SCALAR_MOVE_VM
 */

// pub trait TypeTagResolver {
//     fn get_type_tag(&self, type_: &Type) -> Result<TypeTag, ExecutionError>;
// }
