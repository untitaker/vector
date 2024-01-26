use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use sentry::metrics::FractionUnit;
use sentry::metrics::Metric;
use std::env;
use sysinfo::System;
use vector_lib::codecs::encoding::Framer;

use crate::{
    codecs::{Encoder, Transformer},
    event::{Event},
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
    T: Send + Sync + Unpin,
{
    async fn run(mut self: Box<Self>, mut input: BoxStream<'_, Event>) -> Result<(), ()> {
        let sentry_dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN not set");
        let _guard = sentry::init((sentry_dsn, sentry::ClientOptions {
          release: sentry::release_name!(),
          ..Default::default()
        }));

        let mut sys = System::new_all();
        while let Some(_event) = input.next().await {
            sys.refresh_memory();
            let memory = sys.used_memory() as f64 / sys.total_memory() as f64;
            Metric::gauge("memory", memory).with_unit(FractionUnit::Ratio).send();
				    println!("sent {}", memory);
        }

        Ok(())
    }
}
