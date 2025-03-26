"use strict";

const assert = require("assert");

const addon = require("../lib");
const { describe, it } = require("node:test");

describe("connection", () => {
  it("Should work with valid protocols", async () => {
    assert.doesNotThrow(() => {
      addon.createClient("esdb://localhost:2113");
      addon.createClient("kurrentdb://localhost:2113");
      addon.createClient("kurrent://localhost:2113");
      addon.createClient("kdb://localhost:2113");
      addon.createClient("esdb+discover://localhost:2113");
      addon.createClient("kurrentdb+discover://localhost:2113");
      addon.createClient("kurrent+discover://localhost:2113");
      addon.createClient("kdb+discover://localhost:2113");
    });
  });

  it("Should not throw when creating client with unavailable server", async () => {
    assert.doesNotThrow(() => addon.createClient("kurrentdb://localhost:1111"));
  });

  it("Should throw ParseError with an invalid connection string", async () => {
    assert.throws(
      () => addon.createClient("unknownprotocol://localhost:2113"),
      {
        name: "ParseError",
        message: "Unknown URL scheme: unknownprotocol",
      }
    );
  });

  it("Should throw ParseError with invalid `defaultDeadline` lower than -1", async () => {
    assert.throws(
      () =>
        addon.createClient("kurrentdb://localhost:2113?defaultDeadline=-10"),
      {
        name: "ParseError",
        message:
          "Invalid defaultDeadline of -10. Please provide a positive integer, or -1 to disable",
      }
    );
  });
});
