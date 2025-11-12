use std::env;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const ITERATIONS_PER_THREAD: usize = 1_000_000;
const BLOCK_SIZE: usize = 1_000;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let thread_count = read_thread_count().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(thread_count > 0, "Use um valor de threads maior que zero");

	let expected_total = thread_count * ITERATIONS_PER_THREAD;

	println!("Atividade 5 — Variando a granularidade do lock");
	println!("Cada thread incrementa o contador {} vezes; valor esperado = {}", ITERATIONS_PER_THREAD, expected_total);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let (per_increment_avg, per_increment_times, per_increment_outputs) =
		measure_runs(|run| lock_each_increment(thread_count, run == 0));
	let (block_avg, block_times, block_outputs) =
		measure_runs(|run| lock_in_blocks(thread_count, run == 0));
	let (single_avg, single_times, single_outputs) =
		measure_runs(|run| lock_once(thread_count, run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) =
		measure_runs(|run| sequential_counter(thread_count, run == 0));

	let per_increment_final = *per_increment_outputs.last().unwrap_or(&0);
	let block_final = *block_outputs.last().unwrap_or(&0);
	let single_final = *single_outputs.last().unwrap_or(&0);
	let sequential_final = *sequential_outputs.last().unwrap_or(&0);

	println!("\nTabela de tempos medios (ms, apos aquecimento):");
	println!(
		"  T = {} | lock a cada inc.: {:.6} | lock a cada {}: {:.6} | lock unico: {:.6}",
		thread_count,
		per_increment_avg * 1_000.0,
		BLOCK_SIZE,
		block_avg * 1_000.0,
		single_avg * 1_000.0
	);
	println!("  Referencia sequencial: {:.6}", sequential_avg * 1_000.0);

	println!("\nDetalhes dos tempos com lock a cada incremento (ms):");
	log_durations(&per_increment_times);
	println!("\nDetalhes dos tempos com lock em blocos de {} (ms):", BLOCK_SIZE);
	log_durations(&block_times);
	println!("\nDetalhes dos tempos com lock unico por thread (ms):");
	log_durations(&single_times);
	println!("\nTempos sequenciais (ms):");
	log_durations(&sequential_times);

	println!("\nValor esperado: {}", expected_total);
	println!("Valor obtido lock a cada incremento: {}", per_increment_final);
	println!("Valor obtido lock por bloco: {}", block_final);
	println!("Valor obtido lock unico: {}", single_final);
	println!("Sequencial confirma: {}", sequential_final);
	println!(
		"Comparacao percentual: inc->bloco = {:.2}% | inc->unico = {:.2}% | bloco->unico = {:.2}%",
		percentage_change(per_increment_avg, block_avg),
		percentage_change(per_increment_avg, single_avg),
		percentage_change(block_avg, single_avg)
	);
	println!(
		"Discussao: granularidade grossa reduz a contencao e o overhead de travamento; \
	locks frequentes aumentam o tempo medio por serializar a secao critica a cada incremento, enquanto acumulo local minimiza sincronizacoes."
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

fn lock_each_increment(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(Mutex::new(0usize));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			for iter in 0..ITERATIONS_PER_THREAD {
				let mut guard = counter_clone.lock().expect("Mutex poisoned");
				// Granularidade fina: cada incremento entra na secao critica.
				*guard += 1;
				if iter % 1024 == 0 {
					thread::yield_now();
				}
			}
			if should_print {
				println!("Thread {} finalizada (lock por incremento)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	let guard = counter.lock().expect("Mutex poisoned");
	*guard
}

fn lock_in_blocks(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(Mutex::new(0usize));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			let mut local_batch = 0usize;
			for iter in 0..ITERATIONS_PER_THREAD {
				local_batch += 1;
				if local_batch == BLOCK_SIZE {
					let mut guard = counter_clone.lock().expect("Mutex poisoned");
					// Travamento apenas quando o lote atinge o tamanho definido.
					*guard += BLOCK_SIZE;
					local_batch = 0;
				}
				if iter % 4096 == 0 {
					thread::yield_now();
				}
			}
			if local_batch > 0 {
				let mut guard = counter_clone.lock().expect("Mutex poisoned");
				*guard += local_batch;
			}
			if should_print {
				println!("Thread {} finalizada (lock por bloco)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	let guard = counter.lock().expect("Mutex poisoned");
	*guard
}

fn lock_once(thread_count: usize, should_print: bool) -> usize {
	let counter = Arc::new(Mutex::new(0usize));
	let mut handles = Vec::with_capacity(thread_count);

	for thread_id in 0..thread_count {
		let counter_clone = Arc::clone(&counter);
		handles.push(thread::spawn(move || {
			let mut local_total = 0usize;
			for iter in 0..ITERATIONS_PER_THREAD {
				local_total += 1;
				if iter % 8192 == 0 {
					thread::yield_now();
				}
			}
			let mut guard = counter_clone.lock().expect("Mutex poisoned");
			// Travamento unico por thread: acumula tudo localmente.
			*guard += local_total;
			if should_print {
				println!("Thread {} finalizada (lock unico)", thread_id);
			}
		}));
	}

	for handle in handles {
		handle.join().expect("Thread panicked during execution");
	}

	let guard = counter.lock().expect("Mutex poisoned");
	*guard
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