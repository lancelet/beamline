use beamline::{
    style::StyledLine,
    tiler::{TileInfo, Tiler},
    Color, Line, LineCap, LineStyle, P2,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn tile_some_lines() -> (Vec<TileInfo>, Vec<StyledLine>) {
    // worst-case 16x16 tiles
    let tile_width = 16;
    let tile_height = 16;
    // 4k display
    let area_width = 3840;
    let area_height = 2160;

    let mut tiler = Tiler::new(area_width, area_height, tile_width, tile_height);

    let w = area_width as f32;
    let h = area_height as f32;
    let style_round = LineStyle {
        width: 34.2,
        cap: LineCap::Round,
        color: Color::WHITE,
    };
    let style_square = LineStyle {
        width: 36.3,
        cap: LineCap::Square,
        color: Color::WHITE,
    };
    let style_butt = LineStyle {
        width: 33.7,
        cap: LineCap::Butt,
        color: Color::WHITE,
    };

    let mut lines = Vec::new();

    // Create 334 * 3 = 1002 diagonal lines across the tiles.
    let n_cross = 334;
    for j in 0..n_cross {
        let frac = j as f32 / (n_cross - 1) as f32;
        let ya = frac * h;
        let yb = h - ya;

        let pa = P2::new(0.0, ya);
        let pb = P2::new(w, yb);
        let line = Line::new(pa, pb);

        let line_round = StyledLine {
            line,
            style: style_round,
        };
        let line_square = StyledLine {
            line,
            style: style_square,
        };
        let line_butt = StyledLine {
            line,
            style: style_butt,
        };

        lines.push(line_round);
        lines.push(line_square);
        lines.push(line_butt);
    }

    lines
        .into_iter()
        .for_each(|styled_line| tiler.add(styled_line));

    tiler.drain()
}

pub fn tiling_lines_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("Tile Group");
    // group.measurement_time(Duration::new(90, 0)); // run for 1.5 minutes
    // group.significance_level(0.1).sample_size(200);

    group.bench_function("tile_some_lines", |bencher| {
        bencher.iter(|| tile_some_lines())
    });
}

criterion_group!(benches, tiling_lines_benchmark);
criterion_main!(benches);
