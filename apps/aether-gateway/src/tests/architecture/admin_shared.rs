use super::*;

#[test]
fn admin_external_usage_is_confined_to_admin_api() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let mut violations = Vec::new();

    for file in collect_workspace_rust_files("apps/aether-gateway/src") {
        let relative = file
            .canonicalize()
            .expect("workspace file should canonicalize")
            .strip_prefix(&workspace_root)
            .expect("workspace file should be under workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "apps/aether-gateway/src/admin_api.rs"
            || relative.starts_with("apps/aether-gateway/src/handlers/admin/")
            || relative.starts_with("apps/aether-gateway/src/tests/")
        {
            continue;
        }

        let source = std::fs::read_to_string(&file).expect("source file should be readable");
        if source.contains("crate::handlers::admin::")
            || source.contains("use crate::handlers::admin::")
        {
            violations.push(relative);
        }
    }

    assert!(
        violations.is_empty(),
        "gateway code outside admin_api.rs should not directly depend on handlers::admin internals:\n{}",
        violations.join("\n")
    );
}

#[test]
fn admin_wrapped_state_owns_api_key_and_proxy_capabilities() {
    let admin_request =
        read_workspace_module_tree("apps/aether-gateway/src/handlers/admin/request/mod.rs");
    for pattern in [
        "pub(crate) fn has_auth_api_key_writer(&self) -> bool",
        "pub(crate) fn encryption_key(&self) -> Option<&str>",
        "pub(crate) fn encrypt_catalog_secret_with_fallbacks(&self, secret: &str) -> Option<String>",
        "pub(crate) fn decrypt_catalog_secret_with_fallbacks(",
        "pub(crate) async fn list_auth_api_key_snapshots_by_ids(",
        "pub(crate) async fn list_auth_api_key_export_records_by_user_ids(",
        "pub(crate) async fn list_auth_api_key_export_standalone_records_page(",
        "pub(crate) async fn count_auth_api_key_export_standalone_records(",
        "pub(crate) async fn find_auth_api_key_export_standalone_record_by_id(",
        "pub(crate) async fn create_user_api_key(",
        "pub(crate) async fn create_standalone_api_key(",
        "pub(crate) async fn resolve_transport_proxy_snapshot_with_tunnel_affinity(",
        "pub(crate) async fn update_user_api_key_basic(",
        "pub(crate) async fn update_standalone_api_key_basic(",
        "pub(crate) async fn set_standalone_api_key_active(",
        "pub(crate) async fn set_user_api_key_locked(",
        "pub(crate) async fn set_user_api_key_allowed_providers(",
        "pub(crate) async fn summarize_usage_total_tokens_by_api_key_ids(",
        "pub(crate) async fn delete_user_api_key(",
        "pub(crate) async fn delete_standalone_api_key(",
    ] {
        assert!(
            admin_request.contains(pattern),
            "handlers/admin/request/mod.rs should expose admin state capability {pattern}"
        );
    }

    for path in [
    ] {
        let contents = read_workspace_file(path);
        assert!(
            !contents.contains("state.data.has_auth_api_key_writer()"),
            "{path} should use AdminAppState capability instead of raw state.data.has_auth_api_key_writer()"
        );
        assert!(
            !contents.contains("state.data.has_proxy_node_reader()"),
            "{path} should use AdminAppState capability instead of raw state.data.has_proxy_node_reader()"
        );
        assert!(
            !contents.contains(".data\n        .list_auth_api_key_snapshots_by_ids(")
                && !contents.contains(".data.list_auth_api_key_snapshots_by_ids("),
            "{path} should use AdminAppState snapshot capability instead of raw state.data.list_auth_api_key_snapshots_by_ids()"
        );
        assert!(
            !contents.contains("encrypt_catalog_secret_with_fallbacks(state.app(),"),
            "{path} should use AdminAppState encryption capability instead of raw state.app() encryption"
        );
        assert!(
            !contents
                .contains("decrypt_catalog_secret_with_fallbacks(state.app().encryption_key(),"),
            "{path} should use AdminAppState decryption capability instead of raw state.app().encryption_key()"
        );
        assert!(
            !contents.contains(
                "resolve_transport_proxy_snapshot_with_tunnel_affinity(\n            state.app(),"
            ) && !contents
                .contains("resolve_transport_proxy_snapshot_with_tunnel_affinity(state.app(),"),
            "{path} should use AdminAppState proxy capability instead of raw state.app() transport proxy resolution"
        );
    }
}

