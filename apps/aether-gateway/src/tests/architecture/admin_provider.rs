use super::*;








#[test]
fn admin_provider_endpoints_admin_mod_uses_specific_route_owners() {
    let endpoints_admin_mod = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/mod.rs",
    );
    for pattern in [
        "mod create;",
        "mod defaults;",
        "mod delete;",
        "mod detail;",
        "mod list;",
        "mod reads;",
        "mod update;",
        "create::maybe_handle(state, request_context, request_body)",
        "update::maybe_handle(state, request_context, request_body)",
        "delete::maybe_handle(state, request_context, request_body)",
        "list::maybe_handle(state, request_context, request_body)",
        "detail::maybe_handle(state, request_context, request_body)",
        "defaults::maybe_handle(state, request_context, request_body)",
    ] {
        assert!(
            endpoints_admin_mod.contains(pattern),
            "handlers/admin/provider/endpoints_admin/mod.rs should dispatch through explicit route owner {pattern}"
        );
    }

    for forbidden in [
        "mod builders;",
        "mod read_routes;",
        "mod write_routes;",
        "super::builders::",
        "read_routes::maybe_build_local_admin_endpoints_read_response",
        "write_routes::maybe_build_local_admin_endpoints_write_response",
    ] {
        assert!(
            !endpoints_admin_mod.contains(forbidden),
            "handlers/admin/provider/endpoints_admin/mod.rs should not keep route bus seam {forbidden}"
        );
    }

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/create.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/update.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/delete.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/list.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/detail.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/defaults.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/reads.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should exist once endpoints_admin dispatches through specific route owners"
        );
    }

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/builders.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/read_routes.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/write_routes.rs",
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/writes.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be deleted once endpoints_admin stops routing through read/write buses"
        );
    }

    let endpoints_admin_reads = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/reads.rs",
    );
    for pattern in [
        "pub(crate) async fn build_admin_provider_endpoints_payload(",
        "pub(crate) async fn build_admin_endpoint_payload(",
    ] {
        assert!(
            endpoints_admin_reads.contains(pattern),
            "handlers/admin/provider/endpoints_admin/reads.rs should own {pattern}"
        );
    }

    let request_provider_builders =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/request/provider/builders.rs");
    for pattern in [
        "pub(crate) async fn build_admin_create_provider_endpoint_record(",
        "pub(crate) async fn build_admin_update_provider_endpoint_record(",
    ] {
        assert!(
            request_provider_builders.contains(pattern),
            "handlers/admin/request/provider/builders.rs should own {pattern}"
        );
    }

    for (path, expected) in [
        (
            "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/create.rs",
            ".build_admin_create_provider_endpoint_record(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/update.rs",
            ".build_admin_update_provider_endpoint_record(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/list.rs",
            "use super::reads::build_admin_provider_endpoints_payload;",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/endpoints_admin/detail.rs",
            "use super::reads::build_admin_endpoint_payload;",
        ),
    ] {
        let contents = read_workspace_file(path);
        assert!(
            contents.contains(expected),
            "{path} should import or delegate through explicit endpoint owner {expected}"
        );
        assert!(
            !contents.contains("super::builders::"),
            "{path} should not depend on the removed endpoints_admin::builders hub"
        );
        assert!(
            !contents.contains("super::writes::"),
            "{path} should not depend on the removed endpoints_admin::writes owner"
        );
    }
}

