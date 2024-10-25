use rwlock::RWLock;
use std::sync::Arc;
use std::thread;
fn main() {
    for _ in 0..200 {
        let lock = Arc::new(RWLock::new(1));
        let lock1 = lock.clone();
        // read 1
        let t1 = thread::spawn(move || {
            //std::thread::sleep(std::time::Duration::from_secs(3));
            let r = lock1.read();
            println!("r1 == {}", *r);
            //std::thread::sleep(std::time::Duration::from_secs(2));
        });
        let lock2 = lock.clone();
        // read 2
        let t2 = thread::spawn(move || {
            let r = lock2.read();
            println!("r2 == {}", *r);
            //std::thread::sleep(std::time::Duration::from_secs(2));
        });
        let lock3 = lock.clone();
        // writer 1
        let t3 = thread::spawn(move || {
            let mut r = lock3.write();
            println!("w1 == {}", *r);
            *r = 10;
        });
        let lock4 = lock.clone();
        // writer 2
        let t4 = thread::spawn(move || {
            let mut r = lock4.write();
            println!("w2 == {}", *r);
            *r = 11;
        });
        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        t4.join().unwrap();
        //println!("in loop {i}");
        println!("----------------------------------------");
    }
}
