const http = require("http");
const crypto = require("crypto");

/**
 * Helper function to create an event in KurrentDB
 * @param {Object} options - Configuration options
 * @param {string} options.streamName - Name of the stream to write to
 * @param {string} options.eventId - ID of the event (UUID)
 * @param {string} options.eventType - Type of the event
 * @param {Object} options.data - Event data payload
 * @param {Object} options.metadata - Event metadata
 * @returns {Promise<string>} Response body from the request
 */
async function createEvent({
  streamName,
  eventId,
  eventType,
  expectedVersion = -1,
  data = {},
  metadata = {},
} = {}) {
  if (!streamName || streamName.trim() === "")
    throw new Error("streamName is required and cannot be empty");

  if (!eventId || eventId.trim() === "")
    throw new Error("eventId is required and cannot be empty");

  if (!eventType || eventType.trim() === "")
    throw new Error("eventType is required and cannot be empty");

  const eventData = JSON.stringify([
    {
      eventId,
      eventType,
      data,
      metadata,
    },
  ]);

  return new Promise((resolve, reject) => {
    const options = {
      hostname: "localhost",
      port: 2113,
      path: `/streams/${streamName}`,
      method: "POST",
      headers: {
        "Content-Type": "application/vnd.eventstore.events+json",
        "ES-ExpectedVersion": expectedVersion.toString(),
        "Content-Length": Buffer.byteLength(eventData),
      },
    };

    const req = http.request(options, (res) => {
      let responseBody = "";
      res.on("data", (chunk) => {
        responseBody += chunk;
      });
      res.on("end", () => {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(responseBody);
        } else {
          reject(
            new Error(
              `HTTP request failed with status ${res.statusCode}: ${responseBody}`
            )
          );
        }
      });
    });

    req.on("error", (error) => {
      reject(error);
    });

    req.write(eventData);
    req.end();
  });
}

/**
 * Helper function to create multiple test events in a stream
 * @param {string} streamName - Name of the stream to write to
 * @param {number} count - Number of events to create (default: 4)
 * @param {string} type - Event type to use (default: "test")
 * @returns {Promise<Array>} Array of responses from all created events
 */
async function createTestEvents(streamName, count = 4, type = "test") {
  const promises = Array.from({ length: count }, (_, i) =>
    createEvent({
      streamName,
      eventId: crypto.randomUUID(),
      expectedVersion: -2,
      eventType: type,
      data: { message: i },
    })
  );

  return Promise.all(promises);
}

async function collectEvents(stream) {
  const events = [];
  for await (const batch of stream) {
    events.push(...batch);
  }
  return events;
}

module.exports.createEvent = createEvent;
module.exports.createTestEvents = createTestEvents;
module.exports.collectEvents = collectEvents;
