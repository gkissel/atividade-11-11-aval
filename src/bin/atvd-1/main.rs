use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	println!("Atividade 1 — Uma thread \"hello\"");
	println!("Total de execucoes: {} ({} usadas na media apos descartar o aquecimento)", RUNS, RUNS - 1);

	let (parallel_avg, parallel_times, parallel_outputs) = measure_runs(|run| hello_thread(run == 0));
	let (sequential_avg, sequential_times, sequential_outputs) = measure_runs(|run| sequential_hello(run == 0));

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
	println!("Ciclo de vida: main cria a thread, inicia com start e aguarda conclusao via join, recebendo a mensagem.");
}

fn measure_runs<F>(mut job: F) -> (f64, Vec<Duration>, Vec<String>)
where
	F: FnMut(usize) -> String,
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

fn hello_thread(should_print: bool) -> String {
	// A thread executa em paralelo e retorna a mensagem apos o join.
	let handle = thread::spawn(move || {
		let message = String::from("Hello from thread!");
		if should_print {
			println!("Thread: {}", message);
		}
		message
	});

	handle
		.join()
		.expect("Thread panicked during execution")
}

fn sequential_hello(should_print: bool) -> String {
	let message = String::from("Hello from thread!");
	if should_print {
		println!("Sequencial: {}", message);
	}
	message
}