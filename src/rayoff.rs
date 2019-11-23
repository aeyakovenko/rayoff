use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::atomic::{Ordering, AtomicUsize};
use sys_info;

struct Job {
    func: Fn(elems: *mut u64, index: usize)
    elems: *mut u64;
    num: usize;    
    work_index: AtomicUsize;
    done_index: AtomicUsize; 
}

struct Pool {
    senders: Vec<Sender<Arc<Job>>>
    threads: Vec< JoinHandle<()>>,
    signal: CondVar;
}

impl Job {
    fn execute(&self, signal: &CondVar) {
        loop {
            let index = job.work_index.fetch_and_add(Ordering::Relaxed, 1);
            if index >= job.num {
                job.done_index.add(Ordering::Relaxed, 1);
                signal.notify_one();
                break;
            }
            job.func(job.elems, index);
        } 
    }
    fn wait(&self, signal: &CondVar, num: usize) {
        while job.done_index.fetch(Ordering::Relaxed) < num {
            signal.wait();
        }
    }
}

impl Pool {
    pub fn new() -> {
        let num_threads = sys_info::cpu_num().unwrap_or(10);
        let signal:  CondVar::new();
        let mut pool = Self {
            senders: vec![],
            threads: vec![],
            signal: signal.clone(),
        }
        (0 .. num_threads).for_each(|i|  {
            let (sender, recevier) = channel();
            let signal = signal.clone();
            let t = spawn(|| {
                for job in receiver.iter() {
                    job.execute(&signal)
                }
            };
            pool.senders.push(sender);
            pool.threads.push(t);
        }).collect();
        pool
    }

    pub fn dispatch_mut<F, A, I>(&self, elems: &mut [A], func: F) 
        where F: Fn(item: &mut A)
    {
        let job = Job {
            #![allow(clippy::cast_ptr_alignment)]
            elems: elems as *mut u64,
            num: elems.len(), 
            work_index: AtomicUsize::new(0),
            done_index: AtomicUsize::new(0),
            func: |(ptr, index)| {
                #![allow(clippy::cast_ptr_alignment)]
                let elems = ptr as *mut A;
                let items = elems as &mut [A];
                func(&mut items[index])
            }
        }
        for s in &self.senders {
            s.send(job.clone());
        }
        job.execute(&self.signal);
        job.wait(&self.signal, self.senders.len() + 1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pool() {
        let pool = Pool::new();
        let mut array = [0usize ;100];
        pool.dispach_mut(&array, |val| *val += 1);
        let expected = [1usize; 100];
        assert_eq!(array, expected);
    }
}
