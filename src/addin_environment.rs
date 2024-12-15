use addin1c::{name, AddinResult, MethodInfo, Methods, PropInfo, SimpleAddin, Variant};
use rabbitmq_stream_client::{Environment, EnvironmentBuilder, TlsConfiguration};
use std::error::Error;
use tokio::runtime::Runtime;

use crate::environments::{insert_environment, remove_environment};

#[derive(Default)]
struct TlsProperties {
    pub client_certificate_path: String,
    pub client_private_key_path: String,
    pub server_certificate_path: String,
    pub trust_certificates: bool,
}

pub struct AddinEnvironment {
    environment_builder: Option<Box<EnvironmentBuilder>>,
    tls_properties: Option<Box<TlsProperties>>,
    runtime: Runtime,
    id: i32,
    last_error: Option<Box<dyn Error>>,
}

impl AddinEnvironment {
    pub fn new() -> Self {
        Self {
            environment_builder: Some(Box::new(Environment::builder())),
            tls_properties: None,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create runtime"),
            id: -1,
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

    pub fn set_environment<F>(&mut self, f: F) -> AddinResult
    where
        F: FnOnce(EnvironmentBuilder) -> EnvironmentBuilder,
    {
        let builder = self
            .environment_builder
            .take()
            .ok_or("Parameter cannot be set")?;

        let builder = f(*builder);
        self.environment_builder = Some(Box::new(builder));

        Ok(())
    }

    fn set_tls<F>(&mut self, f: F) -> AddinResult
    where
        F: FnOnce(&mut TlsProperties),
    {
        let mut props = self.tls_properties.take().unwrap_or_default();
        f(&mut props);
        self.tls_properties = Some(props);
        Ok(())
    }

    fn set_host(&mut self, host: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let host = host.get_string()?;
        self.set_environment(|x| x.host(&host))
    }

    fn set_port(&mut self, port: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let port = port.get_i32()? as u16;
        self.set_environment(|x| x.port(port))
    }

    fn set_username(&mut self, username: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let username = username.get_string()?;
        self.set_environment(|x| x.username(&username))
    }

    fn set_password(&mut self, password: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let password = password.get_string()?;
        self.set_environment(|x| x.password(&password))
    }

    fn set_virtual_host(
        &mut self,
        virtual_host: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let virtual_host = virtual_host.get_string()?;
        self.set_environment(|x| x.virtual_host(&virtual_host))
    }

    fn set_heartbeat(&mut self, heartbeat: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
        let heartbeat = heartbeat.get_i32()? as u32;
        self.set_environment(|x| x.heartbeat(heartbeat))
    }

    fn set_load_balancer_mode(
        &mut self,
        mode: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let mode = mode.get_bool()?;
        self.set_environment(|x| x.load_balancer_mode(mode))
    }

    fn add_client_certificates_keys(
        &mut self,
        certificate_path: &mut Variant,
        private_key_path: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let certificate_path = certificate_path.get_string()?;
        let private_key_path = private_key_path.get_string()?;
        self.set_tls(|props| {
            props.client_certificate_path = certificate_path;
            props.client_private_key_path = private_key_path
        })
    }

    fn add_root_certificates(
        &mut self,
        certificate_path: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let certificate_path = certificate_path.get_string()?;
        self.set_tls(|props| {
            props.server_certificate_path = certificate_path;
        })
    }

    fn trust_certificates(
        &mut self,
        trust_certificates: &mut Variant,
        _ret_value: &mut Variant,
    ) -> AddinResult {
        let trust_certificates = trust_certificates.get_bool()?;
        self.set_tls(|props| {
            props.trust_certificates = trust_certificates;
        })
    }

    fn build(&mut self, ret_value: &mut Variant) -> AddinResult {
        let mut environment = *self
            .environment_builder
            .take()
            .ok_or("EnvironmentBuilder not exists")?;

        if let Some(tls_properties) = self.tls_properties.take() {
            let mut tls_builder = TlsConfiguration::builder();
            if !tls_properties.client_certificate_path.is_empty() {
                tls_builder = tls_builder.add_client_certificates_keys(
                    tls_properties.client_certificate_path,
                    tls_properties.client_private_key_path,
                );
            }

            if !tls_properties.server_certificate_path.is_empty() {
                tls_builder =
                    tls_builder.add_root_certificates(tls_properties.server_certificate_path);
            }

            tls_builder = tls_builder.trust_certificates(tls_properties.trust_certificates);

            environment = environment.tls(tls_builder.build());
        }

        let env = self.runtime.block_on(environment.build())?;

        let id = insert_environment(env);
        self.id = id;
        ret_value.set_i32(id);
        Ok(())
    }

    fn handle(&mut self, ret_value: &mut Variant) -> AddinResult {
        if self.id > 0 {
            ret_value.set_i32(self.id);
            return Ok(());
        }
        Err("Environment not build".into())
    }
}

impl SimpleAddin for AddinEnvironment {
    fn name() -> &'static [u16] {
        name!("RabbitMQ.Stream.EnvironmentBuilder")
    }

    fn save_error(&mut self, err: Option<Box<dyn Error>>) {
        self.last_error = err;
    }

    fn methods() -> &'static [MethodInfo<Self>] {
        &[
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
                name: name!("Build"),
                method: Methods::Method0(Self::build),
            },
            MethodInfo {
                name: name!("Handle"),
                method: Methods::Method0(Self::handle),
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

impl Drop for AddinEnvironment {
    fn drop(&mut self) {
        if self.id > 0 {
            remove_environment(self.id);
        }
    }
}
