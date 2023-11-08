// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/*
 * 2023-11-02 TaiVV
 * copy and modify from sui-types/src/storage/mod.rs
 * Tags: SCALAR_STORAGE
 */

mod object_store_trait;
mod read_store;
mod shared_in_memory_store;
mod write_store;

use crate::base_types::{TransactionDigest, VersionNumber};
use crate::committee::EpochId;
use crate::error::SuiError;
use crate::execution::{DynamicallyLoadedObjectMetadata, ExecutionResults};
use crate::move_package::MovePackage;
use crate::move_types::language_storage::ModuleId;
use crate::transaction::{SenderSignedData, TransactionDataAPI};
use crate::{
    base_types::{ObjectID, ObjectRef, SequenceNumber},
    error::SuiResult,
    object::Object,
};
use itertools::Itertools;
use move_binary_format::CompiledModule;
pub use object_store_trait::ObjectStore;
pub use read_store::ReadStore;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
pub use shared_in_memory_store::SharedInMemoryStore;
pub use shared_in_memory_store::SingleCheckpointSharedInMemoryStore;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
pub use write_store::WriteStore;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum WriteKind {
    /// The object was in storage already but has been modified
    Mutate,
    /// The object was created in this transaction
    Create,
    /// The object was previously wrapped in another object, but has been restored to storage
    Unwrap,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum DeleteKind {
    /// An object is provided in the call input, and gets deleted.
    Normal,
    /// An object is not provided in the call input, but gets unwrapped
    /// from another object, and then gets deleted.
    UnwrapThenDelete,
    /// An object is provided in the call input, and gets wrapped into another object.
    Wrap,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum MarkerValue {
    /// An object was received at the given version in the transaction and is no longer able
    /// to be received at that version in subequent transactions.
    Received,
    /// An owned object was deleted (or wrapped) at the given version, and is no longer able to be
    /// accessed or used in subsequent transactions.
    OwnedDeleted,
    /// A shared object was deleted by the transaction and is no longer able to be accessed or
    /// used in subsequent transactions.
    SharedDeleted(TransactionDigest),
}

/// DeleteKind together with the old sequence number prior to the deletion, if available.
/// For normal deletion and wrap, we always will consult the object store to obtain the old sequence number.
/// For UnwrapThenDelete however, in the old protocol where simplified_unwrap_then_delete is false,
/// we will consult the object store to obtain the old sequence number, which latter will be put in
/// modified_at_versions; in the new protocol where simplified_unwrap_then_delete is true,
/// we will not consult the object store, and hence won't have the old sequence number.
#[derive(Debug)]
pub enum DeleteKindWithOldVersion {
    Normal(SequenceNumber),
    // This variant will be deprecated when we turn on simplified_unwrap_then_delete.
    UnwrapThenDeleteDEPRECATED(SequenceNumber),
    UnwrapThenDelete,
    Wrap(SequenceNumber),
}

impl DeleteKindWithOldVersion {
    pub fn old_version(&self) -> Option<SequenceNumber> {
        match self {
            DeleteKindWithOldVersion::Normal(version)
            | DeleteKindWithOldVersion::UnwrapThenDeleteDEPRECATED(version)
            | DeleteKindWithOldVersion::Wrap(version) => Some(*version),
            DeleteKindWithOldVersion::UnwrapThenDelete => None,
        }
    }

    pub fn to_delete_kind(&self) -> DeleteKind {
        match self {
            DeleteKindWithOldVersion::Normal(_) => DeleteKind::Normal,
            DeleteKindWithOldVersion::UnwrapThenDeleteDEPRECATED(_)
            | DeleteKindWithOldVersion::UnwrapThenDelete => DeleteKind::UnwrapThenDelete,
            DeleteKindWithOldVersion::Wrap(_) => DeleteKind::Wrap,
        }
    }
}

#[derive(Debug)]
pub enum ObjectChange {
    Write(Object, WriteKind),
    // DeleteKind together with the old sequence number prior to the deletion, if available.
    Delete(DeleteKindWithOldVersion),
}

pub trait StorageView: Storage + ParentSync + ChildObjectResolver {}
impl<T: Storage + ParentSync + ChildObjectResolver> StorageView for T {}

/// An abstraction of the (possibly distributed) store for objects. This
/// API only allows for the retrieval of objects, not any state changes
pub trait ChildObjectResolver {
    /// `child` must have an `ObjectOwner` ownership equal to `owner`.
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<Object>>;

    /// `receiving_object_id` must have an `AddressOwner` ownership equal to `owner`.
    /// `get_object_received_at_version` must be the exact version at which the object will be received,
    /// and it cannot have been previously received at that version. NB: An object not existing at
    /// that version, and not having valid access to the object will be treated exactly the same
    /// and `Ok(None)` must be returned.
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> SuiResult<Option<Object>>;
}

/// An abstraction of the (possibly distributed) store for objects, and (soon) events and transactions
pub trait Storage {
    fn reset(&mut self);

    fn read_object(&self, id: &ObjectID) -> Option<&Object>;

    fn record_execution_results(&mut self, results: ExecutionResults);

    fn save_loaded_runtime_objects(
        &mut self,
        loaded_runtime_objects: BTreeMap<ObjectID, DynamicallyLoadedObjectMetadata>,
    );
}

pub type PackageFetchResults<Package> = Result<Vec<Package>, Vec<ObjectID>>;

pub trait BackingPackageStore {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<Object>>;
    /*
     * 2023-11-02 TaiVV
     * Move code lien quan toi Move ra package rieng (xu ly sau)
     * 2023-11-03: uncomment this function
     * Tags: SCALAR_MOVE_LANGUAGE
     */
    fn get_package(&self, package_id: &ObjectID) -> SuiResult<Option<MovePackage>> {
        self.get_package_object(package_id)
            .map(|opt_obj| opt_obj.and_then(|obj| obj.data.try_into_package()))
    }
}

impl<S: BackingPackageStore> BackingPackageStore for std::sync::Arc<S> {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<Object>> {
        BackingPackageStore::get_package_object(self.as_ref(), package_id)
    }
}

impl<S: ?Sized + BackingPackageStore> BackingPackageStore for &S {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<Object>> {
        BackingPackageStore::get_package_object(*self, package_id)
    }
}

impl<S: ?Sized + BackingPackageStore> BackingPackageStore for &mut S {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<Object>> {
        BackingPackageStore::get_package_object(*self, package_id)
    }
}

/// Returns Ok(<object for each package id in `package_ids`>) if all package IDs in
/// `package_id` were found. If any package in `package_ids` was not found it returns a list
/// of any package ids that are unable to be found>).
pub fn get_package_objects<'a>(
    store: &impl BackingPackageStore,
    package_ids: impl IntoIterator<Item = &'a ObjectID>,
) -> SuiResult<PackageFetchResults<Object>> {
    let package_objects: Vec<Result<Object, ObjectID>> = package_ids
        .into_iter()
        .map(|id| match store.get_package_object(id) {
            Ok(None) => Ok(Err(*id)),
            Ok(Some(o)) => Ok(Ok(o)),
            Err(x) => Err(x),
        })
        .collect::<SuiResult<_>>()?;

    let (fetched, failed_to_fetch): (Vec<_>, Vec<_>) =
        package_objects.into_iter().partition_result();
    if !failed_to_fetch.is_empty() {
        Ok(Err(failed_to_fetch))
    } else {
        Ok(Ok(fetched))
    }
}

/*
 * 2023-11-02 TaiVV
 * Move code lien quan toi Move ra package rieng (xu ly sau)
 * Tags: SCALAR_MOVE_LANGUAGE
 */

// pub fn get_packages<'a>(
//     store: &impl BackingPackageStore,
//     package_ids: impl IntoIterator<Item = &'a ObjectID>,
// ) -> SuiResult<PackageFetchResults<MovePackage>> {
//     let objects = get_package_objects(store, package_ids)?;
//     Ok(objects.and_then(|objects| {
//         let (packages, failed): (Vec<_>, Vec<_>) = objects
//             .into_iter()
//             .map(|obj| {
//                 let obj_id = obj.id();
//                 obj.data.try_into_package().ok_or(obj_id)
//             })
//             .partition_result();
//         if !failed.is_empty() {
//             Err(failed)
//         } else {
//             Ok(packages)
//         }
//     }))
// }

pub fn get_module<S: BackingPackageStore>(
    store: S,
    module_id: &ModuleId,
) -> Result<Option<Vec<u8>>, SuiError> {
    Ok(store
        .get_package(&ObjectID::from(*module_id.address()))?
        .and_then(|package| {
            package
                .serialized_module_map()
                .get(module_id.name().as_str())
                .cloned()
        }))
}

/*
 * 2023-11-02 TaiVV
 * Move code lien quan toi Move ra package rieng (xu ly sau)
 * Doc du lieu (duoi dang binary từ Store và deserialize sang CompiledModule của Move language)
 * Tags: SCALAR_MOVE_LANGUAGE
 */

pub fn get_module_by_id<S: BackingPackageStore>(
    store: S,
    id: &ModuleId,
) -> anyhow::Result<Option<CompiledModule>, SuiError> {
    Ok(get_module(store, id)?
        .map(|bytes| CompiledModule::deserialize_with_defaults(&bytes).unwrap()))
}

pub trait ParentSync {
    /// This function is only called by older protocol versions.
    /// It creates an explicit dependency to tombstones, which is not desired.
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> SuiResult<Option<ObjectRef>>;
}

impl<S: ParentSync> ParentSync for std::sync::Arc<S> {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> SuiResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(self.as_ref(), object_id)
    }
}

