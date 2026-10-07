//! Read-only file mappings that survive the file shrinking underneath them (spec §4.5).
//!
//! Reading a mapped page that lies wholly past the end of a truncated file raises SIGBUS. Every `SafeMap` registers
//! its address range in a fixed, lock-free table, and a process-wide SIGBUS handler checks the faulting address
//! against it: inside a registered mapping, the handler maps private zero pages over the rest of that mapping and
//! returns, so the read sees zeros instead of killing the process. Live follow then notices the shrink and reloads
//! the file. A fault anywhere else goes back to the handler that was installed before ours.

use memmap2::Mmap;
use std::fs::File;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Mappings protected at once. A mapping made while every slot is taken still works, unprotected.
pub const MAX_GUARDED: usize = 1024;

struct Slot {
    used: AtomicBool,
    start: AtomicUsize,
    len: AtomicUsize,
}

static SLOTS: [Slot; MAX_GUARDED] =
    [const { Slot { used: AtomicBool::new(false), start: AtomicUsize::new(0), len: AtomicUsize::new(0) } }; MAX_GUARDED];
static PAGE: AtomicUsize = AtomicUsize::new(0);
/// The SIGBUS action the process had before `install` (faults that are not ours go back to it).
static PREVIOUS: OnceLock<libc::sigaction> = OnceLock::new();
static INSTALLED: OnceLock<()> = OnceLock::new();

fn install() {
    INSTALLED.get_or_init(|| {
        // SAFETY: plain libc calls with valid arguments; the handler only reads atomics and calls mmap/sigaction.
        unsafe {
            PAGE.store((libc::sysconf(libc::_SC_PAGESIZE) as usize).max(4096), Ordering::Relaxed);
            let mut previous: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGBUS, std::ptr::null(), &mut previous);
            let _ = PREVIOUS.set(previous);
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_sigbus as *const () as libc::sighandler_t;
            action.sa_flags = libc::SA_SIGINFO;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGBUS, &action, std::ptr::null_mut());
        }
    });
}

/// End address of the registered mapping containing `addr`.
fn guarded_end(addr: usize) -> Option<usize> {
    SLOTS.iter().find_map(|s| {
        // `len` is published after `start` and cleared before it, so a non-zero `len` means `start` is valid.
        let len = s.len.load(Ordering::Acquire);
        let start = s.start.load(Ordering::Acquire);
        (len > 0 && addr >= start && addr - start < len).then_some(start + len)
    })
}

extern "C" fn on_sigbus(_signal: libc::c_int, info: *mut libc::siginfo_t, _context: *mut libc::c_void) {
    // SAFETY: the kernel passes a valid `siginfo_t` to an SA_SIGINFO handler.
    let addr = unsafe { (*info).si_addr } as usize;
    if let Some(end) = guarded_end(addr) {
        let page = PAGE.load(Ordering::Relaxed);
        let from = addr & !(page - 1);
        // SAFETY: `from..end` lies inside a live mapping registered by a `SafeMap`, which keeps the range mapped until
        // it unregisters; replacing those pages with private zero pages leaves every address in it readable.
        let mapped = unsafe {
            libc::mmap(from as *mut libc::c_void, end - from, libc::PROT_READ, libc::MAP_FIXED | libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0)
        };
        if mapped != libc::MAP_FAILED {
            return;
        }
    }
    // Not ours (or the remap failed): reinstate the previous action; returning re-runs the faulting access under it.
    if let Some(previous) = PREVIOUS.get() {
        // SAFETY: `previous` is the action the process had before `install`.
        unsafe { libc::sigaction(libc::SIGBUS, previous, std::ptr::null_mut()) };
    }
}

fn register(start: usize, len: usize) -> Option<usize> {
    let slot = SLOTS.iter().position(|s| s.used.compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed).is_ok())?;
    SLOTS[slot].start.store(start, Ordering::Release);
    SLOTS[slot].len.store(len, Ordering::Release);
    Some(slot)
}

fn unregister(slot: usize) {
    SLOTS[slot].len.store(0, Ordering::Release);
    SLOTS[slot].start.store(0, Ordering::Release);
    SLOTS[slot].used.store(false, Ordering::Release);
}

/// Mappings currently protected (tests).
pub fn guarded_count() -> usize {
    SLOTS.iter().filter(|s| s.len.load(Ordering::Acquire) > 0).count()
}

/// A read-only mapping of a whole file, protected against the file shrinking (see module docs).
pub struct SafeMap {
    map: Mmap,
    slot: Option<usize>,
}

impl SafeMap {
    /// Maps all of `file` read-only. Fails for an empty file (there is nothing to map).
    pub fn map(file: &File) -> std::io::Result<SafeMap> {
        install();
        // SAFETY: a read-only mapping that rogg never writes through. Another process writing the file only changes
        // what is displayed; a truncation is caught by the SIGBUS guard above.
        let map = unsafe { Mmap::map(file) }?;
        let slot = register(map.as_ptr() as usize, map.len());
        Ok(SafeMap { map, slot })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    /// False when every guard slot was taken at mapping time (the mapping then works unprotected).
    pub fn is_guarded(&self) -> bool {
        self.slot.is_some()
    }
}

impl Drop for SafeMap {
    fn drop(&mut self) {
        // Unregister before `map` unmaps the range.
        if let Some(slot) = self.slot.take() {
            unregister(slot);
        }
    }
}
