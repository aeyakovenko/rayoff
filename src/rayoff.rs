extern crate sys_info;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{spawn, yield_now, JoinHandle};

struct Job {
    func: Box<dyn Fn(*mut u64, usize, usize)>,
    elems: *mut u64,
    num: usize,
    work_index: AtomicUsize,
    done_index: AtomicUsize,
}

struct JobPtr {
    pub job: *const Job,
}

//Safe because Job only lives for the duration of the dispatch call
//and any thread lifetimes are within that call
unsafe impl Send for JobPtr {}
//Safe because data is either atomic or read only
unsafe impl Sync for JobPtr {}

pub struct Pool {
    senders: Vec<Sender<Arc<JobPtr>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Job {
    fn execute(&self) {
        loop {
            let index = self.work_index.fetch_add(1, Ordering::Relaxed);
            if index >= self.num {
                self.done_index.fetch_add(1, Ordering::Relaxed);
                break;
            }
            (self.func)(self.elems, self.num, index);
        }
    }
    fn wait(&self, num: usize) {
        loop {
            let guard = self.done_index.load(Ordering::Relaxed);
            if guard >= num {
                break;
            }
            yield_now();
        }
    }
}

impl Default for Pool {
    fn default() -> Self {
        let num_threads = sys_info::cpu_num().unwrap_or(16) - 1;
        let mut pool = Self {
            senders: vec![],
            threads: vec![],
        };
        (0..num_threads).for_each(|_| {
            let (sender, recvr): (Sender<Arc<JobPtr>>, Receiver<Arc<JobPtr>>) = channel();
            let t = spawn(move || {
                for jobptr in recvr.iter() {
                    let job: &Job = unsafe {std::mem::transmute(jobptr.job) };
                    job.execute()
                }
            });
            pool.senders.push(sender);
            pool.threads.push(t);
        });
        pool
    }
}

impl Pool {
    pub fn dispatch_mut<F, A>(&self, elems: &mut [A], func: F)
    where
        F: Fn(&mut A) + Send + Sync,
    {
        let box_func = Box::into_raw(Box::new(func));
        let func_ptr = box_func as *mut _;
        let func: &'static dyn Fn(&'static mut A) = unsafe { std::mem::transmute(func_ptr) };
        let closure = |ptr: *mut u64, num: usize, index: usize| {
                let ptr: *mut A = unsafe {std::mem::transmute(ptr) };
                let slice = unsafe { std::slice::from_raw_parts_mut(ptr, num) };
                func(&mut slice[index])
        };
        let job = Job {
            elems: elems.as_mut_ptr() as *mut u64,
            num: elems.len(),
            done_index: AtomicUsize::new(0),
            work_index: AtomicUsize::new(0),
            func: Box::new(closure),
        };
        let jobptr = JobPtr { job: &job as *const Job };
        let jobptr = Arc::new(jobptr);
        for s in &self.senders {
            s.send(jobptr.clone()).expect("send should never fail");
        }
        job.execute();
        job.wait(self.senders.len() + 1);
    }
    pub fn map<F, A, B>(&self, inputs: &[A], func: F) -> Vec<B>
    where
        B: Default + Clone,
        F: (Fn(&A) -> B) + Send + Sync,
    {
        let mut outs = Vec::new();
        outs.resize(inputs.len(), B::default());
        let mut elems: Vec<(&A, &mut B)> = inputs.iter().zip(outs.iter_mut()).collect();
        self.dispatch_mut(&mut elems, move |item: &mut (&A, &mut B)| {
            *item.1 = func(item.0);
        });
        outs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pool() {
        let pool = Pool::default();
        let mut array = [0usize; 100];
        pool.dispatch_mut(&mut array, Box::new(|val: &mut usize| *val += 1));
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(array[i], expected[i]);
        }
    }
    #[test]
    fn test_map() {
        let pool = Pool::default();
        let array = [0usize; 100];
        let output = pool.map(&array, |val: &usize| val + 1);
        let expected = [1usize; 100];
        for i in 0..100 {
            assert_eq!(expected[i], output[i]);
        }
    }
}