#[test]
fn admin_wrapped_state_owns_observability_capabilities() {
    let admin_request =
        read_workspace_module_tree("apps/aether-gateway/src/handlers/admin/request/mod.rs");
    for pattern in [
        "pub(crate) fn has_auth_api_key_data_reader(&self) -> bool",
        "pub(crate) fn has_user_data_reader(&self) -> bool",
        "pub(crate) async fn list_provider_catalog_providers(",
        "pub(crate) async fn aggregate_finalized_request_candidate_timeline_by_endpoint_ids_since(",
        "pub(crate) async fn read_recent_request_candidates(",
        "pub(crate) fn provider_key_rpm_reset_at(",
        "pub(crate) async fn update_provider_catalog_key_health_state(",
        "pub(crate) async fn list_usage_audits(",
        "pub(crate) async fn list_users_by_ids(",
    ] {
        assert!(
            admin_request.contains(pattern),
            "handlers/admin/request/mod.rs should expose observability capability {pattern}"
        );
    }
    for pattern in [
        "pub(crate) async fn list_admin_usage_for_range(",
        "pub(crate) async fn list_admin_usage_for_optional_range(",
    ] {
        assert!(
            !admin_request.contains(pattern),
            "handlers/admin/request/mod.rs should not expose deprecated unbounded usage helper {pattern}"
        );
    }
}



#[test]
fn admin_shared_does_not_own_model_global_routes_or_payloads() {
    let admin_shared_paths =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/paths.rs");
    for pattern in [
        "pub(crate) fn is_admin_global_models_root",
        "pub(crate) fn admin_global_model_id_from_path",
        "pub(crate) fn admin_global_model_routing_id",
    ] {
        assert!(
            !admin_shared_paths.contains(pattern),
            "handlers/admin/shared/paths.rs should not own {pattern}"
        );
    }

    let model_shared_paths =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/model/shared/paths.rs");
    for pattern in [
        "pub(crate) fn is_admin_global_models_root",
        "pub(crate) fn admin_global_model_id_from_path",
        "pub(crate) fn admin_global_model_routing_id",
    ] {
        assert!(
            model_shared_paths.contains(pattern),
            "model/shared/paths.rs should own {pattern}"
        );
    }

    let admin_shared_payloads =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/payloads.rs");
    for pattern in [
        "pub(crate) struct AdminGlobalModelCreateRequest",
        "pub(crate) struct AdminGlobalModelUpdateRequest",
        "pub(crate) struct AdminBatchAssignToProvidersRequest",
    ] {
        assert!(
            !admin_shared_payloads.contains(pattern),
            "handlers/admin/shared/payloads.rs should not own {pattern}"
        );
    }

    let model_shared_payloads =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/model/shared/payloads.rs");
    for pattern in [
        "pub(crate) struct AdminGlobalModelCreateRequest",
        "pub(crate) struct AdminGlobalModelUpdateRequest",
        "pub(crate) struct AdminBatchAssignToProvidersRequest",
    ] {
        assert!(
            model_shared_payloads.contains(pattern),
            "model/shared/payloads.rs should own {pattern}"
        );
    }
}

