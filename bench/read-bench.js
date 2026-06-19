"use strict";

const http = require("http");
const crypto = require("crypto");
const addon = require("../lib");

const STREAM = process.env.BENCH_STREAM || "bench-fixed";
const TOTAL = Number(process.env.BENCH_EVENTS || 5000);
const PAYLOAD_BYTES = Number(process.env.BENCH_PAYLOAD || 1024);
const RUNS = Number(process.env.BENCH_RUNS || 7);
const BATCH = 500;

function makeData(bytes) {
  return { blob: "x".repeat(bytes) };
}

function postBatch(events) {
  const body = JSON.stringify(events);
  return new Promise((resolve, reject) => {
    const req = http.request(
      {
        hostname: "localhost",
        port: 2113,
        path: `/streams/${STREAM}`,
        method: "POST",
        headers: {
          "Content-Type": "application/vnd.eventstore.events+json",
          "Content-Length": Buffer.byteLength(body),
        },
      },
      (res) => {
        let b = "";
        res.on("data", (c) => (b += c));
        res.on("end", () =>
          res.statusCode >= 200 && res.statusCode < 300
            ? resolve()
            : reject(new Error(`${res.statusCode}: ${b}`))
        );
      }
    );
    req.on("error", reject);
    req.write(body);
    req.end();
  });
}

async function seed() {
  for (let i = 0; i < TOTAL; i += BATCH) {
    const n = Math.min(BATCH, TOTAL - i);
    const events = Array.from({ length: n }, () => ({
      eventId: crypto.randomUUID(),
      eventType: "bench.event",
      data: makeData(PAYLOAD_BYTES),
      metadata: { ts: 1 },
    }));
    await postBatch(events);
  }
}

async function readOnce() {
  const client = addon.createClient("kurrentdb://localhost:2113?tls=false");
  let count = 0;
  let checksum = 0n;
  let bytes = 0;
  for await (const batch of client.readStream(STREAM, {
    maxCount: BigInt(TOTAL),
  })) {
    for (const ev of batch) {
      count++;
      checksum += BigInt(ev.event.revision);
      bytes += ev.event.data.length;
    }
  }
  return { count, checksum, bytes };
}

function stats(times) {
  const sorted = [...times].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)];
  const min = sorted[0];
  return { median, min };
}

async function read() {
  await readOnce();
  const times = [];
  let res;
  for (let r = 0; r < RUNS; r++) {
    const t0 = process.hrtime.bigint();
    res = await readOnce();
    const t1 = process.hrtime.bigint();
    times.push(Number(t1 - t0) / 1e6);
  }
  const { median, min } = stats(times);
  console.log(
    `read ${res.count} events (${(res.bytes / 1024).toFixed(0)} KiB payload) | ` +
      `median ${median.toFixed(1)} ms | min ${min.toFixed(1)} ms | ` +
      `per-event ${((median * 1000) / res.count).toFixed(2)} us`
  );
  console.log(`  runs(ms): ${times.map((t) => t.toFixed(1)).join(", ")}`);
}

(async () => {
  const mode = process.argv[2] || "all";
  if (mode === "seed" || mode === "all") {
    console.log(
      `Seeding ${TOTAL} events x ~${PAYLOAD_BYTES}B into '${STREAM}' ...`
    );
    await seed();
  }
  if (mode === "read" || mode === "all") {
    await read();
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
