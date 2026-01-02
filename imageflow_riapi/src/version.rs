/// Prelude:
/// The most common change to the APIs that changes the output is tuning for encoders, automatic format selection.
/// And, for example, how 'quality' values are interpreted.
/// This changes outputs in positive ways (we hope), but doesn't break the API contract.
/// So, it makes sense to version these changes as a minor version to allow for backward compatibility.
///
/// Thus, we offer a major.minor numeric. We then translate these versions to EncodeEngineVersion variants for our logic to check.
///
/// We also may want to have an 'ImageResizer' compatibility mode. We currently retain compatibility with ImageResizer 4, to a pretty close degree.
/// However, breaking free of that means we need people to be able to specify when they do and don't want it.
///
/// Thus, we offer an 'ir4' version. Now, we also offer minor versions to control which codec system is used.
/// ir4, of course, will default to a legacy codec mapper
///
///
/// We don't require version be specified, so we must encode 'unspecified'.
/// We want users to be able to specify 'latest' (perhaps the server is configured to a different default version)
///
/// Thus, version values might be \d  \d.\d ir\d.\d 'latest' 'preview' 'ir4', 'ir4.0', and future strings might be added.
/// We accept version values in the querystring as &v=
/// And inside srcset as 'v0.0' 'v0' v-ir4.1', v-latest', v-preview'. Ex: &srcset=v1.1,100w or &srcset=v-latest,100w, &srcset=ir4.1,100w

/// Unspecified will default to server configuration. This value might be parsed from other sources like toml or environment variables or json.
/// We want parsing of this value to produce an ApiVersionInvalid warning if not correct or supported.
/// This helps ensure version compatibility and proper error reporting when the API version is invalid.
/// Callers may choose to surface this as a fatal error or not.
/// The RiapiVersion struct should offer easy methods for code to ergonmocially check for behavior.
/// such as .is_resizer4(), .is_latest(), get_major(),  

/// Represents the encoding and encoder tuning logic version.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeEngineVersion {
    IR4,     // Mimic ImageResizer 4. Fidelity to ImageResizer 4 tuning values may grow over time.
    V2,      // Imageflow 2.X defaults.
    Preview, // Preview and experimental engine logic that may change over time.
}
// impl EncodeEngineVersion {
//     fn choose(version: &RiapiVersionValue) -> EncodeEngineVersion {
//         match version {
//             RiapiVersionValue::Resizer4 { minor: None } |
//             RiapiVersionValue::Resizer4 { minor: Some(0) } => EncodeEngineVersion::IR4,
//             RiapiVersionValue::Version { major: 2, minor: Some(1), .. } => EncodeEngineVersion::Preview,
//             RiapiVersionValue::Version { major: 2, minor: None } => EncodeEngineVersion::V2,
//             _ => EncodeEngineVersion::IR4,
//         }
//     }
// }

// // private enumeration we can evolve to reduce both invariants and API breakage.
// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// enum RiapiVersionValue{
//     Unspecified,
//     Latest,
//     Preview,
//     Version {major: u32, minor: Option<u32>},
//     Resizer4 { minor: Option<u32> },
// }
// impl RiapiVersionValue {
//     fn to_string(&self) -> Option<String> {
//         match self {
//             RiapiVersionValue::Unspecified => None,
//             RiapiVersionValue::Latest => Some("latest".to_string()),
//             RiapiVersionValue::Preview => Some("preview".to_string()),
//             RiapiVersionValue::Version { major, minor } => {
//                 if let Some(minor) = minor {
//                     Some(format!("{}.{}", major, minor))
//                 } else {
//                     Some(major.to_string())
//                 }
//             }
//             RiapiVersionValue::Resizer4 { minor } => {
//                 if let Some(minor) = minor {
//                     Some(format!("ir4.{}", minor))
//                 } else {
//                     Some("ir4".to_string())
//                 }
//             }
//         }
//     }

//     fn is_supported(&self) -> bool {
//         RiapiVersionStatus::recognized_versions().iter().any(|stat| stat.value == *self && stat.supported)
//     }

//     fn is_latest(&self) -> bool {
//         matches!(self, RiapiVersionValue::Latest)
//     }
// }
// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// struct RiapiVersionStatus{
//     supported: bool,
//     encoder: EncodeEngineVersion,
//     value: RiapiVersionValue,
// }

// impl RiapiVersionStatus {
//     fn recognized_versions() -> &'static [RiapiVersionStatus] {
//         &[
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::V2, value: RiapiVersionValue::Latest },
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::Preview, value: RiapiVersionValue::Preview },
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::V2, value: RiapiVersionValue::Version { major: 2, minor: None } },
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::V2, value: RiapiVersionValue::Version { major: 2, minor: Some(0) } },
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::IR4, value: RiapiVersionValue::Resizer4 { minor: None } },
//             RiapiVersionStatus { supported: true, encoder: EncodeEngineVersion::IR4, value: RiapiVersionValue::Resizer4 { minor: Some(0) } },
//         ]
//     }
// }

// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub struct RiapiVersion {
//     v: RiapiVersionValue,
// }

// impl RiapiVersion {

//     pub fn recognized_versions() -> Vec<RiapiVersionStatus> {
//         RiapiVersionStatus::recognized_versions().iter().map(|stat| *stat).collect()
//     }

//     pub fn is_supported(&self) -> bool {
//         RiapiVersionStatus::recognized_versions().iter().any(|stat| stat.value == self.v && stat.supported)
//     }

//     pub fn is_resizer4(&self) -> bool {
//         matches!(self.v, RiapiVersionValue::Resizer4 { .. })
//     }

//     pub fn is_latest(&self) -> bool {
//         matches!(self.v, RiapiVersionValue::Latest)
//     }

//     pub fn get_encoder_engine(&self) -> EncodeEngineVersion {
//         EncodeEngineVersion::choose(&self.v)
//     }

// }
