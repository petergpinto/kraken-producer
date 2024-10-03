use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{BasicPublishArguments, Channel},
    connection::{Connection, OpenConnectionArguments},
    BasicProperties, DELIVERY_MODE_PERSISTENT,
};
use log::{debug, error, info, trace};
use serde_json::Value;
use websocket::{stream::sync::NetworkStream, ws::dataframe::DataFrame};
use chrono::{DateTime, Utc};

static SUB_ALL_TICKERS: &str = "{
    \"method\": \"subscribe\",
    \"params\": {
        \"channel\": \"ticker\",
        \"symbol\": [
            \"*\"
        ]
    }
}";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let connection = Connection::open(&OpenConnectionArguments::new(
        "broker-rabbitmq.rabbitmq.svc.cluster.local",
        5672,
        "rust_producer",
        "vMsUu0H8ESh44_34lU3e_2EALfuKsRMF",
    ))
    .await?;
    connection
        .register_callback(DefaultConnectionCallback)
        .await?;

    let channel = connection.open_channel(None).await?;
    channel.register_callback(DefaultChannelCallback).await?;

    info!("Start of Log");
    let mut connection = KrakenWebsocketConnection::new("wss://ws.kraken.com/v2".to_owned())?;
    let message = websocket::Message::text(SUB_ALL_TICKERS);
    connection.client.send_message(&message)?;
    for raw_message in connection.client.incoming_messages() {
        let recv_time = Utc::now();
        let mut parsed: serde_json::Value =
            serde_json::from_reader(raw_message?.take_payload().as_slice())?;
        parsed["recv_time"] = serde_json::Value::String(recv_time.to_rfc3339());
        trace!("Full Message: {:#?}", parsed);
        trace!("Message Channel: {:#?}", parsed["channel"]);
        match parsed["channel"].as_str().unwrap_or("") {
            "ticker" => parse_tickers(parsed["data"].clone(), channel.clone(), "market", recv_time).await?,
            "heartbeat" => trace!("Heartbeat received"),
            "status" => info!("Status: {:#?}", parsed),
            _ => error!("Message type unknown, channel {:#?}", parsed["channel"]),
        }
    }
    Ok(())
}

async fn parse_tickers(
    data: serde_json::Value,
    channel: Channel,
    exchange_name: &str,
    timestamp: DateTime<Utc>
) -> Result<(), Box<dyn std::error::Error>> {
    let a: &Vec<Value> = &vec![];
    let array = match data.as_array() {
        Some(v) => v,
        None => a,
    };

    for value in array {
        let mut record = value.clone();
        record["recv_time"] = serde_json::Value::String(timestamp.to_rfc3339());
        trace!("Tick entry: {:#?}", value);

        let symbol = record["symbol"]
            .as_str()
            .ok_or("Unable to convert symbol to string")?;
        let routing_key = "market.crypto.".to_owned() + symbol;

        let args = BasicPublishArguments::new(exchange_name, &routing_key);

        debug!("{routing_key}: {:#?}", args);
        channel
            .basic_publish(
                BasicProperties::default()
                    .with_delivery_mode(DELIVERY_MODE_PERSISTENT)
                    .finish(),
                serde_json::to_vec(&record)?,
                args,
            )
            .await?;
    }
    info!("Produced {} messages to broker", array.len());
    Ok(())
}
struct KrakenWebsocketConnection {
    client: websocket::sync::Client<Box<dyn NetworkStream + Send>>,
}

impl KrakenWebsocketConnection {
    fn new(url: String) -> Result<KrakenWebsocketConnection, Box<dyn std::error::Error>> {
        let ws_url = websocket::url::Url::parse(&url)?;
        let ssl_config = websocket::native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;
        let client: websocket::sync::Client<Box<dyn NetworkStream + Send>> =
            websocket::ClientBuilder::from_url(&ws_url.clone())
                .connect(Some(ssl_config.clone()))?;
        Ok(KrakenWebsocketConnection { client })
    }
}
