import { test, expect } from '@playwright/test';

test('creates and edits a connection without losing the workspace layout',async({page})=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto('/');await expect(page.getByRole('heading',{name:'连接你的服务器'})).toBeVisible();
  await page.getByRole('button',{name:'新建 SSH 连接',exact:true}).click();
  const form=page.getByRole('dialog',{name:'SSH 连接设置'});
  await form.getByLabel('名称',{exact:true}).fill('测试主机');await form.getByLabel('主机地址').fill('example.test');await form.getByLabel('分组',{exact:true}).fill('开发环境');await form.getByRole('button',{name:'保存',exact:true}).click();
  await expect(page.getByRole('button',{name:'测试主机 root@example.test:22'})).toBeVisible();
  await page.getByRole('button',{name:'连接管理器',exact:true}).click();
  const manager=page.getByRole('dialog',{name:'连接管理器'});await expect(manager.getByText('example.test',{exact:true})).toBeVisible();
  await manager.getByRole('button',{name:'编辑 测试主机'}).click();await expect(page.getByLabel('主机地址')).toHaveValue('example.test');
  await page.getByLabel('名称',{exact:true}).fill('测试主机-改名');await page.getByRole('dialog',{name:'SSH 连接设置'}).getByRole('button',{name:'保存',exact:true}).click();
  await expect(manager.getByText('测试主机-改名',{exact:true})).toBeVisible();
  await manager.getByRole('button',{name:'关闭',exact:true}).click();
  await expect(page.getByText('连接服务器后浏览远端文件',{exact:true})).toBeVisible();expect(errors).toEqual([]);
});

test('does not invent live metrics or run a server connection in browser preview',async({page})=>{
  await page.goto('/');await expect(page.getByText('界面预览',{exact:true})).toBeVisible();await expect(page.getByText('未连接主机',{exact:true})).toBeVisible();await expect(page.getByText('等待两次采样',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'导入 FinalShell 连接',exact:true}).click();await expect(page.getByRole('status')).toContainText('桌面应用');
});
