use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use rand::seq::IteratorRandom;
use rand::{thread_rng, Rng};

use nats::jetstream::{
    ConsumerInfo, RetentionPolicy, StreamConfig, StreamInfo,
};

struct Cluster {
    clients: Vec<nats::Connection>,
    servers: Vec<Server>,
    paused: HashSet<usize>,
    seed: u64,
}

impl Cluster {
    fn start(args: &Args) -> Cluster {
        let servers: Vec<Server> =
            (0..args.servers).map(|i| server(&args.path, i as u16)).collect();

        let clients: Vec<nats::Connection> = servers
            .iter()
            .cycle()
            .take(args.clients as usize)
            .map(|s| s.nc())
            .collect();

        let seed = args.seed.unwrap_or(thread_rng().gen());

        println!("Starting cluster exerciser with seed {}", seed);

        Cluster { servers, clients, seed, paused: Default::default() }
    }

    fn step(&mut self) {
        match thread_rng().gen_range(0..50) {
            0 => self.restart_server(),
            1..=4 => self.pause_server(),
            5..=9 => self.resume_server(),
            10..=29 => self.publish(),
            30..=49 => self.consume(),
            _ => unreachable!("impossible choice"),
        }
    }

    fn restart_server(&mut self) {
        let idx = thread_rng().gen_range(0..self.servers.len());
        println!("restarting server {}", idx);
    }

    fn pause_server(&mut self) {
        if self.paused.len() == self.servers.len() {
            // all servers already paused
            return;
        }
        let mut idx = thread_rng().gen_range(0..self.servers.len());

        while self.paused.contains(&idx) {
            idx = thread_rng().gen_range(0..self.servers.len());
        }

        println!("pausing server {}", idx);
        self.paused.insert(idx);
    }

    fn resume_server(&mut self) {
        if self.paused.is_empty() {
            // nothing ot resume
            return;
        }

        let idx = *self.paused.iter().choose(&mut thread_rng()).unwrap();

        println!("resuming server {}", idx);
        self.paused.remove(&idx);
    }

    fn publish(&mut self) {
        println!("publishing message");
    }

    fn consume(&mut self) {
        println!("consuming message");
    }
}

struct Server {
    child: Child,
    port: u16,
    storage_dir: String,
}

impl Server {
    fn nc(&self) -> nats::Connection {
        nats::connect(&format!("localhost:{}", self.port)).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        let _ = std::fs::remove_dir_all(&self.storage_dir);
    }
}

/// Starts a local NATS server that gets killed on drop.
fn server<P: AsRef<Path>>(path: P, idx: u16) -> Server {
    let port = idx + 44000;
    let storage_dir = format!("jetstream_test_{}", idx);
    let _ = std::fs::remove_dir_all(&storage_dir);

    let supercluster_conf = format!("confs/supercluster_{}.conf", idx);

    let child = Command::new(path.as_ref())
        .args(&["--port", &port.to_string()])
        .arg("-js")
        .args(&["-sd", &storage_dir])
        .args(&["-c", &supercluster_conf])
        .arg("-V")
        .arg("-D")
        .spawn()
        .expect("unable to spawn nats-server");

    Server { child, port, storage_dir }
}

const USAGE: &str = "
Usage: exercise [--path=</path/to/nats-server>] [--seed=<#>] [--clients=<#>] [--servers=<#>] [--steps=<#>]

Options:
    --path=<p>      Path to nats-server binary [default: nats-server].
    --seed=<#>      Seed for driving faults [default: None].
    --clients=<#>   Number of concurrent clients [default: 2].
    --servers=<#>   Number of cluster servers [default: 3].
    --steps=<#>     Number of steps to take [default: 1000].
";

struct Args {
    path: PathBuf,
    seed: Option<u64>,
    clients: u8,
    servers: u8,
    steps: u64,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            path: "nats-server".into(),
            seed: None,
            clients: 2,
            servers: 3,
            steps: 1000,
        }
    }
}

fn parse<'a, I, T>(mut iter: I) -> T
where
    I: Iterator<Item = &'a str>,
    T: std::str::FromStr,
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    iter.next().expect(USAGE).parse().expect(USAGE)
}

impl Args {
    fn parse() -> Args {
        let mut args = Args::default();
        for raw_arg in std::env::args().skip(1) {
            let mut splits = raw_arg[2..].split('=');
            match splits.next().unwrap() {
                "path" => args.path = parse(&mut splits),
                "seed" => args.seed = Some(parse(&mut splits)),
                "clients" => args.clients = parse(&mut splits),
                "servers" => args.servers = parse(&mut splits),
                "steps" => args.steps = parse(&mut splits),
                other => panic!("unknown option: {}, {}", other, USAGE),
            }
        }
        args
    }
}

fn main() {
    let args = Args::parse();

    let mut cluster = Cluster::start(&args);

    for _ in 0..args.steps {
        cluster.step();
    }

    /*
    nc.stream_info("test1").expect("couldn't get info (2)");
    let _ = nc.delete_stream("test1");

    nc.create_stream(StreamConfig {
        name: "test1".to_string(),
        retention: RetentionPolicy::WorkQueue,
        ..Default::default()
    })
    .expect("couldn't create test1 stream");

    let mut consumer = nc
        .create_consumer("test1", "consumer1")
        .expect("couldn't create consumer");

    for i in 1..=1000 {
        nc.publish("test1", format!("{}", i)).expect("couldn't publish");
    }

    assert_eq!(
        nc.stream_info("test1")
            .expect("couldn't get stream info")
            .state
            .messages,
        1000
    );

    for _ in 1..=1000 {
        consumer.process(|_msg| Ok(())).expect("couldn't process single");
    }

    let mut count = 0;
    while count != 1000 {
        let _: Vec<()> = consumer
            .process_batch(128, |_msg| {
                count += 1;
                Ok(())
            })
            .into_iter()
            .collect::<std::io::Result<Vec<()>>>()
            .expect("couldn't process batch");
    }
    assert_eq!(count, 1000);

    // sequence numbers start with 1
    for i in 1..=500 {
        nc.delete_message("test1", i).expect("couldn't delete");
    }

    assert_eq!(
        nc.stream_info("test1").expect("couldn't get info (2)").state.messages,
        500
    );

    // cleanup
    let streams: io::Result<Vec<StreamInfo>> = nc.list_streams().collect();

    for stream in streams.expect("couldn't get stream list") {
        let consumers: io::Result<Vec<ConsumerInfo>> = nc
            .list_consumers(&stream.config.name)
            .expect("couldn't list consumers (1)")
            .collect();

        for consumer in consumers.expect("couldn't list consumers (2)") {
            nc.delete_consumer(&stream.config.name, &consumer.name)
                .expect("couldn't delete consumer");
        }

        nc.purge_stream(&stream.config.name).expect("couldn't purge stream");

        assert_eq!(
            nc.stream_info(&stream.config.name)
                .expect("couldn't get stream info (3)")
                .state
                .messages,
            0
        );

        nc.delete_stream(&stream.config.name).expect("couldn't delete stream");
    }
    */
}
