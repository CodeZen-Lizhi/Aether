use super::*;

#[test]
fn specialized_decisions_embed_provider_failover_policy_in_report_context() {
    for path in [
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/image/decision.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/decision.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("append_local_failover_policy_to_value(report_context, &transport)"),
            "{path} should embed provider failover policy in every generated report context"
        );
    }
}

#[test]
fn ai_serving_target_structure_removes_legacy_pipeline_boundary() {
    assert!(
        !workspace_file_exists("crates/aether-ai-pipeline"),
        "legacy aether-ai-pipeline crate should be fully removed"
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_pipeline"),
        "gateway ai_pipeline module should be fully removed"
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_pipeline_api.rs"),
        "gateway ai_pipeline_api facade should be fully removed"
    );

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let forbidden = [
        "aether-ai-pipeline",
        "aether_ai_pipeline",
        "ai_pipeline::",
        "ai_pipeline_api",
        "PipelineFinalizeError",
    ];
    let mut violations = Vec::new();
    for root in [
        "apps/aether-gateway/src",
        "crates/aether-ai/serving/src",
        "crates/aether-ai/formats/src",
    ] {
        for file in collect_workspace_rust_files(root) {
            let relative = file
                .canonicalize()
                .expect("workspace file should canonicalize")
                .strip_prefix(&workspace_root)
                .expect("workspace file should be under workspace root")
                .to_string_lossy()
                .replace('\\', "/");
            if relative.starts_with("apps/aether-gateway/src/tests/") {
                continue;
            }
            let source = std::fs::read_to_string(&file).expect("source file should be readable");
            let hits = forbidden
                .iter()
                .filter(|pattern| source.contains(**pattern))
                .copied()
                .collect::<Vec<_>>();
            if !hits.is_empty() {
                violations.push(format!("{relative} -> {}", hits.join(", ")));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "legacy ai-pipeline identifiers should not remain in active AI serving code:\n{}",
        violations.join("\n")
    );

    let workspace_manifest = read_workspace_file("Cargo.toml");
    assert!(
        !workspace_manifest.contains("aether-ai-pipeline"),
        "workspace manifest should not register the retired AI pipeline crate"
    );
    let gateway_manifest = read_workspace_file("apps/aether-gateway/Cargo.toml");
    assert!(
        !gateway_manifest.contains("aether-ai-pipeline"),
        "gateway manifest should not depend on the retired AI pipeline crate"
    );
}


#[test]
fn ai_serving_internal_dtos_use_ai_execution_names() {
    let serving_dto = read_workspace_file("crates/aether-ai/serving/src/dto.rs");
    for expected in [
        "pub struct AiExecutionDecision",
        "pub struct AiExecutionPlanPayload",
        "pub struct AiSyncAttempt",
        "pub struct AiStreamAttempt",
    ] {
        assert!(
            serving_dto.contains(expected),
            "aether-ai-serving dto.rs should own {expected}"
        );
    }

    let gateway_root = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        gateway_root.contains("AiExecutionDecision")
            && gateway_root.contains("AiExecutionPlanPayload")
            && gateway_root.contains("AiSyncAttempt")
            && gateway_root.contains("AiStreamAttempt"),
        "gateway ai_serving root should expose serving-owned DTO names"
    );

    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let legacy_names = [
        ["GatewayControl", "SyncDecisionResponse"].concat(),
        ["GatewayControl", "PlanResponse"].concat(),
        ["LocalSync", "PlanAndReport"].concat(),
        ["LocalStream", "PlanAndReport"].concat(),
    ];
    let mut violations = Vec::new();
    for root in [
        "apps/aether-gateway/src/ai_serving",
        "apps/aether-gateway/src/executor",
        "apps/aether-gateway/src/execution_runtime",
        "crates/aether-ai/serving/src",
        "crates/aether-ai/formats/src",
    ] {
        for file in collect_workspace_rust_files(root) {
            let relative = file
                .canonicalize()
                .expect("workspace file should canonicalize")
                .strip_prefix(&workspace_root)
                .expect("workspace file should be under workspace root")
                .to_string_lossy()
                .replace('\\', "/");
            let source = std::fs::read_to_string(&file).expect("source file should be readable");
            let hits = legacy_names
                .iter()
                .filter_map(|pattern| {
                    source
                        .contains(pattern.as_str())
                        .then_some(pattern.as_str())
                })
                .collect::<Vec<_>>();
            if !hits.is_empty() {
                violations.push(format!("{relative} -> {}", hits.join(", ")));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "AI serving code should not retain legacy GatewayControl*/Local* DTO names:\n{}",
        violations.join("\n")
    );
}

#[test]
fn ai_format_crate_stays_free_of_gateway_runtime_deps() {
    for manifest_path in ["crates/aether-ai/formats/Cargo.toml"] {
        let manifest = read_workspace_file(manifest_path);
        for forbidden in [
            "axum",
            "sqlx",
            "redis",
            "aether-gateway",
            "aether-usage-runtime",
            "aether-provider-transport",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "{manifest_path} should not depend on gateway/runtime adapter dependency {forbidden}"
            );
        }
    }

    let mut violations = Vec::new();
    for root in ["crates/aether-ai/formats/src"] {
        for file in collect_workspace_rust_files(root) {
            let source = std::fs::read_to_string(&file).expect("source file should be readable");
            let hits = [
                "AppState",
                "axum::",
                "sqlx::",
                "redis::",
                "GatewaySyncReportRequest",
                "aether_usage_runtime",
                "aether_provider_transport",
            ]
            .iter()
            .filter(|pattern| source.contains(**pattern))
            .copied()
            .collect::<Vec<_>>();
            if !hits.is_empty() {
                violations.push(format!("{} -> {}", file.display(), hits.join(", ")));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "format crate should stay pure of gateway runtime dependencies:\n{}",
        violations.join("\n")
    );
}

#[test]
fn aether_runtime_stays_free_of_ai_serving_policy() {
    let runtime_manifest = read_workspace_file("crates/aether-runtime/base/Cargo.toml");
    for forbidden in [
        "aether-ai-serving",
        "aether-ai-formats",
        "aether-provider-transport",
        "aether-gateway",
    ] {
        assert!(
            !runtime_manifest.contains(forbidden),
            "aether-runtime should not depend on AI serving/pure/gateway crate {forbidden}"
        );
    }

    let mut violations = Vec::new();
    for file in collect_workspace_rust_files("crates/aether-runtime/base/src") {
        let source = std::fs::read_to_string(&file).expect("source file should be readable");
        let hits = [
            "aether_ai_serving",
            "aether_ai_formats",
            "aether_provider_transport",
            "AiExecution",
            "ExecutionPlan",
            "provider_api_format",
            "client_api_format",
            "OpenAI",
            "OpenAi",
            "Claude",
            "Gemini",
            "finalize",
            "request_candidate",
        ]
        .iter()
        .filter(|pattern| source.contains(**pattern))
        .copied()
        .collect::<Vec<_>>();
        if !hits.is_empty() {
            violations.push(format!("{} -> {}", file.display(), hits.join(", ")));
        }
    }

    assert!(
        violations.is_empty(),
        "aether-runtime should stay execution/runtime infrastructure only, without AI routing, candidate, provider, or finalize policy:\n{}",
        violations.join("\n")
    );
}

#[test]
fn ai_serving_crate_api_is_confined_to_root_seams() {
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
        if relative == "apps/aether-gateway/src/ai_serving/pure/mod.rs"
            || relative == "apps/aether-gateway/src/ai_serving/api.rs"
            || relative.starts_with("apps/aether-gateway/src/tests/")
        {
            continue;
        }

        let source = std::fs::read_to_string(&file).expect("source file should be readable");
        if source.contains("aether_ai_formats::api") {
            violations.push(relative);
        }
    }

    assert!(
        violations.is_empty(),
        "gateway code should only depend on aether_ai_formats::api through ai_serving/pure/mod.rs or ai_serving/api.rs:\n{}",
        violations.join("\n")
    );

    let mut crate_violations = Vec::new();
    for file in collect_workspace_rust_files("apps/aether-gateway/src") {
        let relative = file
            .canonicalize()
            .expect("workspace file should canonicalize")
            .strip_prefix(&workspace_root)
            .expect("workspace file should be under workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "apps/aether-gateway/src/ai_serving/pure/mod.rs"
            || relative == "apps/aether-gateway/src/ai_serving/transport.rs"
            || relative == "apps/aether-gateway/src/ai_serving/api.rs"
            || relative.ends_with("/tests.rs")
            || relative.contains("/tests/")
            || relative.starts_with("apps/aether-gateway/src/tests/")
        {
            continue;
        }

        let source = std::fs::read_to_string(&file).expect("source file should be readable");
        if source.contains("aether_ai_formats::") {
            crate_violations.push(relative);
        }
    }

    assert!(
        crate_violations.is_empty(),
        "gateway code should only depend directly on aether_ai_formats through ai_serving root seams:\n{}",
        crate_violations.join("\n")
    );
}


#[test]
fn ai_serving_routes_provider_transport_deps_through_facade() {
    let patterns = [
        "use crate::provider_transport::",
        "crate::provider_transport::",
    ];

    assert_no_module_dependency_patterns("src/ai_serving/planner", &patterns);
    let mut direct_transport_violations = Vec::new();
    for root in ["apps/aether-gateway/src/ai_serving/planner"] {
        for file in collect_workspace_rust_files(root) {
            let path = file.to_string_lossy().replace('\\', "/");
            if path.ends_with("/tests.rs") || path.contains("/tests/") {
                continue;
            }
            let source = std::fs::read_to_string(&file).expect("source file should be readable");
            let runtime_source = source
                .split("#[cfg(test)]")
                .next()
                .unwrap_or(source.as_str());
            if runtime_source.contains("aether_provider_transport::") {
                direct_transport_violations.push(path);
            }
        }
    }
    assert!(
        direct_transport_violations.is_empty(),
        "gateway ai_serving runtime code should route provider transport through ai_serving/transport.rs:\n{}",
        direct_transport_violations.join("\n")
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/runtime"),
        "ai_serving/runtime should stay removed after facade cleanup"
    );
    assert!(
        !workspace_file_exists("crates/aether-ai/formats/src/transport.rs"),
        "aether-ai-formats should not expose a provider transport bridge"
    );

    let provider_transport_facade =
        read_workspace_file("apps/aether-gateway/src/ai_serving/transport.rs");
    for pattern in [
        "aether_provider_transport::auth",
        "aether_provider_transport::url",
        "aether_provider_transport::policy",
        "aether_provider_transport::snapshot",
    ] {
        assert!(
            provider_transport_facade.contains(pattern),
            "transport.rs should own {pattern}"
        );
    }
    for forbidden in [
        "crate::provider_transport::auth",
        "crate::provider_transport::url",
        "crate::provider_transport::policy",
        "crate::provider_transport::snapshot",
        "aether_ai_formats::transport",
    ] {
        assert!(
            !provider_transport_facade.contains(forbidden),
            "transport.rs should not keep gateway-local provider_transport owner {forbidden}"
        );
    }

    let ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        ai_serving_mod.contains("pub(crate) mod transport;"),
        "ai_serving/mod.rs should expose provider transport capabilities through the root seam module"
    );
    assert!(
        ai_serving_mod.contains("self::transport::build_transport_request_url("),
        "ai_serving/mod.rs should route transport URL construction through ai_serving/transport.rs"
    );
    assert!(
        !ai_serving_mod.contains("crate::provider_transport::"),
        "ai_serving/mod.rs should not bypass the provider transport root seam"
    );
}





#[test]
fn ai_serving_materialization_policy_owns_local_candidate_persistence_modes() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod materialization_policy;"),
        "planner/mod.rs should wire materialization_policy helper module"
    );

    let serving_candidate_persistence_policy =
        read_workspace_file("crates/aether-ai/serving/src/candidate_persistence_policy.rs");
    for pattern in [
        "pub enum AiCandidatePersistencePolicyKind {",
        "pub struct AiCandidatePersistencePolicySpec {",
        "pub fn ai_candidate_persistence_policy_spec(",
        "AiCandidatePersistencePolicyKind::StandardDecision",
        "AiCandidatePersistencePolicyKind::SameFormatProviderDecision",
        "AiCandidatePersistencePolicyKind::OpenAiChatDecision",
        "AiCandidatePersistencePolicyKind::OpenAiResponsesDecision",
        "AiCandidatePersistencePolicyKind::GeminiFilesDecision",
        "AiCandidatePersistencePolicyKind::VideoDecision",
    ] {
        assert!(
            serving_candidate_persistence_policy.contains(pattern),
            "aether-ai-serving should own candidate persistence policy primitive {pattern}"
        );
    }

    let materialization_policy =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/materialization_policy.rs");
    for pattern in [
        "AiCandidatePersistencePolicyKind as LocalCandidatePersistencePolicyKind",
        "pub(crate) struct LocalCandidatePersistencePolicy<'a> {",
        "pub(crate) fn build_local_candidate_persistence_policy<'a>(",
        "ai_candidate_persistence_policy_spec(kind)",
    ] {
        assert!(
            materialization_policy.contains(pattern),
            "planner/materialization_policy.rs should map serving persistence policy into gateway contexts through {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/family/payload.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/payload.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("build_local_candidate_persistence_policy("),
            "{path} should route candidate persistence policy through planner/materialization_policy.rs"
        );
        assert!(
            source.contains("LocalCandidatePersistencePolicyKind::"),
            "{path} should select a shared materialization policy kind"
        );
        for forbidden in [
            "fn available_candidate_persistence_context(",
            "fn skipped_candidate_persistence_context(",
            "LocalAvailableCandidatePersistenceContext {",
            "LocalSkippedCandidatePersistenceContext {",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not inline persistence policy helper {forbidden}"
            );
        }
    }
}

#[test]
fn ai_serving_candidate_metadata_owns_local_execution_candidate_extra_data_shape() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod candidate_metadata;"),
        "planner/mod.rs should wire candidate_metadata helper module"
    );

    let candidate_metadata =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/candidate_metadata.rs");
    let serving_ranking_metadata =
        read_workspace_file("crates/aether-ai/serving/src/ranking_metadata.rs");
    let serving_candidate_metadata =
        read_workspace_file("crates/aether-ai/serving/src/candidate_metadata.rs");
    for pattern in [
        "pub struct AiCandidateMetadataParts<'a> {",
        "pub fn build_ai_candidate_metadata(",
        "pub fn build_ai_candidate_metadata_from_candidate(",
        "pub fn append_ai_execution_contract_fields_to_value(",
        "pub fn ai_local_execution_contract_for_formats(",
        "\"provider_api_format\"",
        "\"global_model_id\"",
        "\"selected_provider_model_name\"",
        "\"provider_contract\"",
    ] {
        assert!(
            serving_candidate_metadata.contains(pattern),
            "aether-ai-serving should own base candidate metadata shape {pattern}"
        );
    }
    for pattern in [
        "pub fn append_ai_ranking_metadata_to_object(",
        "\"ranking_mode\"",
        "\"priority_mode\"",
        "\"ranking_index\"",
        "\"priority_slot\"",
        "\"promoted_by\"",
        "\"demoted_by\"",
    ] {
        assert!(
            serving_ranking_metadata.contains(pattern),
            "aether-ai-serving should own scheduler ranking metadata field helper {pattern}"
        );
    }
    for pattern in [
        "pub(crate) struct LocalExecutionCandidateMetadataParts<'a> {",
        "pub(crate) fn build_local_execution_candidate_metadata(",
        "pub(crate) fn build_local_execution_candidate_contract_metadata(",
        "append_ai_ranking_metadata_to_object(object, ranking)",
        "build_ai_candidate_metadata_from_candidate(",
        "append_ai_execution_contract_fields_to_value(",
    ] {
        assert!(
            candidate_metadata.contains(pattern),
            "planner/candidate_metadata.rs should adapt candidate metadata through {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("ai_local_execution_contract_for_formats("),
            "{path} should delegate local execution strategy/conversion mode policy to aether-ai-serving"
        );
    }
    for forbidden in [
        "\"global_model_id\".to_string()",
        "\"selected_provider_model_name\".to_string()",
        "\"provider_contract\".to_string()",
    ] {
        assert!(
            !candidate_metadata.contains(forbidden),
            "planner/candidate_metadata.rs should not own base candidate metadata field shape {forbidden}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("build_local_execution_candidate_"),
            "{path} should route candidate persistence metadata through candidate_metadata.rs"
        );
        for forbidden in [
            "\"global_model_id\": eligible.candidate.global_model_id.clone()",
            "\"provider_name\": eligible.candidate.provider_name.clone()",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not inline shared candidate metadata field {forbidden}"
            );
        }
    }
}

