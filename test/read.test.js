"use strict";

const assert = require("assert");
const crypto = require("crypto");

const addon = require("../lib");
const { describe, it, before } = require("node:test");
const { createTestEvents, collectEvents } = require("./utils/createEvent");

describe("read", () => {
  let streamName = crypto.randomUUID();

  before(async () => {
    try {
      await createTestEvents(streamName, 4);
    } catch (error) {
      throw error;
    }
  });

  it("Should throw StreamNotFoundError when stream does not exist", async () => {
    // Arrange
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    // Act
    const stream = client.readStream("invalid-stream-name");

    // Assert
    await assert.rejects(() => collectEvents(stream), {
      name: "StreamNotFoundError",
    });
  });

  it("Should read all events from the a single", async () => {
    // Arrange
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    // Act
    const stream = client.readStream(streamName);

    // Assert
    const events = await collectEvents(stream);

    assert.strictEqual(
      events.length,
      4,
      "Count should be 4 after reading stream"
    );
  });

  it("Should read a limited number of events from a single stream", async () => {
    // Arrange
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    // Act
    const stream = client.readStream(streamName, { maxCount: 1n });

    // Assert
    const events = await collectEvents(stream);

    assert.strictEqual(
      events.length,
      1,
      "Count should be 1 after reading stream with maxCount set to 1"
    );
  });

  it("Should read a limited number of events from $all stream", async () => {
    // Arrange
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    // Act
    const stream = client.readAll({ maxCount: 1n });

    // Assert
    const events = await collectEvents(stream);

    assert.strictEqual(
      events.length,
      1,
      "Count should be 10 after reading all events"
    );
  });

  it("Should read with basic credentials", async () => {
    // Arrange
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    // Act
    const stream = client.readStream(streamName, {
      credentials: { username: "admin", password: "changeit" },
    });

    // Assert
    const events = await collectEvents(stream);

    assert.strictEqual(events.length, 4);
  });

  it("Should read with bearer-token credentials", async () => {
    // The credential shape lives entirely on the JS side. The bridge picks the
    // Bearer branch and the Rust client emits `Authorization: Bearer ...`. The
    // test server runs in insecure mode and ignores the header, so success
    // here proves the JS->Rust plumbing carries bearer tokens without error.
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    const stream = client.readStream(streamName, {
      credentials: { bearerToken: "test-bearer-token" },
    });

    const events = await collectEvents(stream);

    assert.strictEqual(events.length, 4);
  });

  it("Should read $all with bearer-token credentials", async () => {
    const client = addon.createClient("kurrentdb://localhost:2113?tls=false");

    const stream = client.readAll({
      maxCount: 1n,
      credentials: { bearerToken: "test-bearer-token" },
    });

    const events = await collectEvents(stream);

    assert.strictEqual(events.length, 1);
  });
});