#[test]
fn admin_shared_does_not_own_system_core_routes_or_payloads() {
    let admin_shared_paths =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/paths.rs");
    for pattern in [
        "pub(crate) fn is_admin_management_tokens_root",
        "pub(crate) fn is_admin_system_configs_root",
        "pub(crate) fn admin_oauth_provider_type_from_path",
    ] {
        assert!(
            !admin_shared_paths.contains(pattern),
            "handlers/admin/shared/paths.rs should not own {pattern}"
        );
    }

    let system_shared_paths =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/system/shared/paths.rs");
    for pattern in [
        "pub(crate) fn is_admin_management_tokens_root",
        "pub(crate) fn is_admin_system_configs_root",
    ] {
        assert!(
            system_shared_paths.contains(pattern),
            "system/shared/paths.rs should own {pattern}"
        );
    }
    for pattern in [
        "admin_oauth_provider_type_from_path",
        "admin_oauth_test_provider_type_from_path",
    ] {
        assert!(
            !system_shared_paths.contains(pattern),
            "system/shared/paths.rs should not own auth oauth path helper {pattern}"
        );
    }

    let admin_shared_payloads =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/payloads.rs");
    assert!(
        !admin_shared_payloads.contains("pub(crate) struct AdminOAuthProviderUpsertRequest"),
        "handlers/admin/shared/payloads.rs should not own AdminOAuthProviderUpsertRequest"
    );


    let admin_shared_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/mod.rs");
    assert!(
        admin_shared_mod.contains("mod proxy_errors;")
            && admin_shared_mod
                .contains("pub(crate) use self::proxy_errors::build_proxy_error_response;"),
        "handlers/admin/shared/mod.rs should expose shared admin proxy error builder"
    );
    let admin_shared_proxy_errors =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/shared/proxy_errors.rs");
    assert!(
        admin_shared_proxy_errors.contains("pub(crate) fn build_proxy_error_response"),
        "handlers/admin/shared/proxy_errors.rs should own build_proxy_error_response"
    );
}



#[test]
fn crate_root_exposes_real_admin_and_ai_serving_facades() {
    let lib_rs = read_workspace_file("apps/aether-gateway/src/lib.rs");
    for pattern in ["mod admin_api;", "mod ai_serving;"] {
        assert!(
            lib_rs.contains(pattern),
            "lib.rs should register crate root facade module {pattern}"
        );
    }
    let forbidden = "pub(crate) use self::handlers::admin_api;";
    assert!(
        !lib_rs.contains(forbidden),
        "lib.rs should not keep alias-only facade wiring {forbidden}"
    );

    let admin_api = read_workspace_file("apps/aether-gateway/src/admin_api.rs");
    assert!(
        admin_api.contains("pub(crate) use crate::handlers::admin::{"),
        "admin_api.rs should own the crate root admin facade instead of re-exporting handlers/admin_api"
    );

    let handlers_mod = read_workspace_file("apps/aether-gateway/src/handlers/mod.rs");
    assert!(
        handlers_mod.contains("pub(super) mod admin;"),
        "handlers/mod.rs should expose admin only to the crate root boundary"
    );
    assert!(
        !handlers_mod.contains("pub(super) mod admin_api;"),
        "handlers/mod.rs should not keep a separate handlers/admin_api facade module"
    );
    assert!(
        !handlers_mod.contains("pub(crate) use self::admin::api as admin_api;"),
        "handlers/mod.rs should not keep alias-only admin_api wiring after crate root facade extraction"
    );

    let ai_serving_api_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/api.rs");
    assert!(
        ai_serving_api_mod
            .contains("use crate::ai_serving::{is_json_request, GatewayControlDecision};"),
        "ai_serving/api.rs should depend on the crate-facing ai_serving seam instead of deep internal modules"
    );
}


