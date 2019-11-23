extern crate sys_info;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::spawn;
use std::thread::JoinHandle;

struct Job {
    func: Box<dyn Fn(*mut u64, usize, usize)>,
    elems: *mut u64,
    num: usize,
    work_index: AtomicUsize,
    done_mutex: Arc<Mutex<usize>>,
    signal: Arc<Condvar>,
}
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

pub struct Pool {
    senders: Vec<Sender<Arc<Job>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Job {
    fn execute(&self) {
        loop {
            let index = self.work_index.fetch_add(1, Ordering::Relaxed);
            if index >= self.num {
                let mut guard = self.done_mutex.lock().unwrap();
                *guard += 1;
                self.signal.notify_one();
                break;
            }
            (self.func)(self.elems, self.num, index);
        }
    }
    fn wait(&self, num: usize) {
        loop {
            let guard = self.done_mutex.lock().unwrap();
            if *guard >= num {
                break;
            }
            let _ = self
                .signal
                .wait(guard)
                .expect("condvar wait should never fail");
        }
    }
}

impl Pool {
    pub fn new() -> Self {
        let num_threads = sys_info::cpu_num().unwrap_or(16) - 1;
        let mut pool = Self {
            senders: vec![],
            threads: vec![],
        };
        (0..num_threads).for_each(|_| {
            let (sender, recvr): (Sender<Arc<Job>>, Receiver<Arc<Job>>) = channel();
            let t = spawn(move || {
                for job in recvr.iter() {
                    job.execute()
                }
            });
            pool.senders.push(sender);
            pool.threads.push(t);
        });
        pool
    }

    pub fn dispatch_mut<F, A>(&self, elems: &mut [A], func: Box<F>)
    where
        F: Fn(&mut A) + 'static,
    {
        let job = Job {
            elems: elems.as_mut_ptr() as *mut u64,
            num: elems.len(),
            done_mutex: Arc::new(Mutex::new(0)),
            work_index: AtomicUsize::new(0),
            signal: Arc::new(Condvar::new()),
            func: Box::new(move |ptr, num, index| {
                let ptr = ptr as *mut A;
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr, num) };
                func(&mut slice[index])
            }),
        };
        let job = Arc::new(job);
        for s in &self.senders {
            s.send(job.clone()).expect("send should never fail");
        }
        job.execute();
        job.wait(self.senders.len() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pool() {
        let pool = Pool::new();
        let mut array = [0usize; 100];
        pool.dispatch_mut(&mut array, Box::new(|val: &mut usize| *val += 1));
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    }
}
