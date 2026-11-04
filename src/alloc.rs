use std::alloc::{GlobalAlloc, Layout, System};

/// macOS malloc keeps freed multi-megabyte blocks dirty and charged to the app,
/// so a closed capture would keep its memory. Pixel-sized blocks go straight to
/// mmap, and freeing them returns the pages at once.
pub struct PixelAwareAlloc;

const BIG: usize = 1 << 20;
const PAGE: usize = 16 * 1024;

fn is_big(layout: Layout) -> bool {
    layout.size() >= BIG && layout.align() <= PAGE
}

unsafe impl GlobalAlloc for PixelAwareAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if is_big(layout) {
            map(layout.size())
        } else {
            unsafe { System.alloc(layout) }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // Fresh anonymous pages are already zero.
        if is_big(layout) {
            map(layout.size())
        } else {
            unsafe { System.alloc_zeroed(layout) }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if is_big(layout) {
            unsafe { libc::munmap(ptr.cast(), layout.size()) };
        } else {
            unsafe { System.dealloc(ptr, layout) };
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        if !is_big(layout) && !is_big(new_layout) {
            return unsafe { System.realloc(ptr, layout, new_size) };
        }
        let new = unsafe { self.alloc(new_layout) };
        if !new.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
        }
        new
    }
}

fn map(size: usize) -> *mut u8 {
    let ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        )
    };
    if ptr == libc::MAP_FAILED {
        std::ptr::null_mut()
    } else {
        ptr.cast()
    }
}
