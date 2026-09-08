use serde::{Deserialize, Serialize};

pub const PRODUCTION: bool = cfg!(feature = "production");
pub const PRODUCT_NAME: &str = if PRODUCTION {
    "Rotor"
} else {
    "Rotor GPUI Development"
};
pub const IDENTIFIER: &str = if PRODUCTION {
    "cc.fluctus.rotor"
} else {
    "cc.fluctus.rotor.gpui-dev"
};
pub const PROFILE_DIRECTORY: &str = if PRODUCTION { ".rotor" } else { ".rotor-gpui" };
pub const EXECUTABLE_NAME: &str = if PRODUCTION { "rotor" } else { "rotor-desktop" };
pub const STARTUP_NAME: &str = PRODUCT_NAME;
pub const LAUNCH_AGENT_LABEL: &str = if PRODUCTION {
    "Rotor"
} else {
    "cc.fluctus.rotor.gpui-dev"
};

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
    BuildInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        production,
        product_name: if production {
            "Rotor"
        } else {
            "Rotor GPUI Development"
        }
        .into(),
        identifier: if production {
            "cc.fluctus.rotor"
        } else {
            "cc.fluctus.rotor.gpui-dev"
        }
        .into(),
        profile_directory: if production { ".rotor" } else { ".rotor-gpui" }.into(),
        executable_name: if production { "rotor" } else { "rotor-desktop" }.into(),
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
        if super::PRODUCTION {
            let legacy: serde_json::Value =
                serde_json::from_str(include_str!("../../../src-tauri/tauri.conf.json")).unwrap();
            assert_eq!(info.identifier, legacy["identifier"].as_str().unwrap());
            assert_eq!(info.product_name, legacy["productName"].as_str().unwrap());
            assert_eq!(info.profile_directory, ".rotor");
        } else {
            let metadata: toml::Value =
                toml::from_str(include_str!("../../../native/app.toml")).unwrap();
            assert_eq!(info.identifier, metadata["identifier"].as_str().unwrap());
            assert_eq!(info.profile_directory, ".rotor-gpui");
        }
        assert_eq!(
            serde_json::from_str::<super::BuildInfo>(&super::build_info_json()).unwrap(),
            info
        );
    }
}
