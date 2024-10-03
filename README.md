Need to timestamp incoming messages


Health checks for state of ingest?
Maybe use different amqp https://github.com/amqp-rs/lapin

Prefer internal service mesh for amqp, amqps for external.  Might disable non-tls on external


Durable queues do not persist messages between broker restarts
Messages are published with a "Delivery mode" that can be persistent or non-persistent
"persistent message has no effect inside a non-durable queue" -> If a queue is durable and messages are being published as persistent, they will survive restarts
