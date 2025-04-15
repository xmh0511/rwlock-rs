use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicI32, Ordering},
};

const IDLE: i32 = 0;
const WRITING: i32 = -1;

pub struct RWLock<T> {
    state: AtomicI32,
    data: UnsafeCell<T>,
}
pub struct ReadOnlyGuard<'a, T> {
    lock: &'a RWLock<T>,
}
impl<'a, T> Deref for ReadOnlyGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
impl<'a, T> Drop for ReadOnlyGuard<'a, T> {
    fn drop(&mut self) {
        // the last reader `Rl` is responsible for setting the `state` to `IDLE`
        self.lock.state.fetch_sub(1, Ordering::Release);
        // [atomics.order] p2
        // An atomic operation A that performs a release operation on an atomic object M
        // synchronizes with an atomic operation B
        // that performs an acquire operation on M and
        // takes its value from any side effect in the release sequence headed by A.

        // modification order: {...,R,R1,R2,...,Rl_drop, W,...}
        // This guarantees that any other reader `R` synchronizes with the writer
        // since the release sequence headed by `R` comprises the drop of the last reader `Rl` writing `IDLE`
        // that synchronizes with the writer `W`
        // this should be upheld(i.e. using release memory ordering), otherwise the reader other than the last would be data race with the writer
    }
}

impl<T> RWLock<T> {
    pub fn new(val: T) -> Self {
        RWLock {
            data: UnsafeCell::new(val),
            state: AtomicI32::new(IDLE),
        }
    }
    pub fn read(&self) -> ReadOnlyGuard<'_, T> {
        // initially assuming the state is IDLE
        let mut current = IDLE;
        // and the corresponding reader count is `1`
        let mut reader_count = 1;
        // if the comparison fails, it means either there exists a writer or at least one reader
        // For the case of having readers, increase the number based on the current number the failed CAS loaded

        // modification order: {...,W_drop,R0,R1,R2,...}
        // For the exclusive writer, just waiting for IDLE
        // [intro.races] p5
        // The drop of the writer releases the `state`, all RMW operations produced by the subsequent readers will be headed by it
        // [atomics.order] p2
        // so the drop of the writer synchronizes with any of them
        while let Err(actual) = self.state.compare_exchange_weak(
            current,
            reader_count,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            if actual == i32::MAX {
                panic!(
                    "the count of readers will exceed the maximum number of supported {}",
                    i32::MAX
                );
            }
            //println!("actual {actual} current {current} reader_count {reader_count}");

            // reader already exists
            if actual > 0 {
                current = actual;
                reader_count = actual + 1; // increase the number of reader
            } else if actual == WRITING || actual == IDLE {
                // writer already exists, so just waiting for `current=IDLE` and setting `reader_count=1`,
                // or comparison failed due to previously existing readers checked in the CAS of the preceding iteration where `current` was set to the number of readers
                // and reader_count was one greater than that count
                // however, the `state` is now `IDLE` anyway, so do something the same as below
                current = IDLE;
                reader_count = 1;
            }
            std::hint::spin_loop();
        }
        ReadOnlyGuard { lock: &self }
    }
    pub fn write(&self) -> LockGuard<'_, T> {
        // acquire the lock iif there is no reader
        while self
            .state
            .compare_exchange_weak(IDLE, WRITING, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        LockGuard { lock: &self }
    }
}
pub struct LockGuard<'a, T> {
    lock: &'a RWLock<T>,
}
impl<'a, T> Deref for LockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
impl<'a, T> DerefMut for LockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
impl<'a, T> Drop for LockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.state.store(IDLE, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for RWLock<T> {}
unsafe impl<T: Sync> Sync for RWLock<T> {}
