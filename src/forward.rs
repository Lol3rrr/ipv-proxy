pub mod manual;

pub trait ForwardingBackend {
    fn forward(&self, service: crate::config::ExposedService, shutdown: tokio_util::sync::CancellationToken) -> core::pin::Pin<Box<dyn Future<Output = ()> + Send>>;
}
