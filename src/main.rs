use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

use binance::coin_futures::websockets::WebSockets as CoinFuturesWebSockets;
use binance::futures::rest_model::CoinFutureContractType;
use binance::websockets::{partial_book_depth_stream, WebSockets};
use binance::ws_model::{CombinedStreamEvent, WebsocketEvent, WebsocketEventUntag};
use chrono::{Local, NaiveDate};
use env_logger::Builder;
use futures::future::BoxFuture;
use log::{error, info, warn};
use tokio::select;
use tokio::sync::mpsc::UnboundedSender;
use tracing::log::LevelFilter;

#[derive(Debug, Clone)]
pub struct ArbitrageRate {
    pub currency: String,  // 币种
    pub spot_instrument_name: String, // 现货交易对
    pub future_instrument_name: String, // 期货交易对
    pub basis: f64, // 基差（合约价格 - 现货价格）
    pub spread_rate: f64, // 价差率
    pub apy: f64, // 收益率（年化）
    pub future_expire_days: i64, // 合约交割剩余天数
    pub timestamp: u64, // 数据时间
}

#[tokio::main]
async fn main() {
    Builder::new().filter_level(LevelFilter::Info).parse_default_env().init();

    spot_future_spider().await;
}


#[allow(dead_code)]
async fn combined_orderbook(symbols: HashSet<String>, cache: Arc<Mutex<HashMap<String, WebsocketEventUntag>>>) {
    let is_spot: bool = symbols.iter().any(|s| s.ends_with("usdt"));

    let keep_running = AtomicBool::new(true);
    let streams: Vec<String> = symbols
        .into_iter()
        .map(|symbol| partial_book_depth_stream(&*symbol, 5, 100))
        .collect();

    let event_handler = |event: CombinedStreamEvent<WebsocketEventUntag>| {
        let mut cache = cache.lock().unwrap();
        if let WebsocketEventUntag::WebsocketEvent(we) = &event.data {
            // println!("{:?} WebsocketEvent: {we:?}", &event.stream);
            let symbol = event.stream.split('@').next().unwrap();
            cache.insert(symbol.parse().unwrap(), WebsocketEventUntag::WebsocketEvent(we.clone()));
        } else if let WebsocketEventUntag::Orderbook(orderbook) = &event.data {
            // println!("{:?} Orderbook: {orderbook:?}", &event.stream);
            let symbol = event.stream.split('@').next().unwrap();
            cache.insert(symbol.parse().unwrap(), WebsocketEventUntag::Orderbook(orderbook.clone()));
        }
        Ok(())
    };

    enum EitherWebSocket<'a> {
        Spot(WebSockets<'a, CombinedStreamEvent<WebsocketEventUntag>>),
        CoinFutures(CoinFuturesWebSockets<'a, CombinedStreamEvent<WebsocketEventUntag>>),
    }

    // websocket可能是现货或币本位合约
    let web_socket = if is_spot {
        EitherWebSocket::Spot(WebSockets::new(event_handler))
    } else {
        EitherWebSocket::CoinFutures(CoinFuturesWebSockets::new(event_handler))
    };


    match web_socket {
        EitherWebSocket::Spot(mut ws) => {
            if let Err(e) = ws.connect_multiple(streams).await {
                error!("Error connecting WebSocket: {e}");
                return;
            }
            if let Err(e) = ws.event_loop(&keep_running).await {
                error!("Error: {e}");
            }
            ws.disconnect().await.unwrap();
        }
        EitherWebSocket::CoinFutures(mut ws) => {
            ws.connect_multiple(streams).await.unwrap();
            if let Err(e) = ws.event_loop(&keep_running).await {
                error!("Error: {e}");
            }
            ws.disconnect().await.unwrap();
        }
    }
}

/// 计算套利收益率排名
#[allow(dead_code)]
async fn calc_apy(spot_cache: Arc<Mutex<HashMap<String, WebsocketEventUntag>>>, delivery_cache: Arc<Mutex<HashMap<String, WebsocketEventUntag>>>) {
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let mut arbitrage_rates: Vec<ArbitrageRate> = vec![];

        // let delivery_cache = delivery_cache.lock().unwrap();
        // 优化：获得锁，然后clone一份数据，行代码结束然后释放锁
        // .clone() 克隆了 guard 引用的数据(在这种情况下是 HashMap),但不克隆 guard 本身。
        // 克隆的 HashMap 被分配给一个新的变量,也叫 delivery_cache。
        // 原始的 MutexGuard 没有被分配给任何变量,所以它立即离开作用域。
        // 当 MutexGuard 离开作用域时,它的 Drop 实现被调用,这会释放互斥锁。

        let delivery_cache = delivery_cache.lock().unwrap().clone();
        for (symbol, we) in delivery_cache.iter() {
            // println!("get_cache: {symbol}: {we:?}");
            //linkusd_240628
            let (base, date_str) = symbol.split_once('_').unwrap();
            let (currency, _) = base.split_once("usd").unwrap();
            let spot_instrument_name = format!("{currency}usdt");

            // 解析日期字符串
            // dbg!(&date_str);
            let mut future_expire_days = 0;
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%y%m%d") {
                // 获取今天的日期
                let today = Local::today().naive_local();
                // 计算日期差
                future_expire_days = date.signed_duration_since(today).num_days() - 1;
            } else {
                warn!("invalid date string: {date_str}");
                continue;
            }
            let mut event_time = 0;
            // 现货 卖一 价格
            // 合约 买一 价格
            let mut future_price: f64 = 0.0;
            let mut spot_price: f64 = 0.0;
            if let WebsocketEventUntag::WebsocketEvent(WebsocketEvent::DepthOrderBook(orderbook)) = we {
                event_time = orderbook.event_time;
                if let Some(best_bid) = orderbook.bids.first() {
                    future_price = best_bid.price;
                    // let spot_cache = spot_cache.lock().unwrap();
                    let spot_orderbook = spot_cache.lock().unwrap().get(&spot_instrument_name).unwrap().clone();
                    if let WebsocketEventUntag::Orderbook(orderbook) = spot_orderbook {
                        if let Some(best_bid) = orderbook.asks.first() {
                            spot_price = best_bid.price;
                        } else {
                            warn!("No best bid found for {spot_instrument_name}");
                            continue;
                        }
                    } else {
                        warn!("unknown data type for {spot_orderbook:?}");
                        continue;
                    }
                } else {
                    warn!("No best bid found for {symbol}");
                    continue;
                }
            } else {
                warn!("unknown data type for {we:?}");
                continue;
            }

            // 计算收益率
            let basis = future_price - spot_price;
            let spread_rate = basis / spot_price * 100.0;
            let apy = spread_rate / future_expire_days as f64 * 365.0;

            let arb_rate = ArbitrageRate {
                currency: currency.to_string(),
                spot_instrument_name,
                future_instrument_name: symbol.clone(),
                basis,
                spread_rate,
                apy,
                future_expire_days,
                timestamp: event_time,
            };
            // dbg!(arb_rate);
            arbitrage_rates.push(arb_rate);
            // break;
        }

        // 根据收益率排序
        arbitrage_rates.sort_by(|a, b| b.apy.partial_cmp(&a.apy).unwrap());
        // 打印前10
        println!("------------Top 10 Arbitrage Rates:-------------");
        for rate in arbitrage_rates.iter().take(10) {
            println!("{:?}", rate);
        }
    }
}

