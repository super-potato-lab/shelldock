import { test, expect } from '@playwright/test';
import { installDesktopFixture } from './desktop-fixture';

test('creates and edits a connection from the compact quick list',async({page})=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('/');await expect(page.getByText('快速连接',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'新建 SSH 连接',exact:true}).click();
  const form=page.getByRole('dialog',{name:'SSH 连接设置'});
  await form.getByLabel('名称',{exact:true}).fill('测试主机');await form.getByLabel('主机地址').fill('example.test');await form.getByLabel('分组',{exact:true}).fill('开发环境');await form.getByRole('button',{name:'保存',exact:true}).click();
  await expect(page.getByRole('button',{name:'连接 测试主机',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'连接管理器',exact:true}).click();
  const manager=page.getByRole('dialog',{name:'连接管理器'});await expect(manager.getByText('example.test',{exact:true})).toBeVisible();
  await manager.getByRole('button',{name:'编辑 测试主机'}).click();await expect(page.getByLabel('主机地址')).toHaveValue('example.test');
  await page.getByLabel('名称',{exact:true}).fill('测试主机-改名');await page.getByRole('dialog',{name:'SSH 连接设置'}).getByRole('button',{name:'保存',exact:true}).click();
  await expect(manager.getByText('测试主机-改名',{exact:true})).toBeVisible();
  await manager.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(page.locator('.file-panel')).toHaveCount(0);expect(errors).toEqual([]);
});

test('shows unavailable values and no file panel before connecting',async({page})=>{
  await page.goto('/');await expect(page.getByText('界面预览',{exact:true})).toBeVisible();await expect(page.getByText('IP -',{exact:true})).toBeVisible();
  await expect(page.locator('.usage-bar').first()).toContainText('—');await expect(page.locator('.usage-chart polyline')).toHaveAttribute('points','');await expect(page.locator('.file-panel')).toHaveCount(0);
  await page.getByRole('button',{name:'导入 FinalShell 连接',exact:true}).click();await expect(page.getByRole('status')).toContainText('桌面应用');
});

test('lists every saved connection and clears quick entries without deleting profiles',async({page})=>{
  await installDesktopFixture(page);await page.goto('/');await expect(page.locator('.quick-table tr')).toHaveCount(28);
  await page.getByRole('button',{name:'清空',exact:true}).click();await expect(page.locator('.quick-table tr')).toHaveCount(0);
  await page.getByRole('button',{name:'连接管理器',exact:true}).click();await expect(page.getByRole('dialog',{name:'连接管理器'}).locator('tbody tr')).toHaveCount(28);
});

