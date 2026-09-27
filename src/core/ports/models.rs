/// Driven port for downloadable speech-model artifacts.
pub trait ModelProvisioner: Send + Sync {
    fn is_ready(&self) -> bool;
    fn install(&self) -> anyhow::Result<()>;
}
