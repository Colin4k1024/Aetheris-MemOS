const { chromium } = require('playwright');

const BASE_URL = 'http://localhost:8000';

// Actual routes based on routes.ts
const pages = [
  { path: '/dashboard', name: 'Dashboard' },
  { path: '/performance', name: 'Performance' },
  { path: '/resource-monitor', name: 'Resource Monitor' },
  { path: '/memory-config', name: 'Memory Config' },
  { path: '/memory-management', name: 'Memory Management' },
  { path: '/task-analysis', name: 'Task Analysis' },
  { path: '/memory-decision-trace', name: 'Decision Trace' },
  { path: '/memory-details', name: 'Memory Details' },
  { path: '/weight-history', name: 'Weight History' },
];

async function testPage(browser, pageInfo) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  const results = { errors: [], warnings: [], blankAreas: [], broken: [], ok: [] };

  const consoleMessages = [];
  const networkErrors = [];
  page.on('console', msg => {
    if (msg.type() === 'error') consoleMessages.push(`[CONSOLE ERROR] ${msg.text()}`);
  });
  page.on('pageerror', err => {
    results.errors.push(`PageError: ${err.message}`);
  });
  page.on('response', resp => {
    if (resp.status() >= 400) {
      networkErrors.push(`${resp.status()} ${resp.url().split('/').pop()}`);
    }
  });

  try {
    const hashUrl = BASE_URL + '/#' + pageInfo.path;
    console.log(`\n--- Testing: ${pageInfo.name} (${hashUrl}) ---`);

    // Intercept /api/currentUser to return mock authenticated user
    await page.route('**/api/currentUser', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          success: true,
          data: {
            name: 'Serati Ma',
            avatar: 'https://gw.alipayobjects.com/zos/antfincdn/XAosXuNZyF/BiazfanxmamNRoxxVxka.png',
            userid: '00000001',
            email: 'antdesign@alipay.com',
            access: 'admin',
            isLogin: true,
          },
        }),
      });
      console.log('  [ROUTE] Mocked /api/currentUser');
    });

    // Navigate directly to the page
    await page.goto(hashUrl, { waitUntil: 'networkidle', timeout: 20000 });
    await page.waitForTimeout(6000); // Wait 6s for data

    const title = await page.title();
    console.log(`  Title: ${title}`);

    const bodyText = await page.evaluate(() => document.body.innerText);
    const hasContent = bodyText.trim().length > 50;
    console.log(`  Content length: ${bodyText.trim().length} chars`);
    if (!hasContent) results.blankAreas.push('Page mostly blank');

    const svgCount = await page.$$eval('svg', els => els.length);
    console.log(`  SVG charts: ${svgCount}`);

    const tableCount = await page.$$eval('table', els => els.length);
    const antTableCount = await page.$$eval('[class*="ant-table"]', els => els.length);
    console.log(`  Tables: ${tableCount}, Ant Tables: ${antTableCount}`);

    const formCount = await page.$$eval('form', els => els.length);
    const inputCount = await page.$$eval('input:not([type="hidden"])', els => els.length);
    console.log(`  Forms: ${formCount}, Inputs: ${inputCount}`);

    const cardCount = await page.$$eval('[class*="ant-card"]', els => els.length);
    const statCount = await page.$$eval('[class*="ant-statistic"]', els => els.length);
    console.log(`  Ant Cards: ${cardCount}, Statistics: ${statCount}`);

    const tabCount = await page.$$eval('[class*="ant-tabs"]', els => els.length);
    console.log(`  Ant Tabs: ${tabCount}`);

    const loadingSpinners = await page.$$eval('[class*="ant-spin"]', els => els.length);
    console.log(`  Loading spinners: ${loadingSpinners}`);

    // Check for error alerts
    const errorAlertCount = await page.$$eval('[class*="ant-alert-error"]', els => els.length);
    if (errorAlertCount > 0) {
      const texts = await page.$$eval('[class*="ant-alert-error"]', els => els.map(e => e.textContent.trim()).slice(0, 2));
      results.broken.push(`Error alerts visible: ${texts.join(' | ')}`);
      console.log(`  ERROR ALERTS: ${errorAlertCount} - ${texts.join(' | ')}`);
    }

    // Check for 404
    const is404 = title.includes('404') || bodyText.includes('404') || bodyText.includes('does not exist');
    if (is404) {
      results.broken.push('Page shows 404 - route not found');
      console.log(`  BROKEN: Page is 404`);
    }

    // Check for login redirect (still on login page)
    const isLogin = bodyText.includes('Login') && bodyText.includes('Account') && antTableCount === 0 && cardCount === 0 && !bodyText.includes('Dashboard') && !bodyText.includes('dashboard');
    if (isLogin) {
      results.broken.push('Still on login page - authentication failed');
      console.log(`  BROKEN: Still on login page`);
    }

    const contentPreview = bodyText.trim().substring(0, 300).replace(/\n+/g, ' | ');
    console.log(`  Content preview: ${contentPreview}`);

    // Network errors
    if (networkErrors.length > 0) {
      const criticalErrors = networkErrors.filter(e => !e.includes('favicon') && !e.includes('hot-reload'));
      if (criticalErrors.length > 0) {
        console.log(`  Network errors: ${criticalErrors.slice(0, 5).join(', ')}`);
        results.warnings.push(`Network errors: ${criticalErrors.slice(0, 3).join(', ')}`);
      }
    }

    // Console errors
    if (consoleMessages.length > 0) {
      const criticalConsole = consoleMessages.filter(m =>
        !m.includes('Warning:') &&
        !m.includes('CORS') &&
        !m.includes('net::ERR_') &&
        !m.includes('Failed to load resource')
      );
      if (criticalConsole.length > 0) {
        console.log(`  Critical console errors (${criticalConsole.length}):`);
        criticalConsole.slice(0, 3).forEach(m => console.log(`    ${m.substring(0, 200)}`));
      }
    }

    // Functional assessment
    const functional = [];
    if (cardCount > 0 || statCount > 0) functional.push(`cards/stats=${cardCount + statCount}`);
    if (svgCount > 0) functional.push(`charts=${svgCount}`);
    if (antTableCount > 0) functional.push(`tables=${antTableCount}`);
    if (formCount > 0) functional.push(`forms=${formCount}`);
    if (tabCount > 0) functional.push(`tabs=${tabCount}`);
    console.log(`  Functional: ${functional.length > 0 ? functional.join(', ') : 'NONE'}`);

    if (functional.length === 0 && !is404 && !isLogin) {
      results.broken.push('No functional elements found on page');
    }

  } catch (err) {
    results.broken.push(`Error: ${err.message}`);
    console.log(`  ERROR: ${err.message}`);
  } finally {
    await context.close();
  }

  return results;
}

async function main() {
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  console.log('Browser launched\n');

  const allResults = {};
  for (const pageInfo of pages) {
    allResults[pageInfo.name] = await testPage(browser, pageInfo);
  }

  await browser.close();

  console.log('\n\n=== FINAL SUMMARY ===\n');
  let totalBroken = 0, totalWarnings = 0;
  for (const [name, result] of Object.entries(allResults)) {
    if (result.broken.length > 0) {
      console.log(`FAIL - ${name}:`);
      result.broken.forEach(b => console.log(`  BROKEN: ${b}`));
      totalBroken++;
    } else if (result.warnings.length > 0) {
      console.log(`WARN - ${name}:`);
      result.warnings.forEach(w => console.log(`  WARNING: ${w}`));
      totalWarnings++;
    } else {
      console.log(`PASS - ${name}`);
    }
  }
  console.log(`\nTotal: ${pages.length} | Broken: ${totalBroken} | Warnings: ${totalWarnings} | OK: ${pages.length - totalBroken}`);
}

main().catch(console.error);