#[test]
fn admin_provider_ops_routes_directoryized() {
    let routes_mod_path =
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/mod.rs";
    assert!(
        workspace_file_exists(routes_mod_path),
        "handlers/admin/provider/ops/providers/routes/mod.rs must exist after directoryizing provider ops routes"
    );
    let routes_mod = read_workspace_file(routes_mod_path);

    for pattern in [
        "mod batch;",
        "mod config;",
        "mod verify;",
        "mod connect;",
        "mod actions;",
        "mod read;",
    ] {
        assert!(
            routes_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/routes/mod.rs should register {pattern}"
        );
    }

    for pattern in [
        "batch::handle_admin_provider_ops_batch_balance(",
        "config::handle_admin_provider_ops_save_config(",
        "verify::handle_admin_provider_ops_verify(",
        "connect::handle_admin_provider_ops_connect(",
        "actions::handle_admin_provider_ops_action(",
        "read::handle_admin_provider_ops_read(",
    ] {
        assert!(
            routes_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/routes/mod.rs should delegate {pattern} to the owner module"
        );
    }

    assert!(
        routes_mod.contains(
            "pub(crate) async fn maybe_build_local_admin_provider_ops_providers_response("
        ),
        "handlers/admin/provider/ops/providers/routes/mod.rs should keep the provider ops entry seam"
    );
    for forbidden in [
        "admin_provider_ops_local_action_response(",
        "build_admin_provider_ops_saved_config_value(",
        "admin_provider_ops_local_verify_response(",
        "build_admin_provider_ops_status_payload(",
        "build_admin_provider_ops_config_payload(",
    ] {
        assert!(
            !routes_mod.contains(forbidden),
            "handlers/admin/provider/ops/providers/routes/mod.rs should not own provider ops implementation {forbidden}"
        );
    }
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes.rs"
        ),
        "handlers/admin/provider/ops/providers/routes.rs should be removed once routes are directoryized"
    );

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/batch.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/config.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/verify.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/connect.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/actions.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/read.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should exist after directoryizing provider ops routes"
        );
    }
}

#[test]
fn admin_provider_ops_route_owners_stay_explicit() {
    let batch = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/batch.rs",
    );
    assert!(
        batch.contains("super::super::actions::admin_provider_ops_local_action_response"),
        "batch.rs should depend directly on actions owner"
    );

    let config = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/config.rs",
    );
    assert!(
        config.contains("super::super::config::build_admin_provider_ops_saved_config_value"),
        "config.rs should depend directly on config owner"
    );

    let verify = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/verify.rs",
    );
    assert!(
        verify.contains("super::super::verify::admin_provider_ops_local_verify_response"),
        "verify.rs should depend directly on gateway verify runtime owner"
    );

    let connect = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/connect.rs",
    );
    assert!(
        connect.contains("super::super::support::{"),
        "connect.rs should depend directly on support owner"
    );

    let actions = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/actions.rs",
    );
    assert!(
        actions.contains("super::super::actions::{"),
        "actions.rs should depend directly on actions owner"
    );

    let read = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/routes/read.rs",
    );
    assert!(
        read.contains("super::super::config::{"),
        "read.rs should depend directly on config payload owner"
    );
}

#[test]
fn admin_provider_ops_architecture_registry_uses_pure_owner() {
    let architectures =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/ops/architectures.rs");
    for pattern in [
        "use aether_admin::provider::ops::{
get_architecture,
list_architectures,
};",
        "list_architectures(false)",
        "get_architecture(architecture_id)",
    ] {
        assert!(
            architectures.contains(pattern),
            "handlers/admin/provider/ops/architectures.rs should delegate architecture registry to pure owner {pattern}"
        );
    }
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/architectures.all.json"
        ),
        "handlers/admin/provider/ops/architectures.all.json should be removed after moving architecture registry into aether-admin"
    );
}

