/*
 * 23-11-07 TaiVV
 * Copy "MOVE part" from sui-json-rpc-types/src/sui-object
 */
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write;
use std::fmt::{Display, Formatter};

use anyhow::anyhow;
use colored::Colorize;
use fastcrypto::encoding::Base64;
//use move_bytecode_utils::module_cache::GetModule;
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::StructTag;
use move_core_types::value::{MoveStruct, MoveStructLayout};
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_with::serde_as;
use serde_with::DisplayFromStr;

use super::{SuiMoveStruct, SuiMoveValue};
use scalar_json_rpc_types::Page;
use scalar_types::base_types::{
    ObjectDigest, ObjectID, ObjectInfo, ObjectRef, ObjectType, SequenceNumber, SuiAddress,
    TransactionDigest,
};
use scalar_types::error::{
    ExecutionError, SuiObjectResponseError, UserInputError, UserInputResult,
};
use scalar_types::gas_coin::GasCoin;
use scalar_types::messages_checkpoint::CheckpointSequenceNumber;
use scalar_types::move_package::{MovePackage, TypeOrigin, UpgradeInfo};
use scalar_types::object::{Data, MoveObject, Object, ObjectFormatOptions, ObjectRead, Owner};
use scalar_types::scalar_serde::BigInt;
use scalar_types::scalar_serde::SequenceNumber as AsSequenceNumber;
use scalar_types::scalar_serde::SuiStructTag;
use sui_protocol_config::ProtocolConfig;

pub trait SuiData: Sized {
    type ObjectType;
    type PackageType;
    fn try_from_object(object: MoveObject, layout: MoveStructLayout)
        -> Result<Self, anyhow::Error>;
    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error>;
    fn try_as_move(&self) -> Option<&Self::ObjectType>;
    fn try_as_package(&self) -> Option<&Self::PackageType>;
    fn type_(&self) -> Option<&StructTag>;
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(tag = "dataType", rename_all = "camelCase", rename = "RawData")]
pub enum SuiRawData {
    // Manually handle generic schema generation
    MoveObject(SuiRawMoveObject),
    Package(SuiRawMovePackage),
}

impl SuiData for SuiRawData {
    type ObjectType = SuiRawMoveObject;
    type PackageType = SuiRawMovePackage;

    fn try_from_object(object: MoveObject, _: MoveStructLayout) -> Result<Self, anyhow::Error> {
        Ok(Self::MoveObject(object.into()))
    }

    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error> {
        Ok(Self::Package(package.into()))
    }

    fn try_as_move(&self) -> Option<&Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(o),
            Self::Package(_) => None,
        }
    }

    fn try_as_package(&self) -> Option<&Self::PackageType> {
        match self {
            Self::MoveObject(_) => None,
            Self::Package(p) => Some(p),
        }
    }

    fn type_(&self) -> Option<&StructTag> {
        match self {
            Self::MoveObject(o) => Some(&o.type_),
            Self::Package(_) => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(tag = "dataType", rename_all = "camelCase", rename = "Data")]
pub enum SuiParsedData {
    // Manually handle generic schema generation
    MoveObject(SuiParsedMoveObject),
    Package(SuiMovePackage),
}

impl SuiData for SuiParsedData {
    type ObjectType = SuiParsedMoveObject;
    type PackageType = SuiMovePackage;

    fn try_from_object(
        object: MoveObject,
        layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self::MoveObject(SuiParsedMoveObject::try_from_layout(
            object, layout,
        )?))
    }

    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error> {
        Ok(Self::Package(SuiMovePackage {
            disassembled: package.disassemble()?,
        }))
    }

    fn try_as_move(&self) -> Option<&Self::ObjectType> {
        match self {
            Self::MoveObject(o) => Some(o),
            Self::Package(_) => None,
        }
    }

    fn try_as_package(&self) -> Option<&Self::PackageType> {
        match self {
            Self::MoveObject(_) => None,
            Self::Package(p) => Some(p),
        }
    }

    fn type_(&self) -> Option<&StructTag> {
        match self {
            Self::MoveObject(o) => Some(&o.type_),
            Self::Package(_) => None,
        }
    }
}

impl Display for SuiParsedData {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut writer = String::new();
        match self {
            SuiParsedData::MoveObject(o) => {
                writeln!(writer, "{}: {}", "type".bold().bright_black(), o.type_)?;
                write!(writer, "{}", &o.fields)?;
            }
            SuiParsedData::Package(p) => {
                write!(
                    writer,
                    "{}: {:?}",
                    "Modules".bold().bright_black(),
                    p.disassembled.keys()
                )?;
            }
        }
        write!(f, "{}", writer)
    }
}

impl SuiParsedData {
    pub fn try_from_object_read(object_read: ObjectRead) -> Result<Self, anyhow::Error> {
        match object_read {
            ObjectRead::NotExists(id) => Err(anyhow::anyhow!("Object {} does not exist", id)),
            ObjectRead::Exists(_object_ref, o, layout) => {
                let data = match o.data {
                    Data::Move(m) => {
                        let layout = layout.ok_or_else(|| {
                            anyhow!("Layout is required to convert Move object to json")
                        })?;
                        SuiParsedData::try_from_object(m, layout)?
                    }
                    Data::Package(p) => SuiParsedData::try_from_package(p)?,
                };
                Ok(data)
            }
            ObjectRead::Deleted((object_id, version, digest)) => Err(anyhow::anyhow!(
                "Object {} was deleted at version {} with digest {}",
                object_id,
                version,
                digest
            )),
        }
    }
}

pub trait SuiMoveObject: Sized {
    fn try_from_layout(object: MoveObject, layout: MoveStructLayout)
        -> Result<Self, anyhow::Error>;

