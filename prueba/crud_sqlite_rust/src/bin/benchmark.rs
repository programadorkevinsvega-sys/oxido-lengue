use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn request(address: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(address).expect("could not connect to API");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("could not write request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("could not read API response");
    let (headers, body) = response
        .split_once("\r\n\r\n")
        .expect("incomplete HTTP response");
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .expect("missing HTTP status")
        .parse::<u16>()
        .expect("invalid HTTP status");
    (status, body.to_string())
}

fn server_pid(address: &str) -> u32 {
    let (status, body) = request(address, "GET", "/__test/instance", "");
    assert_eq!(status, 200);
    let start = body.find("\"pid\":").expect("server response has no pid") + 6;
    body[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse::<u32>()
        .expect("invalid server pid")
}

fn cpu_ticks(pid: u32) -> u64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let Some(close_paren) = stat.rfind(')') else {
        return 0;
    };
    let fields = stat[close_paren + 2..].split_whitespace().collect::<Vec<_>>();
    if fields.len() < 13 {
        return 0;
    }
    fields[11].parse::<u64>().unwrap_or(0) + fields[12].parse::<u64>().unwrap_or(0)
}

fn rss_kib(pid: u32) -> u64 {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
}

fn measure_batch(address: &str, prefix: &str, count: u32) -> (usize, Vec<u128>, Duration) {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let mut tasks: Vec<JoinHandle<(u16, u128)>> = Vec::new();
    for index in 0..count {
        let thread_gate = Arc::clone(&gate);
        let thread_address = address.to_string();
        let name = format!("{prefix}-{index}");
        tasks.push(thread::spawn(move || {
            let (lock, condition) = &*thread_gate;
            let ready = lock.lock().expect("benchmark gate poisoned");
            let started = condition
                .wait_while(ready, |is_started| !*is_started)
                .expect("benchmark gate poisoned");
            drop(started);
            let body = format!("{{\"nombre\":\"{name}\",\"precio\":2.5}}");
            let started = Instant::now();
            let (status, _) = request(&thread_address, "POST", "/productos", &body);
            (status, started.elapsed().as_nanos())
        }));
    }
    let batch_started = Instant::now();
    let (lock, condition) = &*gate;
    let mut started = lock.lock().expect("benchmark gate poisoned");
    *started = true;
    condition.notify_all();
    drop(started);

    let mut successes = 0;
    let mut latencies = Vec::with_capacity(count as usize);
    for task in tasks {
        let (status, latency) = task.join().expect("benchmark worker panicked");
        if status == 201 {
            successes += 1;
        }
        latencies.push(latency);
    }
    (successes, latencies, batch_started.elapsed())
}

fn percentile_ms(latencies: &[u128], percentile: usize) -> f64 {
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();
    let index = (sorted.len() * percentile).div_ceil(100) - 1;
    sorted[index] as f64 / 1_000_000.0
}

fn main() {
    let address = std::env::var("CRUD_TEST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38212".to_string());
    let pid = server_pid(&address);
    let client_pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_nanos();
    let prefix = format!("Bench-{client_pid}-{nonce}");
    let initial_cpu = cpu_ticks(pid);
    let peak_rss = Arc::new(AtomicU64::new(rss_kib(pid)));
    let stop_sampling = Arc::new(AtomicBool::new(false));
    let sampler_peak = Arc::clone(&peak_rss);
    let sampler_stop = Arc::clone(&stop_sampling);
    let sampler = thread::spawn(move || {
        while !sampler_stop.load(Ordering::Relaxed) {
            sampler_peak.fetch_max(rss_kib(pid), Ordering::Relaxed);
            thread::sleep(Duration::from_millis(10));
        }
    });

    let mut requests = 0_usize;
    for round in 0..2 {
        for count in [50_u32, 100, 200] {
            let batch_prefix = format!("{prefix}-{round}-{count}");
            let (successes, latencies, elapsed) = measure_batch(&address, &batch_prefix, count);
            requests += count as usize;
            let errors = count as usize - successes;
            let requests_per_second = successes as f64 / elapsed.as_secs_f64();
            println!(
                "benchmark load={count} success={successes} error={errors} qps={requests_per_second:.2} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2}",
                percentile_ms(&latencies, 50),
                percentile_ms(&latencies, 95),
                percentile_ms(&latencies, 99),
            );
        }
    }

    stop_sampling.store(true, Ordering::Relaxed);
    sampler.join().expect("resource sampler panicked");
    let cpu_ticks_delta = cpu_ticks(pid).saturating_sub(initial_cpu);
    let peak_rss_kib = peak_rss.load(Ordering::Relaxed);
    if peak_rss_kib == 0 {
        println!("server resources: /proc unavailable; requests measured={requests}");
    } else {
        println!(
            "server resources during load: cpu_ticks_delta={cpu_ticks_delta} sampled_peak_rss_kib={peak_rss_kib}"
        );
    }
    println!("Exploratory local benchmark; compilation is outside the measured runtime.");
}
