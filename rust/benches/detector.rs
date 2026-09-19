use shakespot::core::Detector;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
    time::Instant,
};

struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
// SAFETY: 所有分配与释放都转交同一个系统分配器，计数不分配内存。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Relaxed);
        // SAFETY: 使用调用方传入的原始布局。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 指针与布局原样交回其分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn detector_case(name: &str, samples: &[(f64, f64); 4096]) {
    let mut detector = Detector::default();
    for i in 0..16_384 {
        let (x, y) = samples[i % samples.len()];
        black_box(detector.add(black_box(i as f64 * 8.0), x, y, false));
    }
    detector.reset();
    let start = Instant::now();
    let initial = ALLOCATIONS.load(Relaxed);
    let mut triggers = 0usize;
    let count = 500_000usize;
    for i in 0..count {
        let (x, y) = samples[i % samples.len()];
        triggers +=
            usize::from(detector.add(black_box(i as f64 * 8.0), black_box(x), black_box(y), false));
    }
    let allocations = ALLOCATIONS.load(Relaxed) - initial;
    let elapsed = start.elapsed();
    assert_eq!(allocations, 0, "Detector hot path allocated memory");
    println!(
        "case={name} samples={count} elapsed_ms={:.3} ns_per_sample={:.2} allocations={allocations} detector_bytes={} triggers={triggers}",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_nanos() as f64 / count as f64,
        std::mem::size_of::<Detector>()
    );
}
fn main() {
    for name in [
        "horizontal",
        "diagonal",
        "straight",
        "drift",
        "tiny_jitter",
        "stationary",
    ] {
        let samples = std::array::from_fn(|i| {
            let t = i as f64 * 8.0;
            let wave = 85.0 * (t / 48.0).sin();
            match name {
                "horizontal" => (wave, 0.0),
                "diagonal" => (wave / 2.0_f64.sqrt(), wave / 2.0_f64.sqrt()),
                "straight" => (t * 0.18, t * 0.08),
                "drift" => (wave, t * 4.0),
                "tiny_jitter" => (2.0 * (t / 12.0).sin(), 2.0 * (t / 9.0).cos()),
                _ => (0.0, 0.0),
            }
        });
        detector_case(name, &samples);
    }
    let count = 2_000_000usize;
    let raw_positions: [i64; 4096] = std::array::from_fn(|i| {
        (340.0 * (i as f64 * std::f64::consts::TAU / 2048.0).sin()).round() as i64
    });
    let mut tracker = shakespot::input::MotionTracker::default();
    let mut previous = 0;
    let mut raw_samples = 0;
    let mut raw_triggers = 0;
    let initial = ALLOCATIONS.load(Relaxed);
    let start = Instant::now();
    for i in 0..count {
        let position = raw_positions[i % raw_positions.len()];
        let time = i as f64 / 8.0;
        let result = tracker.push(
            black_box(shakespot::input::RawMotion {
                device: 1,
                mode: shakespot::input::CoordinateMode::Relative,
                started: time,
                time,
                x: position - previous,
                y: 0,
            }),
            (1.0, 1.0),
        );
        previous = position;
        raw_samples += result.samples;
        raw_triggers += usize::from(result.triggered);
    }
    let allocations = ALLOCATIONS.load(Relaxed) - initial;
    let elapsed = start.elapsed();
    assert_eq!(allocations, 0, "Raw motion hot path allocated memory");
    assert!(raw_triggers > 0, "Benchmark waveform did not trigger");
    println!(
        "raw_packets={count} simulated_hz=8000 elapsed_ms={:.3} ns_per_packet={:.2} allocations={allocations} tracker_bytes={} samples={raw_samples} triggers={raw_triggers}",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_nanos() as f64 / count as f64,
        std::mem::size_of::<shakespot::input::MotionTracker>()
    );
    let mut pixels = vec![0u32; 192 * 256];
    let initial = ALLOCATIONS.load(Relaxed);
    let start = Instant::now();
    for i in 0..1000 {
        shakespot::raster::render(black_box(&mut pixels), 256, i & 1 != 0, i & 2 != 0).unwrap();
    }
    let allocations = ALLOCATIONS.load(Relaxed) - initial;
    let elapsed = start.elapsed();
    assert_eq!(allocations, 0, "Rasterizer allocated memory");
    println!(
        "raster_frames=1000 height=256 elapsed_ms={:.3} us_per_frame={:.2} allocations={allocations}",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0
    );
}
