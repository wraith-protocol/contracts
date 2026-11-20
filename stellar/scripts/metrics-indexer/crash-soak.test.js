import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { runCrashSoak } from './crash-soak.js';

describe('metrics indexer crash and replay soak harness', () => {
  it('recovers exactly across process deaths and deterministic generated replays', async () => {
    const report = await runCrashSoak({ seed: 279, pages: 112 });

    assert.equal(report.verification, 'passed');
    assert.equal(report.generatedPages, 112);
    assert.equal(report.generatedEvents, 224);
    assert.ok(report.duplicatePages > 0);
    assert.ok(report.rewoundPages > 0);
    assert.ok(report.reorderedDuplicatePages > 0);
    assert.ok(report.emptyPages > 0);
    assert.ok(report.restarts > 0);
    assert.equal(report.killBeforeWrite, 1);
    assert.equal(report.killAfterWrite, 1);
    assert.equal(report.truncatedStates, 1);
    assert.equal(report.cursorStallChecks, 1);
  });
});
