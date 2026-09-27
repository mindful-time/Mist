use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

use crate::domain::{InferenceProviderId, ProviderCapability};

#[derive(Clone, Debug)]
pub struct ModelPreferencesStore {
    root: PathBuf,
}

impl ModelPreferencesStore {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load(
        &self,
        fallback: &InferenceProviderId,
        capabilities: &[ProviderCapability],
    ) -> InferenceProviderId {
        fs::read_to_string(self.path())
            .ok()
            .and_then(|value| {
                capabilities
                    .iter()
                    .find(|capability| capability.id.as_str() == value.trim())
                    .map(|capability| capability.id.clone())
            })
            .unwrap_or_else(|| fallback.clone())
    }

    pub fn save(&self, provider: &InferenceProviderId) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("could not create {}", self.root.display()))?;
        let destination = self.path();
        let temporary = destination.with_extension("new");
        fs::write(&temporary, format!("{}\n", provider.as_str()))
            .with_context(|| format!("could not write {}", temporary.display()))?;
        if destination.exists() {
            fs::remove_file(&destination)
                .with_context(|| format!("could not replace {}", destination.display()))?;
        }
        fs::rename(&temporary, &destination)
            .with_context(|| format!("could not save {}", destination.display()))
    }

    fn path(&self) -> PathBuf {
        self.root.join("model-preferences")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(id: &str) -> ProviderCapability {
        ProviderCapability {
            id: InferenceProviderId::new(id).unwrap(),
            display_name: "Test",
            performance: crate::domain::ProviderPerformance::Standard,
            available: true,
            detail: "Test provider",
        }
    }

    #[test]
    fn provider_round_trips_and_invalid_values_fall_back_to_auto() {
        let temporary = tempfile::tempdir().unwrap();
        let store = ModelPreferencesStore::at(temporary.path());
        let automatic = InferenceProviderId::new("auto").unwrap();
        let cpu = InferenceProviderId::new("cpu").unwrap();
        let capabilities = [capability("auto"), capability("cpu")];

        assert_eq!(store.load(&automatic, &capabilities), automatic);
        store.save(&cpu).unwrap();
        assert_eq!(store.load(&automatic, &capabilities), cpu);

        fs::write(store.path(), "not-a-provider\n").unwrap();
        assert_eq!(store.load(&automatic, &capabilities), automatic);
    }
}
