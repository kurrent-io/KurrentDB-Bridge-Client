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
    const stream = client.readAll({ maxCount: 10n });

    // Assert
    const events = await collectEvents(stream);

    assert.strictEqual(
      events.length,
      10,
      "Count should be 10 after reading all events"
    );
  });
});
