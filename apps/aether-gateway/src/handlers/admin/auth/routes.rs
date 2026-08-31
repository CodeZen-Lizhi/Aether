use super::api_keys;
use crate::handlers::admin::request::{AdminRouteRequest, AdminRouteResult};

pub(crate) async fn maybe_build_local_admin_auth_response(
    request: AdminRouteRequest<'_>,
) -> AdminRouteResult {
    if let Some(response) = api_keys::maybe_build_local_admin_api_keys_response(
        &request.state(),
        &request.request_context(),
        request.request_headers(),
        request.request_body(),
    )
    .await?
    {
        return Ok(Some(response));
    }

    Ok(None)
}
