use std::{ffi::CStr, ptr};

/// This file contains the C runtime functions which are used by the C code in
/// sabfs. These functions are compiled by WASI SDK and linked into the final
/// binary. Normally these functions are provided by the compiler, but we are
/// targeting WASM with Shared Memory support which does not support relocations
/// at the moment. So we re-implement these functions here on top of whatever
/// the Rust compiler emits to get its own stuff working.
/// In other words, we use Rust's allocator to allocate memory for C code.

// list of everything that we allocated here for C
static mut ALLOCATED: Vec<*mut u8> = Vec::new();

#[no_mangle]
/// Frees the memory at the given address
pub unsafe extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    if !ALLOCATED.contains(&ptr) {
        panic!("C tried to free memory belonging to Rust!");
    }
    std::alloc::dealloc(ptr as *mut u8, std::alloc::Layout::new::<u8>());
}

#[no_mangle]
/// Allocates a block of memory of the given size
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    let ptr = std::alloc::alloc(std::alloc::Layout::from_size_align_unchecked(size, 1));
    ALLOCATED.push(ptr);
    ptr
}

#[no_mangle]
/// Allocates a block of memory of the given size and initializes it to zero
pub unsafe extern "C" fn calloc(n: usize, size: usize) -> *mut u8 {
    let ptr = std::alloc::alloc_zeroed(std::alloc::Layout::from_size_align_unchecked(n * size, 1));
    ALLOCATED.push(ptr);
    ptr
}

#[no_mangle]
/// Reallocates a block of memory to the given size
pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(size);
    }
    if !ALLOCATED.contains(&ptr) {
        panic!("C tried to realloc memory belonging to Rust!");
    }
    let ptr = std::alloc::realloc(ptr as *mut u8, std::alloc::Layout::new::<u8>(), size);
    ALLOCATED.push(ptr);
    ptr
}

#[no_mangle]
/// Returns the length of the initial portion of "str" which consists only of characters that are part of "accept".
pub unsafe extern "C" fn strspn(str: *const u8, accept: *const u8) -> usize {
    if str.is_null() || accept.is_null() || *str == 0 || *accept == 0 {
        return 0;
    }
    let str = CStr::from_ptr(str as *const i8);
    let accept = CStr::from_ptr(accept as *const i8);
    str.to_bytes()
        .iter()
        .take_while(|&&c| accept.to_bytes().contains(&c))
        .count()
}

#[no_mangle]
/// Returns the length of the initial portion of "str" which consists only of characters that are not part of "reject".
pub unsafe extern "C" fn strcspn(str: *const u8, reject: *const u8) -> usize {
    if str.is_null() || reject.is_null() || *str == 0 || *reject == 0 {
        return 0;
    }
    let str = CStr::from_ptr(str as *const i8);
    let reject = CStr::from_ptr(reject as *const i8);
    str.to_bytes()
        .iter()
        .take_while(|&&c| !reject.to_bytes().contains(&c))
        .count()
}

#[no_mangle]
/// Returns a pointer to the first occurrence of "c" in "str".
pub unsafe extern "C" fn strchr(str: *const u8, c: i32) -> *mut u8 {
    if str.is_null() || *str == 0 {
        return ptr::null_mut();
    }
    let str = CStr::from_ptr(str as *const i8);
    let ptr = str
        .to_bytes()
        .iter()
        .position(|&ch| ch == c as u8)
        .map(|i| str.as_ptr().offset(i as isize))
        .unwrap_or(ptr::null_mut());
    ptr as *mut u8
}

#[no_mangle]
/// Copies the first "n" characters of source to destination.
pub unsafe extern "C" fn strncpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let mut dest_ptr = dest;
    let mut src_ptr = src;
    let mut i = 0;
    while i < n && !src_ptr.is_null() && *src_ptr != 0 {
        *dest_ptr = *src_ptr;
        dest_ptr = dest_ptr.offset(1);
        src_ptr = src_ptr.offset(1);
        i += 1;
    }
    while i < n {
        *dest_ptr = 0;
        dest_ptr = dest_ptr.offset(1);
        i += 1;
    }
    dest
}

#[no_mangle]
/// C's assert function on top of Rust's panic
pub unsafe extern "C" fn __assert_fail(
    assertion: *const u8,
    file: *const u8,
    line: u32,
    function: *const u8,
) {
    let assertion = CStr::from_ptr(assertion as *const i8);
    let file = CStr::from_ptr(file as *const i8);
    let function = CStr::from_ptr(function as *const i8);
    panic!(
        "Assertion failed: {}, file {}, line {}, function {}",
        assertion.to_str().unwrap(),
        file.to_str().unwrap(),
        line,
        function.to_str().unwrap()
    );
}

#[no_mangle]
/// Returns the length of the given C-style null terminated string
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut len = 0;
    let mut ptr = s;
    while !ptr.is_null() && unsafe { *ptr != 0 } {
        len += 1;
        ptr = unsafe { ptr.offset(1) };
    }
    len
}

#[no_mangle]
/// Concatenates the given source strings into the destination
pub unsafe extern "C" fn strcat(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let mut dest_ptr = dest;
    while !dest_ptr.is_null() && unsafe { *dest_ptr != 0 } {
        dest_ptr = unsafe { dest_ptr.offset(1) };
    }
    let mut src_ptr = src;
    while !src_ptr.is_null() && unsafe { *src_ptr != 0 } {
        unsafe {
            *dest_ptr = *src_ptr;
            dest_ptr = dest_ptr.offset(1);
            src_ptr = src_ptr.offset(1);
        }
    }
    unsafe {
        *dest_ptr = 0; // \0
    }
    dest
}

#[no_mangle]
/// Copies the given source string into the destination
pub unsafe extern "C" fn strcpy(dest: *mut u8, src: *const u8) -> *mut u8 {
    if dest.is_null() || src.is_null() {
        return dest;
    }
    let mut dest_ptr = dest;
    let mut src_ptr = src;
    while !src_ptr.is_null() && unsafe { *src_ptr != 0 } {
        unsafe {
            *dest_ptr = *src_ptr;
            dest_ptr = dest_ptr.offset(1);
            src_ptr = src_ptr.offset(1);
        }
    }
    unsafe {
        *dest_ptr = 0; // \0
    }
    dest
}

#[no_mangle]
/// Compares the given strings
pub unsafe extern "C" fn strcmp(s1: *const u8, s2: *const u8) -> i32 {
    if s1.is_null() && s2.is_null() {
        return 0;
    }
    let mut s1_ptr = s1;
    let mut s2_ptr = s2;
    while !s1_ptr.is_null() && !s2_ptr.is_null() && unsafe { *s1_ptr == *s2_ptr } {
        s1_ptr = unsafe { s1_ptr.offset(1) };
        s2_ptr = unsafe { s2_ptr.offset(1) };
    }
    if s1_ptr.is_null() && s2_ptr.is_null() {
        0
    } else if s1_ptr.is_null() {
        -1
    } else if s2_ptr.is_null() {
        1
    } else {
        unsafe { *s1_ptr as i32 - *s2_ptr as i32 }
    }
}
