import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
const browser = (...args) => execFileSync('agent-browser', ['--session', 'burd-ui-test', ...args], { encoding: 'utf8' }).trim();
// Run against `npm run dev`; browser fixtures never reach native services.
test('settings captures focus, traps Tab and closes with Escape', () => {
  try {
    browser('open', process.env.BURD_PREVIEW_URL || 'http://127.0.0.1:1420');
    browser('wait', '--text', 'General Settings');
    browser('find', 'role', 'button', 'click', '--name', 'Instances', '--exact');
    browser('set', 'viewport', '1100', '650');
    assert.equal(browser('eval', 'Array.from(document.querySelectorAll(".status-dot")).every(dot => dot.getBoundingClientRect().width >= 8)'), 'true');
    browser('eval', 'document.querySelector("button[title=Settings]").click()');
    browser('wait', '[role=dialog]');
    assert.equal(browser('eval', 'document.querySelector("[role=dialog]").contains(document.activeElement)'), 'true');
    browser('press', 'Shift+Tab');
    assert.equal(browser('eval', 'document.querySelector("[role=dialog]").contains(document.activeElement)'), 'true');
    browser('press', 'Escape');
    assert.equal(browser('eval', 'document.querySelectorAll("[role=dialog]").length'), '0');
  } finally { browser('close'); }
});

test('instance creation accepts nested subdomains and rejects malformed labels', () => {
  try {
    browser('open', process.env.BURD_PREVIEW_URL || 'http://127.0.0.1:1420');
    browser('wait', '--text', 'General Settings');
    browser('find', 'role', 'button', 'click', '--name', 'Instances', '--exact');
    browser('find', 'role', 'button', 'click', '--name', '+ New Instance', '--exact');
    browser('wait', '.domain-input');
    assert.equal(browser('eval', '(() => { const input = document.querySelector(".domain-input"); input.value = "app.brand"; return input.checkValidity(); })()'), 'true');
    assert.equal(browser('eval', '(() => { const input = document.querySelector(".domain-input"); input.value = "app..brand"; return input.checkValidity(); })()'), 'false');
  } finally { browser('close'); }
});

test('inspector backdrop and focus restoration work after component extraction', () => {
  try {
    browser('open', process.env.BURD_PREVIEW_URL || 'http://127.0.0.1:1420');
    browser('wait', '--text', 'General Settings');
    browser('find', 'role', 'button', 'click', '--name', 'Instances', '--exact');
    browser('eval', 'document.querySelector(".more-actions").open = true; Array.from(document.querySelectorAll("button")).find(button => button.title === "View Logs").click()');
    browser('wait', '.logs-content');
    assert.ok(browser('eval', 'document.querySelector(".logs-content").textContent').includes('Preview logs'));
    browser('eval', 'document.querySelector("[role=document]").click()');
    assert.equal(browser('eval', 'document.querySelectorAll("[role=dialog]").length'), '1');
    browser('press', 'Escape');
    assert.equal(browser('eval', 'document.querySelectorAll("[role=dialog]").length'), '0');
    browser('eval', 'document.querySelector("button[title=Settings]").focus(); document.querySelector("button[title=Settings]").click()');
    browser('wait', '[role=dialog]');
    browser('eval', 'document.querySelector("[role=dialog]").click()');
    assert.equal(browser('eval', 'document.querySelectorAll("[role=dialog]").length'), '0');
    assert.equal(browser('eval', 'document.activeElement.title'), '"Settings"');
  } finally { browser('close'); }
});

test('stopped instance settings persist and unsupported preview commands fail', () => {
  try {
    browser('open', process.env.BURD_PREVIEW_URL || 'http://127.0.0.1:1420');
    browser('wait', '--text', 'General Settings');
    browser('find', 'role', 'button', 'click', '--name', 'Instances', '--exact');
    browser('eval', 'Array.from(document.querySelectorAll(".grid-row")).find(row => row.textContent.includes("getapp")).querySelector("button[title=Settings]").click()');
    browser('wait', 'input[placeholder="Instance name..."]');
    browser('find', 'label', 'Name', 'fill', 'renamed-app');
    browser('find', 'role', 'button', 'click', '--name', 'Save Settings', '--exact');
    browser('wait', '--text', 'renamed-app');
    assert.equal(browser('eval', 'document.querySelectorAll("[role=dialog]").length'), '0');
    assert.equal(browser('eval', 'window.__TAURI_INTERNALS__.invoke("unsupported_test_command", {}).then(() => false, () => true)'), 'true');
  } finally { browser('close'); }
});
