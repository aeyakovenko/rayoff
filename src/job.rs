use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::yield_now;

pub struct Job {
    func: Progress,
    ctx: *mut c_void,
    elems: *mut c_void,
    num: u64,
    work_index: AtomicUsize,
    done_index: AtomicUsize,
}

/// Safe because ctx/elems are only touched until job is complete.
/// Job::wait must be called to complete the job
unsafe impl Send for Job {}

/// Safe because data access either atomic or read only
/// elems atomic access is guarded by the work_index
/// Job::wait must be called to complete the job
unsafe impl Sync for Job {}

type Progress = extern "C" fn(*mut c_void, *mut c_void, u64);

impl Job {
    /// This function is unsafe because the Job object must
    /// be complete.  Users must call Job::wait befor returning.
    pub unsafe fn new<F, A>(elems: &mut [A], func: F) -> Self
    where
        F: Fn(&mut A) + Send + Sync,
    {
        let mut closure = |ptr: *mut c_void, index: u64| {
            let elem: &mut A = std::mem::transmute((ptr as *mut A).add(index as usize));
            (func)(elem)
        };
        let (ctx, func) = Job::unpack_closure(&mut closure);
        Self {
            elems: elems.as_mut_ptr() as *mut c_void,
            num: elems.len() as u64,
            done_index: AtomicUsize::new(0),
            work_index: AtomicUsize::new(0),
            func,
            ctx,
        }
    }

    /// This function unpacks the closure into a context and a trampoline
    /// Source: https://s3.amazonaws.com/temp.michaelfbryan.com/callbacks/index.html
    unsafe fn unpack_closure<F>(closure: &mut F) -> (*mut c_void, Progress)
    where
        F: FnMut(*mut c_void, u64),
    {
        extern "C" fn trampoline<F>(data: *mut c_void, elems: *mut c_void, n: u64)
        where
            F: FnMut(*mut c_void, u64),
        {
            let closure: &mut F = unsafe { &mut *(data as *mut F) };
            (*closure)(elems, n);
        }

        (closure as *mut F as *mut c_void, trampoline::<F>)
    }
    pub fn execute(&self) {
        loop {
            let index = self.work_index.fetch_add(1, Ordering::Relaxed);
            if index as u64 >= self.num {
                break;
            }
            (self.func)(self.ctx, self.elems, index as u64);
            self.done_index.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn wait(&self) {
        loop {
            let guard = self.done_index.load(Ordering::Relaxed);
            if guard >= self.num as usize {
                break;
            }
            yield_now();
        }
    }
}
