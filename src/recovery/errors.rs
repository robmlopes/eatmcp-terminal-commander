use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    CommandNotFound,
    PermissionDenied,
    Timeout,
    ServiceMissing,
    PackageNotFound,
    GeneralFailure,
}

impl std::fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorClass::CommandNotFound => write!(f, "COMMAND_NOT_FOUND"),
            ErrorClass::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            ErrorClass::Timeout => write!(f, "TIMEOUT"),
            ErrorClass::ServiceMissing => write!(f, "SERVICE_MISSING"),
            ErrorClass::PackageNotFound => write!(f, "PACKAGE_NOT_FOUND"),
            ErrorClass::GeneralFailure => write!(f, "GENERAL_FAILURE"),
        }
    }
}
