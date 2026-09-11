use serde::{Deserialize, Serialize};

pub const PRODUCTION: bool = cfg!(feature = "production");
#[path = "native_identity.rs"]
mod identity;
pub use identity::{Identity, DEVELOPMENT, PRODUCTION_IDENTITY};
pub const CURRENT: Identity = identity::for_production(PRODUCTION);
pub const PRODUCT_NAME: &str = CURRENT.product_name;
pub const IDENTIFIER: &str = CURRENT.identifier;
pub const PROFILE_DIRECTORY: &str = CURRENT.profile_directory;
pub const EXECUTABLE_NAME: &str = CURRENT.executable_name;
pub const STARTUP_NAME: &str = PRODUCT_NAME;
pub const LAUNCH_AGENT_LABEL: &str = IDENTIFIER;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: String,
    pub production: bool,
    pub product_name: String,
    pub identifier: String,
    pub profile_directory: String,
    pub executable_name: String,
}
pub fn build_info_for(production: bool) -> BuildInfo {
    let identity = identity::for_production(production);
    BuildInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        production,
        product_name: identity.product_name.into(),
        identifier: identity.identifier.into(),
        profile_directory: identity.profile_directory.into(),
        executable_name: identity.executable_name.into(),
    }
}

pub fn build_info() -> BuildInfo {
    build_info_for(PRODUCTION)
}
pub fn build_info_json() -> String {
    serde_json::to_string(&build_info()).expect("static build information serializes")
}

#[cfg(test)]
mod tests {
    #[test]
    fn build_identity_matches_the_selected_namespace() {
        let info = super::build_info();
        assert_eq!(info.identifier, super::IDENTIFIER);
        assert_eq!(info.product_name, super::PRODUCT_NAME);
        assert_eq!(info.profile_directory, super::PROFILE_DIRECTORY);
        assert_eq!(info.executable_name, super::EXECUTABLE_NAME);
        let metadata: toml::Value =
            toml::from_str(include_str!("../../../native/app.toml")).unwrap();
        for production in [false, true] {
            let identity = super::build_info_for(production);
            let prefix = if production { "production_" } else { "" };
            for (key, value) in [
                ("identifier", identity.identifier),
                ("product_name", identity.product_name),
                ("data_directory", identity.profile_directory),
                ("executable_name", identity.executable_name),
            ] {
                assert_eq!(
                    metadata[format!("{prefix}{key}")].as_str(),
                    Some(value.as_str())
                );
            }
        }
        assert_ne!(
            super::DEVELOPMENT.identifier,
            super::PRODUCTION_IDENTITY.identifier
        );
        assert_ne!(
            super::DEVELOPMENT.profile_directory,
            super::PRODUCTION_IDENTITY.profile_directory
        );
        assert_eq!(
            serde_json::from_str::<super::BuildInfo>(&super::build_info_json()).unwrap(),
            info
        );
    }
}
