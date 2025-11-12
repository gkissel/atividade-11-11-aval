use std::env;
use std::io::{self, Write};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const DEFAULT_VECTOR_LEN: usize = 20_000_000;
const THREAD_COUNTS: [usize; 4] = [1, 2, 4, 8];

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let vector_len = read_vector_len().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	println!("Atividade 9 — Soma paralela de vetor (map-reduce)");
	println!("Tamanho do vetor: {} elementos", vector_len);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let data = Arc::new(generate_vector(vector_len));
	let expected_sum = arithmetic_series_sum(vector_len as i64 - 1);

	let (seq_avg, seq_durations, seq_outputs) =
		measure_runs(|run| sequential_sum(&data, run == 0));

	println!("\nTempos sequenciais (ms):");
	log_durations(&seq_durations);
	println!("Tempo medio sequencial (ms): {:.6}", seq_avg * 1_000.0);

	let sequential_result = *seq_outputs.last().unwrap_or(&0);
	let mut stats = Vec::new();

	for &threads in &THREAD_COUNTS {
		let (avg, durations, outputs) =
			measure_runs(|run| parallel_sum(&data, threads, run == 0));

		println!("\nTempos com {} thread(s) (ms):", threads);
		log_durations(&durations);
		println!("Tempo medio (ms): {:.6}", avg * 1_000.0);

		let correct = outputs.iter().skip(1).all(|&sum| sum == sequential_result);
		stats.push(ParallelStats {
			threads,
			avg_seconds: avg,
			is_correct: correct,
		});
	}

	println!("\nTabela de desempenho:");
	println!("Threads | Tempo (ms) | Speedup | Eficiencia | Corretude");
	for entry in &stats {
		let speedup = seq_avg / entry.avg_seconds;
		let efficiency = speedup / entry.threads as f64;
		println!(
			"{:>7} | {:>10.3} | {:>7.3} | {:>9.3} | {}",
			entry.threads,
			entry.avg_seconds * 1_000.0,
			speedup,
			efficiency,
			if entry.is_correct { "OK" } else { "FALHOU" }
		);
	}

	let total_matches = sequential_result == expected_sum;
	println!(
		"\nVerificacao final: soma sequencial = {}, formula esperada = {}, confere = {}",
		sequential_result,
		expected_sum,
		total_matches
	);
}

fn read_vector_len() -> Result<usize, String> {
	if let Some(arg) = env::args().nth(1) {
		return arg
			.parse::<usize>()
			.map_err(|_| format!("Argumento invalido para tamanho do vetor: {}", arg));
	}

	print!("Informe o tamanho do vetor (ENTER para usar {}): ", DEFAULT_VECTOR_LEN);
	io::stdout().flush().map_err(|err| format!("Falha ao limpar stdout: {}", err))?;

	let mut input = String::new();
	io::stdin()
		.read_line(&mut input)
		.map_err(|err| format!("Falha ao ler entrada: {}", err))?;

	let trimmed = input.trim();
	if trimmed.is_empty() {
		return Ok(DEFAULT_VECTOR_LEN);
	}

	trimmed
		.parse::<usize>()
		.map_err(|_| format!("Entrada invalida para tamanho do vetor: {}", trimmed))
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

struct ParallelStats {
	threads: usize,
	avg_seconds: f64,
	is_correct: bool,
}

fn generate_vector(len: usize) -> Vec<i64> {
	(0..len as i64).collect()
}

fn arithmetic_series_sum(last_value: i64) -> i64 {
	if last_value < 0 {
		return 0;
	}
	last_value * (last_value + 1) / 2
}

fn sequential_sum(data: &Arc<Vec<i64>>, should_log: bool) -> i64 {
	if should_log {
		println!("Executando soma sequencial de {} elementos", data.len());
	}
	data.iter().copied().sum()
}

fn parallel_sum(data: &Arc<Vec<i64>>, threads: usize, should_log: bool) -> i64 {
	let len = data.len();
	let actual_threads = threads.min(len.max(1));
	if should_log {
		println!("Soma paralela com {} thread(s) para {} elementos", actual_threads, len);
	}
	if actual_threads <= 1 {
		return data.iter().copied().sum();
	}

	let chunk_size = (len + actual_threads - 1) / actual_threads;
	let mut handles = Vec::with_capacity(actual_threads);

	for chunk_idx in 0..actual_threads {
		let start = chunk_idx * chunk_size;
		if start >= len {
			break;
		}
		let end = (start + chunk_size).min(len);
		let data_clone = Arc::clone(data);
		handles.push(thread::spawn(move || data_clone[start..end].iter().copied().sum::<i64>()));
	}

	let mut total = 0_i64;
	for handle in handles {
		total += handle.join().expect("Thread panicked durante o map-reduce");
	}
	total
}