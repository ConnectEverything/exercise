fn main() {
    let args = exercise::Args::parse();

    println!("starting fault injector with arguments:");
    println!("{:?}", args);

    let steps = if args.burn_in { u64::MAX } else { args.steps };

    let mut cluster = exercise::Cluster::start(args);

    for _ in 0..steps {
        cluster.step();
    }
}