#[test]
fn admin_provider_summary_mod_stays_thin() {
    let summary_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/summary/mod.rs");
    for pattern in [
        "mod aggregates;",
        "mod health;",
        "mod list;",
        "mod value;",
        "pub(crate) use self::aggregates::{",
        "pub(crate) use self::health::build_admin_provider_health_monitor_payload;",
        "pub(crate) use self::list::build_admin_providers_payload;",
        "pub(crate) use self::value::build_admin_provider_summary_value;",
    ] {
        assert!(
            summary_mod.contains(pattern),
            "handlers/admin/provider/summary/mod.rs should keep explicit summary boundary {pattern}"
        );
    }
    for forbidden in [
        "pub(crate) async fn build_admin_providers_payload(",
        "pub(crate) async fn build_admin_provider_summary_payload(",
        "pub(crate) async fn build_admin_providers_summary_payload(",
        "pub(crate) async fn build_admin_provider_health_monitor_payload(",
        "pub(crate) fn build_admin_provider_summary_value(",
    ] {
        assert!(
            !summary_mod.contains(forbidden),
            "handlers/admin/provider/summary/mod.rs should not own concrete summary implementation {forbidden}"
        );
    }

    assert!(
        !workspace_file_exists("apps/aether-gateway/src/handlers/admin/provider/summary.rs"),
        "handlers/admin/provider/summary.rs should be removed once provider summary is directoryized"
    );

    for (path, expected) in [
        (
            "apps/aether-gateway/src/handlers/admin/provider/summary/list.rs",
            "pub(crate) async fn build_admin_providers_payload(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/summary/value.rs",
            "pub(crate) fn build_admin_provider_summary_value(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/summary/aggregates.rs",
            "pub(crate) async fn build_admin_provider_summary_payload(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/summary/health.rs",
            "pub(crate) async fn build_admin_provider_health_monitor_payload(",
        ),
    ] {
        let contents = read_workspace_file(path);
        assert!(contents.contains(expected), "{path} should own {expected}");
    }
}

#[test]
fn admin_provider_strategy_uses_shared_billing_normalizers() {
    let strategy_builders =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/strategy/builders.rs");
    assert!(
        !strategy_builders.contains("use super::super::write::{"),
        "handlers/admin/provider/strategy/builders.rs should not borrow billing/time normalizers from provider::write"
    );
    assert!(
        strategy_builders.contains("crate::handlers::admin::provider::shared::support::{"),
        "handlers/admin/provider/strategy/builders.rs should import shared provider normalizers from provider::shared::support"
    );

    let provider_shared_support =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/shared/support.rs");
    for pattern in [
        "pub(crate) fn normalize_provider_billing_type(",
        "pub(crate) fn parse_optional_rfc3339_unix_secs(",
    ] {
        assert!(
            provider_shared_support.contains(pattern),
            "handlers/admin/provider/shared/support.rs should own provider-wide billing/time normalizer {pattern}"
        );
    }
}


#[test]
fn admin_provider_query_and_strategy_use_specific_local_owners() {
    let query_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/query/mod.rs");
    for pattern in ["mod payload;", "mod response;", "mod routes;"] {
        assert!(
            query_mod.contains(pattern),
            "handlers/admin/provider/query/mod.rs should register specific local owner {pattern}"
        );
    }
    assert!(
        !query_mod.contains("mod shared;"),
        "handlers/admin/provider/query/mod.rs should not retain a generic shared module"
    );

    let query_model_owners = read_workspace_module_tree(
        "apps/aether-gateway/src/handlers/admin/provider/query/models/mod.rs",
    );
    assert!(
        !query_model_owners.contains("super::shared::{"),
        "handlers/admin/provider/query/models should not depend on a generic query::shared hub"
    );

    let path = "apps/aether-gateway/src/handlers/admin/provider/query/routes.rs";
    let contents = read_workspace_file(path);
    assert!(
        !contents.contains("super::shared::{"),
        "{path} should not depend on a generic query::shared hub"
    );

    assert!(
        workspace_file_exists("apps/aether-gateway/src/handlers/admin/provider/query/payload.rs"),
        "handlers/admin/provider/query/payload.rs should own provider query parsing and extractors"
    );
    assert!(
        workspace_file_exists("apps/aether-gateway/src/handlers/admin/provider/query/response.rs"),
        "handlers/admin/provider/query/response.rs should own provider query response helpers"
    );
    let query_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/query/routes.rs");
    assert!(
        query_routes.contains("state\n        .maybe_build_admin_provider_query_route_response(")
            || query_routes.contains("state.maybe_build_admin_provider_query_route_response("),
        "handlers/admin/provider/query/routes.rs should delegate to request/provider route owner"
    );

    let strategy_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/strategy/mod.rs");
    for pattern in ["mod builders;", "mod responses;", "mod routes;"] {
        assert!(
            strategy_mod.contains(pattern),
            "handlers/admin/provider/strategy/mod.rs should register specific local owner {pattern}"
        );
    }
    assert!(
        !strategy_mod.contains("mod shared;"),
        "handlers/admin/provider/strategy/mod.rs should not retain a generic shared module"
    );

    let strategy_routes =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/strategy/routes.rs");
    assert!(
        strategy_routes
            .contains("state\n        .maybe_build_admin_provider_strategy_route_response(")
            || strategy_routes
                .contains("state.maybe_build_admin_provider_strategy_route_response("),
        "handlers/admin/provider/strategy/routes.rs should delegate to request/provider route owner"
    );
    assert!(
        !strategy_routes.contains("use super::shared::{")
            && !strategy_routes.contains("use super::responses::{")
            && !strategy_routes.contains("use super::builders::{"),
        "handlers/admin/provider/strategy/routes.rs should stay as a thin bridge without local implementation imports"
    );

    let strategy_builders =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/strategy/builders.rs");
    assert!(
        !strategy_builders.contains("use super::shared::"),
        "handlers/admin/provider/strategy/builders.rs should keep provider-not-found response local"
    );

    assert!(
        workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/strategy/responses.rs"
        ),
        "handlers/admin/provider/strategy/responses.rs should own strategy route-level shared responses"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/strategy/shared.rs"
        ),
        "handlers/admin/provider/strategy/shared.rs should be removed once the local shared hub is narrowed"
    );
}





