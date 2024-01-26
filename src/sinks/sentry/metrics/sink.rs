use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use sentry::metrics::FractionUnit;
use sentry::metrics::Metric;
use sysinfo::System;

use crate::{
    event::{Event},
    sinks::util::StreamSink,
};

pub struct SentryMetricsSink {
    pub dsn: Option<String>,
}

#[async_trait]
impl StreamSink<Event> for SentryMetricsSink {
    async fn run(mut self: Box<Self>, mut input: BoxStream<'_, Event>) -> Result<(), ()> {
        let dsn = self.dsn.as_deref().unwrap_or("<missing>");
        let _guard = sentry::init((dsn, sentry::ClientOptions {
          release: sentry::release_name!(),
          ..Default::default()
        }));

        let mut sys = System::new_all();
        while let Some(_event) = input.next().await {
            sys.refresh_memory();
            let memory = sys.used_memory() as f64 / sys.total_memory() as f64;
            Metric::gauge("memory", memory).with_unit(FractionUnit::Ratio).send();
				    println!("sent {} to '{}'", memory, dsn);
        }

        Ok(())
    }
}
