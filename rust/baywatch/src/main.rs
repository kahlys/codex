#[macro_use]
extern crate prettytable;

use bollard::Docker;
use clap::Parser;
use futures::future::join_all;
use prettytable::format;
use prettytable::Table;
use std::fs::File;

mod docker;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    image: String,

    #[arg(short, long)]
    output: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let image = args.image;

    let docker = Docker::connect_with_local_defaults().unwrap();
    let d_info = docker.info().await.unwrap();
    let host_ncpu = d_info.ncpu.unwrap();
    let host_memory = d_info.mem_total.unwrap();

    println!("Docker infos");
    println!("host ncpu : {:?}", host_ncpu);
    println!("host memtotal : {:?}\n", host_memory);

    let mut table = Table::new();
    table.set_format(
        format::FormatBuilder::new()
            .column_separator('│')
            .borders('│')
            .separator(
                format::LinePosition::Top,
                format::LineSeparator::new('─', '┬', '┌', '┐'),
            )
            .separator(
                format::LinePosition::Title,
                format::LineSeparator::new('─', '┼', '├', '┤'),
            )
            .separator(
                format::LinePosition::Bottom,
                format::LineSeparator::new('─', '┴', '└', '┘'),
            )
            .padding(1, 1)
            .build(),
    );

    table.set_titles(row!["CPU", "DURATION (ms)",]);
    let res = join_all(
        (1..(host_ncpu + 1))
            .rev()
            .map(|x| docker::run_container(&docker, &image, x)),
    )
    .await;

    for r in res.iter().flatten() {
        table.add_row(row![r.0, r.1,]);
    }
    table.printstd();

    if let Some(o) = args.output {
        let file = File::create(o).unwrap();
        table.to_csv(file).unwrap();
    }
}
