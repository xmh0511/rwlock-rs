use rwlock::RWLock;
use std::sync::Arc;
use std::thread;
fn main() {
    for i in 0..200000 {
        let lock = Arc::new(RWLock::new(1));
        let lock1 = lock.clone();
        // read 1
        let t1 = thread::spawn(move || {
            //std::thread::sleep(std::time::Duration::from_secs(3));
            let r = lock1.read();
			_ = *r;
            //println!("r1 == {}", *r);
            //assert!(*r==1,"assert in t1");
            //std::thread::sleep(std::time::Duration::from_secs(2));
        });
        let lock2 = lock.clone();
        // read 2
        let t2 = thread::spawn(move || {
            let r = lock2.read();
			_ = *r;
            //println!("r2 == {}", *r);
            //std::thread::sleep(std::time::Duration::from_secs(2));
            //assert!(*r==1,"assert in t2");
        });
        let lock3 = lock.clone();
        // writer 1
        let t3 = thread::spawn(move || {
            let mut r = lock3.write();
            //println!("w1 == {}", *r);
            *r = 10;
        });
        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
		println!("in loop {i}");
        //println!("----------------------------------------");
    }
}
