use super::*;

#[test]
fn admin_system_build_version_contract_uses_explicit_local_build_arg() {
    let build_rs = read_workspace_file("apps/aether-gateway/build.rs");
    for pattern in [
        "cargo:rerun-if-env-changed=AETHER_BUILD_VERSION",
        "env::var(\"AETHER_BUILD_VERSION\")",
    ] {
        assert!(
            build_rs.contains(pattern),
            "apps/aether-gateway/build.rs should consume explicit build version pattern {pattern}"
        );
    }

    let dockerfile = read_workspace_file("Dockerfile.app.local");
    for pattern in [
        "ARG AETHER_BUILD_VERSION",
        "ENV AETHER_BUILD_VERSION=${AETHER_BUILD_VERSION}",
        "AETHER_VERSION=${AETHER_BUILD_VERSION}",
    ] {
        assert!(
            dockerfile.contains(pattern),
            "Dockerfile.app.local should pass explicit build version pattern {pattern}"
        );
    }

    let deploy = read_workspace_file("deploy.sh");
    for pattern in [
        "detect_build_version()",
        "git describe --tags --match 'v[0-9]*' --always --dirty",
        "AETHER_BUILD_VERSION=\"${AETHER_BUILD_VERSION:-$(detect_build_version)}\"",
        "--build-arg \"AETHER_BUILD_VERSION=$AETHER_BUILD_VERSION\"",
        ">>> AETHER_BUILD_VERSION",
    ] {
        assert!(
            deploy.contains(pattern),
            "deploy.sh should pass deterministic local build version pattern {pattern}"
        );
    }

    let vite_config = read_workspace_file("frontend/vite.config.ts");
    for pattern in [
        "process.env.AETHER_BUILD_VERSION",
        "process.env.AETHER_VERSION",
        "git describe --tags --match \"v[0-9]*\" --always --dirty",
        "trimmed.startsWith('tunnel-v')",
    ] {
        assert!(
            vite_config.contains(pattern),
            "frontend/vite.config.ts should consume local build version pattern {pattern}"
        );
    }

    let core_api = read_workspace_file("apps/aether-gateway/src/api/core.rs");
    for pattern in [
        "option_env!(\"AETHER_BUILD_VERSION\")",
        "\"version\": current_gateway_version()",
    ] {
        assert!(
            core_api.contains(pattern),
            "api/core.rs should expose build version pattern {pattern}"
        );
    }

    let build_rs = read_workspace_file("apps/aether-gateway/build.rs");
    for pattern in [
        "\"--match\"",
        "\"v[0-9]*\"",
        "trimmed.starts_with(\"tunnel-v\")",
    ] {
        assert!(
            build_rs.contains(pattern),
            "apps/aether-gateway/build.rs should ignore tunnel release tags for gateway version pattern {pattern}"
        );
    }
}

