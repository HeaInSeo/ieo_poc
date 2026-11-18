use std::{env, error::Error, future::pending, pin::Pin, time::Duration};

use serde_json::json;
use tokio::{signal::unix, time::interval};
use tokio_stream::{Stream, StreamExt, wrappers::IntervalStream};
use tonic::{Request, Response, Status, transport::Server};

use crate::wire::{
    ReadRequest, Reading, WatchRequest,
    probe_server::{Probe, ProbeServer},
};

#[derive(Clone)]
pub(crate) struct Fixture {
    pod: String,
    uid: String,
    revision: String,
}

impl Fixture {
    pub(crate) fn new(pod: String, uid: String, revision: String) -> Self {
        Self { pod, uid, revision }
    }

    fn reading(&self, request_id: String, seq: u64) -> Reading {
        Reading {
            request_id,
            pod: self.pod.clone(),
            uid: self.uid.clone(),
            revision: self.revision.clone(),
            value: "x05-fixture-value".to_owned(),
            seq,
        }
    }
}

#[tonic::async_trait]
impl Probe for Fixture {
    async fn read(&self, request: Request<ReadRequest>) -> Result<Response<Reading>, Status> {
        let ReadRequest {
            request_id,
            hold_before_reply,
        } = request.into_inner();
        if hold_before_reply || request_id.starts_with("reissue-") {
            println!(
                "{}",
                json!({
                    "event": "read_started",
                    "request_id": request_id,
                    "uid": self.uid,
                    "hold": hold_before_reply,
                })
            );
        }
        if hold_before_reply {
            pending::<()>().await;
        }
        Ok(Response::new(self.reading(request_id, 0)))
    }

    type WatchStream = Pin<Box<dyn Stream<Item = Result<Reading, Status>> + Send>>;

    async fn watch(
        &self,
        request: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let WatchRequest {
            request_id,
            after_seq,
        } = request.into_inner();
        let mut sequences = after_seq
            .checked_add(1)
            .ok_or_else(|| Status::out_of_range("after_seq"))?
            ..=u64::MAX;
        let fixture = self.clone();
        let stream =
            IntervalStream::new(interval(Duration::from_millis(250))).map_while(move |_| {
                sequences
                    .next()
                    .map(|seq| Ok(fixture.reading(request_id.clone(), seq)))
            });
        Ok(Response::new(Box::pin(stream)))
    }
}

pub async fn serve() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new(
        env::var("POD_NAME")?,
        env::var("POD_UID")?,
        env::var("PROBE_REVISION")?,
    );
    let addr = env::var("PROBE_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_owned())
        .parse()?;
    let mut sigterm = unix::signal(unix::SignalKind::terminate())?;
    Server::builder()
        .add_service(ProbeServer::new(fixture))
        .serve_with_shutdown(addr, async move {
            sigterm.recv().await;
        })
        .await?;
    Ok(())
}
