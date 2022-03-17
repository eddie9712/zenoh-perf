//
// Copyright (c) 2017, 2020 ADLINK Technology Inc.
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ADLINK zenoh team, <zenoh@adlink-labs.tech>
//
use async_std::stream::StreamExt;
use async_std::sync::{Arc, Mutex};
use async_std::task;
use clap::Parser;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zenoh::config::Config;
use zenoh::net::protocol::core::WhatAmI;
use zenoh::net::protocol::io::reader::{HasReader, Reader};
use zenoh::net::protocol::io::SplitBuffer;
use zenoh::prelude::*;
use zenoh::publication::CongestionControl;

#[derive(Debug, Parser)]
#[clap(name = "z_ping")]
struct Opt {
    /// locator(s), e.g. --locator tcp/127.0.0.1:7447,tcp/127.0.0.1:7448
    #[clap(short, long)]
    locator: Option<String>,

    /// peer, router, or client
    #[clap(short, long)]
    mode: String,

    /// payload size (bytes)
    #[clap(short, long)]
    payload: usize,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    scenario: String,

    /// interval of sending message (sec)
    #[clap(short, long)]
    interval: f64,

    /// spawn a task to receive or not
    #[clap(long = "parallel")]
    parallel: bool,

    /// declare a numerical ID for key expression
    #[clap(long)]
    use_expr: bool,

    /// declare publication before the publisher
    #[clap(long)]
    declare_publication: bool,
}

async fn parallel(opt: Opt, config: Config) {
    let session = zenoh::open(config).await.unwrap();
    let session = Arc::new(session);

    let mut sub = if opt.use_expr {
        // declare subscriber
        let key_expr_pong = session.declare_expr("/test/pong").await.unwrap();
        session.subscribe(&key_expr_pong).reliable().await.unwrap()
    } else {
        session.subscribe("/test/pong").reliable().await.unwrap()
    };
    let mut key_expr_num = 0;
    if opt.use_expr {
        key_expr_num = session.declare_expr("/test/ping").await.unwrap();
        if opt.declare_publication {
            session.declare_publication(key_expr_num).await.unwrap();
        }
    } else {
        if opt.declare_publication {
            session.declare_publication("/test/ping").await.unwrap();
        }
    }

    // The hashmap with the pings
    let pending = Arc::new(Mutex::new(HashMap::<u64, Instant>::new()));

    let c_pending = pending.clone();
    let scenario = opt.scenario;
    let name = opt.name;
    let interval = opt.interval;
    task::spawn(async move {
        while let Some(sample) = sub.next().await {
            let mut payload_reader = sample.value.payload.reader();
            let mut count_bytes = [0u8; 8];
            if payload_reader.read_exact(&mut count_bytes) {
                let count = u64::from_le_bytes(count_bytes);

                let instant = c_pending.lock().await.remove(&count).unwrap();
                println!(
                    "zenoh,{},latency.parallel,{},{},{},{},{}",
                    scenario,
                    name,
                    sample.value.payload.len(),
                    interval,
                    count,
                    instant.elapsed().as_micros()
                );
            } else {
                panic!("Fail to fill the buffer");
            }
        }
        panic!("Invalid value!");
    });

    let mut count: u64 = 0;
    loop {
        let count_bytes: [u8; 8] = count.to_le_bytes();
        let mut payload = vec![0u8; opt.payload];
        payload[0..8].copy_from_slice(&count_bytes);

        pending.lock().await.insert(count, Instant::now());
        if opt.use_expr {
            session
                .put(key_expr_num, payload)
                .congestion_control(CongestionControl::Block)
                .await
                .unwrap();
        } else {
            session
                .put("/test/ping", payload)
                .congestion_control(CongestionControl::Block)
                .await
                .unwrap();
        }

        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

async fn single(opt: Opt, config: Config) {
    let session = zenoh::open(config).await.unwrap();

    let scenario = opt.scenario;
    let name = opt.name;
    let interval = opt.interval;

    let mut sub = if opt.use_expr {
        // declare subscriber
        let key_expr_pong = session.declare_expr("/test/pong").await.unwrap();
        session.subscribe(&key_expr_pong).reliable().await.unwrap()
    } else {
        session.subscribe("/test/pong").reliable().await.unwrap()
    };
    let mut key_expr_num = 0;
    if opt.use_expr {
        key_expr_num = session.declare_expr("/test/ping").await.unwrap();
        if opt.declare_publication {
            session.declare_publication(key_expr_num).await.unwrap();
        }
    } else {
        if opt.declare_publication {
            session.declare_publication("/test/ping").await.unwrap();
        }
    }

    let mut count: u64 = 0;
    loop {
        let count_bytes: [u8; 8] = count.to_le_bytes();
        let mut payload = vec![0u8; opt.payload];
        payload[0..8].copy_from_slice(&count_bytes);

        let now = Instant::now();

        if opt.use_expr {
            session
                .put(key_expr_num, payload)
                .congestion_control(CongestionControl::Block)
                .await
                .unwrap();
        } else {
            session
                .put("/test/ping", payload)
                .congestion_control(CongestionControl::Block)
                .await
                .unwrap();
        }

        match sub.next().await {
            Some(sample) => {
                let mut payload_reader = sample.value.payload.reader();
                let mut count_bytes = [0u8; 8];
                if payload_reader.read_exact(&mut count_bytes) {
                    let s_count = u64::from_le_bytes(count_bytes);

                    println!(
                        "zenoh,{},latency.sequential,{},{},{},{},{}",
                        scenario,
                        name,
                        sample.value.payload.len(),
                        interval,
                        s_count,
                        now.elapsed().as_micros()
                    );
                } else {
                    panic!("Fail to fill the buffer");
                }
            }
            _ => panic!("Invalid value"),
        }
        task::sleep(Duration::from_secs_f64(opt.interval)).await;
        count += 1;
    }
}

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    // Parse the args
    let opt = Opt::parse();

    let mut config = Config::default();
    match opt.mode.as_str() {
        "peer" => config.set_mode(Some(WhatAmI::Peer)).unwrap(),
        "client" => config.set_mode(Some(WhatAmI::Client)).unwrap(),
        _ => panic!("Unsupported mode: {}", opt.mode),
    };

    if let Some(ref l) = opt.locator {
        config.scouting.multicast.set_enabled(Some(false)).unwrap();
        config
            .peers
            .extend(l.split(',').map(|v| v.parse().unwrap()));
    } else {
        config.scouting.multicast.set_enabled(Some(true)).unwrap();
    }

    if opt.parallel {
        parallel(opt, config).await;
    } else {
        single(opt, config).await;
    }
}
