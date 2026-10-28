use crate::enums::PassportElementType;
use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassportElementError {
    DataField { kind: PassportElementType, field_name: String, data_hash: Vec<u8>, message: String },
    FrontSide { kind: PassportElementType, file_hash: Vec<u8>, message: String },
    ReverseSide { kind: PassportElementType, file_hash: Vec<u8>, message: String },
    Selfie { kind: PassportElementType, file_hash: Vec<u8>, message: String },
    File { kind: PassportElementType, file_hash: Vec<u8>, message: String },
    Files { kind: PassportElementType, file_hashes: Vec<Vec<u8>>, message: String },
    TranslationFile { kind: PassportElementType, file_hash: Vec<u8>, message: String },
    TranslationFiles { kind: PassportElementType, file_hashes: Vec<Vec<u8>>, message: String },
    Unspecified { kind: PassportElementType, element_hash: Vec<u8>, message: String },
}

impl PassportElementError {
    pub(crate) fn write(self) -> base::SecureValueError {
        use base::SecureValueError as E;
        match self {
            Self::DataField { kind, field_name, data_hash, message } => {
                E::SecureValueErrorData(Box::new(types::SecureValueErrorData {
                    r#type: kind.write(),
                    data_hash,
                    field: field_name,
                    text: message,
                }))
            }
            Self::FrontSide { kind, file_hash, message } => {
                let error = types::SecureValueErrorFrontSide { r#type: kind.write(), file_hash, text: message };
                E::SecureValueErrorFrontSide(Box::new(error))
            }
            Self::ReverseSide { kind, file_hash, message } => {
                let error = types::SecureValueErrorReverseSide { r#type: kind.write(), file_hash, text: message };
                E::SecureValueErrorReverseSide(Box::new(error))
            }
            Self::Selfie { kind, file_hash, message } => {
                let error = types::SecureValueErrorSelfie { r#type: kind.write(), file_hash, text: message };
                E::SecureValueErrorSelfie(Box::new(error))
            }
            Self::File { kind, file_hash, message } => {
                let error = types::SecureValueErrorFile { r#type: kind.write(), file_hash, text: message };
                E::SecureValueErrorFile(Box::new(error))
            }
            Self::Files { kind, file_hashes, message } => {
                let error =
                    types::SecureValueErrorFiles { r#type: kind.write(), file_hash: file_hashes, text: message };
                E::SecureValueErrorFiles(Box::new(error))
            }
            Self::TranslationFile { kind, file_hash, message } => {
                let error = types::SecureValueErrorTranslationFile { r#type: kind.write(), file_hash, text: message };
                E::SecureValueErrorTranslationFile(Box::new(error))
            }
            Self::TranslationFiles { kind, file_hashes, message } => {
                let error = types::SecureValueErrorTranslationFiles {
                    r#type: kind.write(),
                    file_hash: file_hashes,
                    text: message,
                };
                E::SecureValueErrorTranslationFiles(Box::new(error))
            }
            Self::Unspecified { kind, element_hash, message } => {
                let error = types::SecureValueError { r#type: kind.write(), hash: element_hash, text: message };
                E::SecureValueError(Box::new(error))
            }
        }
    }
}
