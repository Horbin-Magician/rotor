// Shared by runtime identity and Windows resource compilation.
#[derive(Clone, Copy)]
pub struct Identity {
    pub product_name: &'static str,
    pub identifier: &'static str,
    pub profile_directory: &'static str,
    pub executable_name: &'static str,
}
pub const DEVELOPMENT: Identity = Identity {
    product_name: "Rotor Development",
    identifier: "cc.fluctus.rotor.dev",
    profile_directory: ".rotor-dev",
    executable_name: "rotor-desktop",
};
pub const PRODUCTION_IDENTITY: Identity = Identity {
    product_name: "Rotor",
    identifier: "cc.fluctus.rotor",
    profile_directory: ".rotor",
    executable_name: "rotor",
};
pub const fn for_production(production: bool) -> Identity {
    if production {
        PRODUCTION_IDENTITY
    } else {
        DEVELOPMENT
    }
}
