use std::io::{self, BufRead, IsTerminal, Write};

use crate::Client;
use crate::client::Login;
use crate::errors::Result;
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
            match self.sign_in(&phone, &sent.phone_code_hash, &c).await {
                Ok(_) => return Ok(()),
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
}