#[test]
fn admin_provider_ops_providers_mod_stays_thin() {
    let providers_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/ops/providers/mod.rs");
    for pattern in [
        "pub(crate) mod actions;",
        "mod config;",
        "mod routes;",
        "mod support;",
        "mod verify;",
        "pub(super) use self::routes::maybe_build_local_admin_provider_ops_providers_response;",
    ] {
        assert!(
            providers_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/mod.rs should keep explicit boundary {pattern}"
        );
    }
    for pattern in [
        "pub(crate) use self::actions::admin_provider_ops_local_action_response;",
        "const ADMIN_PROVIDER_OPS_SENSITIVE_FIELDS:",
        "const ADMIN_PROVIDER_OPS_CONNECT_RUST_ONLY_MESSAGE:",
        "const ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE:",
        "const ADMIN_PROVIDER_OPS_VERIFY_RUST_ONLY_MESSAGE:",
        "struct AdminProviderOpsSaveConfigRequest",
        "struct AdminProviderOpsConnectRequest",
        "struct AdminProviderOpsExecuteActionRequest",
        "struct AdminProviderOpsCheckinOutcome",
    ] {
        assert!(
            !providers_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/mod.rs should not keep helper/data owner {pattern}"
        );
    }

    let providers_support = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/support.rs",
    );
    for pattern in [
        "pub(super) const ADMIN_PROVIDER_OPS_SENSITIVE_FIELDS:",
        "pub(super) const ADMIN_PROVIDER_OPS_CONNECT_RUST_ONLY_MESSAGE:",
        "pub(super) const ADMIN_PROVIDER_OPS_ACTION_RUST_ONLY_MESSAGE:",
        "pub(super) const ADMIN_PROVIDER_OPS_VERIFY_RUST_ONLY_MESSAGE:",
        "pub(super) struct AdminProviderOpsSaveConfigRequest",
        "pub(super) struct AdminProviderOpsConnectRequest",
        "pub(super) struct AdminProviderOpsExecuteActionRequest",
        "ProviderOpsCheckinOutcome as AdminProviderOpsCheckinOutcome",
    ] {
        assert!(
            providers_support.contains(pattern),
            "handlers/admin/provider/ops/providers/support.rs should own {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/maintenance/runtime.rs",
        "apps/aether-gateway/src/maintenance/runtime/provider_checkin.rs",
    ] {
        let contents = read_workspace_file(path);
        assert!(
            (contents.contains("admin_api::admin_provider_ops_local_action_response")
                || (contents.contains("use crate::admin_api::{")
                    && contents.contains("admin_provider_ops_local_action_response"))),
            "{path} should call provider ops action helper through crate::admin_api facade"
        );
        assert!(
            !contents.contains(
                "provider::ops::providers::actions::admin_provider_ops_local_action_response"
            ) && !contents
                .contains("provider::ops::providers::admin_provider_ops_local_action_response"),
            "{path} should not depend on provider ops internal module paths"
        );
    }
}

