use std::env;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let thread_count = read_thread_count().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(thread_count > 0, "Use um valor de threads maior que zero");

	println!("Atividade 7 — Barreira de sincronizacao em duas fases");
	println!("Cada thread executa duas fases; barreira garante sincronizacao entre elas. Threads = {}", thread_count);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	println!("\nLogs da execucao de aquecimento (run 1):");

	let (parallel_avg, parallel_times, parallel_outputs) =
		measure_runs(|run| barrier_two_phase(thread_count, run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) =
		measure_runs(|_| sequential_two_phase(thread_count, false));

	let parallel_ok = parallel_outputs.iter().skip(1).all(|&ok| ok);
	let sequential_ok = sequential_outputs.iter().skip(1).all(|&ok| ok);

	println!("\nTempos com barreira (ms):");
	log_durations(&parallel_times);
	println!("Tempo medio com barreira (ms): {:.6}", parallel_avg * 1_000.0);

	println!("\nTempos sequenciais (ms):");
	log_durations(&sequential_times);
	println!("Tempo medio sequencial (ms): {:.6}", sequential_avg * 1_000.0);

	println!("\nCorretude apos aquecimento: barreira = {}, sequencial = {}", parallel_ok, sequential_ok);
	println!(
		"Conclusao: nenhuma thread inicia a Fase 2 antes da barreira liberar; a versao sequencial serve como referencia para verificacao."
	);
}

fn read_thread_count() -> Result<usize, String> {
	if let Some(arg) = env::args().nth(1) {
		return arg
			.parse::<usize>()
			.map_err(|_| format!("Argumento invalido para numero de threads: {}", arg));
	}

	print!("Informe o numero de threads: ");
	io::stdout().flush().map_err(|err| format!("Falha ao limpar stdout: {}", err))?;

	let mut input = String::new();
	io::stdin()
		.read_line(&mut input)
		.map_err(|err| format!("Falha ao ler entrada: {}", err))?;

	input
		.trim()
		.parse::<usize>()
		.map_err(|_| format!("Entrada invalida para threads: {}", input.trim()))
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

fn barrier_two_phase(thread_count: usize, should_log: bool) -> bool {
	let barrier = Arc::new(Barrier::new(thread_count));
	let phase1_counter = Arc::new(AtomicUsize::new(0));
	let violation = Arc::new(AtomicBool::new(false));
	let mut handles = Vec::with_capacity(thread_count);

	for id in 0..thread_count {
		let barrier_clone = Arc::clone(&barrier);
		let counter_clone = Arc::clone(&phase1_counter);
		let violation_clone = Arc::clone(&violation);
		handles.push(thread::spawn(move || {
			if should_log {
				println!("Thread {} - Fase 1 iniciada", id);
			}
			thread::sleep(Duration::from_micros(200));
			let done = counter_clone.fetch_add(1, Ordering::SeqCst) + 1;
			if should_log {
				println!("Thread {} - Fase 1 concluida ({}/{})", id, done, thread_count);
			}

			let wait_result = barrier_clone.wait();
			if wait_result.is_leader() && should_log {
				println!("Barrier liberou Fase 2");
			}

			if counter_clone.load(Ordering::SeqCst) < thread_count {
				violation_clone.store(true, Ordering::SeqCst);
			}

			if should_log {
				println!("Thread {} - Fase 2 iniciada", id);
			}
			thread::sleep(Duration::from_micros(200));
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	!violation.load(Ordering::SeqCst)
}

fn sequential_two_phase(thread_count: usize, should_log: bool) -> bool {
	let mut phase1_completed = 0usize;

	for id in 0..thread_count {
		if should_log {
			println!("Sequencial {} - Fase 1 iniciada", id);
		}
		thread::sleep(Duration::from_micros(50));
		phase1_completed += 1;
		if should_log {
			println!("Sequencial {} - Fase 1 concluida ({}/{})", id, phase1_completed, thread_count);
		}
	}

	if should_log {
		println!("Sequencial - Todas as threads virtuais prontas; iniciando Fase 2");
	}

	for id in 0..thread_count {
		if should_log {
			println!("Sequencial {} - Fase 2 iniciada", id);
		}
		thread::sleep(Duration::from_micros(50));
	}

	true
}