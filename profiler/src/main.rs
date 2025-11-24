// Rust High-frequency profiler (cumulative totals output)
// Filename: src/main.rs
// This program samples cumulative counters (not deltas) and writes JSON-lines per sample.
// It collects:
// - /proc/<pid>/io: read_bytes, write_bytes
// - /proc/<pid>/status: VmRSS, voluntary & nonvoluntary context switches
// - /proc/<pid>/stat: minor & major page faults
// - perf_event_open hardware counters: cycles, instructions, cache-misses
// - /proc/stat: per-CPU jiffies and system context switches (ctxt)

/*
Build & run:
  - Requires Rust toolchain and libc crate (std available)
  - On Linux
  - For perf counters you likely need root privileges

  cargo build --release
  sudo ./target/release/highfreq_profiler_cumulative --pid 12345 --interval-ms 1000 --out samples.jsonl

Flags:
  --pid <pid>           PID to monitor (0 = all system-wide for perf)
  --interval-ms <ms>    Sampling interval (ms)
  --duration-s <s>      Duration to run (0 = infinite)
  --out <path>          Output file (JSONL). Use '-' for stdout (default '-').
*/

use clap::Parser;
use sonic_rs::{Deserialize, Serialize}; 
use std::collections::HashMap;
use std::fs::{read_to_string, File};
use std::io::{BufRead, BufReader, Write};
use std::mem::size_of;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH, Duration, Instant};

// libc for perf
use libc::{c_int, c_ulong, pid_t};

const PERF_EVENT_IOC_ENABLE: libc::c_ulong  = 0x2400;
const PERF_EVENT_IOC_DISABLE: libc::c_ulong = 0x2401;

#[derive(Parser, Debug)]
#[command(author, version, about = "High-frequency profiler - cumulative totals")]
struct Args {
    /// PID of target process to monitor (0 = system-wide)
    #[arg(long)]
    pid: i32,

    /// Sampling interval in milliseconds
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,

    /// Duration to run in seconds (0 = infinite)
    #[arg(long, default_value_t = 0)]
    duration_s: u64,

    /// Output file (JSON lines). '-' for stdout
    #[arg(long, default_value_t = String::from("-"))]
    out: String,
}

#[derive(Serialize, Debug, Default, Clone)]
struct Sample {
    ts_ms: u128,
    pid: i32,

    // /proc/<pid>/io
    io_read_bytes_total: Option<u64>,
    io_write_bytes_total: Option<u64>,

    // memory
    rss_kb_total: Option<u64>,

    // context switches
    voluntary_ctx_switches_total: Option<u64>,
    nonvoluntary_ctx_switches_total: Option<u64>,

    // page faults
    minor_faults_total: Option<u64>,
    major_faults_total: Option<u64>,

    // perf counters (cumulative)
    cycles_total: Option<u64>,
    instructions_total: Option<u64>,
    cache_misses_total: Option<u64>,

    // per-cpu jiffies (cumulative)
    per_cpu_jiffies: HashMap<String, Vec<u64>>,

    // system ctxt total (cumulative)
    ctxt_total: Option<u64>,
}

// minimal perf_event_attr
#[repr(C)]
#[derive(Default)]
struct perf_event_attr {
    type_: u32,
    size: u32,
    config: u64,

    sample_period: u64,
    sample_type: u64,
    read_format: u64,

    flags: u64,

    wakeup_events: u32,
    __reserved: u32,

    bp_type: u32,
    bp_addr: u64,
    bp_len: u64,
}

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
const PERF_COUNT_HW_CACHE_MISSES: u64 = 3;

fn perf_event_open(attr: &mut perf_event_attr, pid: pid_t, cpu: c_int, group_fd: c_int, flags: c_ulong) -> RawFd {
    unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            attr as *mut perf_event_attr,
            pid as libc::pid_t,
            cpu,
            group_fd,
            flags,
        ) as RawFd
    }
}

