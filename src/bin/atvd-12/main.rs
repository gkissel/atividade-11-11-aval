use std::collections::HashMap;
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const READERS: usize = 5;
const WRITERS: usize = 2;
const OPS_PER_READER: usize = 5_000;
const OPS_PER_WRITER: usize = 3_000;
const ACCOUNT_KEYS: usize = 64;

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	println!("Atividade 12 — Leitores e Escritores");
	println!(
		"Config: {} leitores, {} escritores, {} leituras/leitor, {} escritas/escritor",
		READERS,
		WRITERS,
		OPS_PER_READER,
		OPS_PER_WRITER
	);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let expected_final = expected_final_sum();

	let (mutex_avg, mutex_durations, mutex_runs) = measure_runs(|run| run_with_mutex(run == 0));
	println!("\nTempos com Mutex exclusivo (ms):");
	log_durations(&mutex_durations);
	println!("Tempo medio Mutex (ms): {:.6}", mutex_avg * 1_000.0);

	let baseline_sum = mutex_runs
		.last()
		.map(|res| res.final_sum)
		.expect("Executar mutex");
	assert_eq!(baseline_sum, expected_final, "Mutex final sum divergiu do esperado");

	let (rw_avg, rw_durations, rw_runs) = measure_runs(|run| run_with_rwlock(run == 0));
	println!("\nTempos com RwLock (leitura-escrita) (ms):");
	log_durations(&rw_durations);
	println!("Tempo medio RwLock (ms): {:.6}", rw_avg * 1_000.0);

	let rw_correct = rw_runs.iter().skip(1).all(|res| res.final_sum == expected_final);
	assert!(rw_correct, "RwLock produziu estado final incorreto");

	println!("\nVerificacao: leitura acumulada (mutex) = {}, leitura acumulada (rwlock) = {}",
		sum_read_acc(&mutex_runs[1..]),
		sum_read_acc(&rw_runs[1..])
	);

	println!("\nTabela de desempenho (medias sem aquecimento):");
	println!("Approach        | Tempo (ms) | Speedup vs mutex | Final sum");
	println!(
		"{:<14} | {:>10.3} | {:>16.3} | {:>9}",
		"Mutex",
		mutex_avg * 1_000.0,
		1.0,
		baseline_sum
	);
	println!(
		"{:<14} | {:>10.3} | {:>16.3} | {:>9}",
		"RwLock",
		rw_avg * 1_000.0,
		mutex_avg / rw_avg,
		expected_final
	);

	println!("\nExplicacao: a primitiva equivalente ao java.util.concurrent.locks.ReentrantReadWriteLock permite multiplos leitores simultaneos enquanto nenhum escritor solicita o lock. Com um Mutex exclusivo (similar a um lock unico), cada leitura precisa esperar, ainda que ela apenas consulte dados. A versao leitores-escritores deixa as consultas fluirem em paralelo, reduzindo tempo total quando ha muito mais leituras que escritas. Apenas quando um escritor entra todos os leitores bloqueiam, garantindo consistencia sem sacrificar o throughput de consultas.");
}

fn expected_final_sum() -> i64 {
	let initial = initial_db().values().copied().sum::<i64>();
	let writer_delta: i64 = (0..WRITERS)
		.map(|writer| (writer as i64 + 1) * OPS_PER_WRITER as i64)
		.sum();
	initial + writer_delta
}

