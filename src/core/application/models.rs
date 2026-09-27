use super::super::ports::ModelProvisioner;

/// Application use case for explicit first-run model installation.
pub struct InstallModel<M> {
    models: M,
}

impl<M> InstallModel<M>
where
    M: ModelProvisioner,
{
    pub fn new(models: M) -> Self {
        Self { models }
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        if !self.models.is_ready() {
            self.models.install()?;
        }
        Ok(())
    }
}
