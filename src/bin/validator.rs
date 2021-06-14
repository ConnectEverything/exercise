use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::io;
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, SeedableRng};

use nats::jetstream::{ConsumerConfig, RetentionPolicy, StreamConfig};

const STREAM: &str = "exercise_stream";

const SERVERS: usize = 3;

const USAGE: &str = "
Usage: exercise [--path=</path/to/nats-server>]

Options:
    --clients=<#>   Number of concurrent clients [default: 3].
    --steps=<#>     Number of steps to take [default: 10000].
    --replicas=<#>  Number of replicas for the JetStream test stream [default: 1].
    --burn-in       Ignore steps and run tests until we crash [default: unset].
";

// generates unique (for this test run) ID
fn idgen() -> u64 {
    static IDGEN: AtomicU64 = AtomicU64::new(0);
    IDGEN.fetch_add(1, SeqCst)
}

fn nc(server_number: usize) -> nats::Connection {
    assert!(
        server_number > 0 && server_number < 130,
        "invalid server number {}, must be > 0 and < 130",
        server_number
    );
    loop {
        // we add 1 to the server number IP because the network may have 10.20.20.1 already used
        if let Ok(nc) = nats::connect(&format!("10.20.20.{}:4222", server_number + 1)) {
            return nc;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10))
        }
    }
}

struct Cluster {
    clients: Vec<Consumer>,
    args: Args,
    rng: StdRng,
    unvalidated_consumers: HashSet<usize>,
    durability_model: DurabilityModel,
}

impl Cluster {
    fn start(args: Args) -> Cluster {
        let rng = SeedableRng::seed_from_u64(0);

        println!("creating testing stream {}", STREAM);

        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));

            let nc = nc(1);

            let _ = nc.delete_stream(STREAM);

            let ret = nc.create_stream(StreamConfig {
                num_replicas: args.num_replicas,
                name: STREAM.to_string(),
                retention: RetentionPolicy::Limits,
                ..Default::default()
            });

            if ret.is_ok() {
                break;
            }
        }

        let clients: Vec<Consumer> = (1..=SERVERS)
            .cycle()
            .enumerate()
            .take(args.clients as usize)
            .map(|(id, s)| {
                let consumer_name = format!("consumer_{}", id);
                println!("creating testing consumer {}", consumer_name);

                let nc = nc(s);
                let conf = ConsumerConfig {
                    deliver_subject: Some(consumer_name.clone()),
                    durable_name: consumer_name.into(),
                    ..Default::default()
                };
                let mut inner = nc
                    .create_consumer(STREAM, conf)
                    .expect("couldn't create consumer");

                inner.timeout = std::time::Duration::from_millis(10);

                Consumer {
                    inner,
                    observed: Default::default(),
                    id,
                }
            })
            .collect();

        Cluster {
            clients,
            rng: rng,
            args,
            durability_model: Default::default(),
            unvalidated_consumers: Default::default(),
        }
    }

    fn step(&mut self) {
        match self.rng.gen_range(0..10) {
            0..=2 => self.publish(),
            3..=10 => self.consume(),
            _ => unreachable!("impossible choice"),
        }
        self.validate();
    }

    fn publish(&mut self) {
        let c = self.clients.choose(&mut self.rng).unwrap();
        let data = idgen().to_le_bytes();
        c.inner.nc.publish(STREAM, data).unwrap();
    }

    fn consume(&mut self) {
        let c = self.clients.choose_mut(&mut self.rng).unwrap();
        let proc_ret: io::Result<(u64, u64)> = c.inner.process_timeout(|msg| {
            let info = msg.jetstream_message_info().unwrap();

            let id = u64::from_le_bytes((&*msg.data).try_into().unwrap());
            Ok((info.stream_seq, id))
        });

        if let Ok((seq, id)) = proc_ret {
            c.observed.insert(seq, id);
            self.unvalidated_consumers.insert(c.id);
        }
    }

    fn validate(&mut self) {
        // assert all consumers have witnessed messages in the correct order
        let unvalidated_consumers = mem::take(&mut self.unvalidated_consumers);

        for id in unvalidated_consumers {
            let c = &mut self.clients[id];

            let observed = mem::take(&mut c.observed);

            for (id, value) in observed {
                if let Some(old_value) = self.durability_model.observed.insert(id, value) {
                    if value != old_value {
                        eprintln!(
                            "
                            Correctness violation detected after running for {:?}.
                            Consumers received different values for the same \
                            stream sequence.
                                stream sequence: {}
                                first observed value: {}
                                second observed value: {}
                            ",
                            self.args.start_time.elapsed(),
                            id,
                            old_value,
                            value,
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

struct Consumer {
    inner: nats::jetstream::Consumer,
    observed: HashMap<u64, u64>,
    id: usize,
}

// we record every sid:uuid pair, and
// ensure that consumers never observe
// different uuid's for the same stream id.
#[derive(Default, Debug)]
struct DurabilityModel {
    observed: HashMap<u64, u64>,
}

#[derive(Debug)]
struct Args {
    clients: u8,
    steps: u64,
    num_replicas: usize,
    burn_in: bool,
    start_time: std::time::Instant,
}

impl Default for Args {
    fn default() -> Args {
        Args {
            clients: 3,
            steps: 10000,
            num_replicas: 1,
            burn_in: false,
            start_time: std::time::Instant::now(),
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
                "clients" => args.clients = parse(&mut splits),
                "steps" => args.steps = parse(&mut splits),
                "replicas" => args.num_replicas = parse(&mut splits),
                "burn-in" => args.burn_in = true,
                other => panic!("unknown option: {}, {}", other, USAGE),
            }
        }
        args
    }
}

fn main() {
    let args = Args::parse();

    println!("starting validator with arguments:");
    println!("{:?}", args);

    let steps = if args.burn_in { u64::MAX } else { args.steps };

    let mut cluster = Cluster::start(args);

    println!("cluster ready for fault injection");

    println!("workload and correctness assertions begin in 3 seconds");

    std::thread::sleep(std::time::Duration::from_secs(3));

    println!("starting workload and correctness assertions now");

    for _ in 0..steps {
        cluster.step();
    }

    println!(
        "validator found no correctness violations after \
        executing {} operations. finished.",
        steps
    );
}