#[test]
fn admin_provider_ops_actions_mod_stays_thin() {
    let actions_mod = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/mod.rs",
    );
    for pattern in [
        "mod checkin;",
        "mod query_balance;",
        "mod responses;",
        "mod support;",
        "pub(super) fn admin_provider_ops_is_valid_action_type(",
        "pub(crate) async fn admin_provider_ops_local_action_response(",
    ] {
        assert!(
            actions_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/mod.rs should keep thin entry seam {pattern}"
        );
    }
    for forbidden in [
        "fn admin_provider_ops_action_response(",
        "fn admin_provider_ops_checkin_payload(",
        "fn admin_provider_ops_new_api_balance_payload(",
        "fn admin_provider_ops_yescode_balance_payload(",
        "fn admin_provider_ops_run_checkin_action(",
        "fn admin_provider_ops_run_query_balance_action(",
    ] {
        assert!(
            !actions_mod.contains(forbidden),
            "handlers/admin/provider/ops/providers/actions/mod.rs should not keep helper owner {forbidden}"
        );
    }
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions.rs"
        ),
        "handlers/admin/provider/ops/providers/actions.rs should be removed once actions logic is directoryized"
    );

    let actions_responses = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/responses.rs",
    );
    for pattern in [
        "pub(super) fn admin_provider_ops_action_response(",
        "pub(super) fn admin_provider_ops_action_error(",
        "pub(super) fn admin_provider_ops_action_not_configured(",
        "pub(super) fn admin_provider_ops_action_not_supported(",
    ] {
        assert!(
            actions_responses.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/responses.rs should own {pattern}"
        );
    }

    let actions_support = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/support.rs",
    );
    for pattern in [
        "pub(super) fn admin_provider_ops_checkin_data(",
        "pub(super) fn admin_provider_ops_json_object_map(",
        "pub(super) fn admin_provider_ops_request_url(",
        "pub(super) fn admin_provider_ops_request_method(",
        "pub(super) fn admin_provider_ops_parse_rfc3339_unix_secs(",
    ] {
        assert!(
            actions_support.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/support.rs should own {pattern}"
        );
    }

    let actions_checkin_mod = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/checkin/mod.rs",
    );
    for pattern in [
        "mod probe;",
        "mod run;",
        "mod shared;",
        "pub(super) use probe::admin_provider_ops_probe_new_api_checkin;",
        "pub(super) use run::admin_provider_ops_run_checkin_action;",
    ] {
        assert!(
            actions_checkin_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/checkin/mod.rs should keep thin checkin entry seam {pattern}"
        );
    }
    let actions_checkin_shared = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/checkin/shared.rs",
    );
    for pattern in [
        "pub(super) fn admin_provider_ops_checkin_already_done(",
        "pub(super) fn admin_provider_ops_checkin_auth_failure(",
        "pub(super) fn admin_provider_ops_checkin_payload(",
    ] {
        assert!(
            actions_checkin_shared.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/checkin/shared.rs should own {pattern}"
        );
    }
    let actions_checkin_probe = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/checkin/probe.rs",
    );
    assert!(
        actions_checkin_probe.contains("async fn admin_provider_ops_probe_new_api_checkin("),
        "handlers/admin/provider/ops/providers/actions/checkin/probe.rs should own new-api probe flow"
    );
    let actions_checkin_run = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/checkin/run.rs",
    );
    assert!(
        actions_checkin_run.contains("async fn admin_provider_ops_run_checkin_action("),
        "handlers/admin/provider/ops/providers/actions/checkin/run.rs should own checkin action execution"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/checkin.rs"
        ),
        "handlers/admin/provider/ops/providers/actions/checkin.rs should be removed once checkin is directoryized"
    );

    let actions_query_balance_mod = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/query_balance/mod.rs",
    );
    for pattern in [
        "mod sub2api;",
        "mod yescode;",
        "pub(super) async fn admin_provider_ops_run_query_balance_action(",
        "parse_query_balance_payload(",
        "yescode::admin_provider_ops_yescode_balance_payload(",
        "sub2api::admin_provider_ops_sub2api_balance_payload(",
    ] {
        assert!(
            actions_query_balance_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/actions/query_balance/mod.rs should keep thin query_balance entry seam {pattern}"
        );
    }
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/query_balance/parsers.rs"
        ),
        "handlers/admin/provider/ops/providers/actions/query_balance/parsers.rs should be removed after moving balance parsing into aether-admin"
    );
    let pure_actions = read_workspace_file("crates/aether-admin/src/provider/ops/actions.rs");
    for pattern in [
        "pub fn parse_query_balance_payload(",
        "pub fn parse_sub2api_balance_payload(",
        "pub fn parse_yescode_combined_balance_payload(",
        "pub fn attach_balance_checkin_outcome(",
    ] {
        assert!(
            pure_actions.contains(pattern),
            "crates/aether-admin/src/provider/ops/actions.rs should own {pattern}"
        );
    }
    let actions_query_balance_yescode = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/query_balance/yescode.rs",
    );
    assert!(
        actions_query_balance_yescode
            .contains("pub(super) async fn admin_provider_ops_yescode_balance_payload("),
        "handlers/admin/provider/ops/providers/actions/query_balance/yescode.rs should own yescode balance flow"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/handlers/admin/provider/ops/providers/actions/query_balance.rs"
        ),
        "handlers/admin/provider/ops/providers/actions/query_balance.rs should be removed once query_balance is directoryized"
    );
}

