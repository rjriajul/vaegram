use vaegram::Client;
use vaegram::raw::base;

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("my_account", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH")).start().await?;

    if let base::User::User(me) = app.get_me().await? {
        println!("Logged in as {} (id {})", me.first_name.unwrap_or_default(), me.id);
    }
    Ok(())
}
