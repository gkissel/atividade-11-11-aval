use std::env;
use std::f64::consts::PI;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const THREAD_OPTIONS: [usize; 4] = [1, 2, 4, 8];
const DEFAULT_SAMPLES_PER_THREAD: usize = 200_000;
const WORKLOAD_MULTIPLIERS: [usize; 3] = [1, 5, 25];

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let base_samples = read_samples_per_thread().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(base_samples > 0, "K (amostras por thread) precisa ser positivo");

	println!("Atividade 10 — Estimativa de π (Monte Carlo)");
	println!(
		"Amostras base por thread (K): {} | Multiplicadores avaliados: {:?}",
		base_samples,
		WORKLOAD_MULTIPLIERS
	);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let workloads: Vec<usize> = WORKLOAD_MULTIPLIERS
		.iter()
		.map(|&mult| base_samples.saturating_mul(mult))
		.collect();

	let mut table = Vec::new();

	for &samples_per_thread in &workloads {
		println!("\n=== K = {} amostras por thread ===", samples_per_thread);

		for &threads in &THREAD_OPTIONS {
			let (avg, durations, results) = measure_runs(|run| {
				estimate_pi_parallel(samples_per_thread, threads, run == 0)
			});

			println!("\nTempos para {} thread(s) (ms):", threads);
			log_durations(&durations);
			println!("Tempo medio (ms): {:.6}", avg * 1_000.0);

			let last = results.last().cloned().unwrap_or_default();
			let error = (last.pi_estimate - PI).abs();

			table.push(SummaryRow {
				threads,
				samples_per_thread,
				avg_seconds: avg,
				pi_estimate: last.pi_estimate,
				error,
			});
		}
	}

	println!("\nTabela de resultados:");
	println!("Threads | K por thread | Tempo (ms) | π_est | |π_est-π| | Speedup | Eficiência");

	for &k in &workloads {
		for row in table.iter().filter(|row| row.samples_per_thread == k) {
			let baseline = table
				.iter()
				.find(|r| r.samples_per_thread == k && r.threads == 1)
				.expect("Baseline com 1 thread nao encontrado");

			let speedup = baseline.avg_seconds / row.avg_seconds;
			let efficiency = speedup / row.threads as f64;

			println!(
				"{:>7} | {:>12} | {:>10.3} | {:>6.4} | {:>8.6} | {:>7.3} | {:>9.3}",
				row.threads,
				row.samples_per_thread,
				row.avg_seconds * 1_000.0,
				row.pi_estimate,
				row.error,
				speedup,
				efficiency
			);
		}
	}

	println!(
		"\nObservacao: aumentos em K reduzem a variancia e amortizam overhead de threads; 
		(speedups) tendem a melhorar quando cada thread processa lotes maiores."
	);
}

fn read_samples_per_thread() -> Result<usize, String> {
	if let Some(arg) = env::args().nth(1) {
		return arg
			.parse::<usize>()
			.map_err(|_| format!("Argumento invalido para amostras por thread: {}", arg));
	}

	print!(
		"Informe K (amostras por thread, ENTER usa {}): ",
		DEFAULT_SAMPLES_PER_THREAD
	);
	io::stdout().flush().map_err(|err| format!("Falha ao limpar stdout: {}", err))?;

	let mut input = String::new();
	io::stdin()
		.read_line(&mut input)
		.map_err(|err| format!("Falha ao ler entrada: {}", err))?;

	let trimmed = input.trim();
	if trimmed.is_empty() {
		return Ok(DEFAULT_SAMPLES_PER_THREAD);
	}

	trimmed
		.parse::<usize>()
		.map_err(|_| format!("Entrada invalida para K: {}", trimmed))
}

fn measure_runs<F, T>(mut job: F) -> (f64, Vec<Duration>, Vec<T>)
where
	F: FnMut(usize) -> T,
{
	let mut durations = Vec::with_capacity(RUNS);
	let mut outputs = Vec::with_capacity(RUNS);

	for run in 0..RUNS {
		let start = Instant::now();
		let result = job(run);
		let elapsed = start.elapsed();

		durations.push(elapsed);
		outputs.push(result);
	}

	let avg = durations
		.iter()
		.skip(1)
		.map(Duration::as_secs_f64)
		.sum::<f64>()
		/ (RUNS - 1) as f64;

	(avg, durations, outputs)
}

fn log_durations(durations: &[Duration]) {
	for (index, duration) in durations.iter().enumerate() {
		println!("  Execucao {}: {:.6}", index + 1, duration.as_secs_f64() * 1_000.0);
	}
	println!("  Obs.: primeira execucao funciona como aquecimento.");
}

#[derive(Clone, Copy, Default)]
struct MonteCarloResult {
	total_samples: usize,
	inside_circle: usize,
	pi_estimate: f64,
}

struct SummaryRow {
	threads: usize,
	samples_per_thread: usize,
	avg_seconds: f64,
	pi_estimate: f64,
	error: f64,
}

fn estimate_pi_parallel(samples_per_thread: usize, threads: usize, should_log: bool) -> MonteCarloResult {
	if should_log {
		println!(
			"Estimando pi com {} thread(s), {} amostras por thread",
			threads,
			samples_per_thread
		);
	}

	if threads <= 1 {
		return run_monte_carlo(samples_per_thread, 0, should_log);
	}

	let handles: Vec<_> = (0..threads)
		.map(|id| {
			let should_log_thread = should_log && id == 0;
			thread::spawn(move || run_monte_carlo(samples_per_thread, id as u64, should_log_thread))
		})
		.collect();

	let mut total_samples = 0usize;
	let mut inside = 0usize;

	for handle in handles {
		let result = handle
			.join()
			.expect("Thread panicked during Monte Carlo run");
		total_samples += result.total_samples;
		inside += result.inside_circle;
	}

	let aggregate_pi = 4.0 * (inside as f64) / (total_samples as f64);

	MonteCarloResult {
		total_samples,
		inside_circle: inside,
		pi_estimate: aggregate_pi,
	}
}

fn run_monte_carlo(samples: usize, seed_offset: u64, should_log: bool) -> MonteCarloResult {
	let mut generator = XorShift64::new(0x9E3779B97F4A7C15u64.wrapping_add(seed_offset));
	let mut inside = 0usize;

	for idx in 0..samples {
		let x = generator.next_f64();
		let y = generator.next_f64();
		let dx = x - 0.5;
		let dy = y - 0.5;
		if dx * dx + dy * dy <= 0.25 {
			inside += 1;
		}
		if should_log && idx < 5 {
			println!(
				"  Amostra {} -> ({:.4}, {:.4}) dentro = {}",
				idx,
				x,
				y,
				dx * dx + dy * dy <= 0.25
			);
		}
	}

	let pi_estimate = 4.0 * (inside as f64) / (samples as f64);
	MonteCarloResult {
		total_samples: samples,
		inside_circle: inside,
		pi_estimate,
	}
}

struct XorShift64 {
	state: u64,
}

impl XorShift64 {
	fn new(seed: u64) -> Self {
		let state = if seed == 0 { 0xA511E9B7C3D2_1234 } else { seed };
		Self { state }
	}

	fn next_u64(&mut self) -> u64 {
		let mut x = self.state;
		x ^= x << 13;
		x ^= x >> 7;
		x ^= x << 17;
		self.state = x;
		x
	}

	fn next_f64(&mut self) -> f64 {
		let value = self.next_u64();
		(value as f64) / (u64::MAX as f64)
	}
}