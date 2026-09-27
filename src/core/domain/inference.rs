#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InferenceProviderId(String);

impl InferenceProviderId {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.trim().is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapability {
    pub id: InferenceProviderId,
    pub display_name: &'static str,
    pub performance: ProviderPerformance,
    pub available: bool,
    pub detail: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderPerformance {
    Recommended,
    Standard,
    Accelerated,
    Planned,
}
