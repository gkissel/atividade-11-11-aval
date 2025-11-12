use std::env;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

	println!("Atividade 3 — Condicao de corrida na pratica");
	println!(
		"Cada thread incrementa o contador {} vezes; valor esperado = {}",
		ITERATIONS_PER_THREAD,
		expected_total
	);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let (parallel_avg, parallel_times, parallel_outputs) =
		measure_runs(|run| race_condition_counter(thread_count, run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) =
		measure_runs(|run| sequential_counter(thread_count, run == 0));

	let is_correct = parallel_outputs
		.iter()
		.skip(1)
		.zip(sequential_outputs.iter().skip(1))
		.all(|(parallel, sequential)| parallel == sequential);

	let final_parallel = *parallel_outputs.last().unwrap_or(&0);
	let final_sequential = *sequential_outputs.last().unwrap_or(&0);
	let loss = expected_total.saturating_sub(final_parallel);

	println!("\nTempos paralelos (ms):");
	log_durations(&parallel_times);
	println!("Tempo medio paralelo (ms): {:.6}", parallel_avg * 1_000.0);

	println!("\nTempos sequenciais (ms):");
	log_durations(&sequential_times);
	println!("Tempo medio sequencial (ms): {:.6}", sequential_avg * 1_000.0);

	println!("\nValor esperado: {}", expected_total);
	println!("Valor obtido (ultima execucao paralela): {}", final_parallel);
	println!("Perda estimada por corrida: {}", loss);
	println!("Sequencial confirma: {}", final_sequential);
	println!(
		"Corretude apos aquecimento: {}",
		if is_correct {
			"OK"
		} else {
			"FALHOU (corrida detectada)"
		}
	);
	println!(
		"Por que ha perda: threads intercalam leitura e escrita do contador sem exclusao mutua;\
	quando duas leem o mesmo valor antes de atualizar, uma sobrescreve a soma da outra e o total final fica menor."
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

	// Descarte o primeiro tempo (aquecimento) para reduzir variacao do cache/JIT.
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
				// Atualizacao nao atomica (load + store) que causa perda quando outras threads escrevem entre as operacoes.
				counter_clone.store(current + 1, Ordering::Relaxed);
				if iter % 1024 == 0 {
					thread::yield_now();
				}
			}
			if should_print {
				println!("Thread {} finalizada", thread_id);
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