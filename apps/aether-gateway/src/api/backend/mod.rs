mod admin;
mod internal;
mod public;

pub(crate) use admin::mount_admin_routes;
pub(crate) use internal::mount_internal_routes;
pub(crate) use public::mount_public_support_routes;
