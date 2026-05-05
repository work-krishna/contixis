use parking_lot::Mutex;
use std::sync::Arc;

/// Pre-allocated fixed-size buffer pool.
///
/// Avoids per-frame allocations on the hot input/video path.
/// Buffers are returned to the pool when the `PoolBuf` guard is dropped.
pub struct BufferPool {
    free: Arc<Mutex<Vec<Vec<u8>>>>,
    buf_size: usize,
}

impl BufferPool {
    pub fn new(buf_size: usize, initial_count: usize) -> Self {
        let free: Vec<Vec<u8>> = (0..initial_count)
            .map(|_| vec![0u8; buf_size])
            .collect();
        Self {
            free: Arc::new(Mutex::new(free)),
            buf_size,
        }
    }

    /// Acquire a buffer.  If the pool is empty a new buffer is allocated.
    pub fn acquire(&self) -> PoolBuf {
        let buf = self.free.lock().pop()
            .unwrap_or_else(|| vec![0u8; self.buf_size]);
        PoolBuf { buf, pool: self.free.clone() }
    }
}

/// RAII guard that returns the buffer to its pool when dropped.
pub struct PoolBuf {
    buf: Vec<u8>,
    pool: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl PoolBuf {
    pub fn as_slice(&self) -> &[u8] { &self.buf }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.buf }
    pub fn len(&self) -> usize { self.buf.len() }
}

impl std::ops::Deref for PoolBuf {
    type Target = [u8];
    fn deref(&self) -> &[u8] { &self.buf }
}

impl std::ops::DerefMut for PoolBuf {
    fn deref_mut(&mut self) -> &mut [u8] { &mut self.buf }
}

impl Drop for PoolBuf {
    fn drop(&mut self) {
        let buf = std::mem::take(&mut self.buf);
        self.pool.lock().push(buf);
    }
}
