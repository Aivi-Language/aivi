import assert from "node:assert/strict";
import test from "node:test";

import { AsyncSerialQueue, errorText, withTimeout } from "../src/lifecycle";

test("restart operations remain serialized across success and failure", async () => {
  const queue = new AsyncSerialQueue();
  const events: string[] = [];
  let active = 0;
  let maximumActive = 0;

  const operation = (name: string, fail = false) => queue.run(async () => {
    active += 1;
    maximumActive = Math.max(maximumActive, active);
    events.push(`${name}:start`);
    await new Promise((resolve) => setTimeout(resolve, 5));
    events.push(`${name}:end`);
    active -= 1;
    if (fail) throw new Error(`${name} failed`);
  });

  const first = operation("first");
  const second = operation("second", true);
  const third = operation("third");
  await first;
  await assert.rejects(second, /second failed/);
  await third;
  await queue.drain();

  assert.equal(maximumActive, 1);
  assert.deepEqual(events, [
    "first:start", "first:end",
    "second:start", "second:end",
    "third:start", "third:end",
  ]);
});

test("startup timeout is bounded and preserves useful errors", async () => {
  await assert.rejects(
    withTimeout(new Promise(() => undefined), 5, "startup expired"),
    /startup expired/
  );
  assert.match(errorText(new Error("missing aivi")), /missing aivi/);
  assert.equal(errorText("plain failure"), "plain failure");
});
