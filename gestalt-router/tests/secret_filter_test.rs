use gestalt_router::xavier_sink::{contains_secret, redact, redact_json};
use serde_json::json;

#[test]
fn test_contains_secret_identifies_sensitive_patterns() {
    // Sensitive patterns detected
    assert!(contains_secret("XAVIER_TOKEN=somevalue"));
    assert!(contains_secret("password: secretpassword"));
    assert!(contains_secret("using token 123456"));
    assert!(contains_secret("this is a secret"));
    assert!(contains_secret("api_key=mykey"));
    assert!(contains_secret("api-key=mykey"));
    assert!(contains_secret("apikey: value"));
    assert!(contains_secret("ghp_1234567890abcdefghijklmnopqrstuvwx"));
    assert!(contains_secret("sk-proj-abcdefghijklmnopqrstuvwxyz"));

    // Case insensitivity
    assert!(contains_secret("xavier_token=somevalue"));
    assert!(contains_secret("PASSWORD=123"));
    assert!(contains_secret("SecretKey=xyz"));

    // Normal strings unchanged / not detected
    assert!(!contains_secret("hello world"));
    assert!(!contains_secret("normal summary passes unchanged"));
    assert!(!contains_secret("checkpoint committed"));
}

#[test]
fn test_redact_filters_sensitive_patterns() {
    // Test summary with XAVIER_TOKEN=abc123 -> [REDACTED]
    let res = redact("XAVIER_TOKEN=abc123");
    assert!(res.contains("[REDACTED]"));
    assert!(!res.contains("abc123"));

    // Test sk- OpenAI-style key and ghp_ GitHub token detected and redacted
    let res_sk = redact("OpenAI key is sk-proj-12345abcde and github token is ghp_12345xyz");
    assert!(res_sk.contains("[REDACTED]"));
    assert!(!res_sk.contains("sk-proj-12345abcde"));
    assert!(!res_sk.contains("ghp_12345xyz"));

    // Test normal summary passes unchanged
    let normal = "normal summary passes unchanged";
    assert_eq!(redact(normal), normal);

    // Test other variations of key-value pairs
    let res_pwd = redact("password: 'abc'");
    assert!(res_pwd.contains("[REDACTED]"));
    assert!(!res_pwd.contains("abc"));

    let res_apikey = redact("api-key=my-api-key-value");
    assert!(res_apikey.contains("[REDACTED]"));
    assert!(!res_apikey.contains("my-api-key-value"));
}

#[test]
fn test_redact_json_filters_recursively() {
    let payload = json!({
        "agent": "hermes",
        "nested": {
            "password": "mysecretpassword",
            "normal_field": "hello world"
        },
        "array_field": [
            "normal string",
            "ghp_12345678"
        ],
        "XAVIER_TOKEN": "some_token_value"
    });

    let redacted_payload = redact_json(payload);

    // Verify sensitive keys are redacted
    assert_eq!(redacted_payload["XAVIER_TOKEN"], "[REDACTED]");
    assert_eq!(redacted_payload["nested"]["password"], "[REDACTED]");

    // Verify recursive strings in array are redacted
    assert_eq!(redacted_payload["array_field"][1], "[REDACTED]");

    // Verify normal fields are untouched
    assert_eq!(redacted_payload["nested"]["normal_field"], "hello world");
    assert_eq!(redacted_payload["array_field"][0], "normal string");
}
