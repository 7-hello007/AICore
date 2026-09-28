//! run_with_timeout sandbox testing.

use std::time::Duration;

use aicore_plugin::{run_with_timeout, SandboxError};

#[test]
fn fast_call_returns_value() {
    let result = run_with_timeout(|| 42u32, Duration::from_secs(1)).unwrap();
    assert_eq!(result, Some(42));
}

#[test]
fn slow_call_times_out() {
    let start = std::time::Instant::now();
    let result = run_with_timeout(
        || {
            std::thread::sleep(Duration::from_secs(5));
            7u32
        },
        Duration::from_millis(100),
    )
    .unwrap();

    assert!(result.is_none(), "should have timed out");
    // Make sure the caller isn't blocked.
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn panicking_call_returns_err() {
    let result = run_with_timeout::<_, u32>(|| panic!("boom"), Duration::from_secs(1));
    assert!(matches!(result, Err(SandboxError::Panicked)));
}

#[test]
fn string_payload() {
    let result = run_with_timeout(|| "hello".to_string(), Duration::from_secs(1)).unwrap();
    assert_eq!(result, Some("hello".to_string()));
}
