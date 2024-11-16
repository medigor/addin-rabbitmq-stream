use addin1c::{name, AddinResult, MethodInfo, Methods, PropInfo, SimpleAddin, Variant};
use chrono::{DateTime, Utc};
use rabbitmq_stream_client::{
    types::{Message, ResponseCode, SimpleValue},
    Dedup, NoDedup, Producer,
};
use std::{collections::HashMap, error::Error, mem, time::Duration};
use tokio::runtime::Runtime;

use crate::{environment_builder, environment_impl};

#[derive(Default)]
struct ProducerBuilder {
    pub name: Option<String>,
    pub batch_delay: Option<Duration>,
    pub batch_size: Option<usize>,
}

enum ProducerWrapper {
    ProducerDedup(Producer<Dedup>),
    ProducerNoDedup(Producer<NoDedup>),
    Unknown,
}

impl ProducerWrapper {
    fn batch_send(
        &mut self,
        messages: Vec<Message>,
        runtime: &Runtime,
    ) -> Result<Vec<ResponseCode>, Box<dyn Error>> {
        let mut result = match self {
            ProducerWrapper::ProducerDedup(producer) => {
                runtime.block_on(producer.batch_send_with_confirm(messages))?
            }
            ProducerWrapper::ProducerNoDedup(producer) => {
                runtime.block_on(producer.batch_send_with_confirm(messages))?
            }
            ProducerWrapper::Unknown => return Err("No producer".into()),
        };
        result.sort_by_key(|x| x.publishing_id());

        Ok(result
            .into_iter()
            .map(|x| x.status().clone())
            .collect::<Vec<_>>())
    }
}

pub struct AddinProducer {
    environment_builder: environment_builder::Builder,
    producer_builder: Option<Box<ProducerBuilder>>,
    runtime: Runtime,
    producer: ProducerWrapper,
    messages: Vec<Message>,
    application_properties: HashMap<String, SimpleValue>,
    statuses: Vec<ResponseCode>,
    last_error: Option<Box<dyn Error>>,
}

impl AddinProducer {
    pub fn new() -> Self {
        Self {
            environment_builder: environment_builder::Builder::new(),
            producer_builder: Some(Box::new(ProducerBuilder::default())),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime"),
            producer: ProducerWrapper::Unknown,
            messages: Vec::new(),
            application_properties: HashMap::new(),
            statuses: Vec::new(),
            last_error: None,
        }
    }

    fn last_error(&mut self, value: &mut Variant) -> AddinResult {
        match &self.last_error {
            Some(err) => value
                .set_str1c(err.to_string().as_str())
                .map_err(|e| e.into()),
            None => value.set_str1c("").map_err(|e| e.into()),
        }
    }

    fn set_name(&mut self, name: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let name = name.get_string()?;
        if let Some(builder) = self.producer_builder.as_mut() {
            builder.name = Some(name);
        };
        Ok(())
    }

    fn set_batch_delay(
        &mut self,
        batch_delay: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let batch_delay = batch_delay.get_i32()? as u64;
        if let Some(builder) = self.producer_builder.as_mut() {
            builder.batch_delay = Some(Duration::from_millis(batch_delay));
        }
        Ok(())
    }

    fn set_batch_size(
        &mut self,
        batch_size: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let name = batch_size.get_i32()? as usize;
        if let Some(builder) = self.producer_builder.as_mut() {
            builder.batch_size = Some(name);
        };
        Ok(())
    }

    fn build(&mut self, stream: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let stream = stream.get_string()?;
        let environment = self.runtime.block_on(self.environment_builder.build())?;

        let producer_properties = *self
            .producer_builder
            .take()
            .ok_or("ProducerBuilder not exists")?;

        let mut producer_builder = environment.producer();
        if let Some(delay) = producer_properties.batch_delay {
            producer_builder = producer_builder.batch_delay(delay);
        }
        if let Some(size) = producer_properties.batch_size {
            producer_builder = producer_builder.batch_size(size);
        }

        self.producer = if let Some(name) = producer_properties.name {
            let producer = self
                .runtime
                .block_on(producer_builder.name(&name).build(&stream))?;
            ProducerWrapper::ProducerDedup(producer)
        } else {
            let producer = self.runtime.block_on(producer_builder.build(&stream))?;
            ProducerWrapper::ProducerNoDedup(producer)
        };

        Ok(())
    }

    fn add_message(&mut self, data: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let data = data.get_blob()?;
        let mut builder = Message::builder();

        if !data.is_empty() {
            builder = builder.body(data);
        }

        for (key, value) in mem::take(&mut self.application_properties) {
            builder = builder
                .application_properties()
                .insert(key, value)
                .message_builder();
        }

        self.messages.push(builder.build());
        Ok(())
    }

