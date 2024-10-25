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
    data: &'a T,
    lock: &'a RWLock<T>,
}
impl<'a, T> Deref for ReadOnlyGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
impl<'a, T> Drop for ReadOnlyGuard<'a, T> {
    fn drop(&mut self) {
        // the last reader is responsible for setting the `state` to `IDLE`
		self.lock.state.fetch_sub(1, Ordering::Release);
		// [atomics.order] p2
		// An atomic operation A that performs a release operation on an atomic object M 
		// synchronizes with an atomic operation B 
		// that performs an acquire operation on M and 
		// takes its value from any side effect in the release sequence headed by A.

		// This guarantees that any other reader `R` synchronizes with the writer
		// since the release sequence headed by `R` comprises the last reader writing `IDLE` 
		// that synchronizes with writer
		// this should be upheld, otherwise the reader other than the last would be data race with the writer
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
		// the corresponding reader count is `1`
		let mut reader_count = 1;
		// if the comparison fails, it means either there exits a writer or at least one reader
		// For the case when having readers, increase the number based on the current numbers
		// For the exclusive writer, waiting for IDLE
		// The drop of the writer releases the `state`, all RMW operations of the subsequent readers will be headed by it
		// so the drop of the writer synchronizes with any of them
        while let Err(actual) =
            self.state
                .compare_exchange_weak(current, reader_count, Ordering::Acquire, Ordering::Relaxed)
        {
			// reader already exists
            if actual > 0 {
                current = actual;
				reader_count = actual +1; // increase the number of reader
            }
            std::hint::spin_loop();
        }
        ReadOnlyGuard {
            data: unsafe { &*self.data.get() },
            lock: &self,
        }
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
        LockGuard {
            data: unsafe { &mut *self.data.get() },
            lock: &self,
        }
    }
}
pub struct LockGuard<'a, T> {
    data: &'a mut T,
    lock: &'a RWLock<T>,
}
impl<'a, T> Deref for LockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}
impl<'a, T> DerefMut for LockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
impl<'a, T> Drop for LockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.state.store(IDLE, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for RWLock<T> {}
unsafe impl<T: Sync> Sync for RWLock<T> {}
