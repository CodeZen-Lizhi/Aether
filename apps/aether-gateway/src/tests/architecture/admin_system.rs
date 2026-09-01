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