#[test]
fn admin_proxy_uses_single_admin_routes_entrypoint() {
    let proxy_local = read_workspace_file("apps/aether-gateway/src/handlers/proxy/local.rs");
    assert!(
        proxy_local.contains("admin_api::maybe_build_local_admin_response("),
        "handlers/proxy/local.rs should delegate admin dispatch through crate root admin_api facade"
    );
    assert!(
        proxy_local.contains("admin_api::AdminRouteRequest::new("),
        "handlers/proxy/local.rs should construct AdminRouteRequest through crate root admin_api facade"
    );
    let admin_api = read_workspace_file("apps/aether-gateway/src/admin_api.rs");
    for pattern in [
        "pub(crate) use crate::handlers::admin::{",
        "AdminAppState",
        "AdminRequestContext",
        "AdminRouteRequest",
        "AdminRouteResponse",
        "AdminRouteResult",
        "maybe_build_local_admin_response",
    ] {
        assert!(
            admin_api.contains(pattern),
            "admin_api.rs should own the public admin entry seam {pattern}"
        );
    }

    for forbidden in [
        "auth as admin_auth",
        "billing as admin_billing",
        "endpoint as admin_endpoint",
        "features as admin_features",
        "model as admin_model",
        "observability as admin_observability",
        "provider as admin_provider",
        "system as admin_system",
        "users as admin_users",
        "public::maybe_build_local_admin_announcements_response(",
        "admin_auth::maybe_build_local_admin_auth_response(",
        "admin_observability::maybe_build_local_admin_observability_response(",
        "admin_features::maybe_build_local_admin_features_response(",
        "admin_users::maybe_build_local_admin_users_response(",
        "admin_provider::maybe_build_local_admin_provider_oauth_response(",
        "admin_provider::maybe_build_local_admin_provider_response(",
        "admin_system::maybe_build_local_admin_core_response(",
        "admin_system::maybe_build_local_admin_system_response(",
        "admin_billing::maybe_build_local_admin_billing_routes_response(",
    ] {
        assert!(
            !proxy_local.contains(forbidden),
            "handlers/proxy/local.rs should not dispatch admin subdomains directly for {forbidden}"
        );
    }

    let admin_routes = read_workspace_file("apps/aether-gateway/src/handlers/admin/routes.rs");
    for pattern in [
        "use super::{",
        "pub(crate) async fn maybe_build_local_admin_response(",
        "request::AdminRouteRequest<'_>",
        ") -> request::AdminRouteResult {",
        "auth::maybe_build_local_admin_auth_response(",
        "observability::maybe_build_local_admin_observability_response(",
        "model::maybe_build_local_admin_model_response(",
        "provider::maybe_build_local_admin_provider_response(",
        "system::maybe_build_local_admin_system_response(",
        "endpoint::maybe_build_local_admin_endpoints_response(",
    ] {
        assert!(
            admin_routes.contains(pattern),
            "handlers/admin/routes.rs should own admin proxy dispatch seam {pattern}"
        );
    }

    for forbidden in [
        "use super::super::public;",
        "public::maybe_build_local_admin_announcements_response(",
        "auth::maybe_build_local_admin_security_response(",
        "auth::maybe_build_local_admin_api_keys_response(",
        "auth::maybe_build_local_admin_ldap_response(",
        "observability::maybe_build_local_admin_stats_response(",
        "observability::maybe_build_local_admin_monitoring_response(",
        "observability::maybe_build_local_admin_usage_response(",
        "model::maybe_build_local_admin_global_models_response(",
        "model::maybe_build_local_admin_model_catalog_response(",
        "features::maybe_build_local_admin_video_tasks_response(",
        "features::maybe_build_local_admin_gemini_files_response(",
        "provider::maybe_build_local_admin_provider_oauth_response(",
        "provider::maybe_build_local_admin_provider_models_response(",
        "provider::maybe_build_local_admin_providers_response(",
        "provider::maybe_build_local_admin_provider_ops_response(",
        "provider::maybe_build_local_admin_provider_query_response(",
        "provider::maybe_build_local_admin_provider_strategy_response(",
        "billing::maybe_build_local_admin_billing_response(",
        "billing::maybe_build_local_admin_payments_response(",
        "billing::maybe_build_local_admin_wallets_response(",
    ] {
        assert!(
            !admin_routes.contains(forbidden),
            "handlers/admin/routes.rs should not dispatch provider or billing internals directly for {forbidden}"
        );
    }

    let admin_request =
        read_workspace_module_tree("apps/aether-gateway/src/handlers/admin/request/mod.rs");
    for pattern in [
        "pub(crate) struct AdminAppState<'a>",
        "pub(crate) fn new(app: &'a AppState) -> Self",
        "pub(crate) fn app(&self) -> &AppState",
        "pub(crate) fn has_provider_catalog_data_reader(&self) -> bool",
        "pub(crate) fn has_provider_catalog_data_writer(&self) -> bool",
        "pub(crate) fn has_request_candidate_data_reader(&self) -> bool",
        "pub(crate) fn has_global_model_data_reader(&self) -> bool",
        "pub(crate) fn has_usage_data_reader(&self) -> bool",
        "pub(crate) fn has_auth_module_writer(&self) -> bool",
        "pub(crate) async fn count_active_local_admin_users_with_valid_password(",
        "pub(crate) async fn list_oauth_provider_configs(",
        "pub(crate) async fn get_oauth_provider_config(",
        "pub(crate) async fn upsert_oauth_provider_config(",
        "pub(crate) async fn delete_oauth_provider_config(",
        "pub(crate) fn mark_provider_key_rpm_reset(&self, key_id: &str, now_unix_secs: u64)",
        "pub(crate) async fn list_proxy_nodes(",
        "pub(crate) async fn find_proxy_node(",
        "pub(crate) async fn read_provider_catalog_endpoints_by_ids(",
        "pub(crate) async fn count_distinct_video_task_users(",
        "pub(crate) struct AdminRequestContext<'a>",
        "pub(crate) fn new(context: &'a GatewayPublicRequestContext) -> Self",
        "pub(crate) fn decision(&self) -> Option<&GatewayControlDecision>",
        "pub(crate) fn method(&self) -> &Method",
        "pub(crate) fn path(&self) -> &str",
        "pub(crate) fn query_string(&self) -> Option<&str>",
        "pub(crate) fn public(&self) -> &GatewayPublicRequestContext",
        "impl<'a> Deref for AdminRequestContext<'a>",
        "pub(crate) type AdminRouteResponse",
        "pub(crate) type AdminRouteResult",
        "pub(crate) struct AdminRouteRequest<'a>",
        "pub(crate) fn new(",
        "state: AdminAppState<'a>",
        "request_context: AdminRequestContext<'a>",
        "request_body: Option<&'a Bytes>",
        "pub(crate) fn state(self) -> AdminAppState<'a>",
        "pub(crate) fn request_context(self) -> AdminRequestContext<'a>",
        "pub(crate) fn request_body(self) -> Option<&'a Bytes>",
    ] {
        assert!(
            admin_request.contains(pattern),
            "handlers/admin/request/mod.rs should own unified admin request injection field {pattern}"
        );
    }
}

