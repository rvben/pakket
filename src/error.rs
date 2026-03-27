pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const GENERAL_ERROR: i32 = 1;
    pub const CONFIG_ERROR: i32 = 2;
    pub const API_ERROR: i32 = 3;
    pub const NOT_FOUND: i32 = 4;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("{0} not found")]
    NotFound(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Config(_) => exit_codes::CONFIG_ERROR,
            Error::Api(_) => exit_codes::API_ERROR,
            Error::NotFound(_) => exit_codes::NOT_FOUND,
            Error::Http(_) | Error::Other(_) => exit_codes::GENERAL_ERROR,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_config() {
        let err = Error::Config("test".to_string());
        assert_eq!(err.exit_code(), exit_codes::CONFIG_ERROR);
    }

    #[test]
    fn exit_code_api() {
        let err = Error::Api("test".to_string());
        assert_eq!(err.exit_code(), exit_codes::API_ERROR);
    }

    #[test]
    fn exit_code_not_found() {
        let err = Error::NotFound("test".to_string());
        assert_eq!(err.exit_code(), exit_codes::NOT_FOUND);
    }

    #[test]
    fn exit_code_other() {
        let err = Error::Other("test".to_string());
        assert_eq!(err.exit_code(), exit_codes::GENERAL_ERROR);
    }

    #[test]
    fn exit_code_constants() {
        assert_eq!(exit_codes::SUCCESS, 0);
        assert_eq!(exit_codes::GENERAL_ERROR, 1);
        assert_eq!(exit_codes::CONFIG_ERROR, 2);
        assert_eq!(exit_codes::API_ERROR, 3);
        assert_eq!(exit_codes::NOT_FOUND, 4);
    }

    #[test]
    fn error_display() {
        let err = Error::Config("missing key".to_string());
        assert_eq!(err.to_string(), "Configuration error: missing key");
    }
}
