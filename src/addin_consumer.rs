use std::{error::Error, mem::transmute, time::Duration};

use addin1c::{name, AddinResult, MethodInfo, Methods, PropInfo, SimpleAddin, Variant};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use rabbitmq_stream_client::{
    error::{ClientError, ConsumerStoreOffsetError},
    types::{Delivery, OffsetSpecification, ResponseCode, SimpleValue},
    Consumer,
};
use tokio::{runtime::Runtime, time};

use crate::{environment_builder, environment_impl};

#[derive(Default)]
struct ConsumerProperties {
    pub name: Option<String>,
}

pub struct AddinConsumer {
    environment_builder: environment_builder::Builder,
    consumer_properties: Option<Box<ConsumerProperties>>,
    runtime: Runtime,
    consumer: Option<Consumer>,
    delivery: Option<Delivery>,
    last_error: Option<Box<dyn Error>>,
}

impl AddinConsumer {
    pub fn new() -> Self {
        Self {
            environment_builder: environment_builder::Builder::new(),
            consumer_properties: Some(Default::default()),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime"),
            consumer: None,
            delivery: None,
            last_error: None,
        }
    }

    fn last_error(&mut self, value: &mut Variant) -> AddinResult {
        match &self.last_error {
            Some(err) => value.set_str1c(err.to_string())?,
            None => value.set_str1c("")?,
        };
        Ok(())
    }

    fn set_name(&mut self, name: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let name = name.get_string()?;
        if let Some(builder) = self.consumer_properties.as_mut() {
            builder.name = Some(name);
        };
        Ok(())
    }

    fn build(&mut self, stream: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let stream = stream.get_string()?;
        let environment = self.runtime.block_on(self.environment_builder.build())?;

        let consumer_properties = *self
            .consumer_properties
            .take()
            .ok_or("ConsumerBuilder not exists")?;

        let mut builder = environment.consumer();
        if let Some(name) = &consumer_properties.name {
            builder = builder.name(name);
        }

        let temp_consumer = self.runtime.block_on(builder.build(&stream))?;
        let first_offset = match self.runtime.block_on(temp_consumer.query_offset()) {
            Ok(offset) => offset + 1,
            Err(ConsumerStoreOffsetError::Client(ClientError::RequestError(
                ResponseCode::OffsetNotFound,
            ))) => 0,
            Err(err) => return Err(err.into()),
        };

        let mut builder = environment.consumer();
        if let Some(name) = &consumer_properties.name {
            builder = builder.name(name);
        }
        builder = builder.offset(OffsetSpecification::Offset(first_offset));

        let consumer = self.runtime.block_on(builder.build(&stream))?;
        self.consumer = Some(consumer);

        Ok(())
    }

    fn recv(&mut self, timeout: &mut Variant, ret_value: &mut Variant) -> AddinResult {
        let timeout = timeout.get_i32()? as _;
        let Some(consumer) = &mut self.consumer else {
            return Err("No consumer".into());
        };
        let _guard = self.runtime.handle().enter();
        let delivery = self.runtime.block_on(time::timeout(
            Duration::from_millis(timeout),
            consumer.next(),
        ));
        let Ok(delivery) = delivery else {
            // timeout
            ret_value.set_bool(false);
            return Ok(());
        };
        let Some(delivery) = delivery else {
            return Err("Stream closed".into());
        };
        self.delivery = Some(delivery?);
        ret_value.set_bool(true);
        Ok(())
    }

    fn message_body(&mut self, ret_value: &mut Variant) -> AddinResult {
        let delivery = self.delivery.as_ref().ok_or("No message")?;
        if let Some(data) = delivery.message().data() {
            ret_value.set_blob(data)?;
        };
        Ok(())
    }