#[test]
fn ai_serving_runtime_miss_owns_local_execution_miss_state_machine() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod runtime_miss;"),
        "planner/mod.rs should wire runtime_miss helper module"
    );

    let serving_runtime_miss = read_workspace_file("crates/aether-ai/serving/src/runtime_miss.rs");
    for pattern in [
        "pub trait AiRuntimeMissDiagnosticPort",
        "pub trait AiRuntimeMissDiagnosticFields",
        "pub fn set_ai_runtime_miss_diagnostic_reason",
        "pub fn build_ai_runtime_execution_exhausted_diagnostic",
        "pub fn set_ai_runtime_execution_exhausted_diagnostic",
        "pub fn build_ai_runtime_candidate_evaluation_diagnostic",
        "pub fn set_ai_runtime_candidate_evaluation_diagnostic",
        "pub fn apply_ai_runtime_candidate_evaluation_progress",
        "pub fn apply_ai_runtime_candidate_evaluation_progress_preserving_candidate_signal",
        "pub fn apply_ai_runtime_candidate_terminal_reason",
        "pub fn record_ai_runtime_candidate_skip_reason",
        "pub fn apply_ai_runtime_candidate_evaluation_progress_to_diagnostic",
        "pub fn apply_ai_runtime_candidate_terminal_plan_reason_to_diagnostic",
        "pub fn record_ai_runtime_candidate_skip_reason_on_diagnostic",
    ] {
        assert!(
            serving_runtime_miss.contains(pattern),
            "aether-ai-serving should own runtime miss diagnostic state-machine primitive {pattern}"
        );
    }

    let runtime_miss =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/runtime_miss.rs");
    for pattern in [
        "impl AiRuntimeMissDiagnosticFields for LocalExecutionRuntimeMissDiagnostic",
        "impl AiRuntimeMissDiagnosticPort for GatewayRuntimeMissDiagnosticPort",
        "pub(crate) fn set_local_runtime_miss_diagnostic_reason(",
        "pub(crate) fn build_local_runtime_execution_exhausted_diagnostic(",
        "pub(crate) fn set_local_runtime_execution_exhausted_diagnostic(",
        "pub(crate) fn build_local_runtime_candidate_evaluation_diagnostic(",
        "pub(crate) fn set_local_runtime_candidate_evaluation_diagnostic(",
        "pub(crate) fn apply_local_runtime_candidate_evaluation_progress(",
        "pub(crate) fn apply_local_runtime_candidate_evaluation_progress_preserving_candidate_signal(",
        "pub(crate) fn apply_local_runtime_candidate_terminal_reason(",
        "pub(crate) fn record_local_runtime_candidate_skip_reason(",
        "set_ai_runtime_miss_diagnostic_reason(",
        "build_ai_runtime_execution_exhausted_diagnostic(",
        "apply_ai_runtime_candidate_evaluation_progress_to_diagnostic(",
        "apply_ai_runtime_candidate_terminal_plan_reason_to_diagnostic(",
        "record_ai_runtime_candidate_skip_reason_on_diagnostic(",
        "apply_ai_runtime_candidate_evaluation_progress_preserving_candidate_signal(",
        "record_ai_runtime_candidate_skip_reason(",
    ] {
        assert!(
            runtime_miss.contains(pattern),
            "planner/runtime_miss.rs should adapt gateway runtime miss state through {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/build.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/build.rs",
        "apps/aether-gateway/src/ai_serving/planner/candidate_materialization.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/mod.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/diagnostic.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/sync.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/stream.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("runtime_miss")
                || source.contains("set_local_runtime_")
                || source.contains("apply_local_runtime_"),
            "{path} should route runtime miss state handling through planner/runtime_miss.rs"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/build.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/build.rs",
        "apps/aether-gateway/src/ai_serving/planner/candidate_materialization.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/mod.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/diagnostic.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/sync.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/stream.rs",
    ] {
        let source = read_workspace_file(path);
        for forbidden in [
            "state.set_local_execution_runtime_miss_diagnostic(",
            "state.mutate_local_execution_runtime_miss_diagnostic(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not inline runtime miss state mutation {forbidden}"
            );
        }
    }
}


