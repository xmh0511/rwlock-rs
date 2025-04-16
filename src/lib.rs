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
impl<T> Deref for ReadOnlyGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
impl<T> Drop for ReadOnlyGuard<'_, T> {
    fn drop(&mut self) {
        // the last reader `Rl` is responsible for setting the `state` to `IDLE`
        self.lock.state.fetch_sub(1, Ordering::Release);
        // [atomics.order] p2
        // An atomic operation A that performs a release operation on an atomic object M
        // synchronizes with an atomic operation B
        // that performs an acquire operation on M and
        // takes its value from any side effect in the release sequence headed by A.

        // [intro.races] p5
        // A release sequence headed by a release operation A on an atomic object M is a maximal contiguous
        // sub-sequence of side effects in the modification order of M, where the first operation is A,
        // and every subsequent operation is an atomic read-modify-write operation.

        // modification order: {...,R_drop,...,Rl_drop, W,W_drop,...}
        // This guarantees that any drop of the other readers `R_drop` synchronizes with the writer `W`
        // since the release sequence headed by `R_drop` comprises the drop of the last reader `Rl` writing `IDLE`
        // that synchronizes with the writer `W`
        // for example, read_op is sequenced-before R_drop, R_drop synchronizes with W, W is sequenced-before write_op
        // so read_op happens-before write_op
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
        // and the set reader count is `1`
        let mut reader_count = 1;
        // If the comparison fails, for the first comparison, it means either there exists a writer or at least one reader
        // or the readers indicated by the preceding CAS were all dropped(including the subsequent winning writer that was dropped)
        // For the case of having readers, increase the number based on the current number the failed CAS loaded

        // For the case of having the exclusive writer, just waiting for IDLE
        // modification order: {...,W,W_drop,R0,R1,R2,...}
        // [intro.races] p5
        // The drop of the writer releases the `state`, all RMW operations produced by the subsequent readers will be headed by it
        // [atomics.order] p2
        // so the drop of the writer synchronizes with any of them
        // for example, write_op is sequenced-before W_drop, W_drop synchronizes with R, R is sequenced-before read_op
        // so write_op happens-before read_op
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
            if actual >= 0 {
                // Failed Reason for this branch:
                // 1. The actual number of readers is greater than the number 0 that we assumed before the first CAS.
                // 2. The comparison of the existing readers in the CAS of the preceding iteration failed, such that the `current`
                // was set to the number of readers loaded by CAS,
                // however, at this time, the previously existing readers were all dropped(including immediately followed by a writer that was dropped),
                // anyway, the state is `IDLE`(i.e. 0) now.
                current = actual;
                reader_count = actual + 1; // increase the number of reader
            } else if actual == WRITING {
                // writer already exists, so just waiting for `current=IDLE` and setting `reader_count=1`,
                current = IDLE;
                reader_count = 1;
                std::hint::spin_loop();
            } else {
                unreachable!("The actual state == {actual}, which is not expected");
            }
        }
        ReadOnlyGuard { lock: self }
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
        LockGuard { lock: self }
    }
}
pub struct LockGuard<'a, T> {
    lock: &'a RWLock<T>,
}
impl<T> Deref for LockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}
impl<T> DerefMut for LockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
impl<T> Drop for LockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.state.store(IDLE, Ordering::Release);
    }
}

unsafe impl<T: Send> Send for RWLock<T> {}
unsafe impl<T: Sync> Sync for RWLock<T> {}