test('connected workspace has live charts, SFTP tree, Unix modes and working command history',async({page},testInfo)=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await installDesktopFixture(page);await page.goto('/');await page.getByRole('button',{name:'连接 demo-node',exact:true}).click();
  await expect(page.getByLabel('远端路径')).toHaveValue('/home/demo');
  await expect.poll(()=>page.evaluate(()=> (window as any).__testRequests.filter((r:any)=>r.command==='terminal_resize').at(-1)?.args.columns || 0)).toBeGreaterThan(80);
  await expect(page.getByRole('tree',{name:'远端目录树'}).getByRole('button',{name:'打开目录 /home/demo',exact:true})).toBeVisible();
  const table=page.locator('.file-table');await expect(table.getByText('drwxr-xr-x').first()).toBeVisible();await expect(table.getByText('-rw-r--r--').first()).toBeVisible();await expect(table.getByText('1000/1000').first()).toBeVisible();
  await expect(page.locator('.usage-chart polyline')).not.toHaveAttribute('points','',{timeout:10000});await expect(page.locator('.disks-table').getByText('/run',{exact:true})).toBeVisible();
  await page.getByRole('tree').getByRole('button',{name:'打开目录 /var',exact:true}).click();await expect(page.getByLabel('远端路径')).toHaveValue('/var');await expect(table.getByText('log',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'展开 /var',exact:true}).click();await expect(page.getByRole('button',{name:'打开目录 /var/log',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'打开目录 /home/demo',exact:true}).click();
  const input=page.getByRole('textbox',{name:'命令输入框'});await input.fill('echo verified');await input.press('Enter');
  await expect.poll(()=>page.evaluate(()=> (window as any).__testRequests.filter((r:any)=>r.command==='terminal_input').map((r:any)=>new TextDecoder().decode(new Uint8Array(r.args.data))).join(''))).toContain('echo verified\r');
  await page.locator('.floating-command').getByRole('button',{name:'历史',exact:true}).click();await page.locator('.command-history').getByRole('button',{name:'1 echo verified',exact:true}).click();await expect(input).toHaveValue('echo verified');
  await input.press('Escape');await expect(input).toHaveCount(0);await page.keyboard.press('Control');await page.keyboard.press('Control');await expect(input).toBeVisible();
  const rect=await page.evaluate(()=>({sidebar:document.querySelector('.monitor-panel')!.getBoundingClientRect().width,bar:document.querySelector('.session-tabs')!.getBoundingClientRect().height,file:document.querySelector('.file-panel')!.getBoundingClientRect().height,workspace:document.querySelector('.session-workspace')!.getBoundingClientRect().height,overflow:document.documentElement.scrollWidth>innerWidth}));
  expect(rect.sidebar).toBe(320);expect(rect.bar).toBe(34);expect(rect.file/rect.workspace).toBeCloseTo(.43,2);expect(rect.overflow).toBe(false);
  await testInfo.attach('connected-workspace',{body:await page.screenshot(),contentType:'image/png'});
  await page.getByRole('button',{name:'新标签页',exact:true}).click();await expect(page.locator('.file-panel')).toHaveCount(0);await expect(page.locator('.quick-table tr')).toHaveCount(28);
  await page.locator('.session-tab').filter({hasText:'demo-node'}).click();await expect(page.getByLabel('远端路径')).toHaveValue('/home/demo');expect(errors).toEqual([]);
});

test('splitters resize with keyboard and pointer at the minimum supported window',async({page})=>{
  await page.setViewportSize({width:960,height:660});await installDesktopFixture(page,1);await page.goto('/');await page.getByRole('button',{name:'连接 demo-node',exact:true}).click();await expect(page.getByLabel('远端路径')).toHaveValue('/home/demo');
  const sidebar=page.getByRole('separator',{name:'调整监控栏宽度'});await sidebar.focus();await page.keyboard.press('ArrowLeft');await expect(sidebar).toHaveAttribute('aria-valuenow','310');
  const files=page.getByRole('separator',{name:'调整终端与文件区高度'});await files.focus();await page.keyboard.press('ArrowUp');await expect(files).toHaveAttribute('aria-valuenow','45');
  const box=(await files.boundingBox())!;await page.mouse.move(box.x+40,box.y+2);await page.mouse.down();await page.mouse.move(box.x+40,box.y-50);await page.mouse.up();expect(Number(await files.getAttribute('aria-valuenow'))).toBeGreaterThan(45);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBe(960);await expect(page.getByRole('button',{name:'发送命令'})).toBeVisible();
});

test('ignores a late directory response after navigating elsewhere',async({page})=>{
  await installDesktopFixture(page,1);await page.goto('/');await page.getByRole('button',{name:'连接 demo-node',exact:true}).click();await expect(page.locator('.file-table').getByText('notes.txt',{exact:true})).toBeVisible();
  await page.evaluate(()=>{const ipc=(window as any).__TAURI_INTERNALS__;const original=ipc.invoke;ipc.invoke=async(command:string,args:any)=>{if(command==='list_files'&&args.path==='/var')await new Promise(resolve=>setTimeout(resolve,250));return original(command,args);};});
  await page.getByRole('button',{name:'打开目录 /var',exact:true}).click();await page.getByRole('button',{name:'打开目录 /home/demo',exact:true}).click();
  await expect(page.getByLabel('远端路径')).toHaveValue('/home/demo');await expect(page.locator('.file-table').getByText('notes.txt',{exact:true})).toBeVisible();
  await page.waitForTimeout(350);await expect(page.locator('.file-table').getByText('notes.txt',{exact:true})).toBeVisible();await expect(page.locator('.file-table').getByText('log',{exact:true})).toHaveCount(0);
});
