use crate::raw::base;

/// A kind of Telegram Passport data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassportElementType {
    /// Personal details such as name and birth date.
    PersonalDetails,
    /// A passport.
    Passport,
    /// A driver's license.
    DriverLicense,
    /// An identity card.
    IdentityCard,
    /// An internal passport.
    InternalPassport,
    /// A residential address.
    Address,
    /// A utility bill.
    UtilityBill,
    /// A bank statement.
    BankStatement,
    /// A rental agreement.
    RentalAgreement,
    /// The registration page of a passport.
    PassportRegistration,
    /// A temporary registration certificate.
    TemporaryRegistration,
    /// A phone number.
    PhoneNumber,
    /// An email address.
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
