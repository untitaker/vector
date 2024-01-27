use vector_lib::configurable::configurable_component;
use futures::{future, FutureExt};
use vector_lib::codecs::{
    encoding::{FramingConfig},
    JsonSerializerConfig,
};

use crate::{
    codecs::{EncodingConfigWithFraming},
    config::{AcknowledgementsConfig, GenerateConfig, Input, SinkConfig, SinkContext},
    sinks::{sentry::metrics::sink::SentryMetricsSink, Healthcheck, VectorSink},
};

/// Configuration for the `sentry_metrics` sink.
#[configurable_component(sink(
    "sentry_metrics",
    "Send metrics to Sentry."
))]
#[derive(Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SentrySinkConfig {
    #[configurable(derived)]
    pub dsn: String,

    #[serde(flatten)]
    pub encoding: EncodingConfigWithFraming,

    #[configurable(derived)]
    #[serde(
        default,
        deserialize_with = "crate::serde::bool_or_struct",
        skip_serializing_if = "crate::serde::is_default"
    )]
    pub acknowledgements: AcknowledgementsConfig,
}

impl GenerateConfig for SentrySinkConfig {
    fn generate_config() -> toml::Value {
        toml::Value::try_from(Self {
            dsn: String::new(),
            encoding: (None::<FramingConfig>, JsonSerializerConfig::default()).into(),
            acknowledgements: Default::default(),
        })
        .unwrap()
    }
}

#[async_trait::async_trait]
#[typetag::serde(name = "sentry_metrics")]
impl SinkConfig for SentrySinkConfig {
    async fn build(&self, _cx: SinkContext) -> crate::Result<(VectorSink, Healthcheck)> {
        let dsn = self.dsn.clone();
        let sink: VectorSink = VectorSink::from_event_streamsink(SentryMetricsSink {dsn});

        Ok((sink, future::ok(()).boxed()))
    }

    fn input(&self) -> Input {
        Input::new(self.encoding.config().1.input_type())
    }

    fn acknowledgements(&self) -> &AcknowledgementsConfig {
        &self.acknowledgements
    }
}
