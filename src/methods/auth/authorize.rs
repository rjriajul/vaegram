use std::io::{self, BufRead, IsTerminal, Write};

use crate::Client;
use crate::client::Login;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

async fn ask(prompt: &'static str, hidden: bool) -> Result<String> {
    let answer = tokio::task::spawn_blocking(move || {
        if hidden && io::stdin().is_terminal() {
            return rpassword::prompt_password(prompt);
        }
        print!("{prompt}");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line)? == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "no input to log in with"));
        }
        Ok(line)
    })
    .await
    .map_err(io::Error::other)??;
    Ok(if hidden { answer.trim_end_matches(['\r', '\n']).to_owned() } else { answer.trim().to_owned() })
}

impl Client {
    pub(crate) async fn authorize(&self, login: Login) -> Result<()> {
        let value = match (login.bot_token, login.phone_number) {
            (Some(token), _) => token,
            (None, Some(phone)) => phone,
            (None, None) => loop {
                let value = ask("Enter phone number or bot token: ", false).await?;
                if !value.is_empty() {
                    break value;
                }
            },
        };
        if value.contains(':') {
            return self.sign_in_bot(&value).await.map(drop);
        }

        let phone = value.trim_matches(|c| c == ' ' || c == '+').to_owned();
        let sent = self.send_code(&phone).await?;
        println!("The confirmation code has been sent via {}", sent.kind);

        let mut code = login.phone_code;
        loop {
            let c = match code.take() {
                Some(c) => c,
                None => ask("Enter confirmation code: ", false).await?,
            };
            match self.sign_in_raw(&phone, &sent.phone_code_hash, &c).await {
                Ok(base::auth::Authorization::AuthorizationSignUpRequired(required)) => {
                    let first_name = ask("Enter first name: ", false).await?;
                    let last_name = ask("Enter last name (optional): ", false).await?;
                    let (terms, confirm) = (required.terms_of_service, ask("Accept the terms? [y/N]: ", false));
                    let hash = &sent.phone_code_hash;
                    return self.finish_sign_up(&phone, hash, &first_name, &last_name, terms, confirm).await;
                }
                Ok(auth) => return self.authorized(auth).await.map(drop),
                Err(e) if e.is("PHONE_CODE_INVALID") => println!("Wrong code, try again"),
                Err(e) if e.is("SESSION_PASSWORD_NEEDED") => break,
                Err(e) => return Err(e),
            }
        }

        let mut password = login.password;
        loop {
            let p = match password.take() {
                Some(p) => p,
                None => {
                    let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
                    if let Some(hint) = p.hint.filter(|h| !h.is_empty()) {
                        println!("Password hint: {hint}");
                    }
                    ask("Enter password: ", true).await?
                }
            };
            match self.check_password(&p).await {
                Ok(_) => return Ok(()),
                Err(e) if e.is("PASSWORD_HASH_INVALID") => println!("Wrong password, try again"),
                Err(e) => return Err(e),
            }
        }
    }

    /// Signs up, then shows and accepts the terms Telegram sent with the sign-up request. Accepting comes last: when
    /// it fails or is refused, the account already exists and the session is saved, with the terms unaccepted.
    async fn finish_sign_up(
        &self,
        phone: &str,
        phone_code_hash: &str,
        first_name: &str,
        last_name: &str,
        terms: Option<base::help::TermsOfService>,
        confirm: impl Future<Output = Result<String>>,
    ) -> Result<()> {
        self.sign_up(phone, phone_code_hash, first_name, last_name).await?;
        self.accept_shown_terms(terms, confirm).await
    }

    /// Prints the terms and accepts them once `confirm` (only awaited when there are terms) answers yes.
    async fn accept_shown_terms(
        &self,
        terms: Option<base::help::TermsOfService>,
        confirm: impl Future<Output = Result<String>>,
    ) -> Result<()> {
        let Some(base::help::TermsOfService::TermsOfService(terms)) = terms else { return Ok(()) };
        println!("\n{}\n", terms.text);
        if let Some(age) = terms.min_age_confirm {
            println!("You must be at least {age} years old to use Telegram.\n");
        }
        let answer = confirm.await?;
        if !(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")) {
            return Err(Error::InvalidArgument("the terms of service were not accepted"));
        }
        let base::DataJSON::DataJSON(id) = terms.id;
        self.accept_terms_of_service(id.data).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::Error;
    use crate::methods::auth::send_code::tests::user;
    use crate::raw::{Serializable, base, functions, types};

    fn signed_up() -> Vec<u8> {
        let auth = types::auth::Authorization {
            setup_password_required: false,
            otherwise_relogin_days: None,
            tmp_sessions: None,
            future_auth_token: None,
            user: user(7),
        };
        base::auth::Authorization::Authorization(Box::new(auth)).to_bytes()
    }

    fn id() -> base::DataJSON {
        base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{\"id\":\"tos-1\"}".into() }))
    }

    fn sign_up() -> Vec<u8> {
        let request = functions::auth::SignUp {
            no_joined_notifications: false,
            phone_number: "8801".into(),
            phone_code_hash: "hash".into(),
            first_name: "Ada".into(),
            last_name: String::new(),
        };
        request.to_bytes()
    }

    fn terms() -> Option<base::help::TermsOfService> {
        let terms = types::help::TermsOfService {
            popup: true,
            id: id(),
            text: "Be nice.".into(),
            entities: Vec::new(),
            min_age_confirm: Some(16),
        };
        Some(base::help::TermsOfService::TermsOfService(Box::new(terms)))
    }

    fn sent(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[tokio::test]
    async fn sign_up_accepts_the_terms_it_was_shown() {
        for yes in ["y", "Yes"] {
            let (client, mut rx) = Client::scripted(vec![Ok(signed_up()), Ok(true.to_bytes())]);
            let answer = async { Ok::<_, Error>(String::from(yes)) };
            client.finish_sign_up("8801", "hash", "Ada", "", terms(), answer).await.unwrap();
            let accept = functions::help::AcceptTermsOfService { id: id() }.to_bytes();
            assert_eq!(sent(&mut rx), [sign_up(), accept], "{yes}");
            assert_eq!(client.0.me.lock().unwrap().id, 7);
        }
    }

    #[tokio::test]
    async fn refused_terms_stay_unaccepted() {
        for no in ["", "n", "yep"] {
            let (client, mut rx) = Client::scripted(vec![Ok(signed_up())]);
            let answer = async { Ok::<_, Error>(String::from(no)) };
            let result = client.finish_sign_up("8801", "hash", "Ada", "", terms(), answer).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{no:?}: {result:?}");
            // The account exists all the same: only the acceptance is missing.
            assert_eq!((sent(&mut rx), client.0.me.lock().unwrap().id), (vec![sign_up()], 7), "{no:?}");
        }
    }

    #[tokio::test]
    async fn no_terms_no_accept() {
        let (client, mut rx) = Client::scripted(vec![Ok(signed_up())]);
        // Awaiting it would fail the sign-up, so a passing call proves nobody was asked.
        let never_asked = async { Err::<String, _>(Error::InvalidArgument("nothing to confirm")) };
        client.finish_sign_up("8801", "hash", "Ada", "", None, never_asked).await.unwrap();
        assert_eq!(sent(&mut rx), [sign_up()]);
    }
}