#[test]
fn ai_serving_same_format_provider_routes_request_preparation_through_request_payload_seams() {
    let same_format_provider_mod = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/mod.rs",
    );
    assert!(
        same_format_provider_mod.contains("mod request;"),
        "same-format provider mod.rs should wire request seam"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/payload/prepare.rs"
        ),
        "same-format provider payload/prepare.rs should stay removed after request seam extraction"
    );

    let same_format_provider_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/request.rs",
    );
    assert!(
        same_format_provider_request.contains("mod prepare;"),
        "same-format provider request.rs should own its nested prepare module"
    );
    assert!(
        !same_format_provider_request.contains("#[path = \"payload/prepare.rs\"]"),
        "same-format provider request.rs should not path-import payload preparation after seam extraction"
    );
    for pattern in [
        "pub(crate) struct LocalSameFormatProviderCandidatePayloadParts {",
        "pub(crate) async fn resolve_local_same_format_provider_candidate_payload_parts(",
    ] {
        assert!(
            same_format_provider_request.contains(pattern),
            "same-format provider request.rs should own {pattern}"
        );
    }

    let same_format_provider_request_prepare = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/request/prepare.rs",
    );
    for pattern in [
        "pub(super) struct PreparedSameFormatProviderCandidate {",
        "pub(super) async fn prepare_local_same_format_provider_candidate(",
    ] {
        assert!(
            same_format_provider_request_prepare.contains(pattern),
            "same-format provider request/prepare.rs should own {pattern}"
        );
    }

    let same_format_provider_payload = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/payload.rs",
    );
    assert!(
        same_format_provider_payload
            .contains("resolve_local_same_format_provider_candidate_payload_parts("),
        "same-format provider payload.rs should consume request.rs preparation output"
    );
    assert!(
        !same_format_provider_payload.contains("prepare_local_same_format_provider_candidate("),
        "same-format provider payload.rs should not inline request preparation after seam extraction"
    );
}

#[test]
fn ai_serving_video_routes_request_preparation_through_request_payload_seams() {
    let specialized_video_mod =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/specialized/video.rs");
    assert!(
        specialized_video_mod.contains("mod request;"),
        "specialized video mod.rs should wire request seam"
    );

    let specialized_video_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/request.rs",
    );
    for pattern in [
        "pub(super) struct LocalVideoCreateCandidatePayloadParts {",
        "pub(super) async fn resolve_local_video_create_candidate_payload_parts(",
        "build_video_create_request_body(",
        "build_video_create_upstream_url(",
        "build_video_create_headers(",
        "provider_video_create_family(",
    ] {
        assert!(
            specialized_video_request.contains(pattern),
            "specialized video request.rs should adapt through provider-transport via {pattern}"
        );
    }
    for forbidden in [
        "fn build_provider_request_body(",
        "fn build_video_upstream_url(",
        "apply_local_body_rules(",
        "apply_local_header_rules(",
        "build_passthrough_headers_with_auth(",
        "build_gemini_video_predict_long_running_url(",
    ] {
        assert!(
            !specialized_video_request.contains(forbidden),
            "specialized video request.rs should not own provider transport policy {forbidden}"
        );
    }

    let provider_transport_video =
        read_workspace_file("crates/aether-provider/transport/src/video/mod.rs");
    for pattern in [
        "pub enum ProviderVideoCreateFamily",
        "pub fn video_create_transport_unsupported_reason(",
        "pub fn resolve_video_create_auth(",
        "pub fn build_video_create_request_body(",
        "pub fn build_video_create_upstream_url(",
        "pub fn build_video_create_headers(",
    ] {
        assert!(
            provider_transport_video.contains(pattern),
            "aether-provider-transport video.rs should own video create transport policy {pattern}"
        );
    }

    let specialized_video_decision = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/decision.rs",
    );
    assert!(
        specialized_video_decision.contains("resolve_local_video_create_candidate_payload_parts("),
        "specialized video decision.rs should consume request.rs preparation output"
    );
    for forbidden in [
        "resolve_candidate_mapped_model(",
        "build_provider_request_body(",
        "build_video_upstream_url(",
        "resolve_local_openai_bearer_auth(",
        "resolve_local_gemini_auth(",
    ] {
        assert!(
            !specialized_video_decision.contains(forbidden),
            "specialized video decision.rs should not inline request preparation step {forbidden}"
        );
    }
}

#[test]
fn ai_serving_files_routes_request_preparation_through_request_payload_seams() {
    let specialized_files_mod =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/specialized/files.rs");
    assert!(
        specialized_files_mod.contains("mod request;"),
        "specialized files mod.rs should wire request seam"
    );

    let specialized_files_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/request.rs",
    );
    for pattern in [
        "pub(super) struct LocalGeminiFilesCandidatePayloadParts {",
        "pub(super) async fn resolve_local_gemini_files_candidate_payload_parts(",
        "build_gemini_files_upstream_url(",
        "build_gemini_files_request_body(",
        "build_gemini_files_headers(",
    ] {
        assert!(
            specialized_files_request.contains(pattern),
            "specialized files request.rs should adapt through provider-transport via {pattern}"
        );
    }
    for forbidden in [
        "build_gemini_files_passthrough_url(",
        "build_passthrough_headers_with_auth(",
        "apply_local_body_rules(",
        "apply_local_header_rules(",
        "resolve_local_gemini_auth(",
        "local_gemini_transport_unsupported_reason_with_network(",
    ] {
        assert!(
            !specialized_files_request.contains(forbidden),
            "specialized files request.rs should not own provider transport policy {forbidden}"
        );
    }

    let provider_transport_files =
        read_workspace_file("crates/aether-provider/transport/src/gemini_files/mod.rs");
    for pattern in [
        "pub fn gemini_files_transport_unsupported_reason(",
        "pub fn resolve_gemini_files_auth(",
        "pub fn build_gemini_files_upstream_url(",
        "pub fn build_gemini_files_request_body(",
        "pub fn build_gemini_files_headers(",
    ] {
        assert!(
            provider_transport_files.contains(pattern),
            "aether-provider-transport gemini_files.rs should own files transport policy {pattern}"
        );
    }

    let specialized_files_decision = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
    );
    assert!(
        specialized_files_decision.contains("resolve_local_gemini_files_candidate_payload_parts("),
        "specialized files decision.rs should consume request.rs preparation output"
    );
    for forbidden in [
        "supports_local_gemini_transport_with_network(",
        "resolve_local_gemini_auth(",
        "apply_local_body_rules(",
        "apply_local_header_rules(",
        "build_gemini_files_passthrough_url(",
    ] {
        assert!(
            !specialized_files_decision.contains(forbidden),
            "specialized files decision.rs should not inline request preparation step {forbidden}"
        );
    }
}


#[test]
fn ai_serving_same_format_provider_root_request_separates_body_and_url_policy() {
    let provider_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/request.rs",
    );
    for pattern in [
        "mod body;",
        "mod url;",
        "pub(super) use self::body::build_same_format_provider_request_body;",
        "pub(super) use self::url::build_same_format_upstream_url;",
    ] {
        assert!(
            provider_request.contains(pattern),
            "passthrough/provider/request.rs should own request seam pattern {pattern}"
        );
    }
    for forbidden in [
        "fn build_same_format_provider_request_body(",
        "fn build_same_format_upstream_url(",
        "fn maybe_add_gemini_stream_alt_sse(",
        "fn extract_gemini_model_from_path(",
    ] {
        assert!(
            !provider_request.contains(forbidden),
            "passthrough/provider/request.rs should not inline request policy helper {forbidden}"
        );
    }

    let provider_request_body = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/request/body.rs",
    );
    assert!(
        provider_request_body.contains("build_same_format_provider_request_body_impl("),
        "passthrough/provider/request/body.rs should adapt same-format request-body construction through provider-transport"
    );
    for forbidden in [
        "serde_json::Map::from_iter(",
        "apply_local_body_rules(",
        "sanitize_claude_code_request_body(",
    ] {
        assert!(
            !provider_request_body.contains(forbidden),
            "passthrough/provider/request/body.rs should not own provider transport body policy {forbidden}"
        );
    }

    let provider_request_url = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/request/url.rs",
    );
    assert!(
        provider_request_url.contains("build_same_format_provider_upstream_url_impl("),
        "passthrough/provider/request/url.rs should adapt upstream URL construction through provider-transport"
    );
    for forbidden in [
        "crate::ai_serving::build_provider_transport_request_url(",
        "fn maybe_add_gemini_stream_alt_sse(",
    ] {
        assert!(
            !provider_request_url.contains(forbidden),
            "passthrough/provider/request/url.rs should not own provider transport URL policy {forbidden}"
        );
    }

    let same_format_provider_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/request.rs",
    );
    for pattern in [
        "super::super::request::build_same_format_provider_request_body_with_compatibility_report(",
        "super::super::request::build_same_format_upstream_url(",
    ] {
        assert!(
            same_format_provider_request.contains(pattern),
            "same-format provider family request should consume root request seam via {pattern}"
        );
    }
    assert!(
        same_format_provider_request.contains("build_same_format_provider_headers("),
        "same-format provider family request should route header construction through provider-transport"
    );
    for forbidden in [
        "build_complete_passthrough_headers(",
        "build_complete_passthrough_headers_with_auth(",
        "build_claude_code_passthrough_headers(",
        "build_kiro_provider_headers(",
        "apply_local_header_rules(",
        "ensure_upstream_auth_header(",
    ] {
        assert!(
            !same_format_provider_request.contains(forbidden),
            "same-format provider family request should not own provider transport header policy {forbidden}"
        );
    }
}


