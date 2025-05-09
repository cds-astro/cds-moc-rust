use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

use moc::{moc::range::RangeMOC, qty::Hpx};

fn create_cells_u32(depth: u8, n_cells: usize) -> Vec<u32> {
  let mut rng = rand::rng();
  let npix = 12 * 4_u32.pow(depth as u32);
  (0..n_cells)
    .into_iter()
    .map(|_| rng.random_range(0..npix))
    .collect()
}

fn create_cells_u64(depth: u8, n_cells: usize) -> Vec<u64> {
  let mut rng = rand::rng();
  let npix = 12 * 4_u64.pow(depth as u32);
  (0..n_cells)
    .into_iter()
    .map(|_| rng.random_range(0..npix))
    .collect()
}

fn bench_rmoc_from_cells_u32(c: &mut Criterion) {
  let cells = create_cells_u32(12, 10_000_000);

  let mut binding = c.benchmark_group("MocFromCells");
  let group = binding.sample_size(10);

  group.bench_function("RangeMOC::from_fixed_depth_cells_u32", |b| {
    b.iter(|| {
      RangeMOC::<u32, Hpx<u32>>::from_fixed_depth_cells(12, cells.iter().cloned(), Some(100_000))
    })
  });
}

fn bench_rmoc_from_cells_u64(c: &mut Criterion) {
  let cells = create_cells_u64(14, 10_000_000);

  let mut binding = c.benchmark_group("MocFromCells");
  let group = binding.sample_size(10);

  group.bench_function("RangeMOC::from_fixed_depth_cells_u64", |b| {
    b.iter(|| {
      RangeMOC::<u64, Hpx<u64>>::from_fixed_depth_cells(14, cells.iter().cloned(), Some(100_000))
    })
  });
}

criterion_group!(
  benches,
  bench_rmoc_from_cells_u32,
  bench_rmoc_from_cells_u64
);
criterion_main!(benches);
