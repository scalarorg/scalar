/*
 * 2023-11-03 TaiVV
 * Copy Type struct from external-crates/move/move-vm/types/src/loaded_data/runtime_types.rs
 * Tags: SCALAR_MOVE_TYPES
 */

#[derive(Debug, Copy, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CachedStructIndex(pub usize);

#[derive(Debug, Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<Type>),
    Struct(CachedStructIndex),
    StructInstantiation(CachedStructIndex, Vec<Type>),
    Reference(Box<Type>),
    MutableReference(Box<Type>),
    TyParam(u16),
    U16,
    U32,
    U256,
}
