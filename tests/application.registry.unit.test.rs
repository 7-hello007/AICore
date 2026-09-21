use aicore_api::ApplicationRegistry;
use aicore_contracts::application::id::ApplicationId;

#[test]
fn register_and_get() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("com.example.vscode");

    let r = reg
        .register(
            app.clone(),
            "VSCode",
            "1.0.0",
            vec!["gpu.compute".to_string()],
        )
        .unwrap();

    assert_eq!(r.application_id, app);
    assert_eq!(r.name, "VSCode");
    assert_eq!(r.version, "1.0.0");
    assert_eq!(r.capabilities, vec!["gpu.compute".to_string()]);

    let found = reg.get(&app).unwrap().unwrap();
    assert_eq!(found.name, "VSCode");
}

#[test]
fn unregister_returns_true_when_existed() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("com.example.a");
    reg.register(app.clone(), "A", "1", vec![]).unwrap();
    assert!(reg.unregister(&app).unwrap());
    assert!(!reg.unregister(&app).unwrap());
}

#[test]
fn is_registered_reflects_state() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("com.example.b");
    assert!(!reg.is_registered(&app).unwrap());
    reg.register(app.clone(), "B", "1", vec![]).unwrap();
    assert!(reg.is_registered(&app).unwrap());
}

#[test]
fn list_returns_all_registrations() {
    let reg = ApplicationRegistry::new();
    reg.register(ApplicationId::new("a"), "A", "1", vec![])
        .unwrap();
    reg.register(ApplicationId::new("b"), "B", "1", vec![])
        .unwrap();
    reg.register(ApplicationId::new("c"), "C", "1", vec![])
        .unwrap();

    assert_eq!(reg.count().unwrap(), 3);
    let mut names: Vec<String> = reg.list().unwrap().into_iter().map(|r| r.name).collect();
    names.sort();
    assert_eq!(names, vec!["A", "B", "C"]);
}

#[test]
fn register_rejects_empty_id() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("");
    let r = reg.register(app, "Empty", "1", vec![]);
    assert!(r.is_err());
}

#[test]
fn register_rejects_too_long_id() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("x".repeat(200));
    let r = reg.register(app, "Long", "1", vec![]);
    assert!(r.is_err());
}

#[test]
fn re_register_overwrites() {
    let reg = ApplicationRegistry::new();
    let app = ApplicationId::new("com.example.dup");
    reg.register(app.clone(), "Old", "1", vec![]).unwrap();
    reg.register(app.clone(), "New", "2", vec![]).unwrap();

    let r = reg.get(&app).unwrap().unwrap();
    assert_eq!(r.name, "New");
    assert_eq!(r.version, "2");
    assert_eq!(reg.count().unwrap(), 1);
}
