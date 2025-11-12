# Aval Onze Onze — Guia de Execução

Este repositório reúne uma série de atividades práticas sobre paralelismo, sincronização e medição de desempenho em Rust. Cada atividade vive em um binário separado dentro de `src/bin`.

## Pré-requisitos

- [Rust](https://www.rust-lang.org/tools/install) 1.74 ou superior (inclui `cargo`).
- Uma shell compatível (PowerShell, cmd, bash, etc.).
- Opcional: `cargo fmt`/`cargo clippy` instalados para formatar e analisar o código.

Para confirmar que o toolchain está pronto, execute:

```powershell
rustc --version
cargo --version
```

## Preparando o ambiente

Clone o repositório e entre na pasta do projeto:

```powershell
cd C:\rust-packages\aval-onze-onze
```

Recomendamos compilar todos os binários uma vez para baixar dependências e validar o código:

```powershell
cargo check
```

## Estrutura dos binários

Cada atividade tem seu próprio executável, nomeado `atvd-X`, com `X` variando de 1 a 12. Para executar qualquer atividade, use `cargo run --bin atvd-X`. O primeiro run de cada medição é tratado como aquecimento dentro do próprio programa.

### Comandos rápidos

| Atividade | Tópico principal | Comando |
|-----------|------------------|---------|
| 1 | Thread única com medição | `cargo run --bin atvd-1` |
| 2 | Várias threads índice/log | `cargo run --bin atvd-2` |
| 3 | Corrida de dados (contador) | `cargo run --bin atvd-3` |
| 4 | Mutex vs corrida de dados | `cargo run --bin atvd-4` |
| 5 | Granularidade de locks | `cargo run --bin atvd-5` |
| 6 | Mutex vs atômicos | `cargo run --bin atvd-6` |
| 7 | Barreira (duas fases) | `cargo run --bin atvd-7` |
| 8 | Produtor–consumidor | `cargo run --bin atvd-8` |
| 9 | Soma paralela (map-reduce) | `cargo run --bin atvd-9` |
| 10 | Estimativa de π (Monte Carlo) | `cargo run --bin atvd-10 [K]` |
| 11 | Executor / pool fixo | `cargo run --bin atvd-11` |
| 12 | Leitores vs escritores | `cargo run --bin atvd-12` |

### Parâmetros opcionais

- **Atividade 10** aceita um argumento inteiro `K`, representando as amostras base por thread. Se omitido, usa `200_000`:
  ```powershell
  cargo run --bin atvd-10 500000
  ```
- As demais atividades não exigem parâmetros e utilizam constantes internas documentadas no código.

## Reproduzindo medições

Cada programa executa `RUNS = 5` medições e descarta o primeiro resultado como aquecimento. Para obter números mais estáveis:

1. Rode cada binário algumas vezes e considere a média reportada (já calculada no output).
2. Feche aplicações que possam interferir no processador.
3. Caso deseje alterar o número de execuções, ajuste a constante `RUNS` no arquivo correspondente e recompile.

## Organização do código

- `src/bin/atvd-X/main.rs`: código de cada atividade.
- Funções auxiliares (p. ex. `measure_runs`, `log_durations`) são duplicadas em cada atividade para manter os binários independentes.
- Os programas validam o resultado das execuções paralelas contra referências sequenciais ou invariantes definidos (por exemplo, somas esperadas), imprimindo mensagens de verificação.

## Acompanhamento de resultados

Os binários emitem tabelas com tempos médios, speedup e eficiência quando aplicável. Para registrar resultados:

1. Redirecione o output para um arquivo:
   ```powershell
   cargo run --bin atvd-9 > resultados-atvd-9.txt
   ```
2. Compare as métricas entre execuções (por exemplo, mudando número de threads em Atividades 9–12).

## Limpeza

Para remover artefatos de compilação:

```powershell
cargo clean
```

Isso apagará a pasta `target/`, liberando espaço em disco.

---
Dúvidas ou insights adicionais podem ser anotados diretamente nos arquivos das atividades para manter o histórico do que foi observado durante as medições.
