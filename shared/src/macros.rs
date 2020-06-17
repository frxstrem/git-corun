#[macro_export]
macro_rules! lock {
    ($pat:pat = $lock:expr => $body:expr) => {{
        let lock = $lock.lock().unwrap();
        let pat = &mut *lock;
        $body
    }};

    (async $pat:pat = $lock:expr => $body:expr) => {{
        let lock = $lock.lock().await;
        let pat = &mut *lock;
        $body
    }};
}