    fn application_property(&mut self, name: &mut Variant, ret_value: &mut Variant) -> AddinResult {
        let name = name.get_string()?;
        let delivery = self.delivery.as_ref().ok_or("No message")?;
        let Some(props) = delivery.message().application_properties() else {
            return Ok(());
        };
        let value = props.get(&name);
        let Some(value) = value else {
            return Ok(());
        };

        match value {
            SimpleValue::Null => ret_value.set_empty(),
            SimpleValue::Boolean(x) => ret_value.set_bool(*x),
            SimpleValue::Ubyte(x) => ret_value.set_i32(*x as _),
            SimpleValue::Ushort(x) => ret_value.set_i32(*x as _),
            SimpleValue::Uint(x) => {
                if *x > i32::MAX as _ {
                    ret_value.set_f64(*x as _)
                } else {
                    ret_value.set_i32(*x as _)
                }
            }

            SimpleValue::Ulong(x) => {
                if *x > i32::MAX as _ {
                    ret_value.set_f64(*x as _)
                } else {
                    ret_value.set_i32(*x as _)
                }
            }
            SimpleValue::Byte(x) => ret_value.set_i32(*x as _),
            SimpleValue::Short(x) => ret_value.set_i32(*x as _),
            SimpleValue::Int(x) => ret_value.set_i32(*x as _),
            SimpleValue::Long(x) => {
                if *x > i32::MAX as _ || *x < i32::MIN as _ {
                    ret_value.set_f64(*x as _)
                } else {
                    ret_value.set_i32(*x as _)
                }
            }
            SimpleValue::Float(x) => {
                let f = x.clone();
                let f: f32 = unsafe { transmute(f) };
                ret_value.set_f64(f as f64);
            }
            SimpleValue::Double(x) => {
                let f = x.clone();
                let f: f64 = unsafe { transmute(f) };
                ret_value.set_f64(f);
            }
            SimpleValue::Char(_) => todo!(),
            SimpleValue::Timestamp(x) => {
                let x: DateTime<Utc> = unsafe { transmute(x.clone()) };
                ret_value.set_date(x.into());
            }
            SimpleValue::Uuid(uuid) => ret_value.set_str1c(uuid.to_string())?,
            SimpleValue::Binary(x) => ret_value.set_blob(x)?,
            SimpleValue::String(x) => ret_value.set_str1c(x.as_str())?,
            SimpleValue::Symbol(x) => ret_value.set_str1c(x.as_str())?,
        };
        Ok(())
    }

    fn offset(&mut self, ret_value: &mut Variant) -> AddinResult {
        let delivery = self.delivery.as_ref().ok_or("No message")?;
        let bytes = delivery.offset().to_le_bytes();
        ret_value.set_blob(&bytes)?;
        Ok(())
    }

    fn store_offset(&mut self, offset: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let offset = offset.get_blob()?;
        let offset = u64::from_le_bytes(offset.try_into()?);
        let Some(consumer) = &mut self.consumer else {
            return Err("No consumer".into());
        };
        self.runtime.block_on(consumer.store_offset(offset))?;
        Ok(())
    }

    environment_impl! {}
}

impl SimpleAddin for AddinConsumer {
    fn name() -> &'static [u16] {
        name!("RabbitMQ.Stream.Consumer")
    }

    fn save_error(&mut self, err: Option<Box<dyn Error>>) {
        self.last_error = err;
    }

    fn methods() -> &'static [addin1c::MethodInfo<Self>] {
        &[
            MethodInfo {
                name: name!("Recv"),
                method: Methods::Method1(Self::recv),
            },
            MethodInfo {
                name: name!("MessageBody"),
                method: Methods::Method0(Self::message_body),
            },
            MethodInfo {
                name: name!("ApplicationProperty"),
                method: Methods::Method1(Self::application_property),
            },
            MethodInfo {
                name: name!("Offset"),
                method: Methods::Method0(Self::offset),
            },
            MethodInfo {
                name: name!("StoreOffset"),
                method: Methods::Method1(Self::store_offset),
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
