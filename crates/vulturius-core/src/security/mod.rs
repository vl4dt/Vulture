//! Phase 11: Security Audit
//!
//! - Sandboxed file operations (allowed path enforcement)
//! - Permission model (admin vs user, privilege checking)
//! - Path traversal protection (directory escape prevention)
//! - Audit logging (security event recording)

pub mod sandbox;
pub mod permission;
pub mod path_guard;
pub mod audit;

pub use sandbox::*;
pub use permission::*;
pub use path_guard::*;
pub use audit::*;
