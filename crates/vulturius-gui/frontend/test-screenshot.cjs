const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const consoleMessages = [];
  page.on('console', msg => consoleMessages.push(msg.text()));
  page.on('pageerror', err => consoleMessages.push('ERROR: ' + err.message));
  await page.goto('http://localhost:1420/');
  await page.waitForTimeout(3000);
  await page.screenshot({ path: 'D:/tmp/gui-screenshot.png', fullPage: true });
  const html = await page.innerHTML('body');
  console.log('HTML length:', html.length);
  console.log('First 2000 chars:', html.substring(0, 2000));
  console.log('Console messages:', JSON.stringify(consoleMessages));
  await browser.close();
})();
