import assert from "node:assert/strict";
import { test } from "node:test";
import { createFetchDelayGate, normalizeFetchDelay } from "../src/lib/fetch-delay.ts";

function clock(random = 0.5) {
    let time = 0;
    const waits = [];
    const gate = createFetchDelayGate({
        now: () => time,
        random: () => random,
        sleep: async (ms) => { waits.push(ms); time += ms; },
    });
    return { gate, waits, now: () => time, advance: (ms) => { time += ms; } };
}

test("old or invalid settings disable delay; valid settings are bounded", () => {
    for (const value of [undefined, null, "5", NaN, Infinity, -1]) {
        assert.equal(normalizeFetchDelay(value), 0);
    }
    assert.equal(normalizeFetchDelay(4000), 3600);
    assert.equal(normalizeFetchDelay(1.2345), 1.235);
});

test("first fetch starts immediately; later batches/accounts share pacing", async () => {
    const c = clock();
    assert.equal(await c.gate(5, 5, () => false), true);
    assert.equal(c.now(), 0);
    c.advance(2000); // Time spent fetching the previous batch.
    assert.equal(await c.gate(5, 5, () => false), true);
    assert.equal(c.now(), 7500);
    await c.gate(5, 5, () => false);
    assert.equal(c.now(), 15000);
    assert.ok(c.waits.every((ms) => ms <= 100));
});

test("disabled pacing and a long fetch do not introduce unnecessary waits", async () => {
    const c = clock();
    await c.gate(0, 0, () => false);
    await c.gate(0, 0, () => false);
    c.advance(10000);
    await c.gate(5, 0, () => false);
    assert.deepEqual(c.waits, []);
});

test("random-only pacing supports both ends of the random range", async () => {
    for (const random of [0, 0.999]) {
        const c = clock(random);
        await c.gate(0, 5, () => false);
        await c.gate(0, 5, () => false);
        assert.equal(c.now(), random * 5000);
    }
});

test("stop or timeout during a wait prevents the next fetch from starting", async () => {
    const c = clock();
    await c.gate(5, 0, () => false);
    assert.equal(await c.gate(5, 0, () => c.now() >= 200), false);
    assert.equal(c.now(), 200);
    assert.equal(await c.gate(0, 0, () => true), false);
    await c.gate(5, 0, () => false);
    assert.equal(c.now(), 5000); // Cancellation did not reserve a future slot.
});

test("concurrent callers recheck the shared start time after waking", async () => {
    let time = 0;
    const sleepers = [];
    const gate = createFetchDelayGate({
        now: () => time,
        random: () => 0,
        sleep: () => new Promise((resolve) => sleepers.push(resolve)),
    });
    await gate(1, 0, () => false);
    const starts = [];
    const a = gate(1, 0, () => false).then(() => starts.push(time));
    const b = gate(1, 0, () => false).then(() => starts.push(time));
    time = 1000;
    sleepers.splice(0).forEach((resolve) => resolve());
    await a;
    assert.deepEqual(starts, [1000]);
    time = 2000;
    sleepers.splice(0).forEach((resolve) => resolve());
    await b;
    assert.deepEqual(starts, [1000, 2000]);
});
