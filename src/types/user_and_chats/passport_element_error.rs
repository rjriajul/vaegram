use crate::enums::PassportElementType;
use crate::raw::{base, types};

/// An error in a Telegram Passport element, reported back to the user so they can correct it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PassportElementError {
    /// A problem in one field of the element's data.
    DataField {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Name of the data field that has the error.
        field_name: String,
        /// Hash of the section's encrypted data: the `data_hash` from its `DataCredentials`.
        data_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with the front side of a document.
    FrontSide {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the encrypted file with the front side.
        file_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with the reverse side of a document.
    ReverseSide {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the encrypted file with the reverse side.
        file_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with the selfie of the user holding the document.
    Selfie {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the encrypted file with the selfie.
        file_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with one uploaded file of the element.
    File {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the encrypted file.
        file_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with a list of uploaded files.
    Files {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hashes of the encrypted files.
        file_hashes: Vec<Vec<u8>>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with one translation of the document.
    TranslationFile {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the encrypted translation file.
        file_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with a list of translation files.
    TranslationFiles {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hashes of the encrypted translation files.
        file_hashes: Vec<Vec<u8>>,
        /// The error message shown to the user.
        message: String,
    },
    /// A problem with the element as a whole.
    Unspecified {
        /// The Passport section that has the error.
        kind: PassportElementType,
        /// Hash of the whole element.
        element_hash: Vec<u8>,
        /// The error message shown to the user.
        message: String,
    },
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
