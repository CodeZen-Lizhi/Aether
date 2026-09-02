pub(crate) const ROLE_ADMIN: &str = "admin";

pub(crate) fn is_full_admin_role(role: &str) -> bool {
    role.trim().eq_ignore_ascii_case(ROLE_ADMIN)
}

pub(crate) fn can_access_admin_console(role: &str) -> bool {
    is_full_admin_role(role)
}

#[cfg(test)]
mod tests {
    use super::can_access_admin_console;

    #[test]
    fn only_admin_can_access_admin_console() {
        assert!(can_access_admin_console("admin"));
        assert!(can_access_admin_console(" Admin "));
        assert!(!can_access_admin_console("audit_admin"));
        assert!(!can_access_admin_console("user"));
        assert!(!can_access_admin_console("owner"));
    }
}
