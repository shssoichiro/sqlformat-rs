use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sqlformat::*;

fn simple_query(c: &mut Criterion) {
    let input = "SELECT * FROM my_table WHERE id = 1";
    c.bench_function("simple query", |b| {
        b.iter(|| {
            format(
                black_box(input),
                black_box(&QueryParams::None),
                black_box(FormatOptions::default()),
            )
        })
    });
}

fn complex_query(c: &mut Criterion) {
    let input = "SELECT t1.id, t1.name, t1.title, t1.description, t2.mothers_maiden_name, t2.first_girlfriend\nFROM my_table t1 LEFT JOIN other_table t2 ON t1.id = t2.other_id WHERE t2.order BETWEEN  17 AND 30";
    c.bench_function("complex query", |b| {
        b.iter(|| {
            format(
                black_box(input),
                black_box(&QueryParams::None),
                black_box(FormatOptions::default()),
            )
        })
    });
}

fn query_with_named_params(c: &mut Criterion) {
    let input = "SELECT * FROM my_table WHERE id = :first OR id = :second OR id = :third";
    let params = vec![
        ("first".to_string(), "1".to_string()),
        ("second".to_string(), "2".to_string()),
        ("third".to_string(), "3".to_string()),
    ];
    c.bench_function("named params", |b| {
        b.iter(|| {
            format(
                black_box(input),
                black_box(&QueryParams::Named(params.clone())),
                black_box(FormatOptions::default()),
            )
        })
    });
}

fn query_with_explicit_indexed_params(c: &mut Criterion) {
    let input = "SELECT * FROM my_table WHERE id = ?1 OR id = ?2 OR id = ?0";
    let params = vec!["0".to_string(), "1".to_string(), "2".to_string()];
    c.bench_function("explicit indexed params", |b| {
        b.iter(|| {
            format(
                black_box(input),
                black_box(&QueryParams::Indexed(params.clone())),
                black_box(FormatOptions::default()),
            )
        })
    });
}

fn query_with_implicit_indexed_params(c: &mut Criterion) {
    let input = "SELECT * FROM my_table WHERE id = ? OR id = ? OR id = ?";
    let params = vec!["0".to_string(), "1".to_string(), "2".to_string()];
    c.bench_function("implicit indexed params", |b| {
        b.iter(|| {
            format(
                black_box(input),
                black_box(&QueryParams::Indexed(params.clone())),
                black_box(FormatOptions::default()),
            )
        })
    });
}

criterion_group!(
    benches,
    simple_query,
    complex_query,
    query_with_named_params,
    query_with_explicit_indexed_params,
    query_with_implicit_indexed_params
);
criterion_main!(benches);
