use http::Uri;

use crate::handlers::shared::local_proxy_route_requires_buffered_body;

use super::{classify_control_route, headers, GatewayPublicRequestContext};
