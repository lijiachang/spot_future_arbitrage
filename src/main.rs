#[macro_use]
extern crate tokio;

use std::sync::atomic::AtomicBool;

use binance::api::*;
use binance::websockets::*;
use binance::ws_model::{CombinedStreamEvent, WebsocketEvent, WebsocketEventUntag};
use futures::future::BoxFuture;
use futures::stream::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

#[tokio::main]
async fn main() {
    let (logger_tx, mut logger_rx) = tokio::sync::mpsc::unbounded_channel::<WebsocketEvent>();
    let (close_tx, mut close_rx) = tokio::sync::mpsc::unbounded_channel::<bool>();

    let wait_loop = tokio::spawn(async move {
        'hello: loop {
            select! {
                event = logger_rx.recv() => println!("{event:?}"),
                _ = close_rx.recv() => break 'hello
            }
        }
    });

    let streams: Vec<BoxFuture<'static, ()>> = vec![
        Box::pin(combined_orderbook(logger_tx.clone())),
        // Box::pin(custom_event_loop(logger_tx.clone())),
    ];

    for stream in streams {
        tokio::spawn(stream);
    }

    select! {
        _ = wait_loop => { println!("Finished!") }
        _ = tokio::signal::ctrl_c() => {
            println!("Closing websocket stream...");
            close_tx.send(true).unwrap();
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}


#[allow(dead_code)]
async fn combined_orderbook(logger_tx: UnboundedSender<WebsocketEvent>) {
    let keep_running = AtomicBool::new(true);

    let streams: Vec<String> = vec!["btcusdt", "ethusdt"]
        .into_iter()
        .map(|symbol| partial_book_depth_stream(symbol, 5, 1000))
        .collect();

    let mut web_socket: WebSockets<'_, CombinedStreamEvent<_>> =
        WebSockets::new(|event: CombinedStreamEvent<WebsocketEventUntag>| {
            if let WebsocketEventUntag::WebsocketEvent(we) = &event.data {
                logger_tx.send(we.clone()).unwrap();
            }
            let data = event.data;
            if let WebsocketEventUntag::Orderbook(orderbook) = data {
                println!("{orderbook:?}")
            }
            Ok(())
        });

    web_socket.connect_multiple(streams).await.unwrap(); // check error
    if let Err(e) = web_socket.event_loop(&keep_running).await {
        println!("Error: {e}");
    }
    web_socket.disconnect().await.unwrap();
    println!("disconnected");
}