#[test]
fn admin_provider_ops_verify_runtime_and_pure_owners_stay_explicit() {
    let verify_mod = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/mod.rs",
    );
    for pattern in [
        "mod proxy;",
        "mod request;",
        "mod sub2api;",
        "pub(super) async fn admin_provider_ops_local_verify_response(",
    ] {
        assert!(
            verify_mod.contains(pattern),
            "handlers/admin/provider/ops/providers/verify/mod.rs should keep runtime verify entry seam {pattern}"
        );
    }

    let verify_proxy = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/proxy.rs",
    );
    for pattern in [
        "struct AdminProviderOpsAnyrouterChallenge",
        "fn admin_provider_ops_anyrouter_acw_cookie(",
        "fn admin_provider_ops_resolve_proxy_snapshot(",
    ] {
        assert!(
            verify_proxy.contains(pattern),
            "handlers/admin/provider/ops/providers/verify/proxy.rs should own {pattern}"
        );
    }

    let verify_request = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/request.rs",
    );
    for pattern in [
        "fn admin_provider_ops_execute_get_json(",
        "fn admin_provider_ops_execute_proxy_json_request(",
        "fn admin_provider_ops_verify_execution_error_message(",
    ] {
        assert!(
            verify_request.contains(pattern),
            "handlers/admin/provider/ops/providers/verify/request.rs should own {pattern}"
        );
    }

    let verify_sub2api = read_workspace_file(
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/sub2api.rs",
    );
    for pattern in [
        "fn admin_provider_ops_local_sub2api_verify_response(",
        "fn admin_provider_ops_sub2api_exchange_token(",
    ] {
        assert!(
            verify_sub2api.contains(pattern),
            "handlers/admin/provider/ops/providers/verify/sub2api.rs should own {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/helpers.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/headers.rs",
        "apps/aether-gateway/src/handlers/admin/provider/ops/providers/verify/payload.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after splitting verify runtime owners"
        );
    }

    let pure_verify = read_workspace_file("crates/aether-admin/src/provider/ops/verify.rs");
    for pattern in ["pub fn build_headers(", "pub fn parse_verify_payload("] {
        assert!(
            pure_verify.contains(pattern),
            "crates/aether-admin/src/provider/ops/verify.rs should own {pattern}"
        );
    }
}