#[test]
fn admin_system_and_endpoint_roots_stay_thin() {
    let system_mod = read_workspace_file("apps/aether-gateway/src/handlers/admin/system/mod.rs");
    for pattern in [
        "pub(super) use super::auth::{",
        "pub(super) use super::model::{",
        "pub(super) use super::provider::{",
    ] {
        assert!(
            !system_mod.contains(pattern),
            "handlers/admin/system/mod.rs should not act as a cross-domain re-export layer for {pattern}"
        );
    }
    for pattern in [
        "mod routes;",
        "pub(super) use self::routes::maybe_build_local_admin_system_response;",
    ] {
        assert!(
            system_mod.contains(pattern),
            "handlers/admin/system/mod.rs should stay as a thin system subdomain router for {pattern}"
        );
    }
    for forbidden in [
        "pub(crate) use self::adaptive::maybe_build_local_admin_adaptive_response;",
        "pub(crate) use self::core::maybe_build_local_admin_core_response;",
        "pub(crate) use self::management_tokens::maybe_build_local_admin_management_tokens_response;",
        "pub(crate) use self::modules::maybe_build_local_admin_modules_response;",
        "pub(crate) use self::proxy_nodes::maybe_build_local_admin_proxy_nodes_response;",
        "pub(crate) use crate::handlers::admin::provider::pool_admin::maybe_build_local_admin_pool_response;",
    ] {
        assert!(
            !system_mod.contains(forbidden),
            "handlers/admin/system/mod.rs should not remain a public owner export hub for {forbidden}"
        );
    }

    assert!(
        !workspace_file_exists("apps/aether-gateway/src/handlers/admin/system/pool/mod.rs"),
        "handlers/admin/system/pool/mod.rs should be deleted once system root delegates pool admin directly to provider::pool_admin"
    );

    let endpoint_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/endpoint/mod.rs");
    for pattern in [
        "mod routes;",
        "pub(super) use self::routes::maybe_build_local_admin_endpoints_response;",
    ] {
        assert!(
            endpoint_mod.contains(pattern),
            "handlers/admin/endpoint/mod.rs should stay as a thin endpoint router for {pattern}"
        );
    }
    for pattern in [
        "use self::extractors::{",
        "use self::health_builders::{",
        "use self::payloads::{",
    ] {
        assert!(
            !endpoint_mod.contains(pattern),
            "handlers/admin/endpoint/mod.rs should not re-export local helper seam {pattern}"
        );
    }
    assert!(
        endpoint_mod.contains(
            "pub(crate) use self::health_builders::build_admin_endpoint_health_status_payload;"
        ),
        "handlers/admin/endpoint/mod.rs should keep only the crate-facing health status payload seam"
    );

    let system_core_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/system/core/mod.rs");
    for pattern in [
        "maybe_build_local_admin_model_catalog_response",
    ] {
        assert!(
            system_core_mod.contains(pattern),
            "handlers/admin/system/core/mod.rs should call the real owner {pattern}"
        );
    }

    let system_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/system/routes.rs");
    for pattern in [
        "core::maybe_build_local_admin_core_response(",
        "adaptive::maybe_build_local_admin_adaptive_response(",
        "pool_admin::maybe_build_local_admin_pool_response(",
        "proxy_nodes::maybe_build_local_admin_proxy_nodes_response(",
    ] {
        assert!(
            system_routes.contains(pattern),
            "handlers/admin/system/routes.rs should dispatch through specific system owner {pattern}"
        );
    }
    for path in [
        "apps/aether-gateway/src/handlers/admin/system/core/management_tokens_routes.rs",
        "apps/aether-gateway/src/handlers/admin/system/core/model_routes.rs",
        "apps/aether-gateway/src/handlers/admin/system/core/modules_routes.rs",
        "apps/aether-gateway/src/handlers/admin/system/core/oauth_routes.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be deleted once system/core/mod.rs dispatches directly to real owners"
        );
    }

    let system_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/system/core/system_routes.rs");
    assert!(
        !system_routes.contains("use crate::handlers::public::{"),
        "handlers/admin/system/core/system_routes.rs should not borrow system-owned route helpers from handlers/public"
    );
    assert!(
        !system_routes.contains("crate::handlers::admin::auth::build_proxy_error_response")
            && !system_routes.contains("use crate::handlers::admin::auth::build_proxy_error_response;"),
        "handlers/admin/system/core/system_routes.rs should not borrow proxy error builder from auth"
    );
    let endpoint_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/endpoint/routes.rs");
    assert!(
        endpoint_routes.contains(
            "endpoint_keys::maybe_build_local_admin_endpoints_keys_response"
        ),
        "handlers/admin/endpoint/routes.rs should dispatch provider key management directly to provider::endpoint_keys"
    );

    assert!(
        endpoint_routes.contains(
            "endpoints_admin::maybe_build_local_admin_endpoints_routes_response"
        ),
        "handlers/admin/endpoint/routes.rs should dispatch provider endpoint CRUD directly to provider::endpoints_admin"
    );
    {
        let path = "apps/aether-gateway/src/handlers/admin/endpoint/keys.rs";
        assert!(
            !workspace_file_exists(path),
            "{path} should be deleted once endpoint root dispatches directly to provider-owned handlers"
        );
    }
}

#[test]
fn admin_model_root_owns_model_catalog_routes() {
    let model_mod = read_workspace_file("apps/aether-gateway/src/handlers/admin/model/mod.rs");
    assert!(
        model_mod.contains("mod catalog_routes;"),
        "handlers/admin/model/mod.rs should register catalog_routes owner"
    );
    assert!(
        model_mod.contains(
            "pub(super) use self::catalog_routes::maybe_build_local_admin_model_catalog_response;"
        ),
        "handlers/admin/model/mod.rs should expose model catalog route seam"
    );

    let model_catalog_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/model/catalog_routes.rs");
    for pattern in [
        "build_admin_model_catalog_payload",
        "read_admin_external_models_cache",
        "clear_admin_external_models_cache",
        "ADMIN_MODEL_CATALOG_DATA_UNAVAILABLE_DETAIL",
    ] {
        assert!(
            model_catalog_routes.contains(pattern),
            "handlers/admin/model/catalog_routes.rs should own {pattern}"
        );
    }
}

