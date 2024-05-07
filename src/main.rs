use amqprs::{
    callbacks::{DefaultChannelCallback, DefaultConnectionCallback},
    channel::{BasicPublishArguments, Channel, QueueBindArguments, QueueDeclareArguments},
    connection::{Connection, OpenConnectionArguments},
    BasicProperties,
};
use log::{error, info, trace};
use serde_json::Value;
use tokio;
use websocket::{stream::sync::NetworkStream, ws::dataframe::DataFrame};

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
        "kafka-cluster.default.svc.cluster.local",
        5672,
        "default_user_rDifQ_c2ZsRybOVdPaI",
        "vMsUu0H8ESh44_34lU3e_2EALfuKsRMF",
    ))
    .await
    .unwrap();
    connection
        .register_callback(DefaultConnectionCallback)
        .await
        .unwrap();

    let channel = connection.open_channel(None).await.unwrap();
    channel
        .register_callback(DefaultChannelCallback)
        .await
        .unwrap();

    let (queue_name, _, _) = channel
        .queue_declare(QueueDeclareArguments::default())
        .await
        .unwrap()
        .unwrap();

    info!("Start of Log");
    let mut connection = KrakenWebsocketConnection::new("wss://ws.kraken.com/v2".to_owned())?;
    let message = websocket::Message::text(SUB_ALL_TICKERS);
    connection.client.send_message(&message)?;
    for raw_message in connection.client.incoming_messages() {
        let parsed: serde_json::Value =
            serde_json::from_reader(raw_message?.take_payload().as_slice())?;
        trace!("Full Message: {:#?}", parsed);
        trace!("Message Channel: {:#?}", parsed["channel"]);
        match parsed["channel"].as_str().unwrap_or("") {
            "ticker" => {
                parse_tickers(
                    parsed["data"].clone(),
                    channel.clone(),
                    &queue_name,
                    "amq.topic",
                )
                .await
            }
            "heartbeat" => trace!("Heartbeat received"),
            _ => error!("Message type unknown, channel {:#?}", parsed["channel"]),
        }
    }
    Ok(())
}

async fn parse_tickers(
    data: serde_json::Value,
    channel: Channel,
    queue_name: &str,
    exchange_name: &str,
) {
    let a: &Vec<Value> = &vec![].into();
    let array = match data.as_array() {
        Some(v) => v,
        None => a,
    };
    for value in array {
        trace!("Tick entry: {:#?}", value);

        let routing_key = value["symbol"].as_str().unwrap();
        channel
            .queue_bind(QueueBindArguments::new(
                &queue_name,
                exchange_name,
                routing_key,
            ))
            .await
            .unwrap();
        let args = BasicPublishArguments::new(&exchange_name, &routing_key);

        channel
            .basic_publish(
                BasicProperties::default(),
                serde_json::to_vec(value).unwrap(),
                args,
            )
            .await
            .unwrap();
    }
}
struct KrakenWebsocketConnection {
    client: websocket::sync::Client<Box<dyn NetworkStream + Send>>,
}

impl KrakenWebsocketConnection {
    fn new(url: String) -> Result<KrakenWebsocketConnection, Box<dyn std::error::Error>> {
        let ws_url = websocket::url::Url::parse(&url)?;
        let ssl_config = websocket::native_tls::TlsConnector::new()?;
        let client: websocket::sync::Client<Box<dyn NetworkStream + Send>> =
            websocket::ClientBuilder::from_url(&ws_url.clone())
                .connect(Some(ssl_config.clone()))?;
        Ok(KrakenWebsocketConnection { client: client })
    }
}
