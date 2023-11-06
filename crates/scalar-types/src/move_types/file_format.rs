// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

//! Binary format for transactions and modules.
//!
//! This module provides a simple Rust abstraction over the binary format. That is the format of
//! modules stored on chain or the format of the code section of a transaction.
//!
//! `file_format_common.rs` provides the constant values for entities in the binary format.
//! (*The binary format is evolving so please come back here in time to check evolutions.*)
//!
//! Overall the binary format is structured in a number of sections:
//! - **Header**: this must start at offset 0 in the binary. It contains a blob that starts every
//! Diem binary, followed by the version of the VM used to compile the code, and last is the
//! number of tables present in this binary.
//! - **Table Specification**: it's a number of tuple of the form
//! `(table type, starting_offset, byte_count)`. The number of entries is specified in the
//! header (last entry in header). There can only be a single entry per table type. The
//! `starting offset` is from the beginning of the binary. Tables must cover the entire size of
//! the binary blob and cannot overlap.
//! - **Table Content**: the serialized form of the specific entries in the table. Those roughly
//! map to the structs defined in this module. Entries in each table must be unique.
//!
//! We have two formats: one for modules here represented by `CompiledModule`, another
//! for transaction scripts which is `CompiledScript`. Building those tables and passing them
//! to the serializer (`serializer.rs`) generates a binary of the form described. Vectors in
//! those structs translate to tables and table specifications.

/*
 * 2023-11-03 TaiVV
 * Copy Type struct from external-crates/move/move-binary-format/src/file_format.rs
 * Tags: SCALAR_MOVE_TYPES, SCALAR_MOVE_FILE_FORMAT
 */

#[cfg(any(test, feature = "fuzzing"))]
use proptest::{collection::vec, prelude::*, strategy::BoxedStrategy};
use serde::{Deserialize, Serialize};
use std::ops::BitOr;
/// Generic index into one of the tables in the binary format.
pub type TableIndex = u16;

