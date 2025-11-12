use std::cmp::Ordering;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const TASK_COUNT: usize = 400;
const BLOCK_SIZE: usize = 1_000;
const THREAD_POOL_SIZES: [usize; 3] = [2, 4, 8];

fn main() {
	assert!(RUNS >= 3, "Use at least three runs to keep statistics meaningful");

	println!("Atividade 11 — Pool de threads (executors)");
	println!("Tarefas: {} blocos de {} elementos", TASK_COUNT, BLOCK_SIZE);
	println!("Total de execucoes temporizadas: {} ({} entram na media)", RUNS, RUNS - 1);

	let data = Arc::new(generate_data(TASK_COUNT * BLOCK_SIZE));
	let tasks = build_tasks(TASK_COUNT, BLOCK_SIZE);

	let (seq_avg, seq_durations, seq_outputs) =
		measure_runs(|run| sequential_process(&data, &tasks, run == 0));
	println!("\nTempos sequenciais (ms):");
	log_durations(&seq_durations);
	println!("Tempo medio sequencial (ms): {:.6}", seq_avg * 1_000.0);

	let baseline_sum = seq_outputs
		.last()
		.map(|res| res.total_sum)
		.expect("Sequencial nao produziu resultado");

	let (naive_avg, naive_durations, naive_outputs) =
		measure_runs(|run| naive_threads_per_task(&data, &tasks, run == 0));
	println!("\nTempos com criacao por tarefa (ms):");
	log_durations(&naive_durations);
	println!("Tempo medio criacao por tarefa (ms): {:.6}", naive_avg * 1_000.0);

	let naive_correct = naive_outputs.iter().skip(1).all(|res| res.total_sum == baseline_sum);
	assert!(naive_correct, "Resultados da abordagem com threads por tarefa divergiram");

	let mut pool_stats = Vec::new();

	for &workers in &THREAD_POOL_SIZES {
		let (avg, durations, outputs) = measure_runs(|run| {
			run_with_thread_pool(&data, &tasks, workers, run == 0)
		});
		println!("\nTempos com pool fixo de {} worker(s) (ms):", workers);
		log_durations(&durations);
		println!("Tempo medio pool (ms): {:.6}", avg * 1_000.0);

		let correct = outputs.iter().skip(1).all(|res| res.total_sum == baseline_sum);
		assert!(correct, "Pool com {} workers produziu soma incorreta", workers);

		pool_stats.push(PoolStat {
			workers,
			avg_seconds: avg,
		});
	}

	println!("\nTabela de desempenho (medias sem aquecimento):");
	println!(
		"Abordagem         | Workers | Tempo (ms) | Speedup vs naive | Speedup vs seq"
	);
	println!(
		"{:<16} | {:>7} | {:>10.3} | {:>16.3} | {:>14.3}",
		"Sequencial",
		1,
		seq_avg * 1_000.0,
		naive_avg / seq_avg,
		1.0
	);
	println!(
		"{:<16} | {:>7} | {:>10.3} | {:>16.3} | {:>14.3}",
		"Thread por tarefa",
		TASK_COUNT,
		naive_avg * 1_000.0,
		1.0,
		seq_avg / naive_avg
	);

	for stat in &pool_stats {
		let speedup_vs_naive = naive_avg / stat.avg_seconds;
		let speedup_vs_seq = seq_avg / stat.avg_seconds;
		println!(
			"{:<16} | {:>7} | {:>10.3} | {:>16.3} | {:>14.3}",
			"Pool fixo",
			stat.workers,
			stat.avg_seconds * 1_000.0,
			speedup_vs_naive,
			speedup_vs_seq
		);
	}

	if let Some(best) = pool_stats
		.iter()
		.filter(|stat| stat.avg_seconds < naive_avg)
		.min_by(|a, b| {
			a.avg_seconds
				.partial_cmp(&b.avg_seconds)
				.unwrap_or(Ordering::Equal)
		})
	{
		println!(
			"Observacao: a partir de {} worker(s) o pool superou criar {} threads por tarefa, reduzindo overhead em {:.2}%",
			best.workers,
			TASK_COUNT,
			(1.0 - best.avg_seconds / naive_avg) * 100.0
		);
	} else {
		println!(
			"Observacao: com blocos tao pequenos, o overhead de comunicacao do pool ainda supera a criacao direta de threads."
		);
	}
}

