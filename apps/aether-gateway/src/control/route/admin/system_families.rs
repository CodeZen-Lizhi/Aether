use axum::http;

use super::{classified, ClassifiedRoute};

/// 识别系统配置、备份导入导出和数据维护路由。
pub(super) fn classify_admin_system_family_route(
    method: &http::Method,
    normalized_path: &str,
    _normalized_path_no_trailing: &str,
) -> Option<ClassifiedRoute> {
    if method == http::Method::GET && normalized_path == "/api/admin/system/version" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "version",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/aws-regions" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "aws_regions",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/stats" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "stats",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/config/export" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "config_export",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/data/export" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "data_export",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/config/import" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "config_import",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/data/import" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "data_import",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/cleanup" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "cleanup",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/cleanup/runs" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "cleanup_runs",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST
        && normalized_path == "/api/admin/system/cleanup/usage/manual"
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "cleanup_usage_manual",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET
        && normalized_path == "/api/admin/system/cleanup/usage/preview"
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "cleanup_usage_preview",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/purge/config" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_config",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/purge/usage" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_usage",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST
        && normalized_path == "/api/admin/system/purge/audit-logs"
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_audit_logs",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST
        && normalized_path == "/api/admin/system/purge/request-bodies"
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_request_bodies",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST
        && normalized_path == "/api/admin/system/purge/request-bodies/task"
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_request_bodies_task",
            "admin:system",
            false,
        ))
    } else if method == http::Method::POST && normalized_path == "/api/admin/system/purge/stats" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "purge_stats",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET
        && matches!(
            normalized_path,
            "/api/admin/system/configs" | "/api/admin/system/configs/"
        )
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "configs_list",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET
        && normalized_path.starts_with("/api/admin/system/configs/")
        && normalized_path.matches('/').count() == 5
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "config_get",
            "admin:system",
            false,
        ))
    } else if method == http::Method::PUT
        && normalized_path.starts_with("/api/admin/system/configs/")
        && normalized_path.matches('/').count() == 5
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "config_set",
            "admin:system",
            false,
        ))
    } else if method == http::Method::DELETE
        && normalized_path.starts_with("/api/admin/system/configs/")
        && normalized_path.matches('/').count() == 5
    {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "config_delete",
            "admin:system",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/admin/system/api-formats" {
        Some(classified(
            "admin_proxy",
            "system_manage",
            "api_formats",
            "admin:system",
            false,
        ))
    } else {
        None
    }
}
