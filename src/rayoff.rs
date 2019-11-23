use std::mem::transmute;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Condvar};
use std::thread::spawn;
use std::thread::JoinHandle;
use sys_info;

struct Job {
    func: fn(*mut u64, usize),
    elems: *mut u64,
    num: usize,
    work_index: AtomicUsize,
    done_index: AtomicUsize,
}

struct Pool {
    senders: Vec<Sender<Arc<Job>>>,
    threads: Vec<JoinHandle<()>>,
    signal: Condvar,
}

impl Job {
    fn execute(&self, signal: &Condvar) {
        loop {
            let index = self.work_index.fetch_and_add(Ordering::Relaxed, 1);
            if index >= self.num {
                self.done_index.add(Ordering::Relaxed, 1);
                signal.notify_one();
                break;
            }
            self.func(self.elems, index);
        }
    }
    fn wait(&self, signal: &Condvar, num: usize) {
        while self.done_index.fetch(Ordering::Relaxed) < num {
            signal.wait();
        }
    }
}

impl Pool {
    pub fn new() -> Self {
        let num_threads = sys_info::cpu_num().unwrap_or(10);
        let signal = Condvar::new();
        let mut pool = Self {
            senders: vec![],
            threads: vec![],
            signal: signal.clone(),
        };
        (0..num_threads)
            .for_each(|i| {
                let (sender, recvr) = channel();
                let signal = signal.clone();
                let t = spawn(|| {
                    for job in recvr.iter() {
                        job.execute(&signal)
                    }
                });
                pool.senders.push(sender);
                pool.threads.push(t);
            })
            .collect();
        pool
    }

    pub fn dispatch_mut<F, A, I>(&self, elems: &mut [A], func: F)
    where
        F: Fn(&mut A),
    {
        let job = Job {
            elems: elems.as_ptr_mut() as *mut u64,
            num: elems.len(),
            work_index: AtomicUsize::new(0),
            done_index: AtomicUsize::new(0),
            func: |(ptr, index)| {
                let items = std::mem::transmute::<*mut u64, &mut [A]>(ptr);

                func(&mut items[index])
            },
        };
        let job = Arc::new(job);
        for s in &self.senders {
            s.send(job.clone());
        }
        job.execute(&self.signal);
        job.wait(&self.signal, self.senders.len() + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pool() {
        let pool = Pool::new();
        let mut array = [0usize; 100];
        pool.dispach_mut(&array, |val| *val += 1);
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    }
}
