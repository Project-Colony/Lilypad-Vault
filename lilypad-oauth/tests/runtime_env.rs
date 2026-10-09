//! The OAuth App is fixed when Lilypad is built: a `LILYPAD_GITHUB_CLIENT_ID`
//! set at run time must not change it. This test has its own binary because
//! it sets an environment variable, which is unsafe while other test threads
//! read the environment.

use lilypad_oauth::{OAuthConfig, BUILTIN_GITHUB_CLIENT_ID};

#[test]
fn runtime_client_id_variable_changes_nothing() {
    let client_id = || {
        OAuthConfig::builtin_github()
            .map(|config| config.client_id)
            .map_err(|e| e.to_string())
    };

    let before = client_id();
    std::env::set_var("LILYPAD_GITHUB_CLIENT_ID", "runtime-override");
    let after = client_id();
    std::env::remove_var("LILYPAD_GITHUB_CLIENT_ID");

    assert_eq!(before, after);
    match after {
        Ok(id) => assert_eq!(id, BUILTIN_GITHUB_CLIENT_ID),
        Err(msg) => {
            assert!(BUILTIN_GITHUB_CLIENT_ID.is_empty());
            assert!(msg.contains("no GitHub OAuth client id was compiled in"));
        }
    }
}
