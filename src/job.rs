use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{yield_now};

pub struct Job {
    func: *mut (),
    elems: *mut (),
    num: usize,
    work_index: AtomicUsize,
    done_index: AtomicUsize,
}

//Safe because Job only lives for the duration of the dispatch call
//and any thread lifetimes are within that call
unsafe impl Send for Job {}
//Safe because data is either atomic or read only
unsafe impl Sync for Job {}

impl Job {
    pub fn execute(&self) {
        loop {
            let index = self.work_index.fetch_add(1, Ordering::Relaxed);
            if index >= self.num {
                self.done_index.fetch_add(1, Ordering::Relaxed);
                break;
            }
            let func: &dyn Fn(*mut (), usize) = unsafe {std::mem::transmute(self.func) };
            (func)(self.elems, index);
        }
    }
    pub fn wait(&self, num: usize) {
        loop {
            let guard = self.done_index.load(Ordering::Relaxed);
            if guard >= num {
                break;
            }
            yield_now();
        }
    }
}