impl<S: ParentSync> ParentSync for &S {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> SuiResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(*self, object_id)
    }
}

impl<S: ParentSync> ParentSync for &mut S {
    fn get_latest_parent_entry_ref_deprecated(
        &self,
        object_id: ObjectID,
    ) -> SuiResult<Option<ObjectRef>> {
        ParentSync::get_latest_parent_entry_ref_deprecated(*self, object_id)
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for std::sync::Arc<S> {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::read_child_object(
            self.as_ref(),
            parent,
            child,
            child_version_upper_bound,
        )
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            self.as_ref(),
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for &S {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::read_child_object(*self, parent, child, child_version_upper_bound)
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            *self,
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

impl<S: ChildObjectResolver> ChildObjectResolver for &mut S {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::read_child_object(*self, parent, child, child_version_upper_bound)
    }
    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        epoch_id: EpochId,
    ) -> SuiResult<Option<Object>> {
        ChildObjectResolver::get_object_received_at_version(
            *self,
            owner,
            receiving_object_id,
            receive_object_at_version,
            epoch_id,
        )
    }
}

// The primary key type for object storage.
#[serde_as]
#[derive(Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub struct ObjectKey(pub ObjectID, pub VersionNumber);

impl ObjectKey {
    pub const ZERO: ObjectKey = ObjectKey(ObjectID::ZERO, VersionNumber::MIN);

    pub fn max_for_id(id: &ObjectID) -> Self {
        Self(*id, VersionNumber::MAX)
    }

    pub fn min_for_id(id: &ObjectID) -> Self {
        Self(*id, VersionNumber::MIN)
    }
}

impl From<ObjectRef> for ObjectKey {
    fn from(object_ref: ObjectRef) -> Self {
        ObjectKey::from(&object_ref)
    }
}

impl From<&ObjectRef> for ObjectKey {
    fn from(object_ref: &ObjectRef) -> Self {
        Self(object_ref.0, object_ref.1)
    }
}

/// Fetch the `ObjectKey`s (IDs and versions) for non-shared input objects.  Includes owned,
/// and immutable objects as well as the gas objects, but not move packages or shared objects.
pub fn transaction_input_object_keys(tx: &SenderSignedData) -> SuiResult<Vec<ObjectKey>> {
    use crate::transaction::InputObjectKind as I;
    Ok(tx
        .intent_message()
        .value
        .input_objects()?
        .into_iter()
        .filter_map(|object| match object {
            I::MovePackage(_) | I::SharedMoveObject { .. } => None,
            I::ImmOrOwnedMoveObject(obj) => Some(obj.into()),
        })
        .collect())
}

pub fn transaction_receiving_object_keys(tx: &SenderSignedData) -> Vec<ObjectKey> {
    tx.intent_message()
        .value
        .receiving_objects()
        .into_iter()
        .map(|oref| oref.into())
        .collect()
}

pub trait ReceivedMarkerQuery {
    fn have_received_object_at_version(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
        epoch_id: EpochId,
    ) -> Result<bool, SuiError>;
}

impl<T: ReceivedMarkerQuery> ReceivedMarkerQuery for Arc<T> {
    fn have_received_object_at_version(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
        epoch_id: EpochId,
    ) -> Result<bool, SuiError> {
        self.as_ref()
            .have_received_object_at_version(object_id, version, epoch_id)
    }
}

impl<T: ReceivedMarkerQuery> ReceivedMarkerQuery for &T {
    fn have_received_object_at_version(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
        epoch_id: EpochId,
    ) -> Result<bool, SuiError> {
        (*self).have_received_object_at_version(object_id, version, epoch_id)
    }
}

impl Display for DeleteKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DeleteKind::Wrap => write!(f, "Wrap"),
            DeleteKind::Normal => write!(f, "Normal"),
            DeleteKind::UnwrapThenDelete => write!(f, "UnwrapThenDelete"),
        }
    }
}

pub trait BackingStore:
    BackingPackageStore + ChildObjectResolver + ObjectStore + ParentSync
{
    fn as_object_store(&self) -> &dyn ObjectStore;
}

impl<T> BackingStore for T
where
    T: BackingPackageStore,
    T: ChildObjectResolver,
    T: ObjectStore,
    T: ParentSync,
{
    fn as_object_store(&self) -> &dyn ObjectStore {
        self
    }
}

pub trait GetSharedLocks {
    fn get_shared_locks(
        &self,
        transaction_digest: &TransactionDigest,
    ) -> Result<Vec<(ObjectID, SequenceNumber)>, SuiError>;
}
