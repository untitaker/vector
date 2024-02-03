use async_trait::async_trait;
use futures::{stream::BoxStream, StreamExt};
use futures_util::future::ready;
use sentry::metrics::{Metric, MetricBuilder};
use std::time::{Duration, UNIX_EPOCH};

use crate::{
    event::{Event, Metric as SourceMetric, MetricValue},
    sinks::util::StreamSink,
};

pub struct SentryMetricsSink {
    pub dsn: String,
}

fn finish_metric(source: &SourceMetric, mut sentry_metric: MetricBuilder) {
    if let Some(tags) = source.series().tags() {
        for (k, v) in tags.iter_all() {
            if let Some(v) = v {
                sentry_metric = sentry_metric.with_tag(k.to_owned(), v.to_owned());
            }
        }
    }

    if let Some(time) = source.data().time.timestamp {
        if let Ok(time) = u64::try_from(time.timestamp()) {
            let system_time = UNIX_EPOCH + Duration::from_secs(time);
            // XXX: why does the SDK need a SystemTime here?
            sentry_metric = sentry_metric.with_time(system_time);
        }
    }

    sentry_metric.send();
}

#[async_trait]
impl StreamSink<Event> for SentryMetricsSink {
    async fn run(mut self: Box<Self>, input: BoxStream<'_, Event>) -> Result<(), ()> {
        let mut input = input
            // filter out any non-metric events
            .filter_map(|event| ready(event.try_into_metric()));

        // https://github.com/getsentry/sentry-rust/issues/637
        let _guard = sentry::init(self.dsn);

        while let Some(metric) = input.next().await {
            let series = metric.series();
            let series_name = series.name();
            let mut name = series_name.namespace.clone().unwrap_or_default();
            if !name.is_empty() {
                name.push('.');
            }

            name.push_str(&series_name.name);

            match metric.data().value() {
                MetricValue::Counter { value } => {
                    finish_metric(&metric, Metric::incr(name, *value));
                }
                MetricValue::Gauge { value } => {
                    finish_metric(&metric, Metric::gauge(name, *value));
                }
                MetricValue::Set { values } => {
                    for value in values {
                        // XXX: why not submit the entire set at once?
                        finish_metric(&metric, Metric::set(name.clone(), &value));
                    }
                }
                MetricValue::Distribution { samples, .. } => {
                    for sample in samples {
                        for _ in 0..sample.rate {
                            // XXX: sentry should allow me to submit value + count
                            finish_metric(
                                &metric,
                                Metric::distribution(name.clone(), sample.value),
                            );
                        }
                    }
                }
                _ => (),
            }
        }

        Ok(())
    }
}