    fn set_application_property(
        &mut self,
        key: &mut Variant,
        value: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let key = key.get_string()?;

        let value = match value.get() {
            addin1c::ParamValue::Empty => SimpleValue::Null,
            addin1c::ParamValue::Bool(x) => SimpleValue::Boolean(x),
            addin1c::ParamValue::I32(x) => SimpleValue::Int(x),
            addin1c::ParamValue::F64(x) => SimpleValue::Double(x.into()),
            addin1c::ParamValue::Date(tm) => {
                let datetime: DateTime<Utc> = tm.into();
                SimpleValue::Timestamp(datetime.into())
            }
            addin1c::ParamValue::Str(x) => SimpleValue::String(String::from_utf16_lossy(x)),
            addin1c::ParamValue::Blob(x) => SimpleValue::Binary(x.into()),
        };
        self.application_properties.insert(key, value);
        Ok(())
    }

    fn batch_send(&mut self, _ret_value: &mut Variant) -> AddinResult {
        let messages = std::mem::take(&mut self.messages);
        self.statuses = self.producer.batch_send(messages, &self.runtime)?;
        if self.statuses.iter().all(|x| x == &ResponseCode::Ok) {
            Ok(())
        } else {
            Err("Not delivered".into())
        }
    }

    fn statuses(&mut self, ret_value: &mut Variant) -> AddinResult {
        use std::fmt::Write;
        let mut buf = String::new();
        for status in &self.statuses {
            buf.write_fmt(format_args!("{:?}\r\n", status))?;
        }
        ret_value.set_str1c(buf.as_str())?;
        Ok(())
    }

    environment_impl! {}
}

impl SimpleAddin for AddinProducer {
    fn name() -> &'static [u16] {
        name!("RabbitMQ.Stream.Producer")
    }

    fn save_error(&mut self, err: Option<Box<dyn Error>>) {
        self.last_error = err;
    }

    fn methods() -> &'static [MethodInfo<Self>] {
        &[
            MethodInfo {
                name: name!("AddMessage"),
                method: Methods::Method1(Self::add_message),
            },
            MethodInfo {
                name: name!("SetApplicationProperty"),
                method: Methods::Method2(Self::set_application_property),
            },
            MethodInfo {
                name: name!("BatchSend"),
                method: Methods::Method0(Self::batch_send),
            },
            MethodInfo {
                name: name!("Statuses"),
                method: Methods::Method0(Self::statuses),
            },
            MethodInfo {
                name: name!("SetHost"),
                method: Methods::Method1(Self::set_host),
            },
            MethodInfo {
                name: name!("SetPort"),
                method: Methods::Method1(Self::set_port),
            },
            MethodInfo {
                name: name!("SetUsername"),
                method: Methods::Method1(Self::set_username),
            },
            MethodInfo {
                name: name!("SetPassword"),
                method: Methods::Method1(Self::set_password),
            },
            MethodInfo {
                name: name!("SetVirtualHost"),
                method: Methods::Method1(Self::set_virtual_host),
            },
            MethodInfo {
                name: name!("SetHeartbeat"),
                method: Methods::Method1(Self::set_heartbeat),
            },
            MethodInfo {
                name: name!("SetLoadBalancerMode"),
                method: Methods::Method1(Self::set_load_balancer_mode),
            },
            MethodInfo {
                name: name!("AddClientCertificatesKeys"),
                method: Methods::Method2(Self::add_client_certificates_keys),
            },
            MethodInfo {
                name: name!("AddRootCertificates"),
                method: Methods::Method1(Self::add_root_certificates),
            },
            MethodInfo {
                name: name!("TrustCertificates"),
                method: Methods::Method1(Self::trust_certificates),
            },
            MethodInfo {
                name: name!("SetName"),
                method: Methods::Method1(Self::set_name),
            },
            MethodInfo {
                name: name!("SetBatchDelay"),
                method: Methods::Method1(Self::set_batch_delay),
            },
            MethodInfo {
                name: name!("SetBatchSize"),
                method: Methods::Method1(Self::set_batch_size),
            },
            MethodInfo {
                name: name!("Build"),
                method: Methods::Method1(Self::build),
            },
        ]
    }

    fn properties() -> &'static [PropInfo<Self>] {
        &[PropInfo {
            name: name!("LastError"),
            getter: Some(Self::last_error),
            setter: None,
        }]
    }
}
