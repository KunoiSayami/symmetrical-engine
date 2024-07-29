use sysinfo::System;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub unsafe fn kill_process_by_name(process: &str) -> anyhow::Result<()> {
    let s = System::new_all();

    let pids = s
        .processes_by_exact_name(process)
        .map(|p| p.pid().as_u32())
        .collect::<Vec<_>>();

    windows::kill(pids)?;

    Ok(())
}

#[cfg(unix)]
pub unsafe fn kill_process_by_name(process: &str) -> anyhow::Result<()> {
    let s = System::new_all();

    for pid in s.processes_by_exact_name(process) {
        if !pid.kill() {
            log::error!("Fail to kill {}", pid.pid());
        }
    }

    Ok(())
}
