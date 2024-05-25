use std::alloc::Layout;

pub trait GC {
    fn next(&self) -> Option<*mut dyn GC>;
    fn set_next(&mut self, next: Option<*mut dyn GC>);
    fn layout(&self) -> Layout;
}

pub struct GarbageCollector {
    head_object: Option<*mut dyn GC>,
    debug_stress_gc: bool,
    debug_log_gc: bool,
}

impl GarbageCollector {
    pub fn new(debug_stress_gc: bool, debug_log_gc: bool) -> GarbageCollector {
        GarbageCollector {
            head_object: None,
            debug_stress_gc,
            debug_log_gc,
        }
    }

    pub fn heap_alloc<T>(&mut self, mut obj: T) -> *mut T
    where
        T: GC + std::fmt::Display + 'static,
    {
        if self.debug_stress_gc {
            self.collect_garbage(self.debug_log_gc);
        }
        if self.debug_log_gc {
            println!("allocating {}...", obj);
        }
        obj.set_next(self.head_object);
        let layout = Layout::new::<T>();
        unsafe {
            let ptr = std::alloc::alloc(layout) as *mut T;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            *ptr = obj;
            self.head_object = Some(ptr);
            ptr
        }
    }

    pub fn free_objects(&mut self) {
        let mut next = self.head_object;
        while let Some(current_head) = next {
            unsafe {
                next = (*current_head).next();
                std::ptr::drop_in_place(current_head);
                std::alloc::dealloc(current_head as *mut u8, (*current_head).layout());
            }
        }
    }

    fn collect_garbage(&mut self, debug_log_gc: bool) {
        if debug_log_gc {
            println!("-- gc begin");
        }

        if debug_log_gc {
            println!("-- gc end");
        }
    }
}

impl Drop for GarbageCollector {
    fn drop(&mut self) {
        self.free_objects();
    }
}
