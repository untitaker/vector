use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use futures_util::future::ready;
use sentry::metrics::Metric;

use crate::{
    event::{Event, MetricValue},
    sinks::util::StreamSink,
};

pub struct SentryMetricsSink {
    pub dsn: String,
}

#[async_trait]
impl StreamSink<Event> for SentryMetricsSink {
    async fn run(mut self: Box<Self>, input: BoxStream<'_, Event>) -> Result<(), ()> {
        let mut input = input
            // filter out any non-metric events
            .filter_map(|event| ready(event.try_into_metric()));

        let _guard = sentry::init(self.dsn);

        while let Some(metric) = input.next().await {
            let name = metric.series().name().name.clone();
            match metric.data().value() {
                MetricValue::Counter { value } => {
                    Metric::incr(name.clone(), *value).send();
                }
                MetricValue::Gauge { value } => {
                    Metric::gauge(name.clone(), *value).send();
                }
                MetricValue::Set { values } => {
                    for value in values {
                        // XXX: why not submit the entire set at once?
                        Metric::set(name.clone(), &value).send();
                    }
                }
                MetricValue::Distribution { samples, .. } => {
                    for sample in samples {
                        for _ in 0..sample.rate {
                            // XXX: sentry should allow me to submit value + count
                            Metric::distribution(name.clone(), sample.value).send();
                        }
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }
}
