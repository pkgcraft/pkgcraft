use criterion::*;

pub mod check;
pub mod scan;

criterion_group!(check, check::bench);
criterion_group!(scan, scan::bench);
criterion_main!(check, scan);
