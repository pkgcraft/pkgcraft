use criterion::*;

mod scan;

criterion_group!(scan, scan::bench);
criterion_main!(scan);