fn open_cache_miss_counter(pid: i32) -> RawFd {
    let mut attr = perf_event_attr::default();

    attr.type_ = PERF_TYPE_HARDWARE;
    attr.size = std::mem::size_of::<perf_event_attr>() as u32;
    attr.config = PERF_COUNT_HW_CACHE_MISSES;

    // required
    attr.sample_period = 0;
    attr.sample_type = 0;
    attr.read_format = 0;

    // disable at start, we enable later
    attr.flags = 1; // disabled = 1

    unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &attr as *const perf_event_attr,
            pid,
            -1,  // any CPU
            -1,  // not part of group
            0
        ) as RawFd
    }
}

fn read_u64(fd: RawFd) -> Option<u64> {
    if fd < 0 { return None; }
    let mut buf: [u8; 8] = [0; 8];
    let ret = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, 8) };
    if ret == 8 {
        Some(u64::from_ne_bytes(buf))
    } else {
        None
    }
}

fn epoch_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

fn parse_proc_io(pid: i32) -> Option<(u64,u64)> {
    let p = format!("/proc/{}/io", pid);
    if !Path::new(&p).exists() { return None; }
    if let Ok(s) = read_to_string(p) {
        let mut r = None;
        let mut w = None;
        for line in s.lines() {
            if line.starts_with("read_bytes:") { if let Some(v) = line.split_whitespace().nth(1) { r = v.parse().ok(); } }
            if line.starts_with("write_bytes:") { if let Some(v) = line.split_whitespace().nth(1) { w = v.parse().ok(); } }
        }
        return Some((r.unwrap_or(0), w.unwrap_or(0)));
    }
    None
}

fn parse_proc_status(pid: i32) -> Option<(Option<u64>, Option<u64>, Option<u64>)> {
    // returns (VmRSS_kB, voluntary_ctxt_switches, nonvoluntary_ctxt_switches)
    let p = format!("/proc/{}/status", pid);
    if !Path::new(&p).exists() { return None; }
    let mut rss = None;
    let mut vol = None;
    let mut nonvol = None;
    if let Ok(f) = File::open(p) {
        for line in BufReader::new(f).lines().flatten() {
            if line.starts_with("VmRSS:") { if let Some(v) = line.split_whitespace().nth(1) { rss = v.parse().ok(); } }
            if line.starts_with("voluntary_ctxt_switches:") { if let Some(v) = line.split_whitespace().nth(1) { vol = v.parse().ok(); } }
            if line.starts_with("nonvoluntary_ctxt_switches:") { if let Some(v) = line.split_whitespace().nth(1) { nonvol = v.parse().ok(); } }
        }
    }
    Some((rss, vol, nonvol))
}

fn parse_proc_stat_pid(pid: i32) -> Option<(u64,u64)> {
    // minor faults (minflt) field 10, major faults (majflt) field 12
    // Format: pid (comm) state ... minflt ... majflt ...
    // We need to skip past the comm field which is in parentheses
    let p = format!("/proc/{}/stat", pid);
    if !Path::new(&p).exists() { return None; }
    if let Ok(s) = read_to_string(p) {
        // Find the last ')' to skip the comm field which can contain spaces
        if let Some(end_paren) = s.rfind(')') {
            let after_comm = &s[end_paren + 1..];
            let parts: Vec<&str> = after_comm.split_whitespace().collect();
            // After comm, fields are: state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt...
            // So minflt is at index 7, majflt is at index 9 (0-indexed after comm)
            if parts.len() > 9 {
                let minflt = parts[7].parse().unwrap_or(0);
                let majflt = parts[9].parse().unwrap_or(0);
                return Some((minflt, majflt));
            }
        }
    }
    None
}