async fn spot_future_spider() {
    use binance::api::*;
    use binance::errors::Error as BinanceLibError;
    use binance::coin_futures::general::*;

    let general: FuturesGeneral = Binance::new(None, None);

    // 找出所有拥有交割合约的币种
    match general.exchange_info().await {
        Ok(answer) => {
            let mut symbols = HashSet::new();
            let mut delivery_symbols = HashSet::new();
            for item in answer.symbols {
                if item.contract_type == CoinFutureContractType::CurrentQuarter || item.contract_type == CoinFutureContractType::NextQuarter {
                    symbols.insert(item.base_asset);
                    delivery_symbols.insert(item.symbol.to_lowercase());
                }
            }
            let spot_symbols = symbols.into_iter().map(|s| format!("{s}USDT").to_lowercase()).collect::<HashSet<String>>();
            info!("All spot_symbols set: {:?}", spot_symbols);
            info!("All delivery_symbols set: {:?}", delivery_symbols);

            let spot_cache = Arc::new(Mutex::new(HashMap::new()));
            let delivery_cache = Arc::new(Mutex::new(HashMap::new()));

            let writer_spot_cache = Arc::clone(&spot_cache);
            let writer_delivery_cache = Arc::clone(&delivery_cache);
            let reader_spot_cache = Arc::clone(&spot_cache);
            let reader_delivery_cache = Arc::clone(&delivery_cache);

            let streams: Vec<BoxFuture<'static, ()>> = vec![
                Box::pin(combined_orderbook(spot_symbols, writer_spot_cache)),
                Box::pin(combined_orderbook(delivery_symbols, writer_delivery_cache)),
                Box::pin(calc_apy(reader_spot_cache, reader_delivery_cache)),
            ];

            let mut tasks: Vec<_> = vec![];

            for stream in streams {
                tasks.push(tokio::spawn(stream));
            }
            for task in tasks {
                task.await.unwrap();
            }
        }
        Err(e) => error!("Error: {:?}", e),
    }
}

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

