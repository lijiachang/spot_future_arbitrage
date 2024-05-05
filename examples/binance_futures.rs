#[cfg(feature = "futures_api")]
#[macro_use]
extern crate tracing;

use env_logger::Builder;

#[tokio::main]
async fn main() {
    Builder::new().parse_default_env().init();
    // #[cfg(feature = "futures_api")]
    // general().await;
    // #[cfg(feature = "futures_api")]
    // market_data().await;
    #[cfg(feature = "futures_api")]
    get_position_mode().await;
}

#[cfg(feature = "futures_api")]
async fn general() {
    use binance::api::*;
    use binance::errors::Error as BinanceLibError;
    use binance::futures::general::*;

    let general: FuturesGeneral = Binance::new(None, None);

    match general.ping().await {
        Ok(answer) => info!("Ping : {:?}", answer),
        Err(err) => {
            match err {
                BinanceLibError::BinanceError { response } => match response.code {
                    -1000_i32 => error!("An unknown error occured while processing the request"),
                    _ => error!("Uncaught code {}: {}", response.code, response.msg),
                },
                BinanceLibError::Msg(msg) => error!("Binancelib error msg: {}", msg),
                _ => error!("Other errors: {:?}.", err),
            };
        }
    }

    match general.get_server_time().await {
        Ok(answer) => info!("Server Time: {}", answer.server_time),
        Err(e) => error!("Error: {:?}", e),
    }

    match general.exchange_info().await {
        Ok(answer) => info!("Exchange information: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }

    match general.get_symbol_info("btcusdt").await {
        Ok(answer) => info!("Symbol information: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }
}

#[cfg(feature = "futures_api")]
async fn market_data() {
    use binance::api::*;
    use binance::futures::market::*;
    use binance::futures::rest_model::*;

    let market: FuturesMarket = Binance::new(None, None);

    match market.get_depth("btcusdt").await {
        Ok(answer) => info!("Depth update ID: {:?}", answer.last_update_id),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_trades("btcusdt").await {
        Ok(Trades::AllTrades(answer)) => info!("First trade: {:?}", answer[0]),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_agg_trades("btcusdt", None, None, None, 500u16).await {
        Ok(AggTrades::AllAggTrades(answer)) => info!("First aggregated trade: {:?}", answer[0]),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_klines("btcusdt", "5m", 10u16, None, None).await {
        Ok(KlineSummaries::AllKlineSummaries(answer)) => info!("First kline: {:?}", answer[0]),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_24h_price_stats("btcusdt").await {
        Ok(answer) => info!("24hr price stats: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_price("btcusdt").await {
        Ok(answer) => info!("Price: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_all_book_tickers().await {
        Ok(BookTickers::AllBookTickers(answer)) => info!("First book ticker: {:?}", answer[0]),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_book_ticker("btcusdt").await {
        Ok(answer) => info!("Book ticker: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_mark_prices(Some("btcusdt".into())).await {
        Ok(answer) => info!("First mark Prices: {:?}", answer[0]),
        Err(e) => info!("Error: {:?}", e),
    }

    match market.open_interest("btcusdt").await {
        Ok(answer) => info!("Open interest: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }

    match market.get_funding_rate("BTCUSDT", None, None, 10u16).await {
        Ok(answer) => info!("Funding: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }
}

#[cfg(feature = "futures_api")]
async fn account() {
    use binance::{api::Binance, config::Config, futures::account::FuturesAccount};
    let api_key = Some("".into());
    let secret_key = Some("".into());

    let account = FuturesAccount::new_with_config(api_key, secret_key, &Config::testnet());

    match account.account_information().await {
        Ok(answer) => info!("Account Info: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }
}

#[cfg(feature = "futures_api")]
async fn get_position_mode() {
    use binance::{api::Binance, config::Config, futures::account::FuturesAccount};
    let api_key = Some("0bSAApf6juAH3UKSutLBltxthgKnkFevzoR6noiansPmTtRUfanQAXX1mK7wGnpr".into());
    let secret_key = Some("f1ZxrSQUGYvqwO0wcVCHKvKI04QzzY8X3tCdOv9Xu9eWglkd2UUGU8ISUF59t0gC".into());

    let account = FuturesAccount::new_with_config(api_key, secret_key, &Config::testnet());

    match account.get_position_mode().await {
        Ok(answer) => info!("get_position_mode Info: {:?}", answer),
        Err(e) => error!("Error: {:?}", e),
    }
}
