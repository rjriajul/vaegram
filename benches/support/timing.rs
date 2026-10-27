use std::time::{Duration, Instant};

const WARM_UP: Duration = Duration::from_millis(50);
const MEASURE: Duration = Duration::from_millis(300);

pub fn header() {
    println!("{:<28} {:>12}", "case", "ns/op");
}

pub fn bench(name: &str, mut f: impl FnMut()) {
    let warm = Instant::now();
    let mut batch = 0u64;
    while warm.elapsed() < WARM_UP {
        f();
        batch += 1;
    }
    let batch = batch.max(1);

    let mut iters = 0u64;
    let start = Instant::now();
    while start.elapsed() < MEASURE {
        for _ in 0..batch {
            f();
        }
        iters += batch;
    }
    let ns = start.elapsed().as_nanos() as f64 / iters as f64;
    println!("{name:<28} {ns:>12.1}");
}
