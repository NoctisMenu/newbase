use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn worker_dll_path() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("memory crate should be under workspace");
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    target_dir.join("release").join("worker_dll.dll")
}

fn ensure_worker_dll() -> PathBuf {
    let dll = worker_dll_path();
    if dll.exists() {
        return dll;
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .expect("memory crate should be under workspace");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .args(["build", "-p", "worker_dll", "--release"])
        .current_dir(workspace)
        .status()
        .expect("failed to run cargo build for worker_dll");
    assert!(status.success(), "worker_dll build failed");
    assert!(
        dll.exists(),
        "worker_dll was not produced at {}",
        dll.display()
    );
    dll
}

fn spawn_dummy() -> (ChildGuard, u32, u32, usize, usize) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_worker_dummy"))
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn worker_dummy");

    let stdout = child.stdout.take().expect("worker_dummy stdout missing");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("failed to read worker_dummy ready line");

    let parts: Vec<_> = line.split_whitespace().collect();
    assert_eq!(
        parts.first().copied(),
        Some("READY"),
        "bad dummy line: {line:?}"
    );
    let pid = parts[1].parse::<u32>().expect("bad pid");
    let thread_id = parts[2].parse::<u32>().expect("bad thread id");
    let read_addr =
        usize::from_str_radix(parts[3].trim_start_matches("0x"), 16).expect("bad read address");
    let write_addr =
        usize::from_str_radix(parts[4].trim_start_matches("0x"), 16).expect("bad write address");

    (ChildGuard(child), pid, thread_id, read_addr, write_addr)
}

#[test]
fn worker_reads_and_writes_dummy_process() {
    let dll = ensure_worker_dll();
    let (_child, pid, thread_id, read_addr, write_addr) = spawn_dummy();

    let result = (|| {
        let driver = memory::worker::WorkerDriver::attach_to_thread(pid, thread_id, &dll)?;

        let bytes = driver.read_bytes(read_addr, 16)?;
        assert_eq!(&bytes, b"wsw-dummy-read!!");

        let new_value = 0x8877_6655_4433_2211u64;
        driver.write(write_addr, new_value)?;
        let observed: u64 = driver.read(write_addr)?;
        assert_eq!(observed, new_value);

        driver.shutdown()
    })();

    result.expect("worker dummy integration failed");
}
