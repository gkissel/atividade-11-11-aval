use std::env;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const ITERATIONS_PER_THREAD: usize = 1_000_000;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let thread_count = read_thread_count().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(thread_count > 0, "Use um valor de threads maior que zero");

	let expected_total = thread_count * ITERATIONS_PER_THREAD;

	println!("Atividade 6 — Evitando lock com variavel atomica");
	println!("Cada thread incrementa o contador {} vezes; valor esperado = {}", ITERATIONS_PER_THREAD, expected_total);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let (race_avg, race_times, race_outputs) =
		measure_runs(|run| race_condition_counter(thread_count, run == 0));
	let (lock_avg, lock_times, lock_outputs) =
		measure_runs(|run| locked_counter(thread_count, run == 0));
	let (atomic_avg, atomic_times, atomic_outputs) =
		measure_runs(|run| atomic_counter(thread_count, run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) =
		measure_runs(|run| sequential_counter(thread_count, run == 0));

	let race_final = *race_outputs.last().unwrap_or(&0);
	let lock_final = *lock_outputs.last().unwrap_or(&0);
	let atomic_final = *atomic_outputs.last().unwrap_or(&0);
	let sequential_final = *sequential_outputs.last().unwrap_or(&0);

	println!("\nTabela de tempos medios (ms, apos aquecimento):");
	println!(
		"  T = {} | sem trava: {:.6} | com trava: {:.6} | atomico: {:.6}",
		thread_count,
		race_avg * 1_000.0,
		lock_avg * 1_000.0,
		atomic_avg * 1_000.0
	);
	println!("  Referencia sequencial: {:.6}", sequential_avg * 1_000.0);

	println!("\nDetalhes dos tempos sem trava (ms):");
	log_durations(&race_times);
	println!("\nDetalhes dos tempos com trava (ms):");
	log_durations(&lock_times);
	println!("\nDetalhes dos tempos atomicos (ms):");
	log_durations(&atomic_times);
	println!("\nTempos sequenciais (ms):");
	log_durations(&sequential_times);

	println!("\nValor esperado: {}", expected_total);
	println!("Valor obtido sem trava (ultima execucao): {}", race_final);
	println!("Valor obtido com trava (ultima execucao): {}", lock_final);
	println!("Valor obtido atomico (ultima execucao): {}", atomic_final);
	println!("Sequencial confirma: {}", sequential_final);
	println!(
		"Vantagens relativas: atomico vs trava = {:.2}% | atomico vs sem trava = {:.2}%",
		percentage_change(lock_avg, atomic_avg),
		percentage_change(race_avg, atomic_avg)
	);
	println!(
		"Analise: atomicos evitam contencao do mutex e mantem corretude, mas ainda incutem custo de sincronizacao na memoria; \
	em cargas intensas, fetch_add pode superar locks quando contencao e alta, mas continua mais caro que uma versao sem sincronizacao."
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

fn measure_runs<F>(mut job: F) -> (f64, Vec<Duration>, Vec<usize>)
where
	F: FnMut(usize) -> usize,
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

fn race_condition_counter(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			for iter in 0..ITERATIONS_PER_THREAD {
				let current = counter_clone.load(Ordering::Relaxed);
				// Load + store provocam condicao de corrida intencional.
				counter_clone.store(current + 1, Ordering::Relaxed);
				if iter % 1024 == 0 {
					thread::yield_now();
				}
			}
			if should_print {
				println!("Thread {} finalizada (sem trava)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	counter.load(Ordering::Relaxed)
}

fn locked_counter(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(Mutex::new(0usize));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			for iter in 0..ITERATIONS_PER_THREAD {
				let mut guard = counter_clone.lock().expect("Mutex poisoned");
				*guard += 1;
				if iter % 1024 == 0 {
					thread::yield_now();
				}
			}
			if should_print {
				println!("Thread {} finalizada (com trava)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	let guard = counter.lock().expect("Mutex poisoned");
	*guard
}

fn atomic_counter(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(AtomicUsize::new(0));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			for iter in 0..ITERATIONS_PER_THREAD {
				counter_clone.fetch_add(1, Ordering::Relaxed);
				if iter % 1024 == 0 {
					thread::yield_now();
				}
			}
			if should_print {
				println!("Thread {} finalizada (atomico)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	counter.load(Ordering::Relaxed)
}

fn sequential_counter(thread_count: usize, should_print: bool) -> usize {
	let mut counter = 0usize;

	for worker in 0..thread_count {
		for _ in 0..ITERATIONS_PER_THREAD {
			counter += 1;
		}
		if should_print {
			println!("Sequencial concluiu trabalhador {}", worker);
		}
	}

	counter
}

fn percentage_change(from: f64, to: f64) -> f64 {
	if from <= f64::EPSILON {
		return 0.0;
	}
	((to - from) / from) * 100.0
}