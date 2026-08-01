//! Benchmarks for core cryptographic operations.
//!
//! Run with: `cargo bench -p lilypad-core`

use criterion::{criterion_group, criterion_main, Criterion};
use lilypad_core::{decrypt, derive_key, encrypt, KeyDerivationParams, KeyMaterial};

fn bench_encrypt(c: &mut Criterion) {
    let key = KeyMaterial::generate();

    let small = vec![0u8; 256]; // single entry
    let medium = vec![0u8; 64 * 1024]; // small vault (~64 KB)
    let large = vec![0u8; 1024 * 1024]; // large vault (~1 MB)

    let mut group = c.benchmark_group("encrypt");
    group.bench_function("256B", |b| b.iter(|| encrypt(&key, &small).unwrap()));
    group.bench_function("64KB", |b| b.iter(|| encrypt(&key, &medium).unwrap()));
    group.bench_function("1MB", |b| b.iter(|| encrypt(&key, &large).unwrap()));
    group.finish();
}

fn bench_decrypt(c: &mut Criterion) {
    let key = KeyMaterial::generate();

    let small_ct = encrypt(&key, &vec![0u8; 256]).unwrap();
    let medium_ct = encrypt(&key, &vec![0u8; 64 * 1024]).unwrap();
    let large_ct = encrypt(&key, &vec![0u8; 1024 * 1024]).unwrap();

    let mut group = c.benchmark_group("decrypt");
    group.bench_function("256B", |b| b.iter(|| decrypt(&key, &small_ct).unwrap()));
    group.bench_function("64KB", |b| b.iter(|| decrypt(&key, &medium_ct).unwrap()));
    group.bench_function("1MB", |b| b.iter(|| decrypt(&key, &large_ct).unwrap()));
    group.finish();
}

fn bench_derive_key(c: &mut Criterion) {
    // Use minimal-but-valid KDF params for benchmarking so runs finish quickly.
    // Production params (64 MiB, 3 iterations) are intentionally slow.
    let fast_params = KeyDerivationParams {
        salt: [0u8; 16],
        memory_kib: 16 * 1024, // 16 MiB (minimum)
        iterations: 1,
        parallelism: 1,
    };

    let prod_params = KeyDerivationParams::generate();

    let mut group = c.benchmark_group("derive_key");
    group.sample_size(10); // KDF is intentionally slow
    group.bench_function("fast_params", |b| {
        b.iter(|| derive_key("bench-password-42!", &fast_params).unwrap())
    });
    group.bench_function("production_params", |b| {
        b.iter(|| derive_key("bench-password-42!", &prod_params).unwrap())
    });
    group.finish();
}

fn bench_key_generate(c: &mut Criterion) {
    c.bench_function("KeyMaterial::generate", |b| b.iter(KeyMaterial::generate));
}

criterion_group!(
    benches,
    bench_encrypt,
    bench_decrypt,
    bench_derive_key,
    bench_key_generate
);
criterion_main!(benches);
