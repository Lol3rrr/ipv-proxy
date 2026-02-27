mod jool;
mod manual;

pub use jool::JoolForwarding;
pub use manual::ManualForwarding;

pub trait ForwardingBackend {
    fn forward(
        &self,
        service: crate::config::ExposedService,
        shutdown: tokio_util::sync::CancellationToken,
    ) -> core::pin::Pin<Box<dyn Future<Output = ()> + Send>>;
}