fn run_with_mutex(should_log: bool) -> RunMetrics {
	let db = Arc::new(Mutex::new(initial_db()));
	let barrier = Arc::new(Barrier::new(READERS + WRITERS));

	let mut handles = Vec::with_capacity(READERS + WRITERS);

	for reader_id in 0..READERS {
		let db_clone = Arc::clone(&db);
		let barrier_clone = Arc::clone(&barrier);
		handles.push(thread::spawn(move || {
			barrier_clone.wait();
			let mut local_reads = 0usize;
			let mut observed_sum = 0i64;
			for iter in 0..OPS_PER_READER {
				let guard = db_clone.lock().expect("Mutex envenenado");
				let base = ((reader_id * 7 + iter) % ACCOUNT_KEYS) as u32;
				for offset in 0..4 {
					let key = (base + offset as u32) % ACCOUNT_KEYS as u32;
					observed_sum += guard.get(&key).copied().unwrap_or(0);
				}
				local_reads += 1;
				drop(guard);
				if should_log && iter < 2 && reader_id == 0 {
					println!("Mutex leitor {} leu base {}", reader_id, base);
				}
			}
			ThreadStats {
				reads: local_reads,
				writes: 0,
				observed_sum,
			}
		}));
	}

	for writer_id in 0..WRITERS {
		let db_clone = Arc::clone(&db);
		let barrier_clone = Arc::clone(&barrier);
		handles.push(thread::spawn(move || {
			barrier_clone.wait();
			let mut local_writes = 0usize;
			for iter in 0..OPS_PER_WRITER {
				let mut guard = db_clone.lock().expect("Mutex envenenado");
				let key = ((writer_id * 11 + iter) % ACCOUNT_KEYS) as u32;
				let entry = guard.entry(key).or_insert(0);
				*entry += writer_delta(writer_id);
				local_writes += 1;
				drop(guard);
				if should_log && iter < 2 {
					println!("Mutex escritor {} atualizou chave {}", writer_id, key);
				}
			}
			ThreadStats {
				reads: 0,
				writes: local_writes,
				observed_sum: 0,
			}
		}));
	}

	let mut metrics = RunMetrics::default();
	for handle in handles {
		let stats = handle.join().expect("Thread falhou");
		metrics.total_reads += stats.reads;
		metrics.total_writes += stats.writes;
		metrics.read_accumulator += stats.observed_sum;
	}

	metrics.final_sum = Arc::try_unwrap(db)
		.expect("Referencias remanescentes ao banco")
		.into_inner()
		.expect("Mutex envenenado")
		.values()
		.copied()
		.sum();

	metrics
}

fn run_with_rwlock(should_log: bool) -> RunMetrics {
	let db = Arc::new(RwLock::new(initial_db()));
	let barrier = Arc::new(Barrier::new(READERS + WRITERS));
	let mut handles = Vec::with_capacity(READERS + WRITERS);

	for reader_id in 0..READERS {
		let db_clone = Arc::clone(&db);
		let barrier_clone = Arc::clone(&barrier);
		handles.push(thread::spawn(move || {
			barrier_clone.wait();
			let mut local_reads = 0usize;
			let mut observed_sum = 0i64;
			for iter in 0..OPS_PER_READER {
				let guard = db_clone.read().expect("RwLock envenenado");
				let base = ((reader_id * 13 + iter) % ACCOUNT_KEYS) as u32;
				for offset in 0..4 {
					let key = (base + offset as u32) % ACCOUNT_KEYS as u32;
					observed_sum += guard.get(&key).copied().unwrap_or(0);
				}
				local_reads += 1;
				drop(guard);
				if should_log && iter < 2 && reader_id == 0 {
					println!("RwLock leitor {} leu base {}", reader_id, base);
				}
			}
			ThreadStats {
				reads: local_reads,
				writes: 0,
				observed_sum,
			}
		}));
	}

	for writer_id in 0..WRITERS {
		let db_clone = Arc::clone(&db);
		let barrier_clone = Arc::clone(&barrier);
		handles.push(thread::spawn(move || {
			barrier_clone.wait();
			let mut local_writes = 0usize;
			for iter in 0..OPS_PER_WRITER {
				let mut guard = db_clone.write().expect("RwLock envenenado");
				let key = ((writer_id * 17 + iter) % ACCOUNT_KEYS) as u32;
				let entry = guard.entry(key).or_insert(0);
				*entry += writer_delta(writer_id);
				local_writes += 1;
				drop(guard);
				if should_log && iter < 2 {
					println!("RwLock escritor {} atualizou chave {}", writer_id, key);
				}
			}
			ThreadStats {
				reads: 0,
				writes: local_writes,
				observed_sum: 0,
			}
		}));
	}

	let mut metrics = RunMetrics::default();
	for handle in handles {
		let stats = handle.join().expect("Thread falhou");
		metrics.total_reads += stats.reads;
		metrics.total_writes += stats.writes;
		metrics.read_accumulator += stats.observed_sum;
	}

	metrics.final_sum = Arc::try_unwrap(db)
		.expect("Referencias remanescentes ao banco")
		.into_inner()
		.expect("RwLock envenenado")
		.values()
		.copied()
		.sum();

	metrics
}

fn writer_delta(writer_id: usize) -> i64 {
	(writer_id as i64) + 1
}

fn initial_db() -> HashMap<u32, i64> {
	(0..ACCOUNT_KEYS as u32)
		.map(|key| (key, key as i64 * 3 - 50))
		.collect()
}

fn sum_read_acc(runs: &[RunMetrics]) -> i64 {
	runs.iter().map(|metrics| metrics.read_accumulator).sum()
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
struct ThreadStats {
	reads: usize,
	writes: usize,
	observed_sum: i64,
}

#[derive(Clone, Copy, Default)]
struct RunMetrics {
	final_sum: i64,
	total_reads: usize,
	total_writes: usize,
	read_accumulator: i64,
}