#[test]
fn ai_serving_payload_metadata_owns_local_execution_decision_response_shape() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        !planner_mod.contains("mod payload_metadata;"),
        "planner/mod.rs should not keep gateway-owned payload_metadata after serving extraction"
    );

    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/planner/payload_metadata.rs"),
        "gateway payload_metadata.rs should be removed after serving extraction"
    );

    let decision_payload = read_workspace_file("crates/aether-ai/serving/src/decision_payload.rs");
    for pattern in [
        "pub struct AiExecutionDecisionResponseParts {",
        "pub fn build_ai_execution_decision_response(",
        "pub const fn ai_execution_decision_action(",
    ] {
        assert!(
            decision_payload.contains(pattern),
            "aether-ai-serving decision_payload.rs should own {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/payload.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/payload.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/payload.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/payload.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/image/decision.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/decision.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("build_ai_execution_decision_response("),
            "{path} should route local decision payload construction through aether-ai-serving"
        );
        assert!(
            !source.contains("AiExecutionDecision {"),
            "{path} should not inline AiExecutionDecision construction after payload metadata extraction"
        );
    }
}

#[test]
fn ai_serving_owns_pure_planner_diagnostics_and_execution_labels() {
    for path in [
        "apps/aether-gateway/src/ai_serving/planner/failure_diagnostic.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/request_body_diagnostics.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after pure planner diagnostics moved to aether-ai-serving"
        );
    }

    let serving_failure_diagnostic =
        read_workspace_file("crates/aether-ai/serving/src/failure_diagnostic.rs");
    for pattern in [
        "pub enum CandidateFailureDiagnosticKind {",
        "pub struct CandidateFailureDiagnostic {",
        "pub fn upstream_url_missing(",
        "pub fn header_rules_apply_failed(",
        "pub fn body_rules_apply_failed(",
        "pub fn to_extra_data(",
    ] {
        assert!(
            serving_failure_diagnostic.contains(pattern),
            "aether-ai-serving should own candidate failure diagnostic helper {pattern}"
        );
    }

    let serving_request_body_diagnostics =
        read_workspace_file("crates/aether-ai/serving/src/request_body_diagnostics.rs");
    for pattern in [
        "pub fn request_body_build_failure_extra_data(",
        "pub fn same_format_provider_request_body_failure_extra_data(",
        "is_openai_responses_family_format(client_api_format)",
    ] {
        assert!(
            serving_request_body_diagnostics.contains(pattern),
            "aether-ai-serving should own request-body diagnostic helper {pattern}"
        );
    }
    assert!(
        !serving_request_body_diagnostics.contains("crate::ai_serving"),
        "serving request-body diagnostics should not depend on gateway ai_serving seams"
    );

    let standard_mod =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/standard/mod.rs");
    assert!(
        standard_mod.contains("pub(crate) use aether_ai_serving::{")
            && standard_mod.contains("request_body_build_failure_extra_data")
            && standard_mod.contains("same_format_provider_request_body_failure_extra_data"),
        "gateway standard planner should consume request-body diagnostics from aether-ai-serving"
    );

    let serving_dto = read_workspace_file("crates/aether-ai/serving/src/dto.rs");
    for pattern in ["pub enum ExecutionStrategy", "pub enum ConversionMode"] {
        assert!(
            serving_dto.contains(pattern),
            "aether-ai-serving DTO layer should own execution label {pattern}"
        );
    }

    let execution_runtime = read_workspace_file("apps/aether-gateway/src/execution_runtime/mod.rs");
    assert!(
        execution_runtime
            .contains("pub(crate) use aether_ai_serving::{
ConversionMode,
ExecutionStrategy,
};"),
        "gateway execution_runtime should reuse serving-owned execution labels"
    );
    for forbidden in [
        "pub(crate) enum ExecutionStrategy",
        "pub(crate) enum ConversionMode",
    ] {
        assert!(
            !execution_runtime.contains(forbidden),
            "gateway execution_runtime should not own execution labels after serving extraction: {forbidden}"
        );
    }
}



#[test]
fn ai_serving_standard_plan_builders_delegate_fallback_transport_policy() {
    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/plan_builders.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/gemini/plan_builders.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/plan_builders/sync.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/plan_builders/stream.rs",
    ] {
        let source = read_workspace_file(path);
        for pattern in [
            "build_standard_plan_fallback_headers(",
            "StandardPlanFallbackAcceptPolicy",
            "StandardPlanFallbackHeadersInput",
        ] {
            assert!(
                source.contains(pattern),
                "{path} should route fallback transport policy through provider-transport via {pattern}"
            );
        }
        for forbidden in [
            "build_complete_passthrough_headers_with_auth(",
            "build_claude_passthrough_headers(",
            "build_openai_passthrough_headers(",
            "ensure_upstream_auth_header(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not own fallback provider transport detail {forbidden}"
            );
        }
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/plan_builders/sync.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/plan_builders/stream.rs",
    ] {
        let source = read_workspace_file(path);
        for pattern in [
            "build_standard_plan_fallback_openai_chat_url(",
            "build_standard_plan_fallback_openai_responses_url(",
        ] {
            assert!(
                source.contains(pattern),
                "{path} should route OpenAI fallback URL policy through provider-transport via {pattern}"
            );
        }
        for forbidden in ["build_openai_chat_url(", "build_openai_responses_url("] {
            assert!(
                !source.contains(forbidden),
                "{path} should not own OpenAI fallback URL detail {forbidden}"
            );
        }
    }

    let provider_transport_standard =
        read_workspace_file("crates/aether-provider/transport/src/standard/mod.rs");
    for pattern in [
        "pub enum StandardPlanFallbackAcceptPolicy",
        "pub struct StandardPlanFallbackHeadersInput",
        "pub fn build_standard_plan_fallback_headers(",
        "pub fn build_standard_plan_fallback_openai_chat_url(",
        "pub fn build_standard_plan_fallback_openai_responses_url(",
        "build_complete_passthrough_headers_with_auth(",
        "build_claude_passthrough_headers(",
        "build_openai_passthrough_headers(",
        "build_openai_chat_url(",
        "build_openai_responses_url(",
        "ensure_upstream_auth_header(",
    ] {
        assert!(
            provider_transport_standard.contains(pattern),
            "aether-provider-transport standard.rs should own fallback transport policy {pattern}"
        );
    }
}

#[test]
fn ai_serving_specialized_files_attempts_consume_eligible_local_candidates_without_transport_rereads(
) {
    let specialized_files_support = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
    );
    assert!(
        specialized_files_support
            .contains("LocalExecutionCandidateAttempt as LocalGeminiFilesCandidateAttempt"),
        "specialized files attempts should reuse shared LocalExecutionCandidateAttempt"
    );
    assert!(
        specialized_files_support
            .contains("LocalCandidateResolutionMode::WithoutTransportPairGate"),
        "specialized files support should request no-transport-pair-gate runtime gating through candidate materialization"
    );
    assert!(
        !specialized_files_support.contains("rank_local_execution_candidates("),
        "specialized files support should not bypass candidate_resolution with raw affinity ranking"
    );

    let specialized_files_decision = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
    );
    assert!(
        !specialized_files_decision.contains("read_provider_transport_snapshot("),
        "specialized files decision should consume eligibility-owned transport snapshots instead of rereading them"
    );
}

#[test]
fn ai_serving_candidate_sources_share_cross_format_auth_filter_helper() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod candidate_source;"),
        "planner/mod.rs should wire candidate_source helper module"
    );

    let candidate_source =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/candidate_source.rs");
    assert!(
        candidate_source.contains("pub(crate) fn auth_snapshot_allows_cross_format_candidate("),
        "planner/candidate_source.rs should own shared cross-format auth filtering"
    );
    for pattern in [
        "run_ai_candidate_preselection(&port",
        "impl AiCandidatePreselectionPort for GatewayLocalCandidatePreselectionPort",
        "preselect_local_execution_candidates_with_serving",
    ] {
        assert!(
            candidate_source.contains(pattern),
            "planner/candidate_source.rs should implement serving preselection ports through {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("preselect_local_execution_candidates_with_serving("),
            "{path} should use the serving candidate preselection adapter"
        );
        assert!(
            !source.contains("auth_snapshot_allows_cross_format_candidate("),
            "{path} should not hand-roll cross-format auth filtering after preselection extraction"
        );
    }
}

#[test]
fn ai_serving_spec_metadata_owns_family_requested_model_and_plan_builder_routing() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod spec_metadata;"),
        "planner/mod.rs should wire spec_metadata plan-routing adapter"
    );

    let spec_metadata =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/spec_metadata.rs");
    let serving_surface_spec = read_workspace_file("crates/aether-ai/serving/src/surface_spec.rs");
    for pattern in [
        "pub enum AiRequestedModelFamily {",
        "pub struct AiExecutionSurfaceSpecMetadata {",
        "pub const fn ai_requested_model_family_for_standard_source(",
        "pub const fn ai_requested_model_family_for_same_format_provider(",
        "pub const fn ai_requested_model_family_for_video_create(",
        "pub const fn ai_standard_spec_metadata(",
        "pub const fn ai_same_format_provider_spec_metadata(",
        "pub const fn ai_openai_responses_spec_metadata(",
        "pub const fn ai_gemini_files_spec_metadata(",
        "pub const fn ai_video_create_spec_metadata(",
        "pub const fn ai_openai_image_spec_metadata(",
    ] {
        assert!(
            serving_surface_spec.contains(pattern),
            "aether-ai-serving should own pure surface spec metadata {pattern}"
        );
    }
    for pattern in [
        "ai_standard_spec_metadata as local_standard_spec_metadata",
        "ai_same_format_provider_spec_metadata as local_same_format_provider_spec_metadata",
        "ai_openai_responses_spec_metadata as local_openai_responses_spec_metadata",
        "ai_gemini_files_spec_metadata as local_gemini_files_spec_metadata",
        "ai_video_create_spec_metadata as local_video_create_spec_metadata",
        "AiExecutionSurfaceSpecMetadata as LocalExecutionSurfaceSpecMetadata",
        "AiRequestedModelFamily as RequestedModelFamily",
        "pub(crate) fn build_sync_plan_from_requested_model_family(",
        "pub(crate) fn build_stream_plan_from_requested_model_family(",
    ] {
        assert!(
            spec_metadata.contains(pattern),
            "planner/spec_metadata.rs should adapt serving spec metadata or own gateway plan routing through {pattern}"
        );
    }
    for forbidden in [
        "pub(crate) struct LocalExecutionSurfaceSpecMetadata {",
        "pub(crate) fn requested_model_family_for_standard_source(",
        "pub(crate) fn requested_model_family_for_same_format_provider(",
        "pub(crate) fn requested_model_family_for_video_create(",
        "pub(crate) fn local_standard_spec_metadata(",
        "pub(crate) fn local_same_format_provider_spec_metadata(",
        "pub(crate) fn local_openai_responses_spec_metadata(",
        "pub(crate) fn local_gemini_files_spec_metadata(",
        "pub(crate) fn local_video_create_spec_metadata(",
        "pub(crate) fn local_openai_image_spec_metadata(",
    ] {
        assert!(
            !spec_metadata.contains(forbidden),
            "planner/spec_metadata.rs should not own pure surface spec metadata after serving extraction: {forbidden}"
        );
    }

    for (path, pattern) in [
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
            "local_standard_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/build.rs",
            "local_standard_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/request.rs",
            "local_standard_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/payload.rs",
            "local_standard_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/candidates.rs",
            "local_same_format_provider_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
            "local_same_format_provider_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/build.rs",
            "local_same_format_provider_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/request/prepare.rs",
            "local_same_format_provider_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/payload.rs",
            "local_same_format_provider_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
            "local_video_create_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/video.rs",
            "local_video_create_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/video/decision.rs",
            "local_video_create_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/files.rs",
            "local_gemini_files_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
            "local_gemini_files_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/plans.rs",
            "local_openai_responses_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
            "local_openai_responses_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/request.rs",
            "local_openai_responses_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/payload.rs",
            "local_openai_responses_spec_metadata(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/build.rs",
            "build_sync_plan_from_requested_model_family(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/build.rs",
            "build_stream_plan_from_requested_model_family(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
            "build_sync_plan_from_requested_model_family(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
            "build_stream_plan_from_requested_model_family(",
        ),
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains(pattern),
            "{path} should use shared spec metadata helper {pattern}"
        );
    }
}


