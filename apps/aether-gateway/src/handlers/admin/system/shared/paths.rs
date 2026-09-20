pub(crate) fn is_admin_system_configs_root(request_path: &str) -> bool {
    aether_admin::system::is_admin_system_configs_root(request_path)
}

pub(crate) fn admin_system_config_key_from_path(request_path: &str) -> Option<String> {
    aether_admin::system::admin_system_config_key_from_path(request_path)
}
