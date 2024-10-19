use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicI32, AtomicU8, Ordering},
};

const IDLE: u8 = 0;
const READING: u8 = 1;
const WRITING: u8 = 2;

pub struct RWLock<T> {
    state: AtomicU8,
    reader: AtomicI32,
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
        // the last reader, who is responsible for setting the `state` to `IDLE`
        if self.lock.reader.fetch_sub(1, Ordering::Relaxed) == 1 {
            self.lock.state.store(IDLE, Ordering::Release);
        }
    }
}

impl<T> RWLock<T> {
    pub fn new(val: T) -> Self {
        RWLock {
            data: UnsafeCell::new(val),
            state: AtomicU8::new(IDLE),
            reader: AtomicI32::new(0),
        }
    }
    pub fn read(&self) -> ReadOnlyGuard<'_, T> {
        // add the count for reader
        self.reader.fetch_add(1, Ordering::Relaxed);
        // initially assuming the state is IDLE
        let mut current = IDLE;
        // There may be other readers, so the actual `state` is `READING`,
        // so set `current` to `READING` to try to acquire the read lock
        // Because the above `fetch_add` races with `fetch_sub` in reader drop,
        // If the `fetch_sub` in drop of that existed reader wins and set the `state` to `IDLE`,
        // and `current` here is previously set to `READING`,
        // the comparsion will fail, so `current` should be set to `IDLE` for the next comparison
        // another case is that, when `current` is set ot `READING`, the writer instead wins the race
        // so the current is set to `IDLE` to try to acquire the read lock from releasing of writer
        while let Err(actual) =
            self.state
                .compare_exchange_weak(current, READING, Ordering::Acquire, Ordering::Relaxed)
        {
            if actual == IDLE || actual == READING {
                current = actual;
            }
            //assert_ne!(actual,WRITING);
            if actual == WRITING {
                current = IDLE;
            }
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
        {}
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
