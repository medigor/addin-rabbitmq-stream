use std::error::Error;

use addin1c::AddinResult;
use rabbitmq_stream_client::{Environment, EnvironmentBuilder, TlsConfiguration};

#[derive(Default)]
pub struct TlsProperties {
    pub client_certificate_path: String,
    pub client_private_key_path: String,
    pub server_certificate_path: String,
    pub trust_certificates: bool,
}

pub struct Builder {
    environment_builder: Option<Box<EnvironmentBuilder>>,
    tls_properties: Option<Box<TlsProperties>>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            environment_builder: Some(Box::new(Environment::builder())),
            tls_properties: None,
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

    pub fn set_tls<F>(&mut self, f: F) -> AddinResult
    where
        F: FnOnce(&mut TlsProperties),
    {
        let mut props = self.tls_properties.take().unwrap_or_default();
        f(&mut props);
        self.tls_properties = Some(props);
        Ok(())
    }

    pub async fn build(&mut self) -> Result<Environment, Box<dyn Error>> {
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

            environment = environment.tls(tls_builder.build())
        }

        Ok(environment.build().await?)
    }
}

#[macro_export]
macro_rules! environment_impl {
    () => {
        fn set_host(&mut self, host: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
            let host = host.get_string()?;
            self.environment_builder.set_environment(|x| x.host(&host))
        }

        fn set_port(&mut self, port: &mut Variant, _ret_value: &mut Variant) -> AddinResult {
            let port = port.get_i32()? as u16;
            self.environment_builder.set_environment(|x| x.port(port))
        }

        fn set_username(
            &mut self,
            username: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let username = username.get_string()?;
            self.environment_builder
                .set_environment(|x| x.username(&username))
        }

        fn set_password(
            &mut self,
            password: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let password = password.get_string()?;
            self.environment_builder
                .set_environment(|x| x.password(&password))
        }

        fn set_virtual_host(
            &mut self,
            virtual_host: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let virtual_host = virtual_host.get_string()?;
            self.environment_builder
                .set_environment(|x| x.virtual_host(&virtual_host))
        }

        fn set_heartbeat(
            &mut self,
            heartbeat: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let heartbeat = heartbeat.get_i32()? as u32;
            self.environment_builder
                .set_environment(|x| x.heartbeat(heartbeat))
        }

        fn set_load_balancer_mode(
            &mut self,
            mode: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let mode = mode.get_bool()?;
            self.environment_builder
                .set_environment(|x| x.load_balancer_mode(mode))
        }

        fn add_client_certificates_keys(
            &mut self,
            certificate_path: &mut Variant,
            private_key_path: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let certificate_path = certificate_path.get_string()?;
            let private_key_path = private_key_path.get_string()?;
            self.environment_builder.set_tls(|props| {
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
            self.environment_builder.set_tls(|props| {
                props.server_certificate_path = certificate_path;
            })
        }

        fn trust_certificates(
            &mut self,
            trust_certificates: &mut Variant,
            _ret_value: &mut Variant,
        ) -> AddinResult {
            let trust_certificates = trust_certificates.get_bool()?;
            self.environment_builder.set_tls(|props| {
                props.trust_certificates = trust_certificates;
            })
        }
    };
}
