/* eslint-disable  @typescript-eslint/no-explicit-any */

// This module is the CJS entry point for the library.

import * as addon from './load.js';
import { Buffer } from "buffer";

// Use this declaration to assign types to the addon's exports,
// which otherwise by default are `any`.
declare module "./load" {
  function createClient(connStr: string): RawClient;
  function readStreamNext(raw: RawReadStream): Promise<Buffer>;
}


export type Iterable = {
  next(): Promise<{ value: ResolvedEvent[]; done: boolean }>;
};

export type RustClient = {
  readStream(
    stream: string,
    options?: RustReadStreamOptions
  ): AsyncIterable<ResolvedEvent[]>;
  readAll(options?: RustReadAllOptions): AsyncIterable<ResolvedEvent[]>;
};

export type RawClient = {
  readStream(
    stream: string,
    options?: RustReadStreamOptions
  ): Promise<Iterable>;
  readAll(options?: RustReadAllOptions): Promise<Iterable>;
};

export type RawReadStream = unknown;

export type RustReadStreamOptions = {
  fromRevision: bigint | string;
  direction: string;
  maxCount: bigint;
  requiresLeader: boolean;
  resolvesLink: boolean;
  credentials?: { username: string; password: string };
};

export type RustReadAllOptions = {
  fromPosition: { commit: bigint; prepare: bigint } | string;
  direction: string;
  maxCount: bigint;
  requiresLeader: boolean;
  resolvesLink: boolean;
  credentials?: { username: string; password: string };
  filter?: Filter;
};

export const STREAM_NAME = "streamName";
export const EVENT_TYPE = "eventType";

export type FilterOn =
  | typeof EVENT_TYPE
  | typeof STREAM_NAME;

export type RustReadAllFilterBase = {
  filterOn: FilterOn;
};

export type Filter = RegexFilter | PrefixesFilter;

export interface RegexFilter extends RustReadAllFilterBase {
  regex: string;
}

export interface PrefixesFilter extends RustReadAllFilterBase {
  prefixes: string[];
}

export type ResolvedEvent = {
  event?: RecordedEvent;
  link?: RecordedEvent;
  commitPosition?: bigint;
};

export type RecordedEvent = {
  streamId: string;
  id: string;
  type: string;
  isJson: boolean;
  revision: bigint;
  created: Date;
  data: Uint8Array;
  metadata: Uint8Array;
  position?: Position;
};

export type Position = {
  commit: bigint;
  prepare: bigint;
};

export function createClient(connStr: string): RustClient {
  const client = addon.createClient(connStr);

  return {
    readStream(
      stream: string,
      options: RustReadStreamOptions
    ): AsyncIterable<ResolvedEvent[]> {
      let iteratorPromise: Promise<Iterable> | null = null;

      const asyncIterator: AsyncIterator<ResolvedEvent[]> = {
        async next() {
          if (!iteratorPromise)
            iteratorPromise = client.readStream(stream, options);

          const iterator = await iteratorPromise;
          const buffer = await addon.readStreamNext(iterator);
          return JSON.parse(buffer.toString()) as {
            value: ResolvedEvent[];
            done: boolean;
          };
        },
      };

      return {
        [Symbol.asyncIterator]() {
          return asyncIterator;
        },
      };
    },

    readAll(options: RustReadAllOptions): AsyncIterable<ResolvedEvent[]> {
      let iteratorPromise: Promise<Iterable> | null = null;

      const asyncIterator: AsyncIterator<ResolvedEvent[]> = {
        async next() {
          if (!iteratorPromise) iteratorPromise = client.readAll(options);

          const iterator = await iteratorPromise;
          const buffer = await addon.readStreamNext(iterator);
          return JSON.parse(buffer.toString()) as {
            value: ResolvedEvent[];
            done: boolean;
          };
        },
      };

      return {
        [Symbol.asyncIterator]() {
          return asyncIterator;
        },
      };
    },
  };
}

module.exports = {
  createClient,
};
