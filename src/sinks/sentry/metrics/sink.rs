use async_trait::async_trait;
use bytes::BytesMut;
use futures::{stream::BoxStream, StreamExt};
use sentry::metrics::FractionUnit;
use sentry::metrics::Metric;
use std::env;
use sysinfo::System;
use tokio::{io, io::AsyncWriteExt};
use tokio_util::codec::Encoder as _;
use vector_lib::codecs::encoding::Framer;
use vector_lib::{
    internal_event::{
        ByteSize, BytesSent, CountByteSize, EventsSent, InternalEventHandle as _, Output, Protocol,
    },
    EstimatedJsonEncodedSizeOf,
};

use crate::{
    codecs::{Encoder, Transformer},
    event::{Event, EventStatus, Finalizable},
    sinks::util::StreamSink,
};

pub struct WriterSink<T> {
    pub output: T,
    pub transformer: Transformer,
    pub encoder: Encoder<Framer>,
}

#[async_trait]
impl<T> StreamSink<Event> for WriterSink<T>
where
    T: io::AsyncWrite + Send + Sync + Unpin,
{
    async fn run(mut self: Box<Self>, mut input: BoxStream<'_, Event>) -> Result<(), ()> {
        let sentry_dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN not set");
        let _guard = sentry::init((sentry_dsn, sentry::ClientOptions {
          release: sentry::release_name!(),
          ..Default::default()
        }));

        let bytes_sent = register!(BytesSent::from(Protocol("sentry_metrics".into(),)));
        let events_sent = register!(EventsSent::from(Output(None)));
        while let Some(mut event) = input.next().await {
            let event_byte_size = event.estimated_json_encoded_size_of();
            self.transformer.transform(&mut event);

            let finalizers = event.take_finalizers();
            let mut bytes = BytesMut::new();
            self.encoder.encode(event, &mut bytes).map_err(|_| {
                // Error is handled by `Encoder`.
                finalizers.update_status(EventStatus::Errored);
            })?;

            let mut sys = System::new_all();
            sys.refresh_memory();
            let memory = sys.used_memory() as f64 / sys.total_memory() as f64;
            Metric::gauge("memory", memory).with_unit(FractionUnit::Ratio).send();

            match self.output.write_all(&bytes).await {
                Err(error) => {
                    // Error when writing to stdout/stderr is likely irrecoverable,
                    // so stop the sink.
                    error!(message = "Error writing to output. Stopping sink.", %error);
                    finalizers.update_status(EventStatus::Errored);
                    return Err(());
                }
                Ok(()) => {
                    finalizers.update_status(EventStatus::Delivered);

                    events_sent.emit(CountByteSize(1, event_byte_size));
                    bytes_sent.emit(ByteSize(bytes.len()));
                }
            }
        }

        Ok(())
    }
}
