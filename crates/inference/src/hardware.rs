#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn mach_port_deallocate(
        task: libc::mach_port_t,
        name: libc::mach_port_t,
    ) -> libc::kern_return_t;
}
/// Available memory means free plus conservatively reclaimable file-backed pages.
/// Compressed pages occupy their own physical pool; subtracting them from inactive
/// file-backed pages double-counts pressure and can report zero indefinitely.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
pub fn available(fallback: u64) -> u64 {
    unsafe {
        let host = libc::mach_host_self();
        let mut stats = std::mem::zeroed::<libc::vm_statistics64_data_t>();
        let mut count = libc::HOST_VM_INFO64_COUNT;
        let page_size = libc::vm_page_size;
        let ok = libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            (&mut stats as *mut libc::vm_statistics64_data_t).cast(),
            &mut count,
        ) == libc::KERN_SUCCESS
            && page_size > 0;
        let _ = mach_port_deallocate(libc::mach_task_self(), host);
        if !ok {
            return fallback;
        }
        (stats.free_count as u64)
            .saturating_add(stats.inactive_count.min(stats.external_page_count) as u64)
            .saturating_mul(page_size as u64)
    }
}
#[cfg(not(target_os = "macos"))]
pub fn available(fallback: u64) -> u64 {
    fallback
}