#[test]
fn ai_serving_decision_inputs_share_authenticated_input_helper() {
    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    assert!(
        planner_mod.contains("mod decision_input;"),
        "planner/mod.rs should wire decision_input helper module"
    );

    let serving_decision_input =
        read_workspace_file("crates/aether-ai/serving/src/decision_input.rs");
    for pattern in [
        "pub trait AiAuthenticatedDecisionInputPort",
        "pub async fn run_ai_authenticated_decision_input",
        "read_auth_snapshot",
        "resolve_required_capabilities",
        "build_resolved_input",
    ] {
        assert!(
            serving_decision_input.contains(pattern),
            "aether-ai-serving should own authenticated decision-input use-case primitive {pattern}"
        );
    }

    let decision_input =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/decision_input.rs");
    for pattern in [
        "pub(crate) struct ResolvedLocalDecisionAuthInput {",
        "pub(crate) struct LocalRequestedModelDecisionInput {",
        "pub(crate) struct LocalAuthenticatedDecisionInput {",
        "pub(crate) fn build_local_requested_model_decision_input(",
        "pub(crate) fn build_local_authenticated_decision_input(",
        "pub(crate) async fn resolve_local_authenticated_decision_input(",
        "impl AiAuthenticatedDecisionInputPort for GatewayAuthenticatedDecisionInputPort",
        "run_ai_authenticated_decision_input(",
    ] {
        assert!(
            decision_input.contains(pattern),
            "planner/decision_input.rs should keep gateway DTOs and delegate authenticated decision input through {pattern}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/candidates.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/resolve.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("resolve_local_authenticated_decision_input("),
            "{path} should use the shared authenticated decision input helper"
        );
        if path.ends_with("/standard/openai/responses/decision/support.rs")
            || path.ends_with("/standard/openai/chat/plans/resolve.rs")
        {
            assert!(
                source.contains("extract_standard_requested_model("),
                "{path} should use shared standard requested-model extraction"
            );
        } else if !path.ends_with("/specialized/files/support.rs") {
            assert!(
                source.contains("extract_requested_model_from_request("),
                "{path} should use shared family-aware requested-model extraction"
            );
        }
        for forbidden in [
            "read_auth_api_key_snapshot(",
            "resolve_request_candidate_required_capabilities(",
            "fn extract_gemini_model_from_path(",
            "fn extract_gemini_video_model_from_path(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not inline authenticated decision input step {forbidden}"
            );
        }
    }

    for (path, pattern) in [
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/mod.rs",
            "LocalRequestedModelDecisionInput as LocalStandardDecisionInput",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/mod.rs",
            "LocalRequestedModelDecisionInput as LocalSameFormatProviderDecisionInput",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/support.rs",
            "LocalRequestedModelDecisionInput as LocalOpenAiChatDecisionInput",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
            "LocalRequestedModelDecisionInput as LocalOpenAiResponsesDecisionInput",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
            "LocalRequestedModelDecisionInput as LocalVideoCreateDecisionInput",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
            "LocalRequestedModelDecisionInput as LocalGeminiFilesDecisionInput",
        ),
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains(pattern),
            "{path} should rename shared decision input shapes instead of redefining local decision input structs"
        );
    }

    for (path, pattern) in [
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/family/candidates.rs",
            "build_local_requested_model_decision_input(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/candidates.rs",
            "build_local_requested_model_decision_input(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/plans/resolve.rs",
            "build_local_requested_model_decision_input(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
            "build_local_requested_model_decision_input(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
            "build_local_requested_model_decision_input(",
        ),
        (
            "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
            "build_local_requested_model_decision_input(",
        ),
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains(pattern),
            "{path} should build local decision inputs through shared decision_input builders"
        );
    }
}

#[test]
fn ai_serving_leaf_planner_owners_route_contract_specs_through_gateway_seams() {
    for path in [
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/support.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision/support.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            !source.contains("aether_ai_formats::contracts::ExecutionRuntimeAuthContext"),
            "{path} should consume ExecutionRuntimeAuthContext through gateway ai_serving seams"
        );
        assert!(
            source.contains("ExecutionRuntimeAuthContext"),
            "{path} should use the gateway ai_serving root seam for ExecutionRuntimeAuthContext"
        );
    }

    let specialized_files_decision = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/files/decision.rs",
    );
    assert!(
        !specialized_files_decision
            .contains("aether_ai_formats::formats::gemini::files::spec::LocalGeminiFilesSpec"),
        "planner/specialized/files/decision.rs should consume LocalGeminiFilesSpec through the local specialized seam"
    );
    assert!(
        specialized_files_decision.contains("use super::LocalGeminiFilesSpec;"),
        "planner/specialized/files/decision.rs should use the local specialized seam for LocalGeminiFilesSpec"
    );

    let specialized_video_support = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/specialized/video/support.rs",
    );
    assert!(
        specialized_video_support
            .contains("use super::{LocalVideoCreateFamily, LocalVideoCreateSpec};"),
        "planner/specialized/video/support.rs should use local video seams for LocalVideoCreate* types"
    );
}

#[test]
fn ai_serving_m5_moves_contracts_and_route_logic_into_format_crate() {
    for path in [
        "crates/aether-ai/formats/src/contracts/actions.rs",
        "crates/aether-ai/formats/src/contracts/plan_kinds.rs",
        "crates/aether-ai/formats/src/contracts/report_kinds.rs",
        "crates/aether-ai/formats/src/formats/shared/routing.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should exist after initial format crate extraction"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/contracts",
        "apps/aether-gateway/src/ai_serving/contracts/actions.rs",
        "apps/aether-gateway/src/ai_serving/contracts/plan_kinds.rs",
        "apps/aether-gateway/src/ai_serving/contracts/report_kinds.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after moving surface contract ownership"
        );
    }

    let gateway_ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        !gateway_ai_serving_mod.contains("mod contracts;"),
        "gateway ai_serving/mod.rs should not register a contracts module"
    );

    let gateway_route = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/route.rs");
    let gateway_route_runtime = gateway_route
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(gateway_route.as_str());
    assert!(
        gateway_route_runtime.contains("crate::ai_serving::"),
        "planner/route.rs should delegate route logic through the ai_serving root seam"
    );
    assert!(
        gateway_route_runtime.contains("is_matching_stream_http_request"),
        "planner/route.rs should delegate full HTTP stream matching to aether-ai-formats"
    );
    for legacy_literal in [
        "\"openai_chat_stream\"",
        "\"openai_chat_sync\"",
        "\"gemini_files_upload\"",
        "\"openai_video_content\"",
    ] {
        assert!(
            !gateway_route_runtime.contains(legacy_literal),
            "planner/route.rs should not own hardcoded route resolution literal {legacy_literal}"
        );
    }
    for forbidden in [
        "OPENAI_IMAGE_STREAM_PLAN_KIND",
        "is_openai_image_stream_request(",
        "parts.uri.path(), body_json",
    ] {
        assert!(
            !gateway_route_runtime.contains(forbidden),
            "planner/route.rs should not keep surface-specific stream matching branch {forbidden}"
        );
    }

    let surface_route =
        read_workspace_file("crates/aether-ai/formats/src/formats/shared/routing.rs");
    for pattern in [
        "pub fn is_matching_stream_http_request(",
        "is_openai_image_stream_request(parts, body_json, body_base64)",
    ] {
        assert!(
            surface_route.contains(pattern),
            "aether-ai-formats planner/route.rs should own HTTP stream matching format surface logic {pattern}"
        );
    }

    let gateway_api = read_workspace_file("apps/aether-gateway/src/ai_serving/api.rs");
    for pattern in [
        "pub(crate) fn parse_direct_request_body(",
        "pub(crate) fn resolve_execution_runtime_stream_plan_kind(",
        "pub(crate) fn resolve_execution_runtime_sync_plan_kind(",
        "pub(crate) fn is_matching_stream_request(",
        "pub(crate) fn supports_sync_execution_decision_kind(",
        "pub(crate) fn supports_stream_execution_decision_kind(",
    ] {
        assert!(
            gateway_api.contains(pattern),
            "ai_serving/api.rs should own facade wrapper {pattern}"
        );
    }

    let planner_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/mod.rs");
    for pattern in [
        "pub(crate) use self::common::parse_direct_request_body;",
        "pub(crate) use self::route::{",
    ] {
        assert!(
            !planner_mod.contains(pattern),
            "planner/mod.rs should not act as facade hub for {pattern}"
        );
    }

    let finalize_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/finalize/mod.rs");
    for pattern in [
        "pub(crate) use crate::api::response::{build_client_response, build_client_response_from_parts};",
        "pub(crate) use common::build_local_success_outcome;",
        "pub(crate) use internal::{",
    ] {
        assert!(
            !finalize_mod.contains(pattern),
            "finalize/mod.rs should not act as re-export hub for {pattern}"
        );
    }
}



