use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use sentry::metrics::Metric;

use crate::{
    event::{Event, MetricValue},
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

        while let Some(event) = input.next().await {
            let metric = event.as_metric();
            let name = metric.series().name().name.clone();
            match metric.data().value() {
                MetricValue::Counter { .. } => {
                    Metric::count(name).send();
                },
                MetricValue::Gauge { value } => {
                    Metric::gauge(name, *value).send();
                },
                _ => {
                }
            }
        }

        Ok(())
    }
}