fn parse_proc_stat_system() -> Option<(HashMap<String, Vec<u64>>, Option<u64>)> {
    // returns (map of cpu -> fields[], ctxt_total)
    if let Ok(s) = read_to_string("/proc/stat") {
        let mut map = HashMap::new();
        let mut ctxt = None;
        for line in s.lines() {
            if line.starts_with("cpu") {
                let cols: Vec<&str> = line.split_whitespace().collect();
                let key = cols[0].to_string();
                let mut vals = Vec::new();
                for c in cols.iter().skip(1) {
                    if let Ok(v) = c.parse::<u64>() { vals.push(v); }
                }
                map.insert(key, vals);
            } else if line.starts_with("ctxt ") {
                if let Some(v) = line.split_whitespace().nth(1) { ctxt = v.parse().ok(); }
            }
        }
        return Some((map, ctxt));
    }
    None
}

fn main() {
    let args = Args::parse();
    let pid = args.pid;
    let interval = Duration::from_millis(args.interval_ms);
    let duration_s = args.duration_s;

    // // perf counters
    // let fd_cycles = open_cache_miss_counter(pid, PERF_COUNT_HW_CPU_CYCLES, -1);
    // if fd_cycles < 0 { eprintln!("warning: failed to open cycles counter (fd={})", fd_cycles); }
    // let fd_inst = open_counter(pid, PERF_COUNT_HW_INSTRUCTIONS, fd_cycles);
    // if fd_inst < 0 { eprintln!("warning: failed to open instructions counter (fd={})", fd_inst); }
    let fd_cache = open_cache_miss_counter(pid);
    if fd_cache < 0 { eprintln!("warning: failed to open cache-misses counter (fd={})", fd_cache); }

    // // enable group leader
    // if fd_cycles >= 0 {
    //     unsafe { libc::ioctl(fd_cycles, PERF_EVENT_IOC_ENABLE, 0); }
    // }

    let mut writer: Box<dyn Write> = if args.out == "-" { Box::new(std::io::stdout()) } else { Box::new(File::create(&args.out).expect("create out file")) };

    let start = Instant::now();

    loop {
        let ts = epoch_ms();

        // process io
        let (io_r, io_w) = parse_proc_io(pid).unwrap_or((0,0));

        // status
        let (rss_kb, vol_cs, nonvol_cs) = parse_proc_status(pid).unwrap_or((None,None,None));

        // page faults
        let (minflt, majflt) = parse_proc_stat_pid(pid).unwrap_or((0,0));

        // perf
        // let cycles = read_u64(fd_cycles).unwrap_or(0);
        // let inst = read_u64(fd_inst).unwrap_or(0);
        let cache = read_u64(fd_cache).unwrap_or(0);

        // system
        let (cpu_map, ctxt_total) = parse_proc_stat_system().unwrap_or((HashMap::new(), None));

        let sample = Sample {
            ts_ms: ts,
            pid,
            io_read_bytes_total: Some(io_r),
            io_write_bytes_total: Some(io_w),
            rss_kb_total: rss_kb,
            voluntary_ctx_switches_total: vol_cs,
            nonvoluntary_ctx_switches_total: nonvol_cs,
            minor_faults_total: Some(minflt),
            major_faults_total: Some(majflt),
            // cycles_total: if fd_cycles >= 0 { Some(cycles) } else { None },
            // instructions_total: if fd_inst >= 0 { Some(inst) } else { None },
            cycles_total: None,
            instructions_total: None,
        
            cache_misses_total: if fd_cache >= 0 { Some(cache) } else { None },
            per_cpu_jiffies: cpu_map,
            ctxt_total: ctxt_total,
        };

        let jl = sonic_rs::to_string(&sample).expect("serialize");
        writeln!(writer, "{}", jl).expect("write out");
        writer.flush().ok();

        if duration_s > 0 && start.elapsed().as_secs() >= duration_s { break; }

        std::thread::sleep(interval);
    }

    // if fd_cycles >= 0 {
    //     unsafe { libc::ioctl(fd_cycles, PERF_EVENT_IOC_DISABLE, 0); }
    //     unsafe { libc::close(fd_cycles); }
    // }
    // if fd_inst >= 0 { unsafe { libc::close(fd_inst); } }
    if fd_cache >= 0 { unsafe { libc::close(fd_cache); } }

    eprintln!("done sampling");
}
