use std::ptr::null_mut;

use anyhow::anyhow;
use tap::TapFallible;
use winapi::shared::minwindef::DWORD;
use winapi::shared::ntdef::HANDLE;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE};

// https://stackoverflow.com/a/55231715
pub(crate) struct Process(HANDLE);
impl Process {
    unsafe fn open(pid: DWORD) -> anyhow::Result<Process> {
        // https://msdn.microsoft.com/en-us/library/windows/desktop/ms684320%28v=vs.85%29.aspx
        let pc = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_TERMINATE, 0, pid);
        if pc == null_mut() {
            let last_error = GetLastError();

            return Err(anyhow!("OpenProcess error: {last_error}"));
        }
        Ok(Process(pc))
    }

    unsafe fn kill(&self) -> anyhow::Result<()> {
        if TerminateProcess(self.0, 1) != 0 {
            let e = GetLastError();
            return Err(anyhow!("TerminateProcess error: {e}"));
        }
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { winapi::um::handleapi::CloseHandle(self.0) };
    }
}

pub(crate) unsafe fn kill(pids: Vec<u32>) -> anyhow::Result<()> {
    for pid in pids {
        unsafe {
            Process::open(pid)
                .tap_err(|e| log::error!("Pid: {pid} {e}"))?
                .kill()
                .tap_err(|e| log::error!("Pid: {pid} {e}"))?
        };
    }
    Ok(())
}
