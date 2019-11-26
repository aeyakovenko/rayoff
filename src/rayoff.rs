extern crate sys_info;

use std::sync::atomic::{AtomicUsize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};
use job::Job;

pub struct Pool {
    senders: Vec<Sender<Arc<Job>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Default for Pool {
    fn default() -> Self {
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
}

impl Pool {
    pub fn dispatch_mut<F, A>(&self, elems: &mut [A], func: F)
    where
        F: Fn(&mut A) + Send + Sync,
    {
        let func_ptr = Box::into_raw(Box::new(func)) as *mut _;
        let closure = |ptr: *mut (), index: usize| {
                let ptr: *mut A = unsafe {std::mem::transmute(ptr) };
                let elem: &mut A = unsafe { std::mem::transmute(ptr.add(index)) };
                let func: &dyn Fn(&mut A) = unsafe { std::mem::transmute(func_ptr) };
                (func)(elem)
        };
        let closure_ptr = Box::into_raw(Box::new(closure)) as *mut _;
        let job = Job {
            elems: elems.as_mut_ptr() as *mut _,
            num: elems.len(),
            done_index: AtomicUsize::new(0),
            work_index: AtomicUsize::new(0),
            func: closure_ptr,
        };
        let job = Arc::new(job);
        for s in &self.senders {
            s.send(job.clone()).expect("send should never fail");
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