fn generate_data(len: usize) -> Vec<i32> {
	(0..len)
		.map(|idx| ((idx as i32 * 31 + 7) % 1_000) - 500)
		.collect()
}

fn build_tasks(task_count: usize, block_size: usize) -> Vec<(usize, usize)> {
	(0..task_count)
		.map(|task| {
			let start = task * block_size;
			let end = start + block_size;
			(start, end)
		})
		.collect()
}

fn sequential_process(
	data: &Arc<Vec<i32>>,
	tasks: &[(usize, usize)],
	should_log: bool,
) -> ExecutionResult {
	if should_log {
		println!(
			"Processando sequencialmente {} blocos de {} elementos",
			tasks.len(),
			BLOCK_SIZE
		);
	}

	let mut total = 0_i64;
	for &(start, end) in tasks {
		let slice_sum: i64 = data[start..end].iter().map(|&value| value as i64).sum();
		total += slice_sum;
	}

	ExecutionResult { total_sum: total }
}

fn naive_threads_per_task(
	data: &Arc<Vec<i32>>,
	tasks: &[(usize, usize)],
	should_log: bool,
) -> ExecutionResult {
	let mut handles = Vec::with_capacity(tasks.len());

	for (task_idx, &(start, end)) in tasks.iter().enumerate() {
		let data_clone = Arc::clone(data);
		let log_this = should_log && task_idx < 3;
		handles.push(thread::spawn(move || {
			if log_this {
				println!(
					"Thread dedicada {} processa bloco [{}..{})",
					task_idx,
					start,
					end
				);
			}
			data_clone[start..end]
				.iter()
				.map(|&value| value as i64)
				.sum::<i64>()
		}));
	}

	let mut total = 0_i64;
	for handle in handles {
		total += handle.join().expect("Thread dedicada falhou");
	}

	ExecutionResult { total_sum: total }
}

fn run_with_thread_pool(
	data: &Arc<Vec<i32>>,
	tasks: &[(usize, usize)],
	workers: usize,
	should_log: bool,
) -> ExecutionResult {
	assert!(workers > 0, "Pool precisa ter pelo menos um worker");

	let (job_tx, job_rx) = mpsc::channel::<Option<(usize, usize)>>();
	let job_rx = Arc::new(Mutex::new(job_rx));
	let (result_tx, result_rx) = mpsc::channel::<i64>();

	let mut worker_handles = Vec::with_capacity(workers);

	for worker_id in 0..workers {
		let rx_clone = Arc::clone(&job_rx);
		let result_clone = result_tx.clone();
		let data_clone = Arc::clone(data);
		let log_worker = should_log && worker_id == 0;
		worker_handles.push(thread::spawn(move || loop {
			let message = {
				let guard = rx_clone.lock().expect("Mutex de jobs envenenado");
				guard.recv()
			};

			match message {
				Ok(Some((start, end))) => {
					if log_worker {
						println!(
							"Worker {} processa bloco [{}..{})",
							worker_id,
							start,
							end
						);
					}
					let partial: i64 = data_clone[start..end]
						.iter()
						.map(|&value| value as i64)
						.sum();
					result_clone
						.send(partial)
						.expect("Canal de resultados fechado");
				}
				Ok(None) | Err(_) => break,
			}
		}));
	}

	drop(result_tx);

	for &(start, end) in tasks {
		job_tx
			.send(Some((start, end)))
			.expect("Canal de jobs fechado");
	}

	for _ in 0..workers {
		job_tx
			.send(None)
			.expect("Falha ao sinalizar encerramento");
	}

	let mut total = 0_i64;
	for _ in 0..tasks.len() {
		total += result_rx.recv().expect("Worker nao retornou resultado");
	}

	for handle in worker_handles {
		handle.join().expect("Worker do pool falhou");
	}

	ExecutionResult { total_sum: total }
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
struct ExecutionResult {
	total_sum: i64,
}

struct PoolStat {
	workers: usize,
	avg_seconds: f64,
}