use serde::Deserialize;
use summer::App;
use summer_resilience::{timeout, ResiliencePlugin, TimeoutElapsed};
use thiserror::Error;

const PROVIDER_URL: &str = "http://127.0.0.1:18083";

#[derive(Debug, Deserialize)]
struct FxQuote {
    base: String,
    quote: String,
    rate: f64,
}

#[derive(Debug, Error)]
enum QuoteError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    TimedOut(#[from] TimeoutElapsed),
}

#[timeout(name = "fx-quotes")]
async fn fetch_fx_quote(
    client: &reqwest::Client,
    provider_url: &str,
    base: &str,
    quote: &str,
) -> Result<FxQuote, QuoteError> {
    let response = client
        .get(format!("{provider_url}/quotes/{base}/{quote}"))
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}

#[tokio::main]
async fn main() {
    App::new()
        .add_plugin(ResiliencePlugin)
        .build()
        .await
        .expect("application should build");

    let client = reqwest::Client::new();
    match fetch_fx_quote(&client, PROVIDER_URL, "USD", "THB").await {
        Ok(quote) => println!("fast {}/{} rate: {}", quote.base, quote.quote, quote.rate),
        Err(error) => eprintln!("fast quote failed: {error}"),
    }
    println!(
        "slow USD/SGD quote: {:?}",
        fetch_fx_quote(&client, PROVIDER_URL, "USD", "SGD").await
    );
}