macro_rules! define_index {
    {
        name: $name: ident,
        kind: $kind: ident,
        doc: $comment: literal,
    } => {
        #[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(any(test, feature = "fuzzing"), derive(proptest_derive::Arbitrary))]
        #[cfg_attr(any(test, feature = "fuzzing"), proptest(no_params))]
        #[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
        #[cfg_attr(feature = "wasm", derive(Serialize, Deserialize))]
        #[doc=$comment]
        pub struct $name(pub TableIndex);

        /// Returns an instance of the given `Index`.
        impl $name {
            pub fn new(idx: TableIndex) -> Self {
                Self(idx)
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl ModuleIndex for $name {
            const KIND: IndexKind = IndexKind::$kind;

            #[inline]
            fn into_index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

// define_index! {
//     name: ModuleHandleIndex,
//     kind: ModuleHandle,
//     doc: "Index into the `ModuleHandle` table.",
// }
// define_index! {
//     name: StructHandleIndex,
//     kind: StructHandle,
//     doc: "Index into the `StructHandle` table.",
// }
// define_index! {
//     name: FunctionHandleIndex,
//     kind: FunctionHandle,
//     doc: "Index into the `FunctionHandle` table.",
// }
// define_index! {
//     name: FieldHandleIndex,
//     kind: FieldHandle,
//     doc: "Index into the `FieldHandle` table.",
// }
// define_index! {
//     name: StructDefInstantiationIndex,
//     kind: StructDefInstantiation,
//     doc: "Index into the `StructInstantiation` table.",
// }
// define_index! {
//     name: FunctionInstantiationIndex,
//     kind: FunctionInstantiation,
//     doc: "Index into the `FunctionInstantiation` table.",
// }
// define_index! {
//     name: FieldInstantiationIndex,
//     kind: FieldInstantiation,
//     doc: "Index into the `FieldInstantiation` table.",
// }
// define_index! {
//     name: IdentifierIndex,
//     kind: Identifier,
//     doc: "Index into the `Identifier` table.",
// }
// define_index! {
//     name: AddressIdentifierIndex,
//     kind: AddressIdentifier,
//     doc: "Index into the `AddressIdentifier` table.",
// }
// define_index! {
//     name: ConstantPoolIndex,
//     kind: ConstantPool,
//     doc: "Index into the `ConstantPool` table.",
// }
// define_index! {
//     name: SignatureIndex,
//     kind: Signature,
//     doc: "Index into the `Signature` table.",
// }
// define_index! {
//     name: StructDefinitionIndex,
//     kind: StructDefinition,
//     doc: "Index into the `StructDefinition` table.",
// }
// define_index! {
//     name: FunctionDefinitionIndex,
//     kind: FunctionDefinition,
//     doc: "Index into the `FunctionDefinition` table.",
// }

/// Index of a local variable in a function.
///
/// Bytecodes that operate on locals carry indexes to the locals of a function.
pub type LocalIndex = u8;
/// Max number of fields in a `StructDefinition`.
pub type MemberCount = u16;
/// Index into the code stream for a jump. The offset is relative to the beginning of
/// the instruction stream.
pub type CodeOffset = u16;

/// An `Ability` classifies what operations are permitted for a given type
#[repr(u8)]
#[derive(Debug, Clone, Eq, Copy, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(any(test, feature = "fuzzing"), derive(proptest_derive::Arbitrary))]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "wasm", derive(Serialize, Deserialize))]
pub enum Ability {
    /// Allows values of types with this ability to be copied, via CopyLoc or ReadRef
    Copy = 0x1,
    /// Allows values of types with this ability to be dropped, via Pop, WriteRef, StLoc, Eq, Neq,
    /// or if left in a local when Ret is invoked
    /// Technically also needed for numeric operations (Add, BitAnd, Shift, etc), but all
    /// of the types that can be used with those operations have Drop
    Drop = 0x2,
    /// Allows values of types with this ability to exist inside a struct in global storage
    Store = 0x4,
    /// Allows the type to serve as a key for global storage operations: MoveTo, MoveFrom, etc.
    Key = 0x8,
}

impl Ability {
    fn from_u8(u: u8) -> Option<Self> {
        match u {
            0x1 => Some(Ability::Copy),
            0x2 => Some(Ability::Drop),
            0x4 => Some(Ability::Store),
            0x8 => Some(Ability::Key),
            _ => None,
        }
    }

    /// For a struct with ability `a`, each field needs to have the ability `a.requires()`.
    /// Consider a generic type Foo<t1, ..., tn>, for Foo<t1, ..., tn> to have ability `a`, Foo must
    /// have been declared with `a` and each type argument ti must have the ability `a.requires()`
    pub fn requires(self) -> Self {
        match self {
            Self::Copy => Ability::Copy,
            Self::Drop => Ability::Drop,
            Self::Store => Ability::Store,
            Self::Key => Ability::Store,
        }
    }

    /// An inverse of `requires`, where x is in a.required_by() iff x.requires() == a
    pub fn required_by(self) -> AbilitySet {
        match self {
            Self::Copy => AbilitySet::EMPTY | Ability::Copy,
            Self::Drop => AbilitySet::EMPTY | Ability::Drop,
            Self::Store => AbilitySet::EMPTY | Ability::Store | Ability::Key,
            Self::Key => AbilitySet::EMPTY,
        }
    }
}

/// A set of `Ability`s
#[derive(Clone, Eq, Copy, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(arbitrary::Arbitrary))]
pub struct AbilitySet(u8);

impl AbilitySet {
    /// The empty ability set
    pub const EMPTY: Self = Self(0);
    /// Abilities for `Bool`, `U8`, `U64`, `U128`, and `Address`
    pub const PRIMITIVES: AbilitySet =
        Self((Ability::Copy as u8) | (Ability::Drop as u8) | (Ability::Store as u8));
    /// Abilities for `Reference` and `MutableReference`
    pub const REFERENCES: AbilitySet = Self((Ability::Copy as u8) | (Ability::Drop as u8));
    /// Abilities for `Signer`
    pub const SIGNER: AbilitySet = Self(Ability::Drop as u8);
    /// Abilities for `Vector`, note they are predicated on the type argument
    pub const VECTOR: AbilitySet =
        Self((Ability::Copy as u8) | (Ability::Drop as u8) | (Ability::Store as u8));

    /// Ability set containing all abilities
    pub const ALL: Self = Self(
        // Cannot use AbilitySet bitor because it is not const
        (Ability::Copy as u8)
            | (Ability::Drop as u8)
            | (Ability::Store as u8)
            | (Ability::Key as u8),
    );

    pub fn singleton(ability: Ability) -> Self {
        Self(ability as u8)
    }

    pub fn has_ability(self, ability: Ability) -> bool {
        let a = ability as u8;
        (a & self.0) == a
    }

    pub fn has_copy(self) -> bool {
        self.has_ability(Ability::Copy)
    }

    pub fn has_drop(self) -> bool {
        self.has_ability(Ability::Drop)
    }

    pub fn has_store(self) -> bool {
        self.has_ability(Ability::Store)
    }

    pub fn has_key(self) -> bool {
        self.has_ability(Ability::Key)
    }

    pub fn remove(self, ability: Ability) -> Self {
        Self(self.0 & (!(ability as u8)))
    }

    pub fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    fn is_subset_bits(sub: u8, sup: u8) -> bool {
        (sub & sup) == sub
    }

    pub fn is_subset(self, other: Self) -> bool {
        Self::is_subset_bits(self.0, other.0)
    }
    /*
     * 2023-11-03 TaiVV
     * For compilability
     */
    // /// For a polymorphic type, its actual abilities correspond to its declared abilities but
    // /// predicated on its non-phantom type arguments having that ability. For `Key`, instead of needing
    // /// the same ability, the type arguments need `Store`.
    // pub fn polymorphic_abilities<I1, I2>(
    //     declared_abilities: Self,
    //     declared_phantom_parameters: I1,
    //     type_arguments: I2,
    // ) -> PartialVMResult<Self>
    // where
    //     I1: IntoIterator<Item = bool>,
    //     I2: IntoIterator<Item = Self>,
    //     I1::IntoIter: ExactSizeIterator,
    //     I2::IntoIter: ExactSizeIterator,
    // {
    //     let declared_phantom_parameters = declared_phantom_parameters.into_iter();
    //     let type_arguments = type_arguments.into_iter();

    //     if declared_phantom_parameters.len() != type_arguments.len() {
    //         return Err(
    //             PartialVMError::new(StatusCode::VERIFIER_INVARIANT_VIOLATION).with_message(
    //                 "the length of `declared_phantom_parameters` doesn't match the length of `type_arguments`".to_string(),
    //             ),
    //         );
    //     }

    //     // Conceptually this is performing the following operation:
    //     // For any ability 'a' in `declared_abilities`
    //     // 'a' is in the result only if
    //     //   for all (abi_i, is_phantom_i) in `type_arguments` s.t. !is_phantom then a.required() is a subset of abi_i
    //     //
    //     // So to do this efficiently, we can determine the required_by set for each ti
    //     // and intersect them together along with the declared abilities
    //     // This only works because for any ability y, |y.requires()| == 1
    //     let abs = type_arguments
    //         .zip(declared_phantom_parameters)
    //         .filter(|(_, is_phantom)| !is_phantom)
    //         .map(|(ty_arg_abilities, _)| {
    //             ty_arg_abilities
    //                 .into_iter()
    //                 .map(|a| a.required_by())
    //                 .fold(AbilitySet::EMPTY, AbilitySet::union)
    //         })
    //         .fold(declared_abilities, |acc, ty_arg_abilities| {
    //             acc.intersect(ty_arg_abilities)
    //         });
    //     Ok(abs)
    // }
    pub fn from_u8(byte: u8) -> Option<Self> {
        // If there is a bit set in the read `byte`, that bit must be set in the
        // `AbilitySet` containing all `Ability`s
        // This corresponds the byte being a bit set subset of ALL
        // The byte is a subset of ALL if the intersection of the two is the original byte
        if Self::is_subset_bits(byte, Self::ALL.0) {
            Some(Self(byte))
        } else {
            None
        }
    }

    pub fn into_u8(self) -> u8 {
        self.0
    }
}

impl BitOr<Ability> for AbilitySet {
    type Output = Self;
    fn bitor(self, rhs: Ability) -> Self {
        AbilitySet(self.0 | (rhs as u8))
    }
}

impl BitOr<AbilitySet> for AbilitySet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        AbilitySet(self.0 | rhs.0)
    }
}

pub struct AbilitySetIterator {
    set: AbilitySet,
    idx: u8,
}

impl Iterator for AbilitySetIterator {
    type Item = Ability;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx <= 0x8 {
            let next = Ability::from_u8(self.set.0 & self.idx);
            self.idx <<= 1;
            if next.is_some() {
                return next;
            }
        }
        None
    }
}

impl IntoIterator for AbilitySet {
    type Item = Ability;
    type IntoIter = AbilitySetIterator;
    fn into_iter(self) -> Self::IntoIter {
        AbilitySetIterator {
            idx: 0x1,
            set: self,
        }
    }
}

impl std::fmt::Debug for AbilitySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "[")?;
        for ability in *self {
            write!(f, "{:?}, ", ability)?;
        }
        write!(f, "]")
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl Arbitrary for AbilitySet {
    type Strategy = BoxedStrategy<Self>;
    type Parameters = ();

    fn arbitrary_with(_params: Self::Parameters) -> Self::Strategy {
        proptest::bits::u8::masked(AbilitySet::ALL.0)
            .prop_map(|u| AbilitySet::from_u8(u).expect("proptest mask failed for AbilitySet"))
            .boxed()
    }
}