#[test]
fn ai_serving_runtime_adapter_dead_duplicates_are_removed() {
    for path in [
        "apps/aether-gateway/src/ai_serving/runtime/adapters/antigravity/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/antigravity/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/antigravity/request.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/antigravity/url.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/vertex/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/vertex/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/vertex/url.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude_code/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude_code/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude_code/request.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude_code/url.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/openai/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/openai/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/openai/request.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/openai/url.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/gemini/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/gemini/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/gemini/request.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/gemini/url.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude/auth.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude/policy.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude/request.rs",
        "apps/aether-gateway/src/ai_serving/runtime/adapters/claude/url.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after provider-transport ownership consolidation"
        );
    }
}

#[test]
fn ai_serving_planner_route_remains_control_only() {
    let gateway_route = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/route.rs");
    let gateway_route_runtime = gateway_route
        .split("#[cfg(test)]")
        .next()
        .unwrap_or(gateway_route.as_str());

    for forbidden in [
        "crate::scheduler::",
        "crate::request_candidate_runtime::",
        "crate::provider_transport::",
        "crate::execution_runtime::",
    ] {
        assert!(
            !gateway_route_runtime.contains(forbidden),
            "planner/route.rs should not depend on {forbidden}"
        );
    }

    assert!(
        gateway_route_runtime.contains("GatewayControlDecision"),
        "planner/route.rs should stay as the thin adapter from control decisions"
    );
}

#[test]
fn ai_serving_error_body_is_owned_by_format_finalize_module() {
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/conversion"),
        "gateway ai_serving should not keep a conversion directory; format conversion belongs to aether-ai-formats and transport checks belong to provider transport"
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/conversion/error.rs"),
        "ai_serving/conversion/error.rs should stay removed"
    );
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/error_body.rs"),
        "format error response-body helpers should live under finalize/error_body.rs"
    );
    assert!(
        !workspace_file_exists("crates/aether-ai/formats/src/formats/conversion/error.rs"),
        "aether-ai-formats should not keep error response-body helpers under conversion"
    );

    let gateway_ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        !gateway_ai_serving_mod.contains("mod conversion;"),
        "gateway ai_serving/mod.rs should not register a conversion module"
    );

    for forbidden in [
        "pub(crate) enum LocalCoreSyncErrorKind",
        "pub enum LocalCoreSyncErrorKind",
        "fn build_core_error_body_for_client_format(",
    ] {
        assert!(
            !gateway_ai_serving_mod.contains(forbidden),
            "gateway ai_serving/mod.rs should not own {forbidden}"
        );
    }
}

#[test]
fn ai_serving_conversion_request_is_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/conversion/request.rs"),
        "request conversion should live in aether-ai-formats"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/conversion/request/from_openai_chat/claude.rs"
        ),
        "ai_serving/conversion/request/from_openai_chat should not remain in gateway"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/conversion/request/to_openai_chat/claude.rs"
        ),
        "ai_serving/conversion/request/to_openai_chat should not remain in gateway"
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/conversion/request/mod.rs"),
        "gateway conversion/request/mod.rs should be removed after root-seam consolidation"
    );
    let gateway_ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        !gateway_ai_serving_mod.contains("pub(crate) mod request;"),
        "gateway ai_serving/mod.rs should not keep request re-export shell after root-seam consolidation"
    );

    let surface_api = read_workspace_file("crates/aether-ai/formats/src/api.rs");
    assert!(
        surface_api.contains("pub use aether_ai_formats::formats::conversion::request::{"),
        "format API facade should re-export request conversion directly from aether-ai-formats"
    );
}

#[test]
fn ai_serving_conversion_response_is_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/conversion/response.rs"),
        "response conversion should live in aether-ai-formats"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/conversion/response/from_openai_chat/claude_chat.rs"
        ),
        "ai_serving/conversion/response/from_openai_chat should not remain in gateway"
    );
    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/conversion/response/to_openai_chat/claude_chat.rs"
        ),
        "ai_serving/conversion/response/to_openai_chat should not remain in gateway"
    );
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/conversion/response/mod.rs"),
        "gateway conversion/response/mod.rs should be removed after root-seam consolidation"
    );
    let gateway_ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        !gateway_ai_serving_mod.contains("pub(crate) mod response;"),
        "gateway ai_serving/mod.rs should not keep response re-export shell after root-seam consolidation"
    );

    let surface_api = read_workspace_file("crates/aether-ai/formats/src/api.rs");
    assert!(
        surface_api.contains("pub use aether_ai_formats::formats::conversion::response::{"),
        "format API facade should re-export response conversion directly from aether-ai-formats"
    );
}

#[test]
fn ai_format_crate_owns_conversion_and_surface_facade() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/conversion"),
        "aether-ai-formats should own the conversion directory"
    );

    let surface_lib = read_workspace_file("crates/aether-ai/formats/src/lib.rs");
    assert!(
        surface_lib.contains("pub mod protocol;"),
        "aether-ai-formats lib.rs should expose the protocol module"
    );

    let surface_api = read_workspace_file("crates/aether-ai/formats/src/api.rs");
    for pattern in [
        "pub use aether_ai_formats::{",
        "pub use aether_ai_formats::formats::conversion::request::{",
        "pub use aether_ai_formats::formats::conversion::response::{",
        "pub use crate::formats::shared::error_body::{",
    ] {
        assert!(
            surface_api.contains(pattern),
            "format API facade should expose pure dependencies through {pattern}"
        );
    }
}

#[test]
fn ai_serving_finalize_standard_sync_response_converters_are_owned_by_format_crate() {
    for path in [
        "apps/aether-gateway/src/ai_serving/finalize/standard/openai/sync/chat.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/openai/sync/cli.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/claude/sync/chat.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/claude/sync/cli.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/gemini/sync/chat.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/gemini/sync/cli.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be deleted after sync finalize dispatch moved into surface-owned helpers"
        );
    }

    for (candidate_paths, symbol) in [
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_openai_responses_response_to_openai_chat",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "build_openai_responses_response",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_openai_chat_response_to_openai_responses",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_claude_chat_response_to_openai_chat",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_openai_chat_response_to_claude_chat",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_claude_response_to_openai_responses",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_gemini_chat_response_to_openai_chat",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_openai_chat_response_to_gemini_chat",
        ),
        (
            vec!["apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs"],
            "convert_gemini_response_to_openai_responses",
        ),
    ] {
        let sources = candidate_paths
            .iter()
            .map(|path| read_workspace_file(path))
            .collect::<Vec<_>>();
        assert!(
            sources
                .iter()
                .any(|source| source.contains("crate::ai_serving::{") && source.contains(symbol)),
            "{symbol} should stay exposed through the ai_serving root seam from finalize/standard/mod.rs"
        );
    }
}

#[test]
fn ai_serving_finalize_stream_engine_is_owned_by_format_crate() {
    for path in [
        "crates/aether-ai/formats/src/formats/shared/sse.rs",
        "crates/aether-ai/formats/src/formats/shared/stream_core/common.rs",
        "crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs",
        "crates/aether-ai/formats/src/formats/openai/chat/stream.rs",
        "crates/aether-ai/formats/src/formats/claude/messages/stream.rs",
        "crates/aether-ai/formats/src/formats/gemini/generate_content/stream.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should exist in aether-ai-formats finalize engine"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/finalize/standard/openai/stream.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/claude/stream.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/gemini/stream.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after finalize stream wrapper collapse"
        );
    }

    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/finalize/standard/stream_core/common.rs"
        ),
        "stream_core/common.rs should be removed after canonical stream helper collapse"
    );
    for path in [
        "apps/aether-gateway/src/ai_serving/finalize/standard/stream_core/mod.rs",
        "apps/aether-gateway/src/ai_serving/finalize/standard/stream_core/orchestrator.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after stream rewriter collapse"
        );
    }

    let surface_format_matrix = read_workspace_file(
        "crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs",
    );
    for pattern in [
        "pub struct StreamingStandardFormatMatrix",
        "enum ProviderStreamParser",
        "enum ClientStreamEmitter",
    ] {
        assert!(
            surface_format_matrix.contains(pattern),
            "surface stream_core/format_matrix.rs should own {pattern}"
        );
    }

    let gateway_standard_mod =
        read_workspace_file("apps/aether-gateway/src/ai_serving/finalize/standard/mod.rs");
    assert!(
        !gateway_standard_mod.contains("stream_core"),
        "gateway standard finalize module should not retain a stream_core wrapper after stream rewrite collapse"
    );
}


#[test]
fn ai_serving_finalize_stream_rewrite_matrix_is_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/stream_rewrite.rs"),
        "finalize stream rewrite matrix should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/openai/image/stream.rs"),
        "OpenAI image stream rewrite state should live in aether-ai-formats"
    );

    let gateway_stream_rewrite = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/finalize/internal/stream_rewrite.rs",
    );
    assert!(
        gateway_stream_rewrite.contains("maybe_build_ai_surface_stream_rewriter"),
        "gateway internal stream_rewrite should delegate stream rewrite state machine to aether-ai-formats"
    );

    for forbidden in [
        "enum RewriteMode",
        "OpenAiImageStreamState",
        "KiroToClaudeCliStreamState",
        "StreamingStandardConversionState",
        "StreamingStandardFormatMatrix",
        "transform_provider_private_stream_line",
        "resolve_finalize_stream_rewrite_mode",
        "fn transform_standard_bytes(",
        "buffered: Vec<u8>",
        "struct OpenAiImageStreamState",
        "struct OpenAiImageFrame",
        "fn image_failure_error(",
        "fn completed_response_image_result(",
        "fn requested_partial_images(",
        "fn image_partial_event_name(",
        "fn image_completed_event_name(",
        "fn image_failed_event_name(",
        "fn find_sse_block_end(",
        "fn is_standard_provider_api_format(",
        "fn is_standard_chat_client_api_format(",
        "fn is_standard_cli_client_api_format(",
        ".get(\"provider_api_format\")",
        ".get(\"client_api_format\")",
        ".get(\"needs_conversion\")",
        ".get(\"envelope_name\")",
    ] {
        assert!(
            !gateway_stream_rewrite.contains(forbidden),
            "gateway internal stream_rewrite should not own rewrite-matrix detail {forbidden}"
        );
    }
}


