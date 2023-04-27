use std::sync::atomic::{AtomicI64, Ordering};
use std::thread;
pub struct MyRwLock {
    cnt: AtomicI64,
}

impl MyRwLock {
    pub fn new() -> Self {
        MyRwLock {
            cnt: AtomicI64::new(0),
        }
    }

    pub fn read(&self) {
        let mut expected;
        loop {
            expected = self.cnt.load(Ordering::Acquire);
            if expected >= 0
                && self
                    .cnt
                    .compare_exchange(expected, expected + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return;
            } else {
                thread::yield_now();
            }
        }
    }

    pub fn write(&self) {
        let mut expected;
        loop {
            expected = self.cnt.load(Ordering::Acquire);
            if expected == 0
                && self
                    .cnt
                    .compare_exchange(expected, -1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return;
            } else {
                thread::yield_now();
            }
        }
    }

    pub fn unlock_read(&self) {
        self.cnt.fetch_add(-1, Ordering::AcqRel);
    }

    pub fn unlock_write(&self) {
        self.cnt.fetch_add(1, Ordering::AcqRel);
    }

    pub fn is_locked(&self) -> bool {
        self.cnt.load(Ordering::Acquire) != 0
    }
}

pub struct MyMutex {
    my_rw_lock: MyRwLock,
}

impl MyMutex {
    pub fn new() -> Self {
        MyMutex {
            my_rw_lock: MyRwLock::new(),
        }
    }

    pub fn lock(&self) {
        self.my_rw_lock.write();
    }

    pub fn unlock(&self) {
        self.my_rw_lock.unlock_write();
    }

    pub fn is_locked(&self) -> bool {
        self.my_rw_lock.is_locked()
    }
}

#[cfg(test)]
#[allow(unused_must_use)]
mod test {
    use super::*;
    use std::cell::UnsafeCell;
    use std::sync::Arc;
    use std::thread;

    struct SyncUnsafeCell<T>(UnsafeCell<T>);

    unsafe impl<T> Sync for SyncUnsafeCell<T> where T: Send + Sync {}

    impl<T> SyncUnsafeCell<T> {
        fn new(value: T) -> Self {
            SyncUnsafeCell(UnsafeCell::new(value))
        }

        unsafe fn get(&self) -> &T {
            &*self.0.get()
        }

        unsafe fn get_mut(&self) -> &mut T {
            &mut *self.0.get()
        }
    }

    #[test]
    fn test_rw_lock_thread_safety() {
        const NUM_READERS: usize = 10;
        const NUM_WRITERS: usize = 5;
        const NUM_ITERATIONS: usize = 100000;

        let rw_lock = Arc::new(MyRwLock::new());
        let shared_data = Arc::new(SyncUnsafeCell::new(0_usize));
        let mut handles = Vec::new();

        for _ in 0..NUM_READERS {
            let rw_lock_clone = rw_lock.clone();
            let shared_data_clone = shared_data.clone();
            let handle = thread::spawn(move || {
                for _ in 0..NUM_ITERATIONS {
                    rw_lock_clone.read();
                    let read_value = unsafe { *shared_data_clone.get() };
                    assert_eq!(read_value % 2, 0);
                    rw_lock_clone.unlock_read();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let mut handles = Vec::new();

        for _ in 0..NUM_WRITERS {
            let rw_lock_clone = rw_lock.clone();
            let shared_data_clone = shared_data.clone();
            let handle = thread::spawn(move || {
                for _ in 0..NUM_ITERATIONS {
                    rw_lock_clone.write();
                    let write_value = unsafe { &mut *shared_data_clone.get_mut() };
                    *write_value += 1;
                    rw_lock_clone.unlock_write();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(unsafe { *shared_data.get() }, NUM_WRITERS * NUM_ITERATIONS);

        let mut handles = Vec::new();

        // Reader, Writer Mix
        for _ in 0..NUM_READERS {
            let rw_lock_clone = rw_lock.clone();
            let shared_data_clone = shared_data.clone();
            let handle = thread::spawn(move || {
                for _ in 0..NUM_ITERATIONS {
                    rw_lock_clone.read();
                    let read_value = unsafe { *shared_data_clone.get() };
                    assert!(read_value <= NUM_WRITERS * NUM_ITERATIONS * 2);
                    rw_lock_clone.unlock_read();
                }
            });
            handles.push(handle);
        }

        for _ in 0..NUM_WRITERS {
            let rw_lock_clone = rw_lock.clone();
            let shared_data_clone = shared_data.clone();
            let handle = thread::spawn(move || {
                for _ in 0..NUM_ITERATIONS {
                    rw_lock_clone.write();
                    let write_value = unsafe { &mut *shared_data_clone.get_mut() };
                    *write_value += 1;
                    rw_lock_clone.unlock_write();
                }
            });
            handles.push(handle);
        }

        handles
            .into_iter()
            .for_each(|handle| handle.join().unwrap());
        assert_eq!(
            unsafe { *shared_data.get() },
            NUM_WRITERS * NUM_ITERATIONS * 2
        );
    }

    #[test]
    fn test_mutex_thread_safety() {
        const NUM_THREADS: usize = 10;
        const NUM_ITERATIONS: usize = 100000;

        let mutex = Arc::new(MyMutex::new());
        let shared_data = Arc::new(SyncUnsafeCell::new(0_usize));
        let mut handles = Vec::new();

        for _ in 0..NUM_THREADS {
            let mutex_clone = mutex.clone();
            let shared_data_clone = shared_data.clone();
            let handle = thread::spawn(move || {
                for _ in 0..NUM_ITERATIONS {
                    mutex_clone.lock();
                    let data = unsafe { &mut *shared_data_clone.get_mut() };
                    *data += 1;
                    mutex_clone.unlock();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_value = unsafe { *shared_data.get() };
        assert_eq!(final_value, NUM_THREADS * NUM_ITERATIONS);
    }
}
