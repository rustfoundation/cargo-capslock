use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize};
pub use strum::ParseError;
use strum::{EnumString, IntoStaticStr, VariantNames};

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    EnumString,
    IntoStaticStr,
    strum::Display,
    VariantNames,
)]
#[repr(u32)]
pub enum Capability {
    #[strum(serialize = "CAPABILITY_UNSPECIFIED")]
    Unspecified = 0,
    #[strum(serialize = "CAPABILITY_SAFE")]
    Safe = 1,
    #[strum(serialize = "CAPABILITY_FILES")]
    Files = 2,
    #[strum(serialize = "CAPABILITY_NETWORK")]
    Network = 3,
    #[strum(serialize = "CAPABILITY_RUNTIME")]
    Runtime = 4,
    #[strum(serialize = "CAPABILITY_READ_SYSTEM_STATE")]
    ReadSystemState = 5,
    #[strum(serialize = "CAPABILITY_MODIFY_SYSTEM_STATE")]
    ModifySystemState = 6,
    #[strum(serialize = "CAPABILITY_OPERATING_SYSTEM")]
    OperatingSystem = 7,
    #[strum(serialize = "CAPABILITY_SYSTEM_CALLS")]
    SystemCalls = 8,
    #[strum(serialize = "CAPABILITY_ARBITRARY_EXECUTION")]
    ArbitraryExecution = 9,
    #[strum(serialize = "CAPABILITY_CGO")]
    Cgo = 10,
    #[strum(serialize = "CAPABILITY_UNANALYZED")]
    Unanalyzed = 11,
    #[strum(serialize = "CAPABILITY_UNSAFE_POINTER")]
    UnsafePointer = 12,
    #[strum(serialize = "CAPABILITY_REFLECT")]
    Reflect = 13,
    #[strum(serialize = "CAPABILITY_EXEC")]
    Exec = 14,
    #[strum(serialize = "CAPABILITY_DYNAMIC_LOADING")]
    DynamicLoading = 15,
    #[strum(serialize = "CAPABILITY_INSTRUMENTATION")]
    Instrumentation = 16,
    #[strum(serialize = "CAPABILITY_NATIVE_CODE")]
    NativeCode = 17,
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let variant = String::deserialize(deserializer)?;
        Self::from_str(&variant)
            .map_err(|_| serde::de::Error::unknown_variant(&variant, Self::VARIANTS))
    }
}

impl Serialize for Capability {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Hash, EnumString, IntoStaticStr, strum::Display, VariantNames,
)]
#[repr(u32)]
pub enum CapabilityType {
    #[strum(serialize = "CAPABILITY_TYPE_UNSPECIFIED")]
    Unspecified = 0,
    #[strum(serialize = "CAPABILITY_TYPE_DIRECT")]
    Direct = 1,
    #[strum(serialize = "CAPABILITY_TYPE_TRANSITIVE")]
    Transitive = 2,
}

impl Ord for CapabilityType {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Direct, Self::Direct) => Ordering::Equal,
            (Self::Direct, _) => Ordering::Greater,
            (Self::Transitive, Self::Direct) => Ordering::Less,
            (Self::Transitive, Self::Transitive) => Ordering::Equal,
            (Self::Transitive, _) => Ordering::Greater,
            (Self::Unspecified, Self::Unspecified) => Ordering::Equal,
            _ => Ordering::Less,
        }
    }
}

impl PartialOrd for CapabilityType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'de> Deserialize<'de> for CapabilityType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let variant = String::deserialize(deserializer)?;
        Self::from_str(&variant)
            .map_err(|_| serde::de::Error::unknown_variant(&variant, Self::VARIANTS))
    }
}

impl Serialize for CapabilityType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_ordering() {
        assert!(CapabilityType::Direct == CapabilityType::Direct);
        assert!(CapabilityType::Direct > CapabilityType::Transitive);
        assert!(CapabilityType::Direct > CapabilityType::Unspecified);

        assert!(CapabilityType::Transitive == CapabilityType::Transitive);
        assert!(CapabilityType::Transitive < CapabilityType::Direct);
        assert!(CapabilityType::Transitive > CapabilityType::Unspecified);

        assert!(CapabilityType::Unspecified == CapabilityType::Unspecified);
        assert!(CapabilityType::Unspecified < CapabilityType::Direct);
        assert!(CapabilityType::Unspecified < CapabilityType::Transitive);
    }
}
