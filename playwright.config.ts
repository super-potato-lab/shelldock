import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/e2e', fullyParallel: true, use: {baseURL: 'http://127.0.0.1:1420', viewport: {width:1360,height:900}, launchOptions: process.env.SHELLDOCK_CHROMIUM_PATH ? {executablePath:process.env.SHELLDOCK_CHROMIUM_PATH,args:['--no-sandbox','--no-zygote','--single-process','--disable-dev-shm-usage','--disable-gpu']} : undefined},
  webServer: {command:'npm run dev',url:'http://127.0.0.1:1420',reuseExistingServer:!process.env.CI},
});
