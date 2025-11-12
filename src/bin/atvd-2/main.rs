use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let n = read_thread_count().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(n > 0, "Use um valor de N maior que zero");

	println!("Atividade 2 — N threads imprimindo o proprio indice");
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let (parallel_avg, parallel_times, parallel_outputs) =
		measure_runs(|run| spawn_indexed_threads(n, run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) =
		measure_runs(|run| sequential_indices(n, run == 0));

	let is_correct = parallel_outputs
		.iter()
		.skip(1)
		.zip(sequential_outputs.iter().skip(1))
		.all(|(parallel, sequential)| parallel == sequential);

	println!("\nTempos paralelos (ms):");
	log_durations(&parallel_times);
	println!("Tempo medio paralelo (ms): {:.6}", parallel_avg * 1_000.0);

	println!("\nTempos sequenciais (ms):");
	log_durations(&sequential_times);
	println!("Tempo medio sequencial (ms): {:.6}", sequential_avg * 1_000.0);

	println!("\nCorretude apos aquecimento: {}", if is_correct { "OK" } else { "FALHOU" });
	println!(
		"Passagem de dados: cada thread recebe seu indice via closure `move`, que captura `i` ao criar a thread."
	);
}

fn read_thread_count() -> Result<usize, String> {
	if let Some(arg) = env::args().nth(1) {
		return arg
			.parse::<usize>()
			.map_err(|_| format!("Argumento invalido para N: {}", arg));
	}

	print!("Informe N (numero de threads): ");
	io::stdout().flush().map_err(|err| format!("Falha ao limpar stdout: {}", err))?;

	let mut input = String::new();
	io::stdin()
		.read_line(&mut input)
		.map_err(|err| format!("Falha ao ler entrada: {}", err))?;

	input
		.trim()
		.parse::<usize>()
		.map_err(|_| format!("Entrada invalida para N: {}", input.trim()))
}

fn measure_runs<F>(mut job: F) -> (f64, Vec<Duration>, Vec<Vec<usize>>)
where
	F: FnMut(usize) -> Vec<usize>,
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

fn spawn_indexed_threads(n: usize, should_print: bool) -> Vec<usize> {
	let mut handles = Vec::with_capacity(n);

	for i in 0..n {
		handles.push(thread::spawn(move || {
			if should_print {
				println!("Thread {}", i);
			}
			i
		}));
	}

	// Coleta na mesma ordem de criacao para facilitar a validacao.
	handles
		.into_iter()
		.map(|handle| handle.join().expect("Thread panicked during execution"))
		.collect()
}

fn sequential_indices(n: usize, should_print: bool) -> Vec<usize> {
	let mut indices = Vec::with_capacity(n);

	for i in 0..n {
		if should_print {
			println!("Sequencial {}", i);
		}
		indices.push(i);
	}

	indices
}