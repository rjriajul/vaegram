use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassportElementType {
    PersonalDetails,
    Passport,
    DriverLicense,
    IdentityCard,
    InternalPassport,
    Address,
    UtilityBill,
    BankStatement,
    RentalAgreement,
    PassportRegistration,
    TemporaryRegistration,
    PhoneNumber,
    Email,
}

impl PassportElementType {
    pub(crate) fn write(self) -> base::SecureValueType {
        use base::SecureValueType as T;
        match self {
            Self::PersonalDetails => T::SecureValueTypePersonalDetails,
            Self::Passport => T::SecureValueTypePassport,
            Self::DriverLicense => T::SecureValueTypeDriverLicense,
            Self::IdentityCard => T::SecureValueTypeIdentityCard,
            Self::InternalPassport => T::SecureValueTypeInternalPassport,
            Self::Address => T::SecureValueTypeAddress,
            Self::UtilityBill => T::SecureValueTypeUtilityBill,
            Self::BankStatement => T::SecureValueTypeBankStatement,
            Self::RentalAgreement => T::SecureValueTypeRentalAgreement,
            Self::PassportRegistration => T::SecureValueTypePassportRegistration,
            Self::TemporaryRegistration => T::SecureValueTypeTemporaryRegistration,
            Self::PhoneNumber => T::SecureValueTypePhone,
            Self::Email => T::SecureValueTypeEmail,
        }
    }
}
