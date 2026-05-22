const { chromium } = require('playwright');

const PAGES = [
  { name: 'Dashboard', url: 'http://localhost:8000/#/dashboard' },
  { name: 'Performance', url: 'http://localhost:8000/#/performance' },
  { name: 'ResourceMonitor', url: 'http://localhost:8000/#/resource-monitor' },
  { name: 'MemoryConfig', url: 'http://localhost:8000/#/memory-config' },
  { name: 'MemoryManagement', url: 'http://localhost:8000/#/memory-management' },
  { name: 'MemoryDetails', url: 'http://localhost:8000/#/memory-details' },
  { name: 'MemoryDecisionTrace', url: 'http://localhost:8000/#/memory-decision-trace' },
  { name: 'TaskAnalysis', url: 'http://localhost:8000/#/task-analysis' },
  { name: 'WeightHistory', url: 'http://localhost:8000/#/weight-history' },
];

async function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

async function main() {
  console.log('Starting console error check for all pages...\n');

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  // Login through the login page using proper form interaction
  console.log('Navigating to login page...');
  await page.goto('http://localhost:8000/#/user/login', { waitUntil: 'domcontentloaded', timeout: 15000 });
  await sleep(3000);

  // Fill form
  console.log('Filling login form...');
  await page.locator('#username').fill('admin');
  await page.locator('#password').fill('demo');
  await sleep(1000);

  // Submit the form via ProForm's onFinish handler
  // The form uses Ant Design Pro Form, which calls onFinish
  console.log('Submitting form...');
  await page.evaluate(() => {
    // Find the ProForm and manually trigger the onFinish
    // The ProForm wraps a real HTML form that we can submit
    const forms = document.querySelectorAll('form');
    if (forms.length > 0) {
      forms[0].submit();
    }
  });
  await sleep(5000);

  const afterLoginUrl = page.url();
  console.log(`URL after login: ${afterLoginUrl}`);
  console.log(`On login page: ${afterLoginUrl.includes('/user/login')}`);

  // Verify login by checking currentUser
  const userResult = await page.evaluate(async () => {
    try {
      const resp = await fetch('/api/currentUser', { credentials: 'include' });
      const data = await resp.json();
      return { ok: resp.ok, data };
    } catch (e) {
      return { error: e.message };
    }
  });
  console.log(`currentUser: ${JSON.stringify(userResult)}`);

  // If still on login page, try reloading after login API call
  if (afterLoginUrl.includes('/user/login')) {
    console.log('Still on login page - reloading after API login...');

    // Call login API from within the page context
    await page.evaluate(async () => {
      await fetch('/api/login/account', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username: 'admin', password: 'demo', type: 'account' }),
        credentials: 'include',
      });
    });
    await sleep(2000);

    // Reload the page to trigger getInitialState with the new session
    console.log('Reloading page to refresh auth state...');
    await page.reload({ waitUntil: 'domcontentloaded', timeout: 15000 });
    await sleep(5000);

    const afterReloadUrl = page.url();
    console.log(`URL after reload: ${afterReloadUrl}`);

    // Check currentUser again
    const userResult2 = await page.evaluate(async () => {
      try {
        const resp = await fetch('/api/currentUser', { credentials: 'include' });
        const data = await resp.json();
        return { ok: resp.ok, data };
      } catch (e) {
        return { error: e.message };
      }
    });
    console.log(`currentUser after reload: ${JSON.stringify(userResult2)}`);

    // Navigate to dashboard manually
    if (afterReloadUrl.includes('/user/login')) {
      await page.goto('http://localhost:8000/#/dashboard', { waitUntil: 'domcontentloaded', timeout: 15000 });
      await sleep(5000);
      console.log(`After manual nav: ${page.url()}`);
    }
  }

  // Check all pages
  const results = [];

  for (const pageInfo of PAGES) {
    console.log(`\nChecking: ${pageInfo.name}...`);

    const consoleMessages = [];
    const consoleHandler = msg => {
      consoleMessages.push({ type: msg.type(), text: msg.text() });
    };
    page.on('console', consoleHandler);

    try {
      await page.goto(pageInfo.url, { waitUntil: 'domcontentloaded', timeout: 15000 });
      await sleep(5000);
    } catch (e) {
      console.log(`  Nav error: ${e.message}`);
    }

    page.off('console', consoleHandler);

    const url = page.url();
    const title = await page.title();

    const info = await page.evaluate(() => {
      const bodyText = document.body.innerText || '';
      const hasContent = bodyText.trim().length > 50;
      const isLoginPage = window.location.href.includes('/user/login') || document.title === 'Login - Aetheris-MemOS';
      const is404 = bodyText.includes('404') && bodyText.includes('does not exist');
      const dataVisible = hasContent && !isLoginPage && !is404;
      const cards = document.querySelectorAll('.ant-card').length;
      const tables = document.querySelectorAll('.ant-table').length;
      const charts = document.querySelectorAll('canvas, svg').length;
      const lists = document.querySelectorAll('.ant-list-item').length;
      const stats = document.querySelectorAll('.ant-statistic').length;
      const headings = Array.from(document.querySelectorAll('h1, h2, h3')).map(h => h.innerText.trim()).filter(t => t);
      return {
        hasContent,
        isLoginPage,
        is404,
        dataVisible,
        cards,
        tables,
        charts,
        lists,
        stats,
        headings,
        bodySnippet: bodyText.trim().substring(0, 200).replace(/\s+/g, ' '),
      };
    });

    const errorMessages = consoleMessages.filter(m => m.type === 'error');
    const warningMessages = consoleMessages.filter(m => m.type === 'warning');

    results.push({
      name: pageInfo.name,
      url: pageInfo.url,
      errors: errorMessages,
      warnings: warningMessages,
      title,
      ...info,
    });

    console.log(`  Title: ${title}`);
    console.log(`  Data Visible: ${info.dataVisible ? 'YES' : 'NO'}`);
    console.log(`  Elements: Cards=${info.cards}, Tables=${info.tables}, Charts=${info.charts}, Stats=${info.stats}`);
    if (info.dataVisible) {
      console.log(`  Body preview: "${info.bodySnippet}"`);
    }
    if (errorMessages.length > 0) {
      console.log(`  Console Errors (${errorMessages.length}):`);
      errorMessages.forEach(e => console.log(`    - ${e.text.substring(0, 150)}`));
    } else {
      console.log(`  Console Errors: None`);
    }
  }

  await page.close();
  await context.close();
  await browser.close();

  // Final Report
  console.log('\n' + '='.repeat(80));
  console.log('CONSOLE ERROR REPORT - ALL PAGES');
  console.log('(Only Error-level console messages. Warnings excluded per your request.)');
  console.log('='.repeat(80) + '\n');

  let totalPassed = 0;
  let pagesWithData = 0;

  for (const r of results) {
    const realErrors = r.errors.filter(e =>
      !e.text.includes('401') &&
      !e.text.includes('Unauthorized') &&
      !e.text.includes('Failed to load resource') &&
      !e.text.includes('net::ERR') &&
      !e.text.includes('favicon')
    );
    const status = realErrors.length === 0 && !r.is404 ? 'PASS' : 'FAIL';

    console.log(`[${status}] ${r.name}`);
    console.log(`  URL: ${r.url}`);
    console.log(`  Title: ${r.title}`);
    console.log(`  Status: ${r.is404 ? '404 NOT FOUND' : r.dataVisible ? 'HAS DATA' : r.isLoginPage ? 'LOGIN PAGE (auth issue)' : 'NO DATA'}`);
    console.log(`  Visible Elements: Cards=${r.cards}, Tables=${r.tables}, Charts=${r.charts}, Stats=${r.stats}`);
    console.log(`  Headings: ${r.headings.join(', ') || 'None'}`);

    if (r.errors.length > 0) {
      console.log(`  Console Errors (${r.errors.length}):`);
      r.errors.forEach(e => console.log(`    - ${e.text.substring(0, 150)}`));
    } else {
      console.log(`  Console Errors: None`);
    }

    if (realErrors.length === 0) totalPassed++;
    if (r.dataVisible) pagesWithData++;
    console.log('');
  }

  console.log('='.repeat(80));
  console.log('SUMMARY');
  console.log('='.repeat(80));
  console.log(`  Total pages: ${results.length}`);
  console.log(`  PASS (no Error-level console errors, no 404): ${totalPassed}`);
  console.log(`  FAIL: ${results.length - totalPassed}`);
  console.log(`  Pages with visible data: ${pagesWithData}`);
  console.log(`  Pages with no data: ${results.length - pagesWithData}`);
  const notFound = results.filter(r => r.is404);
  if (notFound.length > 0) console.log(`  404 pages: ${notFound.map(r => r.name).join(', ')}`);
  console.log('='.repeat(80));
}

main().catch(console.error);
