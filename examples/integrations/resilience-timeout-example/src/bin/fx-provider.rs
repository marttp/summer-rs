use serde::Serialize;
use std::time::Duration;
use summer::{auto_config, App};
use summer_web::extractor::{Json, Path};
use summer_web::{get, WebConfigurator, WebPlugin};

#[derive(Serialize)]
struct FxQuote {
    base: String,
    quote: String,
    rate: f64,
}

#[get("/quotes/{base}/{quote}")]
async fn quote(Path((base, quote)): Path<(String, String)>) -> Json<FxQuote> {
    let delay = if quote == "SGD" {
        Duration::from_millis(100)
    } else {
        Duration::from_millis(10)
    };
    println!("provider received {base}/{quote} request; responding after {delay:?}");
    tokio::time::sleep(delay).await;

    Json(FxQuote {
        base,
        rate: if quote == "SGD" { 1.28 } else { 35.42 },
        quote,
    })
}

#[auto_config(WebConfigurator)]
#[tokio::main]
async fn main() {
    App::new().add_plugin(WebPlugin).run().await;
}
