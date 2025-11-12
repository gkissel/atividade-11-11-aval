use std::env;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const DEFAULT_TOTAL_ITEMS: usize = 200;
const PRODUCER_COUNT: usize = 2;
const CONSUMER_COUNT: usize = 2;
const QUEUE_CAPACITY: usize = 32;
const SENTINEL: i32 = -1;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	let total_items = read_total_items().unwrap_or_else(|err| {
		eprintln!("{}", err);
		std::process::exit(1);
	});

	assert!(total_items >= PRODUCER_COUNT, "Quantidade total deve ser >= numero de produtores");

	println!("Atividade 8 — Produtor-Consumidor com fila bloqueante");
	println!(
		"Threads: {} produtores, {} consumidores; total previsto: {} itens",
		PRODUCER_COUNT,
		CONSUMER_COUNT,
		total_items
	);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	println!("\nLogs da execucao de aquecimento (run 1):");

	let (avg_time, durations, results) = measure_runs(|run| run_producer_consumer(total_items, run == 0));

	println!("\nTempos com fila bloqueante (ms):");
	log_durations(&durations);
	println!("Tempo medio (ms): {:.6}", avg_time * 1_000.0);

	if let Some(final_result) = results.last() {
		println!("\nResumo da ultima execucao medida:");
		println!("  Produzidos: {} (esperado {})", final_result.produced, total_items);
		println!("  Consumidos: {} (esperado {})", final_result.consumed, total_items);
		println!(
			"  Sentinelas consumidos: {} (esperado {})",
			final_result.sentinels,
			CONSUMER_COUNT
		);
		println!("  Deadlock detectado: {}", final_result.deadlock_detected);
	}

	println!("Conclusao: fila bloqueante coordena produtores e consumidores sem travamentos quando os sentinelas encerram cada consumidor.");
}

fn read_total_items() -> Result<usize, String> {
	if let Some(arg) = env::args().nth(1) {
		return arg
			.parse::<usize>()
			.map_err(|_| format!("Argumento invalido para total de itens: {}", arg));
	}

	print!("Informe o total de itens (ENTER para usar {}): ", DEFAULT_TOTAL_ITEMS);
	io::stdout().flush().map_err(|err| format!("Falha ao limpar stdout: {}", err))?;

	let mut input = String::new();
	io::stdin()
		.read_line(&mut input)
		.map_err(|err| format!("Falha ao ler entrada: {}", err))?;

	let trimmed = input.trim();
	if trimmed.is_empty() {
		return Ok(DEFAULT_TOTAL_ITEMS);
	}

	trimmed
		.parse::<usize>()
		.map_err(|_| format!("Entrada invalida para total de itens: {}", trimmed))
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

#[derive(Clone, Default)]
struct ProducerConsumerResult {
	produced: usize,
	consumed: usize,
	sentinels: usize,
	deadlock_detected: bool,
}

fn run_producer_consumer(total_items: usize, should_log: bool) -> ProducerConsumerResult {
	let (tx, rx) = mpsc::sync_channel::<i32>(QUEUE_CAPACITY);
	let shared_rx = Arc::new(Mutex::new(rx));

	let produced_count = Arc::new(AtomicUsize::new(0));
	let consumed_count = Arc::new(AtomicUsize::new(0));
	let sentinel_count = Arc::new(AtomicUsize::new(0));

	let mut producer_handles = Vec::new();

	let base_items = total_items / PRODUCER_COUNT;
	let remainder = total_items % PRODUCER_COUNT;

	for producer_id in 0..PRODUCER_COUNT {
		let producer_tx = tx.clone();
		let produced_clone = Arc::clone(&produced_count);
		let items_to_produce = base_items + if producer_id < remainder { 1 } else { 0 };
		producer_handles.push(thread::spawn(move || {
			for item_idx in 0..items_to_produce {
				let item = (producer_id * 10_000 + item_idx) as i32;
				producer_tx
					.send(item)
					.expect("Erro ao enviar item para a fila");
				produced_clone.fetch_add(1, Ordering::SeqCst);
				if should_log && item_idx < 5 {
					println!("Produtor {} enviou item {}", producer_id, item);
				}
			}
			if should_log {
				println!(
					"Produtor {} finalizado ({} itens)",
					producer_id,
					items_to_produce
				);
			}
		}));
	}

	let mut consumer_handles = Vec::new();
	for consumer_id in 0..CONSUMER_COUNT {
		let rx_clone = Arc::clone(&shared_rx);
		let consumed_clone = Arc::clone(&consumed_count);
		let sentinel_clone = Arc::clone(&sentinel_count);
		consumer_handles.push(thread::spawn(move || loop {
			let message = {
				let receiver_guard = rx_clone
					.lock()
					.expect("Falha ao adquirir lock do receiver");
				receiver_guard.recv()
			};

			match message {
				Ok(SENTINEL) => {
					sentinel_clone.fetch_add(1, Ordering::SeqCst);
					if should_log {
						println!("Consumidor {} recebeu sentinela", consumer_id);
					}
					break;
				}
				Ok(item) => {
					let current = consumed_clone.fetch_add(1, Ordering::SeqCst) + 1;
					if should_log && current <= 5 {
						println!("Consumidor {} processou item {}", consumer_id, item);
					}
					thread::sleep(Duration::from_micros(150));
				}
				Err(_) => break,
			}
		}));
	}

	for handle in producer_handles {
		handle.join().expect("Produtor panicked durante execucao");
	}

	for _ in 0..CONSUMER_COUNT {
		tx.send(SENTINEL)
			.expect("Falha ao enviar sentinela ao consumidor");
	}
	drop(tx);

	for handle in consumer_handles {
		handle.join().expect("Consumidor panicked durante execucao");
	}

	let produced = produced_count.load(Ordering::SeqCst);
	let consumed = consumed_count.load(Ordering::SeqCst);
	let sentinels = sentinel_count.load(Ordering::SeqCst);
	let deadlock_detected = produced != total_items || consumed != total_items || sentinels != CONSUMER_COUNT;

	ProducerConsumerResult {
		produced,
		consumed,
		sentinels,
		deadlock_detected,
	}
}