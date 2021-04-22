use std::io;
use std::process::{Child, Command};

use nats::jetstream::{
    ConsumerInfo, RetentionPolicy, StreamConfig, StreamInfo,
};

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
fn server(idx: u16) -> Server {
    let port = idx + 44000;
    let storage_dir = format!("jetstream_test_{}", idx);
    let _ = std::fs::remove_dir_all(&storage_dir);

    let supercluster_conf = format!("confs/supercluster_{}.conf", idx);

    let child = Command::new("nats-server")
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

fn main() {
    let s0 = server(0);
    let s1 = server(1);
    let s2 = server(2);

    let nc = s0.nc();

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
}