#[test]
fn ai_serving_root_owns_shared_gemini_request_path_parser() {
    let serving_surface_spec = read_workspace_file("crates/aether-ai/serving/src/surface_spec.rs");
    assert!(
        serving_surface_spec.contains("pub fn extract_ai_gemini_model_from_path("),
        "aether-ai-serving should own shared gemini request-path parsing"
    );

    let ai_serving_mod = read_workspace_file("apps/aether-gateway/src/ai_serving/mod.rs");
    assert!(
        ai_serving_mod
            .contains("extract_ai_gemini_model_from_path as extract_gemini_model_from_path"),
        "ai_serving/mod.rs should expose shared gemini request-path parsing through the serving seam"
    );

    let passthrough_provider_request = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/request.rs",
    );
    assert!(
        !passthrough_provider_request.contains("fn extract_gemini_model_from_path("),
        "passthrough/provider/request.rs should not locally own gemini request-path parsing"
    );

    let auth_credentials =
        read_workspace_file("apps/aether-gateway/src/control/auth/credentials.rs");
    assert!(
        auth_credentials.contains("ai_serving::extract_gemini_model_from_path"),
        "control/auth/credentials.rs should use ai_serving root seam for gemini request-path parsing"
    );
    assert!(
        !auth_credentials.contains("fn extract_gemini_model_from_path("),
        "control/auth/credentials.rs should not inline gemini request-path parsing"
    );
}

#[test]
fn ai_serving_planner_standard_normalize_is_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/standard_normalize.rs"),
        "planner/standard/normalize should live in aether-ai-formats"
    );

    let gateway_normalize =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/standard/normalize.rs");
    let gateway_normalize_chat = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/standard/normalize/chat.rs",
    );
    let gateway_normalize_cli = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/standard/normalize/responses.rs",
    );
    assert!(
        gateway_normalize_chat.contains("crate::ai_serving::")
            && gateway_normalize_cli.contains("crate::ai_serving::"),
        "gateway normalize chat/cli owners should delegate to format standard normalize helpers through the ai_serving root seam"
    );

    for forbidden in [
        "serde_json::Map::from_iter",
        "normalize_openai_responses_request_to_openai_chat_request",
        "parse_openai_tool_result_content",
    ] {
        assert!(
            !gateway_normalize.contains(forbidden),
            "gateway normalize.rs should not keep helper implementation detail {forbidden}"
        );
    }
    for forbidden in [
        ".eq_ignore_ascii_case(\"antigravity\")",
        "build_antigravity_v1internal_url(",
        "apply_local_body_rules(",
        "request_conversion_kind(",
        "build_provider_transport_request_url(",
        "build_openai_responses_url(",
        "build_openai_chat_url(",
        "build_claude_messages_url(",
        "build_passthrough_path_url(",
    ] {
        assert!(
            !gateway_normalize_cli.contains(forbidden),
            "gateway standard/normalize/responses.rs should route provider-private URL policy through provider-transport instead of {forbidden}"
        );
    }
    for forbidden in [
        "apply_local_body_rules(",
        "request_conversion_kind(",
        "build_provider_transport_request_url(",
        "build_openai_responses_url(",
        "build_openai_chat_url(",
        "build_claude_messages_url(",
        "build_passthrough_path_url(",
    ] {
        assert!(
            !gateway_normalize_chat.contains(forbidden),
            "gateway standard/normalize/chat.rs should route provider URL policy through provider-transport instead of {forbidden}"
        );
    }
}

#[test]
fn ai_serving_openai_helpers_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/openai/shared.rs"),
        "planner/openai helper owner should exist in aether-ai-formats"
    );

    let gateway_openai_mod =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/standard/openai/mod.rs");
    assert!(
        gateway_openai_mod.contains("pub(crate) use crate::ai_serving::{"),
        "gateway planner/standard/openai/mod.rs should thinly re-export surface openai helpers through the ai_serving root seam"
    );

    let gateway_openai_chat = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/mod.rs",
    );
    for forbidden in [
        "pub(crate) fn parse_openai_stop_sequences(",
        "pub(crate) fn resolve_openai_chat_max_tokens(",
        "pub(crate) fn value_as_u64(",
        "pub(crate) fn copy_request_number_field(",
        "pub(crate) fn copy_request_number_field_as(",
        "pub(crate) fn map_openai_reasoning_effort_to_claude_output(",
        "pub(crate) fn map_openai_reasoning_effort_to_gemini_budget(",
    ] {
        assert!(
            !gateway_openai_chat.contains(forbidden),
            "gateway planner/standard/openai/chat/mod.rs should not own helper {forbidden}"
        );
    }
}

#[test]
fn ai_serving_standard_matrix_delegates_format_conversion_to_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/request_matrix.rs"),
        "planner/matrix facade should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/standard_matrix.rs"),
        "format standard request-body planner should live in aether-ai-formats"
    );
    for path in [
        "crates/aether-ai/formats/src/protocol/canonical.rs",
        "crates/aether-ai/formats/src/formats/matrix.rs",
        "crates/aether-ai/formats/src/formats/registry.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should own canonical format conversion primitives"
        );
    }
    let surface_matrix =
        read_workspace_file("crates/aether-ai/formats/src/formats/shared/standard_matrix.rs");
    assert!(
        surface_matrix.contains("use aether_ai_formats::formats::registry::{")
            && surface_matrix.contains("convert_request")
            && surface_matrix.contains("FormatContext")
            && surface_matrix.contains("aether_ai_formats::formats::conversion::request::{"),
        "format standard matrix should delegate format conversion to aether-ai-formats"
    );
    for forbidden in [
        "pub fn convert_request(",
        "pub enum RequestConversionKind",
        "pub struct CanonicalRequest",
    ] {
        assert!(
            !surface_matrix.contains(forbidden),
            "format standard matrix should not own format conversion primitive {forbidden}"
        );
    }
    assert!(
        !workspace_file_exists("apps/aether-gateway/src/ai_serving/planner/standard/matrix.rs"),
        "planner/standard/matrix.rs should stay removed after wrapper cleanup"
    );

    let matrix = read_workspace_file("apps/aether-gateway/src/ai_serving/planner/standard/mod.rs");
    assert!(
        matrix.contains("crate::ai_serving::"),
        "planner/standard/mod.rs should delegate canonical conversion through the ai_serving root seam after matrix wrapper cleanup"
    );
    assert!(
        matrix.contains("build_standard_request_body"),
        "planner/standard/mod.rs should still expose build_standard_request_body after matrix wrapper cleanup"
    );
    assert!(
        matrix.contains("build_standard_upstream_url"),
        "planner/standard/mod.rs should still expose build_standard_upstream_url after matrix wrapper cleanup"
    );
    assert!(
        !matrix.contains("mod matrix;"),
        "planner/standard/mod.rs should not keep a local matrix wrapper module"
    );
    {
        let forbidden = "serde_json::Map::from_iter";
        assert!(
            !matrix.contains(forbidden),
            "planner/standard/mod.rs should not keep matrix conversion helper {forbidden}"
        );
    }
}

#[test]
fn ai_serving_standard_family_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/family.rs"),
        "planner/standard/family pure spec owner should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/claude/messages/chat_spec.rs"),
        "planner/standard/claude/chat pure spec resolver should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/claude/messages/cli_spec.rs"),
        "planner/standard/claude/cli pure spec resolver should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists(
            "crates/aether-ai/formats/src/formats/gemini/generate_content/chat_spec.rs"
        ),
        "planner/standard/gemini/chat pure spec resolver should live in aether-ai-formats"
    );
    assert!(
        workspace_file_exists(
            "crates/aether-ai/formats/src/formats/gemini/generate_content/cli_spec.rs"
        ),
        "planner/standard/gemini/cli pure spec resolver should live in aether-ai-formats"
    );

    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/planner/standard/family/types.rs"
        ),
        "planner/standard/family/types.rs should stay removed after wrapper cleanup"
    );

    let family_types =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/standard/family/mod.rs");
    assert!(
        family_types.contains("pub(crate) use crate::ai_serving::{"),
        "gateway planner/standard/family/mod.rs should re-export pure family spec types through the ai_serving root seam"
    );
    for forbidden in [
        "pub(crate) enum LocalStandardSourceFamily",
        "pub(crate) enum LocalStandardSourceMode",
        "pub(crate) struct LocalStandardSpec",
    ] {
        assert!(
            !family_types.contains(forbidden),
            "gateway planner/standard/family/mod.rs should not own pure spec type {forbidden}"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/claude/chat.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/claude/cli.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/gemini/chat.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/gemini/cli.rs",
    ] {
        assert!(
            !workspace_file_exists(path),
            "{path} should be removed after moving pure spec resolvers into the format crate"
        );
    }

    for path in [
        "apps/aether-gateway/src/ai_serving/planner/standard/claude/mod.rs",
        "apps/aether-gateway/src/ai_serving/planner/standard/gemini/mod.rs",
    ] {
        let source = read_workspace_file(path);
        assert!(
            source.contains("crate::ai_serving::"),
            "{path} should delegate pure standard-family spec resolution through the ai_serving root seam"
        );
        for forbidden in [
            "LocalStandardSpec {",
            "report_kind:",
            "require_streaming:",
            "pub(crate) mod chat;",
            "pub(crate) mod cli;",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not own spec construction detail {forbidden}"
            );
        }
    }
}

