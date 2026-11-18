use std::{
    collections::BTreeSet,
    env,
    error::Error,
    net::SocketAddr,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::lookup_host,
    sync::mpsc,
    time::{MissedTickBehavior, interval, timeout},
};
use tonic::transport::{Channel, Endpoint, channel::Change};

use crate::wire::{ReadRequest, Reading, WatchRequest, probe_client::ProbeClient};

type Changes = mpsc::Sender<Change<SocketAddr, Endpoint>>;
type Client = ProbeClient<Channel>;

const LOOKUP_LIMIT: Duration = Duration::from_secs(1);
const CALL_LIMIT: Duration = Duration::from_secs(2);

fn ms(epoch: Instant) -> u64 {
    epoch.elapsed().as_millis().try_into().unwrap()
}

fn emit(epoch: Instant, event: &str, data: Value) {
    println!(
        "{}",
        json!({"at_ms": ms(epoch), "event": event, "data": data})
    );
}

fn endpoint(address: SocketAddr) -> Endpoint {
    Endpoint::from_shared(format!("http://{address}"))
        .expect("valid numeric socket address")
        .connect_timeout(Duration::from_secs(1))
        .http2_keep_alive_interval(Duration::from_secs(2))
        .keep_alive_timeout(Duration::from_secs(2))
        .keep_alive_while_idle(true)
}

async fn reconcile(
    previous: &BTreeSet<SocketAddr>,
    current: &BTreeSet<SocketAddr>,
    changes: &Changes,
) -> Result<(), ()> {
    for &address in current.difference(previous) {
        changes
            .send(Change::Insert(address, endpoint(address)))
            .await
            .map_err(|_| ())?;
    }
    for &address in previous.difference(current) {
        changes
            .send(Change::Remove(address))
            .await
            .map_err(|_| ())?;
    }
    Ok(())
}

async fn discover(host: String, port: u16, changes: Changes, epoch: Instant) {
    let mut timer = interval(Duration::from_secs(1));
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut previous = BTreeSet::new();
    loop {
        timer.tick().await;
        let addresses = match timeout(LOOKUP_LIMIT, lookup_host((host.as_str(), port))).await {
            Ok(Ok(addresses)) => addresses.filter(SocketAddr::is_ipv4).collect(),
            Ok(Err(error)) => {
                emit(epoch, "dns_error", json!(error.to_string()));
                continue;
            }
            Err(_) => {
                emit(epoch, "dns_timeout", Value::Null);
                continue;
            }
        };
        if addresses != previous {
            if reconcile(&previous, &addresses, &changes).await.is_err() {
                return;
            }
            emit(
                epoch,
                "addresses",
                json!({
                    "current": addresses,
                    "added": addresses.difference(&previous).collect::<Vec<_>>(),
                    "removed": previous.difference(&addresses).collect::<Vec<_>>(),
                }),
            );
            previous = addresses;
        }
    }
}

fn reading(reply: Reading) -> Value {
    json!({
        "request_id": reply.request_id,
        "pod": reply.pod,
        "uid": reply.uid,
        "revision": reply.revision,
        "value": reply.value,
        "seq": reply.seq,
    })
}

async fn read(
    mut client: Client,
    epoch: Instant,
    request_id: String,
    index: Option<u64>,
    hold: bool,
    start_ms: u64,
) {
    let request = ReadRequest {
        request_id: request_id.clone(),
        hold_before_reply: hold,
    };
    // A held request ends through the transport. The runner enforces the
    // separate observation limit for the failure injection.
    let result = if hold {
        Ok(client.read(request).await)
    } else {
        timeout(CALL_LIMIT, client.read(request)).await
    };
    let outcome = match result {
        Ok(Ok(reply)) => json!({"reply": reading(reply.into_inner())}),
        Ok(Err(status)) => {
            json!({"code": format!("{:?}", status.code()), "message": status.message()})
        }
        Err(_) => json!({"local_timeout": true}),
    };
    emit(
        epoch,
        "read",
        json!({
            "request_id": request_id, "index": index, "hold": hold, "start_ms": start_ms,
            "outcome": outcome,
        }),
    );
}