#[test]
fn admin_second_layer_route_seams_use_wrapped_request_types() {
    for file in [
    ] {
        let contents = read_workspace_file(file);
        assert!(
            contents.contains("state: &AdminAppState<'_>,"),
            "{file} should accept AdminAppState at the second-layer admin seam",
        );
        assert!(
            contents.contains("request_context: &AdminRequestContext<'_>,"),
            "{file} should accept AdminRequestContext at the second-layer admin seam",
        );
        assert!(
            !contents.contains("let state = state.app();"),
            "{file} should not expose raw AppState as a second-layer local variable",
        );
        assert!(
            !contents.contains("let app_state = state.app();"),
            "{file} should not expose raw AppState aliasing at the second-layer admin seam",
        );
    }

    let admin_request =
        read_workspace_module_tree("apps/aether-gateway/src/handlers/admin/request/mod.rs");
    assert!(
        !admin_request.contains("impl<'a> Deref for AdminAppState<'a>"),
        "handlers/admin/request/mod.rs should not expose AdminAppState via implicit Deref<AppState>",
    );
}

#[test]
fn admin_route_adjacent_owners_use_wrapped_state_types() {
    for file in [
    ] {
        let contents = read_workspace_file(file);
        assert!(
            contents.contains("state: &AdminAppState<'_>,"),
            "{file} should accept AdminAppState at the route-adjacent admin owner layer",
        );
        assert!(
            !contents.contains("state: &AppState,"),
            "{file} should not keep raw AppState in route-adjacent owner signatures",
        );
    }

    for file in [
    ] {
        let contents = read_workspace_file(file);
        assert!(
            contents.contains("state: &AdminAppState<'_>,"),
            "{file} should accept AdminAppState at the route-owner layer",
        );
        assert!(
            contents.contains("request_context: &AdminRequestContext<'_>,")
                || contents.contains("_state: &AdminAppState<'_>,"),
            "{file} should accept wrapped admin request/state types at the route-owner layer",
        );
        assert!(
            !contents.contains("state: &AppState,"),
            "{file} should not keep raw AppState in route-owner signatures",
        );
        assert!(
            !contents.contains("request_context: &GatewayPublicRequestContext,"),
            "{file} should not keep raw GatewayPublicRequestContext in route-owner signatures",
        );
    }

    for file in [
    ] {
        let contents = read_workspace_file(file);
        assert!(
            contents.contains("state: &AdminAppState<'_>,"),
            "{file} should accept AdminAppState at the wrapped helper-owner layer",
        );
        assert!(
            !contents.contains("state: &AppState,"),
            "{file} should not keep raw AppState in helper-owner signatures",
        );
    }
}
