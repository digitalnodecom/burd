//! Runs against a real Redis binary in an isolated profile. Opt in with
//! BURD_TEST_REDIS_BINARY=/absolute/path/to/redis-server cargo test --test native_lifecycle -- --ignored
use burd_lib::config::{get_instance_dir, get_versioned_binary_path, ConfigStore, ServiceType};
use burd_lib::ProcessManager;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

struct ServiceGuard(uuid::Uuid);
impl Drop for ServiceGuard {
    fn drop(&mut self) {
        if ProcessManager::new().is_running(&self.0) {
            let _ = ProcessManager::new().stop(&self.0);
        }
    }
}

fn redis(port: u16, command: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => panic!("Redis failed to start: {error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(command.as_bytes()).unwrap();
    let mut bytes = [0; 1024];
    let count = stream.read(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes[..count]).into_owned()
}

#[test]
#[ignore = "requires an explicit Redis binary and local TCP access"]
fn real_redis_start_stop_restart_and_retained_data() {
    let binary = std::env::var_os("BURD_TEST_REDIS_BINARY").expect("Set BURD_TEST_REDIS_BINARY");
    assert!(std::path::Path::new(&binary).is_absolute());
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("BURD_DATA_DIR", profile.path());
    let versioned = get_versioned_binary_path(ServiceType::Redis, "smoke-test").unwrap();
    std::fs::create_dir_all(versioned.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(binary, &versioned).unwrap();
    let reservation = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    let store = ConfigStore::new().unwrap();
    let instance = store
        .create_instance(
            "smoke-test".into(),
            port,
            ServiceType::Redis,
            "smoke-test".into(),
            serde_json::json!({}),
            None,
        )
        .unwrap();
    drop(reservation);
    let _guard = ServiceGuard(instance.id);
    let processes = ProcessManager::new();
    processes.start(&instance, None, false).unwrap();
    assert_eq!(redis(port, "PING\r\n"), "+PONG\r\n");
    assert_eq!(redis(port, "SET smoke-test retained\r\n"), "+OK\r\n");
    assert!(processes.start(&instance, None, false).is_err());
    assert_eq!(redis(port, "SAVE\r\n"), "+OK\r\n");
    processes.stop(&instance.id).unwrap();
    assert!(!processes.is_running(&instance.id));
    processes.start(&instance, None, false).unwrap();
    assert!(redis(port, "GET smoke-test\r\n").contains("retained"));
    processes.stop(&instance.id).unwrap();
    store.delete_instance(instance.id).unwrap();
    assert!(store.load().unwrap().instances.is_empty());
    assert!(get_instance_dir(&instance.id)
        .unwrap()
        .join("dump.rdb")
        .exists());
}