#[test]
fn ai_serving_same_format_provider_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/passthrough.rs"),
        "planner/passthrough/provider pure spec owner should live in aether-ai-formats"
    );

    assert!(
        !workspace_file_exists(
            "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/types.rs"
        ),
        "planner/passthrough/provider/family/types.rs should stay removed after wrapper cleanup"
    );

    let family_types = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/mod.rs",
    );
    assert!(
        family_types.contains("pub(crate) use crate::ai_serving::"),
        "gateway passthrough/provider/family/mod.rs should re-export pure same-format provider spec types through the ai_serving root seam"
    );
    for forbidden in [
        "pub(crate) enum LocalSameFormatProviderFamily",
        "pub(crate) struct LocalSameFormatProviderSpec",
    ] {
        assert!(
            !family_types.contains(forbidden),
            "gateway passthrough/provider/family/mod.rs should not own pure same-format type {forbidden}"
        );
    }

    let plans = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
    );
    assert!(
        plans.contains("crate::ai_serving::"),
        "gateway passthrough/provider/plans.rs should delegate same-format spec resolution through the ai_serving root seam"
    );
    for forbidden in [
        "claude_chat_sync_success",
        "gemini_cli_stream_success",
        "pub(crate) fn resolve_sync_spec(",
        "pub(crate) fn resolve_stream_spec(",
    ] {
        assert!(
            !plans.contains(forbidden),
            "gateway passthrough/provider/plans.rs should not own same-format resolver detail {forbidden}"
        );
    }
}

#[test]
fn ai_serving_passthrough_provider_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/passthrough.rs"),
        "planner/passthrough/provider pure spec owner should live in aether-ai-formats"
    );

    let family_types = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/mod.rs",
    );
    assert!(
        family_types.contains("pub(crate) use crate::ai_serving::"),
        "gateway passthrough/provider/family/mod.rs should re-export pure spec types through the ai_serving root seam"
    );
    for forbidden in [
        "pub(crate) enum LocalSameFormatProviderFamily",
        "pub(crate) struct LocalSameFormatProviderSpec",
    ] {
        assert!(
            !family_types.contains(forbidden),
            "gateway passthrough/provider/family/mod.rs should not own pure spec type {forbidden}"
        );
    }

    let plans = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/passthrough/provider/plans.rs",
    );
    assert!(
        plans.contains("crate::ai_serving::"),
        "gateway passthrough/provider/plans.rs should delegate same-format spec resolution through the ai_serving root seam"
    );
    for forbidden in [
        "pub(crate) fn resolve_sync_spec(",
        "pub(crate) fn resolve_stream_spec(",
        "CLAUDE_CHAT_SYNC_PLAN_KIND",
        "GEMINI_CLI_STREAM_PLAN_KIND",
        "LocalSameFormatProviderSpec {",
    ] {
        assert!(
            !plans.contains(forbidden),
            "gateway passthrough/provider/plans.rs should not keep pure spec resolver detail {forbidden}"
        );
    }
}

#[test]
fn ai_serving_specialized_files_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/gemini/files/spec.rs"),
        "planner/specialized/files pure spec owner should live in aether-ai-formats"
    );

    let files =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/specialized/files.rs");
    assert!(
        files.contains("crate::ai_serving::"),
        "gateway planner/specialized/files.rs should delegate pure specialized-files spec resolution through the ai_serving root seam"
    );
    for forbidden in [
        "struct LocalGeminiFilesSpec",
        "fn resolve_sync_spec(",
        "fn resolve_stream_spec(",
        "Some(LocalGeminiFilesSpec {",
        "GEMINI_FILES_LIST_PLAN_KIND",
        "GEMINI_FILES_GET_PLAN_KIND",
        "GEMINI_FILES_DELETE_PLAN_KIND",
        "GEMINI_FILES_DOWNLOAD_PLAN_KIND",
    ] {
        assert!(
            !files.contains(forbidden),
            "gateway planner/specialized/files.rs should not keep pure specialized-files resolver detail {forbidden}"
        );
    }
}

#[test]
fn ai_serving_specialized_video_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/shared/video.rs"),
        "planner/specialized/video shared spec seam should live in aether-ai-formats"
    );
    for path in [
        "crates/aether-ai/formats/src/formats/openai/video/spec.rs",
        "crates/aether-ai/formats/src/formats/gemini/video/spec.rs",
    ] {
        assert!(
            workspace_file_exists(path),
            "{path} should own provider-specific video create spec resolution"
        );
    }

    let video =
        read_workspace_file("apps/aether-gateway/src/ai_serving/planner/specialized/video.rs");
    assert!(
        video.contains("crate::ai_serving::"),
        "gateway planner/specialized/video.rs should delegate pure specialized-video spec resolution through the ai_serving root seam"
    );
    for forbidden in [
        "enum LocalVideoCreateFamily",
        "struct LocalVideoCreateSpec",
        "fn resolve_sync_spec(",
        "Some(LocalVideoCreateSpec {",
        "OPENAI_VIDEO_CREATE_SYNC_PLAN_KIND",
        "GEMINI_VIDEO_CREATE_SYNC_PLAN_KIND",
    ] {
        assert!(
            !video.contains(forbidden),
            "gateway planner/specialized/video.rs should not keep pure specialized-video resolver detail {forbidden}"
        );
    }
}

#[test]
fn ai_serving_openai_responses_specs_are_owned_by_format_crate() {
    assert!(
        workspace_file_exists("crates/aether-ai/formats/src/formats/openai/responses/spec.rs"),
        "planner/standard/openai_responses pure spec owner should live in aether-ai-formats"
    );

    let decision = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/decision.rs",
    );
    assert!(
        decision.contains("pub(super) use crate::ai_serving::LocalOpenAiResponsesSpec;"),
        "gateway planner/standard/openai/responses/decision.rs should re-export pure openai-responses spec type through the ai_serving root seam"
    );
    assert!(
        !decision.contains("pub(super) struct LocalOpenAiResponsesSpec"),
        "gateway planner/standard/openai/responses/decision.rs should not own LocalOpenAiResponsesSpec"
    );

    let plans = read_workspace_file(
        "apps/aether-gateway/src/ai_serving/planner/standard/openai/responses/plans.rs",
    );
    assert!(
        plans.contains("crate::ai_serving::"),
        "gateway planner/standard/openai/responses/plans.rs should delegate openai-responses spec resolution through the ai_serving root seam"
    );
    for forbidden in [
        "fn resolve_sync_spec(",
        "fn resolve_stream_spec(",
        "OPENAI_CLI_SYNC_PLAN_KIND",
        "OPENAI_COMPACT_STREAM_PLAN_KIND",
        "LocalOpenAiResponsesSpec {",
    ] {
        assert!(
            !plans.contains(forbidden),
            "gateway planner/standard/openai/responses/plans.rs should not keep pure openai-responses resolver detail {forbidden}"
        );
    }
}

#[test]
fn ai_serving_legacy_api_format_names_stay_out_of_primary_paths() {
    for path in [
        "crates/aether-ai/formats/src/contracts/plan_kinds.rs",
        "crates/aether-ai/formats/src/formats/shared/routing.rs",
        "crates/aether-ai/formats/src/formats/openai/responses/spec.rs",
        "apps/aether-gateway/src/ai_serving/planner/decision/control_plan.rs",
        "apps/aether-gateway/src/execution_runtime/fallback.rs",
    ] {
        let source = read_workspace_file(path);
        for forbidden in [
            "openai:cli",
            "openai:compact",
            "claude:chat",
            "claude:cli",
            "gemini:chat",
            "gemini:cli",
            "openai_cli_",
            "openai_compact_",
            "OPENAI_CLI",
            "OPENAI_COMPACT",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} should not emit or branch on legacy OpenAI Responses aliases: {forbidden}"
            );
        }
    }

    let registry = read_workspace_file("crates/aether-ai/formats/src/formats/registry.rs");
    let implementation = registry
        .split("#[cfg(test)]")
        .next()
        .expect("registry source should have an implementation section");
    for forbidden in [
        "\"openai:cli\"",
        "\"openai:compact\"",
        "\"claude:chat\"",
        "\"claude:cli\"",
        "\"gemini:chat\"",
        "\"gemini:cli\"",
    ] {
        assert!(
            !implementation.contains(forbidden),
            "format conversion registry implementation should not branch on retired API format aliases: {forbidden}"
        );
    }
}

#[test]
fn retired_api_format_occurrences_are_whitelisted() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root should resolve");
    let mut files = Vec::new();
    for root in ["apps", "crates", "frontend/src"] {
        collect_alias_scan_files(&workspace_root.join(root), &mut files);
    }

    let allowed_paths = [
        "apps/aether-gateway/src/handlers/admin/provider/write/normalize.rs",
        "apps/aether-gateway/src/handlers/admin/request/system/import.rs",
        "apps/aether-gateway/src/tests/control/admin/system_import.rs",
        "crates/aether-ai/formats/src/formats/id.rs",
        "crates/aether-ai/formats/src/formats/matrix.rs",
        "crates/aether-ai/formats/src/formats/registry.rs",
        "crates/aether-data/runtime/src/migrate.rs",
        "crates/aether-data/runtime/src/lifecycle/migrate/tests.rs",
        "crates/aether-usage/runtime/src/report.rs",
        "frontend/src/api/endpoints/types/__tests__/api-format.spec.ts",
        "frontend/src/views/admin/module-management/modelDirectivesConfig.ts",
        "frontend/src/views/admin/module-management/__tests__/modelDirectivesConfig.spec.ts",
    ];
    let allowed = allowed_paths
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let patterns = [
        "openai:cli",
        "openai:compact",
        "claude:chat",
        "claude:cli",
        "gemini:chat",
        "gemini:cli",
    ];

    let mut violations = Vec::new();
    for file in files {
        let relative = file
            .strip_prefix(&workspace_root)
            .expect("file should be under workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == "apps/aether-gateway/src/tests/architecture/ai_serving.rs" {
            continue;
        }

        let source = std::fs::read_to_string(&file).expect("source file should be readable");
        let hits = patterns
            .iter()
            .filter(|pattern| source.contains(**pattern))
            .copied()
            .collect::<Vec<_>>();
        if !hits.is_empty() && !allowed.contains(relative.as_str()) {
            violations.push(format!("{relative} -> {}", hits.join(", ")));
        }
    }

    assert!(
        violations.is_empty(),
        "retired API format aliases should stay confined to migration or negative-test files:\n{}",
        violations.join("\n")
    );
}

fn collect_alias_scan_files(root: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(root).expect("directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|value| value.to_str());
            if matches!(name, Some("target" | "node_modules" | ".git")) {
                continue;
            }
            collect_alias_scan_files(&path, files);
            continue;
        }

        if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("rs" | "ts" | "vue")
        ) {
            files.push(path);
        }
    }
}
