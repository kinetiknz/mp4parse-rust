use std::ops::{Index, IndexMut, Deref, DerefMut};
use std::os::raw::c_void;

extern "C" {
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

#[derive(Debug)]
struct Unique<T> {
    ptr: *const T,
    _marker: std::marker::PhantomData<T>
}

impl<T> Unique<T> {
    fn new(ptr: *mut T) -> Unique<T> {
        Unique {
            ptr: ptr,
            _marker: std::marker::PhantomData
        }
    }

    fn empty() -> Unique<T> {
        Unique::new(std::ptr::null_mut())
    }

    fn as_ptr(&self) -> *mut T {
        self.ptr as *mut T
    }
}

#[derive(Debug)]
pub struct Vec<T> {
    ptr: Unique<T>,
    cap: usize,
    len: usize
}

impl<T> Vec<T> {
    pub fn new() -> Vec<T> {
        Vec {
            ptr: Unique::empty(),
            cap: 0,
            len: 0
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, value: T) -> Result<(), ()> {
        if self.len == self.cap {
            self.resize()?;
        }
        unsafe {
            let end = self.ptr.as_ptr().offset(self.len as isize);
            std::ptr::write(end, value);
            self.len += 1;
        }
        Ok(())
    }

    pub fn as_slice(&self) -> &[T] {
        self
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    pub fn append(&mut self, other: &mut Vec<T>) -> Result<(), ()> {
        let mut mine = Vec::new();
        std::mem::swap(&mut mine, other);
        self.reserve(mine.len())?;
        for elem in mine.into_iter() {
            self.push(elem)?;
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        while let Some(_) = self.pop() {}
        debug_assert_eq!(self.len, 0);
    }

    pub fn reserve(&mut self, additional: usize) -> Result<(), ()> {
        self.cap = std::cmp::max(1, self.cap + additional) - 1;
        self.resize()?;
        Ok(())
    }

    fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            unsafe {
                self.len -= 1;
                Some(std::ptr::read(self.ptr.as_ptr().offset(self.len as isize)))
            }
        }
    }

    /*
     * Above is copied from Vec API, below are extensions.
     */

    // Like with_capacity, but len set and data initialized.
    pub fn with_len(size: usize) -> Result<Vec<T>, ()> {
        let mut vec = Vec {
            ptr: Unique::empty(),
            cap: std::cmp::max(1, size) - 1,
            len: 0,
        };
        vec.resize()?;
        vec.len = size;
        Ok(vec)
    }

    fn resize(&mut self) -> Result<(), ()> {
        let elem_size = std::mem::size_of::<T>();
        let new_cap = self.cap + 1; // XXX: double or something
        unsafe {
            let ptr = realloc(self.ptr.as_ptr() as *mut _, new_cap * elem_size) as *mut T;
            if ptr.is_null() {
                return Err(());
            }
            std::ptr::write_bytes(ptr.offset(self.len as isize), 0, new_cap - self.cap);
            self.ptr = Unique::new(ptr);
            self.cap = new_cap;
        }
        Ok(())
    }
}

impl<T> Drop for Vec<T> {
    fn drop(&mut self) {
        while let Some(_) = self.pop() {}
        unsafe {
            free(self.ptr.as_ptr() as *mut _);
        }
    }
}

impl<T> Vec<T> where T: Copy {
    pub fn from_slice(other: &[T]) -> Result<Vec<T>, ()> {
        let mut vec = Vec::with_len(other.len())?;
        vec.copy_from_slice(other);
        Ok(vec)
    }

    pub fn extend_from_slice(&mut self, other: &[T]) -> Result<(), ()> {
        self.reserve(other.len())?;
        let len = self.len;
        self.len += other.len();
        self.as_mut_slice()[len..].copy_from_slice(other);
        Ok(())
    }
}

impl<T> Index<usize> for Vec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        &(**self)[index]
    }
}

impl<T> IndexMut<usize> for Vec<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut (**self)[index]
    }
}

impl<T> Deref for Vec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        if self.is_empty() {
            &[]
        } else {
            unsafe {
                std::slice::from_raw_parts(self.ptr.as_ptr(), self.len)
            }
        }
    }
}

impl<T> DerefMut for Vec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        if self.is_empty() {
            &mut []
        } else {
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len)
            }
        }
    }
}

pub struct IntoIter<T> {
    buf: Unique<T>,
    ptr: *mut T,
    end: *mut T,
}

impl<T> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        let buf = Unique::new(self.ptr.as_ptr());
        let ptr = buf.as_ptr();
        let end = unsafe { buf.as_ptr().offset(self.len() as isize) };
        std::mem::forget(self);
        IntoIter {
            buf: buf,
            ptr: ptr,
            end: end,
        }
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        while let Some(_) = self.next() {}
        unsafe {
            free(self.buf.as_ptr() as *mut _);
        }
    }
}

impl<'a, T> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> std::slice::Iter<'a, T> {
        self.iter()
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.ptr == self.end {
            None
        } else {
            unsafe {
                let ptr = self.ptr;
                self.ptr = self.ptr.offset(1);
                Some(std::ptr::read(ptr))
            }
        }
    }
}

fn oom() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, "OOM")
}

impl std::io::Write for Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.extend_from_slice(buf).is_err() {
            return Err(oom());
        }
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        if self.extend_from_slice(buf).is_err() {
            return Err(oom());
        }
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<T> std::default::Default for Vec<T> {
    fn default() -> Vec<T> {
        Vec::new()
    }
}

impl<T> std::clone::Clone for Vec<T> where T: Clone {
    fn clone(&self) -> Vec<T> {
        // XXX: We can't return a result here, so just panic.
        let mut vec = Vec::new();
        for elem in self {
            vec.push(elem.clone()).expect("OOM");
        }
        vec
    }
}

impl<T> std::cmp::PartialEq for Vec<T> where T: PartialEq {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T> std::cmp::PartialEq<std::vec::Vec<T>> for Vec<T> where T: PartialEq {
    fn eq(&self, other: &std::vec::Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<'a, T> std::cmp::PartialEq<&'a [T]> for Vec<T> where T: PartialEq {
    fn eq(&self, other: &&'a [T]) -> bool {
        self.as_slice() == *other
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