#[test]
fn admin_provider_models_own_provider_model_builders() {
    let provider_models_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/models/mod.rs");
    {
        let pattern = "mod payloads;";
        assert!(
            provider_models_mod.contains(pattern),
            "handlers/admin/provider/models/mod.rs should register local provider-model owner module {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/models/list.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/detail.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/create.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/update.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/batch.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/import.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/available_source.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/assign_global.rs",
    ] {
        let contents = read_workspace_file(path);
        for forbidden in [
            "super::super::super::model::",
            "crate::handlers::admin::model::",
        ] {
            assert!(
                !contents.contains(forbidden),
                "{path} should not borrow provider-model builders from admin/model via {forbidden}"
            );
        }
    }

    let model_mod = read_workspace_file("apps/aether-gateway/src/handlers/admin/model/mod.rs");
    for forbidden in [
        "admin_provider_model_name_exists",
        "build_admin_provider_model_payload",
        "build_admin_provider_model_response",
        "build_admin_provider_models_payload",
        "build_admin_provider_model_create_record",
        "build_admin_provider_model_update_record",
        "build_admin_provider_available_source_models_payload",
        "build_admin_batch_assign_global_models_payload",
        "build_admin_import_provider_models_payload",
    ] {
        assert!(
            !model_mod.contains(forbidden),
            "handlers/admin/model/mod.rs should not export provider-model owner {forbidden}"
        );
    }
}

#[test]
fn admin_provider_models_write_is_absorbed_by_wrapped_state() {
    let models_mod =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/provider/models/mod.rs");
    assert!(
        !models_mod.contains("mod write;"),
        "handlers/admin/provider/models/mod.rs should no longer retain the transitional write module"
    );

    for path in [
        "apps/aether-gateway/src/handlers/admin/provider/models/write.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/mod.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/shared.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/records.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/imports.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/batch_assign.rs",
        "apps/aether-gateway/src/handlers/admin/provider/models/write/available_source.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed once provider-model write owners are absorbed by wrapped state"
        );
    }

    let request_models =
        read_workspace_file("apps/aether-gateway/src/handlers/admin/request/models.rs");
    for pattern in [
        "pub(crate) async fn build_admin_provider_model_create_record(",
        "pub(crate) async fn build_admin_provider_model_update_record(",
        "pub(crate) async fn build_admin_import_provider_models_payload(",
        "pub(crate) async fn build_admin_batch_assign_global_models_payload(",
        "pub(crate) async fn build_admin_provider_available_source_models_payload(",
        "pub(crate) async fn admin_provider_model_name_exists(",
        "pub(crate) async fn resolve_admin_global_model_by_id_or_err(",
    ] {
        assert!(
            request_models.contains(pattern),
            "handlers/admin/request/models.rs should own {pattern}"
        );
    }

    for (path, expected) in [
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/create.rs",
            ".build_admin_provider_model_create_record(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/update.rs",
            ".build_admin_provider_model_update_record(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/import.rs",
            ".build_admin_import_provider_models_payload(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/assign_global.rs",
            ".build_admin_batch_assign_global_models_payload(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/available_source.rs",
            ".build_admin_provider_available_source_models_payload(",
        ),
        (
            "apps/aether-gateway/src/handlers/admin/provider/models/batch.rs",
            ".build_admin_provider_model_create_record(",
        ),
    ] {
        let contents = read_workspace_file(path);
        assert!(
            contents.contains(expected),
            "{path} should delegate provider-model write flows through wrapped state {expected}"
        );
        assert!(
            !contents.contains("super::write::"),
            "{path} should not retain the removed provider/models/write seam"
        );
    }
}