    fn try_from(o: MoveObject, resolver: &impl GetModule) -> Result<Self, anyhow::Error> {
        let layout = o.get_layout(ObjectFormatOptions::default(), resolver)?;
        Self::try_from_layout(o, layout)
    }

    fn type_(&self) -> &StructTag;
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "MoveObject", rename_all = "camelCase")]
pub struct SuiParsedMoveObject {
    #[serde(rename = "type")]
    #[serde_as(as = "SuiStructTag")]
    #[schemars(with = "String")]
    pub type_: StructTag,
    pub has_public_transfer: bool,
    pub fields: SuiMoveStruct,
}

impl SuiMoveObject for SuiParsedMoveObject {
    fn try_from_layout(
        object: MoveObject,
        layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        let move_struct = object.to_move_struct(&layout)?.into();

        Ok(
            if let SuiMoveStruct::WithTypes { type_, fields } = move_struct {
                SuiParsedMoveObject {
                    type_,
                    has_public_transfer: object.has_public_transfer(),
                    fields: SuiMoveStruct::WithFields(fields),
                }
            } else {
                SuiParsedMoveObject {
                    type_: object.type_().clone().into(),
                    has_public_transfer: object.has_public_transfer(),
                    fields: move_struct,
                }
            },
        )
    }

    fn type_(&self) -> &StructTag {
        &self.type_
    }
}

impl SuiParsedMoveObject {
    pub fn try_from_object_read(object_read: ObjectRead) -> Result<Self, anyhow::Error> {
        let parsed_data = SuiParsedData::try_from_object_read(object_read)?;
        match parsed_data {
            SuiParsedData::MoveObject(o) => Ok(o),
            SuiParsedData::Package(_) => Err(anyhow::anyhow!("Object is not a Move object")),
        }
    }

    pub fn read_dynamic_field_value(&self, field_name: &str) -> Option<SuiMoveValue> {
        match &self.fields {
            SuiMoveStruct::WithFields(fields) => fields.get(field_name).cloned(),
            SuiMoveStruct::WithTypes { fields, .. } => fields.get(field_name).cloned(),
            _ => None,
        }
    }
}

pub fn type_and_fields_from_move_struct(
    type_: &StructTag,
    move_struct: MoveStruct,
) -> (StructTag, SuiMoveStruct) {
    match move_struct.into() {
        SuiMoveStruct::WithTypes { type_, fields } => (type_, SuiMoveStruct::WithFields(fields)),
        fields => (type_.clone(), fields),
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "RawMoveObject", rename_all = "camelCase")]
pub struct SuiRawMoveObject {
    #[schemars(with = "String")]
    #[serde(rename = "type")]
    #[serde_as(as = "SuiStructTag")]
    pub type_: StructTag,
    pub has_public_transfer: bool,
    pub version: SequenceNumber,
    #[serde_as(as = "Base64")]
    #[schemars(with = "Base64")]
    pub bcs_bytes: Vec<u8>,
}

impl From<MoveObject> for SuiRawMoveObject {
    fn from(o: MoveObject) -> Self {
        Self {
            type_: o.type_().clone().into(),
            has_public_transfer: o.has_public_transfer(),
            version: o.version(),
            bcs_bytes: o.into_contents(),
        }
    }
}

impl SuiMoveObject for SuiRawMoveObject {
    fn try_from_layout(
        object: MoveObject,
        _layout: MoveStructLayout,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            type_: object.type_().clone().into(),
            has_public_transfer: object.has_public_transfer(),
            version: object.version(),
            bcs_bytes: object.into_contents(),
        })
    }

    fn type_(&self) -> &StructTag {
        &self.type_
    }
}

impl SuiRawMoveObject {
    pub fn deserialize<'a, T: Deserialize<'a>>(&'a self) -> Result<T, anyhow::Error> {
        Ok(bcs::from_bytes(self.bcs_bytes.as_slice())?)
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(rename = "RawMovePackage", rename_all = "camelCase")]
pub struct SuiRawMovePackage {
    pub id: ObjectID,
    pub version: SequenceNumber,
    #[schemars(with = "BTreeMap<String, Base64>")]
    #[serde_as(as = "BTreeMap<_, Base64>")]
    pub module_map: BTreeMap<String, Vec<u8>>,
    pub type_origin_table: Vec<TypeOrigin>,
    pub linkage_table: BTreeMap<ObjectID, UpgradeInfo>,
}

impl From<MovePackage> for SuiRawMovePackage {
    fn from(p: MovePackage) -> Self {
        Self {
            id: p.id(),
            version: p.version(),
            module_map: p.serialized_module_map().clone(),
            type_origin_table: p.type_origin_table().clone(),
            linkage_table: p.linkage_table().clone(),
        }
    }
}

impl SuiRawMovePackage {
    pub fn to_move_package(
        &self,
        max_move_package_size: u64,
    ) -> Result<MovePackage, ExecutionError> {
        MovePackage::new(
            self.id,
            self.version,
            self.module_map.clone(),
            max_move_package_size,
            self.type_origin_table.clone(),
            self.linkage_table.clone(),
        )
    }
}

pub trait ScalarMoveData: Sized {
    type ObjectType;
    type PackageType;
    fn try_from_object(object: MoveObject, layout: MoveStructLayout)
        -> Result<Self, anyhow::Error>;
    fn try_from_package(package: MovePackage) -> Result<Self, anyhow::Error>;
    fn try_as_move(&self) -> Option<&Self::ObjectType>;
    fn try_as_package(&self) -> Option<&Self::PackageType>;
    fn type_(&self) -> Option<&StructTag>;
}