async fn watch(mut client: Client, epoch: Instant, request_id: String, after_seq: u64) {
    emit(
        epoch,
        "watch_start",
        json!({"request_id": request_id, "after_seq": after_seq}),
    );
    let request = WatchRequest {
        request_id: request_id.clone(),
        after_seq,
    };
    let mut stream = match timeout(CALL_LIMIT, client.watch(request)).await {
        Ok(Ok(response)) => response.into_inner(),
        Ok(Err(status)) => {
            emit(
                epoch,
                "watch_open_error",
                json!({"request_id": request_id, "code": format!("{:?}", status.code())}),
            );
            return;
        }
        Err(_) => {
            emit(
                epoch,
                "watch_open_error",
                json!({"request_id": request_id, "local_timeout": true}),
            );
            return;
        }
    };
    loop {
        match stream.message().await {
            Ok(Some(reply)) => emit(epoch, "watch_item", reading(reply)),
            Ok(None) => {
                emit(
                    epoch,
                    "watch_end",
                    json!({"request_id": request_id, "eof": true}),
                );
                break;
            }
            Err(status) => {
                emit(
                    epoch,
                    "watch_end",
                    json!({
                        "request_id": request_id,
                        "code": format!("{:?}", status.code()),
                        "message": status.message(),
                    }),
                );
                break;
            }
        }
    }
}

pub async fn run() -> Result<(), Box<dyn Error>> {
    let host = env::var("PROBE_SERVICE")?;
    let port = env::var("PROBE_PORT")
        .unwrap_or_else(|_| "50051".to_owned())
        .parse()?;
    let epoch = Instant::now();
    let (channel, changes) = Channel::balance_channel(32);
    let client = ProbeClient::new(channel);
    let discovery = tokio::spawn(discover(host, port, changes, epoch));
    let mut commands = BufReader::new(tokio::io::stdin()).lines();
    let mut ticks = interval(Duration::from_millis(100));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut index = 0_u64;
    let mut stream: Option<tokio::task::JoinHandle<()>> = None;
    emit(epoch, "started", Value::Null);
    loop {
        tokio::select! {
            _ = ticks.tick() => {
                let start_ms = ms(epoch);
                let id = format!("sample-{index}");
                tokio::spawn(read(client.clone(), epoch, id, Some(index), false, start_ms));
                index += 1;
            }
            line = commands.next_line() => {
                let Some(line) = line? else { break };
                let parts: Vec<_> = line.split_whitespace().collect();
                match parts.as_slice() {
                    ["mark", name] => emit(epoch, "mark", json!({"name": name, "next_index": index})),
                    ["read", id] | ["hold", id] => {
                        tokio::spawn(read(client.clone(), epoch, (*id).to_owned(), None, parts[0] == "hold", ms(epoch)));
                    }
                    ["watch", id, seq] if stream.as_ref().is_none_or(|task| task.is_finished()) => {
                        stream = Some(tokio::spawn(watch(client.clone(), epoch, (*id).to_owned(), seq.parse()?)));
                    }
                    ["close-watch"] => {
                        if let Some(task) = stream.take() {
                            task.abort();
                            let _ = task.await;
                        }
                    }
                    ["quit"] => break,
                    _ => return Err(format!("invalid fixture command: {line}").into()),
                }
            }
        }
    }
    discovery.abort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::{net::TcpListener, task::JoinHandle};
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::Server;

    use super::*;
    use crate::{server::Fixture, wire::probe_server::ProbeServer};

    async fn backend(name: &str) -> (SocketAddr, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fixture = Fixture::new(name.to_owned(), name.to_owned(), "loopback".to_owned());
        let task = tokio::spawn(async {
            Server::builder()
                .add_service(ProbeServer::new(fixture))
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        (address, task)
    }

    async fn observed(client: &mut Client, count: usize) -> BTreeSet<String> {
        let mut pods = BTreeSet::new();
        for index in 0..count {
            let id = format!("local-{index}");
            let response = timeout(
                CALL_LIMIT,
                client.read(ReadRequest {
                    request_id: id.clone(),
                    hold_before_reply: false,
                }),
            )
            .await
            .unwrap()
            .unwrap()
            .into_inner();
            assert_eq!(response.request_id, id);
            pods.insert(response.uid);
        }
        pods
    }

    #[tokio::test]
    async fn changed_addresses_update_the_reused_channel() {
        let (a, a_task) = backend("a").await;
        let (b, b_task) = backend("b").await;
        let (c, c_task) = backend("c").await;
        let (channel, changes) = Channel::balance_channel(32);
        let mut client = ProbeClient::new(channel);

        let two = BTreeSet::from([a, b]);
        reconcile(&BTreeSet::new(), &two, &changes).await.unwrap();
        let names = |a: &str, b: &str| BTreeSet::from([a.to_owned(), b.to_owned()]);
        assert_eq!(observed(&mut client, 150).await, names("a", "b"));

        // Keep a alive so requests to an accidentally retained address can still succeed.
        let changed = BTreeSet::from([b, c]);
        reconcile(&two, &changed, &changes).await.unwrap();
        assert_eq!(observed(&mut client, 150).await, names("b", "c"));
        for task in [a_task, b_task, c_task] {
            task.abort();
        }
    }
}
