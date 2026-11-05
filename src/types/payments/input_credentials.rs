use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::utils::compute_password_check;

/// How a card form is paid. Its `Debug` prints only the variant, never the payment data.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputCredentials {
    /// A card entered just now.
    New {
        /// The provider's JSON token.
        data: String,
        /// Whether the provider may remember the card.
        allow_save: bool,
    },
    /// A card from [`PaymentForm::saved_credentials`](crate::types::PaymentForm::saved_credentials).
    ///
    /// Needs the account's 2FA password.
    Saved {
        /// The `id` of the saved card.
        saved_credentials_id: String,
        /// The account's 2FA password. It isn't sent: it only proves the password to get a 60-second
        /// temporary password for the payment.
        password: String,
    },
    /// A payment token from Apple Pay.
    ApplePay {
        /// The provider's JSON token.
        data: String,
    },
    /// A payment token from Google Pay.
    GooglePay {
        /// The provider's JSON token.
        data: String,
    },
}

impl InputCredentials {
    /// A card entered just now, from the provider's JSON token. The provider doesn't save it unless
    /// [`allow_save`](InputCredentials::allow_save) is called.
    pub fn new(data: impl Into<String>) -> InputCredentials {
        InputCredentials::New { data: data.into(), allow_save: false }
    }

    /// Lets the provider remember the card. Only a new card can be saved: other kinds are returned unchanged.
    pub fn allow_save(mut self) -> InputCredentials {
        if let InputCredentials::New { allow_save, .. } = &mut self {
            *allow_save = true;
        }
        self
    }

    /// A saved card, confirmed with the account's 2FA password when the payment is sent.
    pub fn saved(saved_credentials_id: impl Into<String>, password: impl Into<String>) -> InputCredentials {
        InputCredentials::Saved { saved_credentials_id: saved_credentials_id.into(), password: password.into() }
    }

    /// An Apple Pay payment token.
    pub fn apple_pay(data: impl Into<String>) -> InputCredentials {
        InputCredentials::ApplePay { data: data.into() }
    }

    /// A Google Pay payment token.
    pub fn google_pay(data: impl Into<String>) -> InputCredentials {
        InputCredentials::GooglePay { data: data.into() }
    }

    pub(crate) async fn write(self, client: &Client) -> Result<base::InputPaymentCredentials> {
        let json = |data: String| base::DataJSON::DataJSON(Box::new(types::DataJSON { data }));
        Ok(match self {
            InputCredentials::New { data, allow_save } => {
                base::InputPaymentCredentials::InputPaymentCredentials(Box::new(types::InputPaymentCredentials {
                    save: allow_save,
                    data: json(data),
                }))
            }
            InputCredentials::ApplePay { data } => base::InputPaymentCredentials::InputPaymentCredentialsApplePay(
                Box::new(types::InputPaymentCredentialsApplePay { payment_data: json(data) }),
            ),
            InputCredentials::GooglePay { data } => base::InputPaymentCredentials::InputPaymentCredentialsGooglePay(
                Box::new(types::InputPaymentCredentialsGooglePay { payment_token: json(data) }),
            ),
            InputCredentials::Saved { saved_credentials_id, password } => {
                let base::account::Password::Password(p) = client.invoke(&functions::account::GetPassword {}).await?;
                let request =
                    functions::account::GetTmpPassword { password: compute_password_check(&p, &password)?, period: 60 };
                let base::account::TmpPassword::TmpPassword(tmp) = client.invoke(&request).await?;
                base::InputPaymentCredentials::InputPaymentCredentialsSaved(Box::new(
                    types::InputPaymentCredentialsSaved { id: saved_credentials_id, tmp_password: tmp.tmp_password },
                ))
            }
        })
    }
}

impl std::fmt::Debug for InputCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InputCredentials::New { .. } => "InputCredentials::New",
            InputCredentials::Saved { .. } => "InputCredentials::Saved",
            InputCredentials::ApplePay { .. } => "InputCredentials::ApplePay",
            InputCredentials::GooglePay { .. } => "InputCredentials::GooglePay",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::InputCredentials;

    #[test]
    fn credentials_debug_hides_secrets() {
        for credentials in [
            InputCredentials::new("card-token-123").allow_save(),
            InputCredentials::saved("id-9", "hunter2"),
            InputCredentials::apple_pay("apple-token-123"),
            InputCredentials::google_pay("google-token-123"),
        ] {
            let shown = format!("{credentials:?} {:?}", [&credentials]);
            for secret in ["token-123", "hunter2", "id-9"] {
                assert!(!shown.contains(secret), "{shown}");
            }
        }
        assert_eq!(format!("{:?}", InputCredentials::new("x")), "InputCredentials::New");
    }

    #[test]
    fn only_a_new_card_can_be_saved() {
        assert_eq!(
            InputCredentials::new("d").allow_save(),
            InputCredentials::New { data: "d".into(), allow_save: true }
        );
        assert_eq!(InputCredentials::apple_pay("d").allow_save(), InputCredentials::apple_pay("d"));
    }
}
