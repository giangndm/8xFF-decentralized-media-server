import { chromium, firefox } from 'playwright';
import fs from 'node:fs/promises';
import path from 'node:path';

const webUrl = process.env.CLIENT_RS_WEB_URL || 'http://127.0.0.1:4173/';
const gateway = process.env.CLIENT_RS_GATEWAY || 'http://127.0.0.1:13100';
const browserName = process.env.CLIENT_RS_BROWSER || 'chromium';
const timeout = Number(process.env.CLIENT_RS_E2E_TIMEOUT || 60000);
const artifacts = path.join(path.resolve(process.env.CLIENT_RS_ARTIFACTS || new URL('./artifacts/', import.meta.url).pathname), browserName);
const allScenarios = ['echo_fast', 'echo_lazy', 'echo_simulcast', 'echo_restart_ice', 'switch_sender_track', 'multi_track_receivers', 'audio_mixer_auto', 'audio_mixer_manual'];
const scenarios = process.env.CLIENT_RS_SCENARIOS?.split(',') || allScenarios;
for (const scenario of scenarios) if (!allScenarios.includes(scenario)) throw new Error(`Unknown scenario: ${scenario}`);
await fs.mkdir(artifacts, { recursive: true });

async function tokenFor(room, peer) {
  const response = await fetch(`${gateway}/token/webrtc`, {
    method: 'POST', headers: { Authorization: `Bearer ${process.env.CLIENT_RS_APP_SECRET || 'insecure'}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ room, peer, ttl: 7200, record: false }), signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`Token issuer HTTP ${response.status}`);
  const body = await response.json();
  if (!body.data?.token) throw new Error(`Token issuer rejected request: ${body.error || 'no token'}`);
  return body.data.token;
}

const launchOptions = { headless: true };
if (process.env.CLIENT_RS_BROWSER_EXECUTABLE) launchOptions.executablePath = process.env.CLIENT_RS_BROWSER_EXECUTABLE;
if (browserName === 'chromium') launchOptions.args = ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', '--autoplay-policy=no-user-gesture-required'];
if (browserName === 'firefox') launchOptions.firefoxUserPrefs = { 'media.navigator.streams.fake': true, 'media.navigator.permission.disabled': true, 'media.autoplay.default': 0 };
const browser = await (browserName === 'firefox' ? firefox : chromium).launch(launchOptions);
const results = [];
async function waitAssertion(page, name) {
  try {
    await page.waitForFunction(key => document.querySelector(`[data-assert="${key}"]`)?.dataset.state === 'pass', name, { timeout });
  } catch (error) {
    throw new Error(`${name} assertion did not pass: ${error.message}`);
  }
}
try {
  for (const scenario of scenarios) {
    const context = await browser.newContext({ permissions: browserName === 'chromium' ? ['camera', 'microphone'] : [], ignoreHTTPSErrors: true });
    const traces = [], pageErrors = [];
    const pages = [];
    const room = `${process.env.CLIENT_RS_E2E_ROOM || 'client-rs'}-${scenario}-${Date.now()}`;
    const started = Date.now();
    let result;
    try {
      // Independent peers also exercise room event routing and mixer sources.
      for (let index = 0; index < 2; index++) {
        const peer = `browser-${index}`;
        const token = await tokenFor(room, peer);
        const page = await context.newPage();
        pages.push(page);
        page.on('console', msg => traces.push({ peer, type: msg.type(), message: msg.text() }));
        page.on('pageerror', error => pageErrors.push({ peer, message: error.message, stack: error.stack }));
        await page.goto(webUrl, { waitUntil: 'networkidle' });
        await page.waitForFunction(() => document.documentElement.dataset.clientReady === 'true', null, { timeout });
        await page.fill('#gateway', gateway);
        await page.fill('#room', room);
        await page.fill('#peer', peer);
        await page.fill('#token', token);
        await page.selectOption('#scenario', scenario);
        await page.click('#connect');
      }
      for (const page of pages) {
        await waitAssertion(page, 'control');
        await waitAssertion(page, 'scenario');
        if (!scenario.startsWith('audio_mixer')) {
          // Independently measure decoded frames: ontrack/srcObject is insufficient.
          await page.waitForFunction(() => {
            const video = document.querySelector('#remote-video');
            return video?.readyState >= 2 && video.getVideoPlaybackQuality?.().totalVideoFrames > 3;
          }, null, { timeout });
          const before = await page.$eval('#remote-video', v => v.getVideoPlaybackQuality().totalVideoFrames);
          await page.waitForFunction(count => document.querySelector('#remote-video').getVideoPlaybackQuality().totalVideoFrames > count + 2, before, { timeout });
          if (scenario === 'multi_track_receivers') {
            await page.waitForFunction(() => document.querySelector('#extra-video')?.getVideoPlaybackQuality().totalVideoFrames > 3, null, { timeout });
          }
        } else {
          // Mixer examples expose observed RTP/audio evidence from Rust stats.
          await waitAssertion(page, 'media');
        }
      }
      if (pageErrors.length) throw new Error(`Browser exceptions (${pageErrors.length}): ${JSON.stringify(pageErrors.slice(0, 2))}`);
      for (const page of pages) {
        await page.click('#disconnect');
        await waitAssertion(page, 'cleanup');
      }
      if (pageErrors.length) throw new Error(`Browser exceptions during cleanup (${pageErrors.length}): ${JSON.stringify(pageErrors.slice(0, 2))}`);
      result = { scenario, browser: browserName, status: 'pass', elapsedMs: Date.now() - started };
      console.log(`PASS ${scenario} (${result.elapsedMs} ms, two real WebRTC peers)`);
    } catch (error) {
      result = { scenario, browser: browserName, status: 'fail', error: String(error), elapsedMs: Date.now() - started, pageErrors };
      for (let i = 0; i < pages.length; i++) {
        await pages[i].screenshot({ path: path.join(artifacts, `${scenario}-${i}.png`) }).catch(() => {});
        traces.push({ peer: i, type: 'dom', message: await pages[i].locator('#event-log').textContent().catch(() => '') });
      }
      console.error(`FAIL ${scenario}: ${error}`);
    } finally {
      results.push(result);
      // Never include token input or request Authorization in retained artifacts.
      await fs.writeFile(path.join(artifacts, `${scenario}.json`), JSON.stringify({ ...result, traces }, null, 2));
      await context.close();
    }
    if (result.status === 'fail' && process.env.CLIENT_RS_KEEP_GOING !== '1') break;
  }
} finally {
  await browser.close();
  await fs.writeFile(path.join(artifacts, 'report.json'), JSON.stringify(results, null, 2));
}
if (results.some(r => r.status !== 'pass') || results.length !== scenarios.length) process.exitCode = 1;
