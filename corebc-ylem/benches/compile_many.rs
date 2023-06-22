//! compile many benches
#[macro_use]
extern crate criterion;

use corebc_ylem::{CompilerInput, Ylem};
use criterion::Criterion;
use std::path::Path;

fn compile_many_benchmark(c: &mut Criterion) {
    let inputs = load_compiler_inputs();
    let ylem = Ylem::default();

    let mut group = c.benchmark_group("compile many");
    group.sample_size(10);
    group.bench_function("sequential", |b| {
        b.iter(|| {
            for i in inputs.iter() {
                let _ = ylem.compile(i).unwrap();
            }
        });
    });

    #[cfg(feature = "full")]
    {
        let tasks = inputs.into_iter().map(|input| (Ylem::default(), input)).collect::<Vec<_>>();
        let num = tasks.len();
        group.bench_function("concurrently", |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let _ = Ylem::compile_many(tasks.clone(), num).await.flattened().unwrap();
            });
        });
    }
}

fn load_compiler_inputs() -> Vec<CompilerInput> {
    let mut inputs = Vec::new();
    for file in std::fs::read_dir(Path::new(&env!("CARGO_MANIFEST_DIR")).join("test-data/in"))
        .unwrap()
        .take(5)
    {
        let file = file.unwrap();
        let input = std::fs::read_to_string(file.path()).unwrap();
        let input: CompilerInput = serde_json::from_str(&input).unwrap();
        inputs.push(input);
    }
    inputs
}

criterion_group!(benches, compile_many_benchmark);
criterion_main!(benches);
